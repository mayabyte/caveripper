/// Parsing for CaveInfo files

mod section;

use crate::{
    caveinfo::{
        util::{expand_rotations, sort_cave_units},
        CaveInfo, TekiInfo, ItemInfo, CapInfo, GateInfo,
        DoorLink, DoorUnit, CaveUnit, SpawnPoint, RoomType,
        Waterbox, Waypoint
    },
    assets::{AssetManager, CaveConfig}, point::Point
};
use itertools::Itertools;
use once_cell::sync::Lazy;
use pest::Parser;
use pest_derive::Parser;
use regex::Regex;
use std::path::PathBuf;
use error_stack::{Result, ResultExt, IntoReport, report, Report};

use self::section::{Section, InfoLine};

use super::error::CaveInfoError;

#[derive(Parser)]
#[grammar = "caveinfo/p2_cfg_grammar.pest"]
struct CaveinfoParser;


fn parse_sections(file_contents: &str) -> Result<impl Iterator<Item = Section>, CaveInfoError> {
    let pairs = CaveinfoParser::parse(Rule::section_file, file_contents)
        .into_report().change_context(CaveInfoError::MalformedFile)
        .attach_printable("Couldn't parse file into sections")?
        .next().unwrap();
    Ok(pairs.into_inner().filter_map(|pair| pair.try_into().ok()))
}

/// Takes the entire raw text of a CaveInfo file and parses it into several
/// CaveInfo structs - one for each floor - ready for passing to the generator.
pub(crate) fn parse_caveinfo(cave_cfg: &CaveConfig) -> Result<Vec<CaveInfo>, CaveInfoError> {
    let caveinfo_txt = AssetManager::get_txt_file(cave_cfg.get_caveinfo_path())
        .change_context(CaveInfoError::FileRead)
        .attach_printable_lazy(|| cave_cfg.get_caveinfo_path().to_string_lossy().into_owned())?;

    let mut caveinfos = parse_sections(caveinfo_txt)
        .attach_printable_lazy(|| format!("Failed to parse {} into sections", cave_cfg.caveinfo_filename))?
        .skip(1)
        .chunks(5).into_iter()
        .map(|section_chunk| -> Result<CaveInfo, CaveInfoError> {
            let (header, teki, item, gate, cap) = section_chunk
                .collect_tuple()
                .ok_or(report!(CaveInfoError::MalformedFile))
                .attach_printable("Incorrect number of sections in CaveInfo file")?;

            Ok(CaveInfo {
                cave_cfg: cave_cfg.clone(),
                floor_num: header.get_tag("{f000}")?,
                max_main_objects: header.get_tag("{f002}")?,
                max_treasures: header.get_tag("{f003}")?,
                max_gates: header.get_tag("{f004}")?,
                num_rooms: header.get_tag("{f005}")?,
                corridor_probability: header.get_tag("{f006}")?,
                cap_probability: header.get_tag::<f32>("{f014}")? / 100f32,
                has_geyser: header.get_tag::<u32>("{f007}")? > 0,
                exit_plugged: header.get_tag::<u32>("{f010}")? > 0,
                cave_units: expand_rotations(sort_cave_units(
                        parse_unitfile(&header.get_tag::<String>("{f008}")?, cave_cfg)?
                    )),
                teki_info: teki.try_into()?,
                item_info: item.try_into()?,
                gate_info: gate.try_into()?,
                cap_info: cap.try_into()?,
                is_final_floor: false,
                waterwraith_timer: header.get_tag("{f016}").unwrap_or(0.0f32),
            })
        })
        .collect::<Result<Vec<CaveInfo>, CaveInfoError>>()?;
    caveinfos.last_mut().unwrap().is_final_floor = true;

    Ok(caveinfos)
}

fn parse_unitfile(unitfile: &str, cave_cfg: &CaveConfig) -> Result<Vec<CaveUnit>, CaveInfoError> {
    let unitfile_path = PathBuf::from(&cave_cfg.game).join("unitfiles").join(unitfile);
    let unitfile_txt = AssetManager::get_txt_file(&unitfile_path)
        .change_context(CaveInfoError::FileRead)?;
    parse_sections(unitfile_txt)
        .change_context(CaveInfoError::CaveUnitDefinition)
        .attach_printable_lazy(|| unitfile_path.to_string_lossy().into_owned())?
        .map(|section| try_parse_caveunit(&section, cave_cfg))
        .collect::<Result<_, _>>()
}

fn try_parse_caveunit(section: &Section, cave: &CaveConfig) -> Result<CaveUnit, CaveInfoError> {
    let unit_folder_name: String = section.get_line(1)?.get_line_item(0)?;
    let width = section.get_line(2)?.get_line_item(0)?;
    let height = section.get_line(2)?.get_line_item(1)?;
    let room_type = section
        .get_line(3)?
        .get_line_item::<usize>(0)?
        .into();
    let num_doors = section.get_line(5)?.get_line_item(0)?;

    // DoorUnits
    let doors = if num_doors > 0 {
        let num_lines_per_door_unit = (section.lines.len() - 6) / num_doors;
        section.lines[6..]
            .chunks(num_lines_per_door_unit)
            .map(
                |doorunit_lines: &[InfoLine]| -> Result<DoorUnit, CaveInfoError> {
                    doorunit_lines.try_into()
                },
            )
            .collect::<Result<Vec<_>, _>>()?
    } else {
        vec![]
    };

    // Cave Unit Layout File (spawn points)
    let layoutfile_path = PathBuf::from(&cave.game).join("mapunits").join(&unit_folder_name).join("texts/layout.txt");
    let mut spawnpoints = match AssetManager::get_txt_file(&layoutfile_path) {
        Ok(cave_unit_layout_file_txt) => {
            parse_sections(cave_unit_layout_file_txt)?
                .map(TryInto::try_into)
                .collect::<Result<Vec<SpawnPoint>, CaveInfoError>>()
                .change_context(CaveInfoError::LayoutFile)
                .attach_printable_lazy(|| layoutfile_path.to_string_lossy().into_owned())?
        },
        Err(_) => Vec::new(),
    };

    // Waterboxes file
    let waterboxes = match AssetManager::get_txt_file(
        &PathBuf::from(&cave.game).join("mapunits").join(&unit_folder_name).join("texts/waterbox.txt")
    ) {
        Ok(waterboxes_file_txt) => {
            parse_sections(waterboxes_file_txt)?.next().unwrap().try_into()
                .change_context(CaveInfoError::WaterboxFile)
                .attach_printable_lazy(|| format!("{unit_folder_name}/texts/waterbox.txt"))?
        },
        Err(_) => Vec::new(),
    };

    // route.txt file (Waypoints)
    let waypoints_file_txt = AssetManager::get_txt_file(
        &PathBuf::from(&cave.game).join("mapunits").join(&unit_folder_name).join("texts/route.txt"))
        .change_context(CaveInfoError::FileRead)?;
    let waypoints = parse_sections(waypoints_file_txt)
        .change_context(CaveInfoError::RouteFile)
        .attach_printable_lazy(|| format!("{unit_folder_name}/texts/route.txt"))?
        .map(<Waypoint as TryFrom::<Section>>::try_from)
        // Move the coordinates so they're oriented around the center of the unit
        .map(|r| {
            r.map(|mut wp| {
                wp.pos[0] += width as f32 * 170.0 / 2.0;
                wp.pos[2] += height as f32 * 170.0 / 2.0;
                wp
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Add special Hole/Geyser spawnpoints to Cap and Hallway units. These aren't
    // present in Caveinfo files but the generation algorithm acts as if they're there,
    // so adding them here is a simplification.
    // Group 9 is a special group specifically for these 'fake' hole/geyser spawnpoints.
    // It does not appear in the game code or on the TKB.
    if (room_type == RoomType::DeadEnd && unit_folder_name.starts_with("item")) || room_type == RoomType::Hallway {
        spawnpoints.push(
            SpawnPoint {
                group: 9,
                pos: Point::default(),
                angle_degrees: 0.0,
                radius: 0.0,
                min_num: 1,
                max_num: 1
            }
        );
    }

    Ok(CaveUnit {
        unit_folder_name,
        width,
        height,
        room_type,
        num_doors,
        doors,
        rotation: 0,
        spawnpoints,
        waterboxes,
        waypoints,
    })
}

impl TryFrom<Section<'_>> for Vec<TekiInfo> {
    type Error = Report<CaveInfoError>;
    fn try_from(section: Section) -> std::result::Result<Vec<TekiInfo>, Self::Error> {
        section
            .lines
            .iter()
            .skip(1) // First line contains the number of Teki
            .tuples()
            .map(
                |(item_line, group_line)| -> std::result::Result<TekiInfo, Self::Error> {
                    let internal_identifier: String = item_line.get_line_item(0)?;
                    let amount_code: u32 = item_line.get_line_item(1)?;
                    let group: u32 = group_line.get_line_item(0)?;

                    let (spawn_method, internal_name, carrying) =
                        extract_internal_identifier(&internal_identifier);

                    // Determine amount and filler_distribution_weight based on teki type
                    let minimum_amount: u32;
                    let filler_distribution_weight: u32;
                    if group == 6 {
                        // 6 is the group number for decorative teki
                        minimum_amount = amount_code;
                        filler_distribution_weight = 0;
                    } else {
                        // If there is only one digit, it represents the filler_distribution_weight
                        // and minimum_amount defaults to 0.
                        minimum_amount = amount_code / 10;
                        filler_distribution_weight = amount_code % 10;
                    }

                    Ok(TekiInfo {
                        internal_name,
                        carrying,
                        minimum_amount,
                        filler_distribution_weight,
                        group,
                        spawn_method,
                    })
                },
            )
            .collect::<Result<Vec<_>, _>>()
    }
}

impl TryFrom<Section<'_>> for Vec<ItemInfo> {
    type Error = Report<CaveInfoError>;
    fn try_from(section: Section) -> std::result::Result<Vec<ItemInfo>, Self::Error> {
        section
            .lines
            .iter()
            .skip(1)
            .map(|line| -> std::result::Result<ItemInfo, Self::Error> {
                let amount_code: u32 = line.get_line_item(1)?;
                Ok(ItemInfo {
                    internal_name: line.get_line_item(0)?,
                    min_amount: amount_code as u8 / 10,
                    filler_distribution_weight: amount_code % 10,
                })
            })
            .collect()
    }
}

impl TryFrom<Section<'_>> for Vec<GateInfo> {
    type Error = Report<CaveInfoError>;
    fn try_from(section: Section) -> std::result::Result<Vec<GateInfo>, Self::Error> {
        section
            .lines
            .iter()
            .skip(1)
            .tuples()
            .map(
                |(health_line, spawn_distribution_weight_line)| -> std::result::Result<GateInfo, Self::Error> {
                    Ok(GateInfo {
                        health: health_line.get_line_item(1)?,
                        spawn_distribution_weight: spawn_distribution_weight_line.get_line_item::<u32>(0)? % 10
                    })
                },
            )
            .collect()
    }
}

impl TryFrom<Section<'_>> for Vec<CapInfo> {
    /// Almost an exact duplicate of the code for TekiInfo, which is unfortunately
    /// necessary with how the code is currently structured. May refactor in the future.
    type Error = Report<CaveInfoError>;
    fn try_from(section: Section) -> std::result::Result<Vec<CapInfo>, Self::Error> {
        section
            .lines
            .iter()
            .skip(1) // First line contains the number of Teki
            .tuples()
            .map(
                |(_, item_line, group_line)| -> std::result::Result<CapInfo, Self::Error> {
                    let internal_identifier: String = item_line.get_line_item(0)?;
                    let amount_code: u32 = item_line.get_line_item(1)?;
                    let group: u8 = group_line.get_line_item(0)?;

                    let (spawn_method, internal_name, carrying) =
                        extract_internal_identifier(&internal_identifier);

                    // If there is only one digit, it represents the filler_distribution_weight
                    // and minimum_amount defaults to 0.
                    let minimum_amount = amount_code / 10;
                    let filler_distribution_weight = amount_code % 10;

                    Ok(CapInfo {
                        internal_name,
                        carrying,
                        minimum_amount,
                        filler_distribution_weight,
                        group,
                        spawn_method,
                    })
                },
            )
            .collect()
    }
}

impl TryFrom<&[InfoLine<'_>]> for DoorUnit {
    type Error = Report<CaveInfoError>;
    fn try_from(lines: &[InfoLine]) -> std::result::Result<DoorUnit, Self::Error> {
        let direction = lines[1].get_line_item(0)?;
        let side_lateral_offset = lines[1].get_line_item(1)?;
        let waypoint_index = lines[1].get_line_item(2)?;
        let num_links = lines[2].get_line_item(0)?;
        let door_links = lines[3..]
            .iter()
            .map(|line| line.try_into())
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(DoorUnit {
            direction,
            side_lateral_offset,
            waypoint_index,
            num_links,
            door_links,
        })
    }
}

impl TryFrom<&InfoLine<'_>> for DoorLink {
    type Error = Report<CaveInfoError>;
    fn try_from(line: &InfoLine) -> std::result::Result<DoorLink, Self::Error> {
        let distance = line.get_line_item(0)?;
        let door_id = line.get_line_item(1)?;
        let tekiflag = line.get_line_item::<u32>(2)? > 0;
        Ok(DoorLink {
            distance,
            door_id,
            tekiflag,
        })
    }
}

impl TryFrom<Section<'_>> for SpawnPoint {
    type Error = Report<CaveInfoError>;
    fn try_from(section: Section) -> std::result::Result<SpawnPoint, Self::Error> {
        Ok(
            SpawnPoint {
                group: section.get_line(0)?.get_line_item(0)?,
                pos: Point([
                    section.get_line(1)?.get_line_item(0)?,
                    section.get_line(1)?.get_line_item(1)?,
                    section.get_line(1)?.get_line_item(2)?,
                ]),
                angle_degrees: section.get_line(2)?.get_line_item(0)?,
                radius: section.get_line(3)?.get_line_item(0)?,
                min_num: section.get_line(4)?.get_line_item(0)?,
                max_num: section.get_line(5)?.get_line_item(0)?,
            }
        )
    }
}

impl TryFrom<Section<'_>> for Vec<Waterbox> {
    type Error = Report<CaveInfoError>;
    fn try_from(section: Section<'_>) -> std::result::Result<Self, Self::Error> {
        let num_waterboxes: usize = section.get_line(0)?.get_line_item(0)?;
        let mut waterboxes = Vec::with_capacity(num_waterboxes);
        for i in 0..num_waterboxes {
            waterboxes.push(Waterbox {
                p1: Point([
                    section.get_line(i+1)?.get_line_item(0)?,
                    section.get_line(i+1)?.get_line_item(1)?,
                    section.get_line(i+1)?.get_line_item(2)?,
                ]),
                p2: Point([
                    section.get_line(i+1)?.get_line_item(3)?,
                    section.get_line(i+1)?.get_line_item(4)?,
                    section.get_line(i+1)?.get_line_item(5)?,
                ])
            });
        }
        Ok(waterboxes)
    }
}

impl TryFrom<Section<'_>> for Waypoint {
    type Error = Report<CaveInfoError>;
    fn try_from(section: Section<'_>) -> std::result::Result<Self, Self::Error> {
        let num_links: usize = section.get_line(1)?.get_line_item(0)?;
        let coords_line = section.get_line(num_links + 2)?;
        Ok(Waypoint {
            pos: Point([
                coords_line.get_line_item(0)?,
                coords_line.get_line_item(1)?,
                coords_line.get_line_item(2)?,
            ]),
            r: coords_line.get_line_item(3)?,
            index: section.get_line(0)?.get_line_item(0)?,
            links: (2..num_links+2).into_iter()
                .map(|line_no| -> std::result::Result<_, Self::Error> {
                    section.get_line(line_no)?.get_line_item(0)
                })
                .collect::<Result<Vec<_>, _>>()?
        })
    }
}


// ************************
//    Utility Functions
// ************************

static SPAWN_METHOD_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\$\d?)").unwrap());

/// Retrieves Spawn Method, Internal Name, and Carrying Item from a combined
/// internal identifier as used by TekiInfo and CapInfo.
fn extract_internal_identifier(internal_combined_name: &str) -> (Option<String>, String, Option<String>) {
    let spawn_method_match = SPAWN_METHOD_RE.find_at(internal_combined_name, 0);
    let (spawn_method, internal_combined_name) = if let Some(mtch) = spawn_method_match {
        (Some(mtch.as_str().strip_prefix('$').unwrap().to_owned()), &internal_combined_name[mtch.end()..])
    }
    else {
        (None, internal_combined_name)
    };

    let teki = internal_combined_name.split('_')
        .enumerate()
        .take_while(|(i, part)| i == &0 || part.chars().next().unwrap().is_ascii_uppercase() || part == &"s" || part == &"l")
        .map(|(_, part)| part)
        .join("_");
    let treasure_name = internal_combined_name.split('_')
        .enumerate()
        .skip_while(|(i, part)| i == &0 || part.chars().next().unwrap().is_ascii_uppercase() || part == &"s" || part == &"l")
        .map(|(_, part)| part)
        .join("_");
    let treasure = if treasure_name.is_empty() { None } else { Some(treasure_name) };

    // Some special teki have an "F" variant that doesn't move. These are treated as separate
    // teki in code but use the same assets, so we normalize them here.
    let teki = match teki.as_str() {
        "FminiHoudai" => "MiniHoudai".to_string(),
        "Fkabuto" => "Kabuto".to_string(),
        _ => teki,
    };

    (spawn_method, teki, treasure)
}
