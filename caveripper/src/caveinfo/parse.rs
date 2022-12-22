/// Parsing for CaveInfo files
use crate::{
    caveinfo::{
        util::{expand_rotations, sort_cave_units},
        CaveInfo, TekiInfo, ItemInfo, CapInfo, GateInfo,
        DoorLink, DoorUnit, CaveUnit, SpawnPoint, RoomType,
        Waterbox, Waypoint
    },
    errors::CaveripperError,
    assets::{AssetManager, Treasure, CaveConfig}
};
use itertools::Itertools;
use once_cell::sync::Lazy;
use pest::{Parser, iterators::Pair};
use pest_derive::Parser;
use regex::Regex;
use std::{str::FromStr, path::PathBuf, error::Error};
use error_stack::{Result, ResultExt, IntoReport, report, Report};

#[derive(Parser)]
#[grammar = "caveinfo/p2_cfg_grammar.pest"]
struct CaveinfoParser;


fn parse_sections(file_contents: &str) -> Result<impl Iterator<Item = Section>, CaveripperError> {
    let pairs = CaveinfoParser::parse(Rule::section_file, file_contents)
        .into_report().change_context(CaveripperError::CaveinfoError)?
        .next().unwrap();
    Ok(pairs.into_inner().map(Section::from))
}

/// Takes the entire raw text of a CaveInfo file and parses it into several
/// CaveInfo structs - one for each floor - ready for passing to the generator.
pub(crate) fn parse_caveinfo(cave_cfg: &CaveConfig) -> Result<Vec<CaveInfo>, CaveripperError> {
    let caveinfo_txt = AssetManager::get_txt_file(cave_cfg.get_caveinfo_path())
        .change_context(CaveripperError::CaveinfoError)?;

    let mut caveinfos = parse_sections(caveinfo_txt)
        .attach_printable_lazy(|| format!("Failed to parse {} into sections", cave_cfg.caveinfo_filename))?
        .skip(1)
        .chunks(5).into_iter()
        .map(|section_chunk| -> Result<CaveInfo, CaveripperError> {
            let (header, teki, item, gate, cap) = section_chunk
                .collect_tuple()
                .ok_or(CaveripperError::CaveinfoError)?;

            println!("{teki:#?}");

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
                cave_units: expand_rotations(
                    sort_cave_units(
                        parse_unitfile(&header.get_tag::<String>("{f008}")?, cave_cfg)?)),
                teki_info: teki.try_into()?,
                item_info: item.try_into()?,
                gate_info: gate.try_into()?,
                cap_info: cap.try_into()?,
                is_final_floor: false,
                waterwraith_timer: header.get_tag("{f016}")?,
            })
        })
        .collect::<Result<Vec<CaveInfo>, CaveripperError>>()?;
    caveinfos.last_mut().unwrap().is_final_floor = true;

    Ok(caveinfos)
}

fn parse_unitfile(unitfile: &str, cave_cfg: &CaveConfig) -> Result<Vec<CaveUnit>, CaveripperError> {
    let unitfile_path = PathBuf::from(&cave_cfg.game).join("unitfiles").join(unitfile);
    let unitfile_txt = AssetManager::get_txt_file(unitfile_path)
        .change_context(CaveripperError::CaveinfoError)?;
    parse_sections(unitfile_txt)?
        .map(|section| try_parse_caveunit(section, cave_cfg))
        .collect()
}

fn try_parse_caveunit(section: Section, cave: &CaveConfig) -> Result<CaveUnit, CaveripperError> {
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
                |doorunit_lines: &[InfoLine]| -> Result<DoorUnit, CaveripperError> {
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
                .collect::<Result<Vec<SpawnPoint>, CaveripperError>>()
                .attach(layoutfile_path)?
        },
        Err(_) => Vec::new(),
    };

    // Waterboxes file
    let waterboxes = match AssetManager::get_txt_file(
        &PathBuf::from(&cave.game).join("mapunits").join(&unit_folder_name).join("texts/waterbox.txt")
    ) {
        Ok(waterboxes_file_txt) => {
            parse_sections(waterboxes_file_txt)?.next().unwrap().try_into()
                .attach_printable_lazy(|| format!("{unit_folder_name}/texts/waterbox.txt"))?
        },
        Err(_) => Vec::new(),
    };

    // route.txt file (Waypoints)
    let waypoints_file_txt = AssetManager::get_txt_file(
        &PathBuf::from(&cave.game).join("mapunits").join(&unit_folder_name).join("texts/route.txt"))
        .change_context(CaveripperError::CaveinfoError)?;
    let waypoints = parse_sections(waypoints_file_txt)
        .attach_printable_lazy(|| format!("{unit_folder_name}/texts/route.txt"))?
        .map(TryInto::try_into)
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
                pos_x: 0.0,
                pos_y: 0.0,
                pos_z: 0.0,
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

/// One 'section' enclosed by curly brackets in a CaveInfo file.
#[derive(Clone, Debug)]
struct Section<'a> {
    pub lines: Vec<InfoLine<'a>>,
}

impl<'a> From<Pair<'a, Rule>> for Section<'a> {
    fn from(pair: Pair<'a, Rule>) -> Self {
        let lines: Vec<_> = pair.into_inner()
            .map(|section_line| {
                let mut line_items = section_line.into_inner()
                    .map(|li| li.as_str())
                    .peekable();
                if let Some(item) = line_items.peek() && item.starts_with('{') && item.ends_with('}') {
                    InfoLine {
                        tag: line_items.next(),
                        items: line_items.collect(),
                    }
                }
                else {
                    InfoLine { tag: None, items: line_items.collect() }
                }
            })
            .collect();
        Section { lines }
    }
}

impl<'a> Section<'a> {
    /// Gets and parses the one useful value out of a tagged CaveInfo line.
    /// See https://pikmintkb.com/wiki/Cave_generation_parameters#FloorInfo
    pub fn get_tag<T: FromStr>(&self, tag: &str) -> Result<T, CaveripperError>
    where <T as FromStr>::Err: Error + Send + Sync + 'static
    {
        self.get_nth_tag(tag, 1)
    }

    pub fn get_nth_tag<T: FromStr>(&self, tag: &str, idx: usize) -> Result<T, CaveripperError>
    where <T as FromStr>::Err: Error + Send + Sync + 'static
    {
        self.get_tagged_line(tag)?
            .get(idx).ok_or(report!(CaveripperError::CaveinfoError))?
            .parse()
            .into_report().change_context(CaveripperError::CaveinfoError).attach_printable_lazy(|| tag.to_string())
    }

    pub fn get_tagged_line(&self, tag: &str) -> Result<&Vec<&'a str>, CaveripperError> {
        self.lines.iter()
            .find(|line| line.tag.contains(&tag))
            .map(|line| &line.items)
            .ok_or(report!(CaveripperError::CaveinfoError))
    }

    pub fn get_line(&self, index: usize) -> Result<&InfoLine, CaveripperError> {
        self.lines.get(index)
            .ok_or(report!(CaveripperError::CaveinfoError))
    }
}

/// Simple helper struct to make working with individual lines easier.
#[derive(Clone, Debug)]
struct InfoLine<'a> {
    pub tag: Option<&'a str>,
    pub items: Vec<&'a str>,
}

impl InfoLine<'_> {
    pub fn get_line_item<T: FromStr>(&self, item: usize) -> Result<T, CaveripperError>
    where <T as FromStr>::Err: Error + Send + Sync + 'static
    {
        self.items.get(item)
            .ok_or(report!(CaveripperError::CaveinfoError))?
            .parse()
            .into_report().change_context(CaveripperError::CaveinfoError)
    }
}

impl TryFrom<Section<'_>> for Vec<TekiInfo> {
    type Error = Report<CaveripperError>;
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
    type Error = Report<CaveripperError>;
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
    type Error = Report<CaveripperError>;
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
    type Error = Report<CaveripperError>;
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
    type Error = Report<CaveripperError>;
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
    type Error = Report<CaveripperError>;
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
    type Error = Report<CaveripperError>;
    fn try_from(section: Section) -> std::result::Result<SpawnPoint, Self::Error> {
        Ok(
            SpawnPoint {
                group: section.get_line(0)?.get_line_item(0)?,
                pos_x: section.get_line(1)?.get_line_item(0)?,
                pos_y: section.get_line(1)?.get_line_item(1)?,
                pos_z: section.get_line(1)?.get_line_item(2)?,
                angle_degrees: section.get_line(2)?.get_line_item(0)?,
                radius: section.get_line(3)?.get_line_item(0)?,
                min_num: section.get_line(4)?.get_line_item(0)?,
                max_num: section.get_line(5)?.get_line_item(0)?,
            }
        )
    }
}

impl TryFrom<Section<'_>> for Vec<Waterbox> {
    type Error = Report<CaveripperError>;
    fn try_from(section: Section<'_>) -> std::result::Result<Self, Self::Error> {
        let num_waterboxes: usize = section.get_line(0)?.get_line_item(0)?;
        let mut waterboxes = Vec::with_capacity(num_waterboxes);
        for i in 0..num_waterboxes {
            waterboxes[i+1] = Waterbox {
                x1: section.get_line(i+1)?.get_line_item(0)?,
                y1: section.get_line(i+1)?.get_line_item(1)?,
                z1: section.get_line(i+1)?.get_line_item(2)?,
                x2: section.get_line(i+1)?.get_line_item(3)?,
                y2: section.get_line(i+1)?.get_line_item(4)?,
                z2: section.get_line(i+1)?.get_line_item(5)?,
            }
        }
        Ok(waterboxes)
    }
}

impl TryFrom<Section<'_>> for Waypoint {
    type Error = Report<CaveripperError>;
    fn try_from(section: Section<'_>) -> std::result::Result<Self, Self::Error> {
        let num_links: usize = section.get_line(1)?.get_line_item(0)?;
        let coords_line = section.get_line(num_links + 2)?;
        Ok(Waypoint {
            x: coords_line.get_line_item(0)?,
            y: coords_line.get_line_item(1)?,
            z: coords_line.get_line_item(2)?,
            r: coords_line.get_line_item(3)?,
            index: section.get_line(0)?.get_line_item(0)?,
            links: (2..num_links+2).into_iter()
                .map(|line_no| -> std::result::Result<_, Self::Error> {
                    Ok(section.get_line(line_no)?.get_line_item(0)?)
                })
                .collect::<Result<Vec<_>, _>>()?
        })
    }
}


// ************************
//    Utility Functions
// ************************

/// Retrieves Spawn Method, Internal Name, and Carrying Item from a combined
/// internal identifier as used by TekiInfo and CapInfo.
static INTERNAL_IDENTIFIER_RE: Lazy<Regex> = Lazy::new(|| {
    // Captures an optional Spawn Method and the Internal Name with the
    // Carrying item still attached.
    Regex::new(r"(\$\d?)?([A-Za-z_-]+)").unwrap()
});
fn extract_internal_identifier(internal_combined_name: &str) -> (Option<String>, String, Option<Treasure>) {
    let captures = INTERNAL_IDENTIFIER_RE
        .captures(internal_combined_name)
        .unwrap_or_else(|| panic!("Not able to capture info from combined internal identifier {internal_combined_name}!"));

    // Extract spawn method
    let spawn_method = captures.get(1)
        .map(|s| s.as_str())
        .and_then(|sm| sm.strip_prefix('$'))
        .map(|spawn_method| spawn_method.to_string());
    let mut combined_name = captures.get(2).unwrap().as_str();

    // Some teki have an 'F' at the beginning of their name, indicating that they're
    // fixed in place (e.g. tower groink on scx7). Remove this if it's present.
    if let Some(candidate) = combined_name.strip_prefix('F') {
        if AssetManager::teki_list().expect("No teki list!").iter().any(|teki| candidate.to_ascii_lowercase().starts_with(teki)) {
            combined_name = candidate;
        }
    }

    // Attempt to separate the candidate name into a teki and treasure component.
    // Teki carrying treasures are written as "Tekiname_Treasurename", but unfortunately
    // both teki and treasures can have underscores as part of their names, so splitting
    // the two is non-trivial. To make things worse, some treasure names are strict
    // prefixes or suffixes of others ('fire', 'fire_helmet', 'suit_fire'). The only robust
    // way I've found to ensure the right teki/treasure combination is extracted is to
    // exhaustively check against all possible combinations of teki and treasure names.
    // This is an expensive operation, but this should only have to be done at caveinfo
    // loading time so it shouldn't affect performance where it matters.
    if combined_name.contains('_') {
        let combined_name_lower = combined_name.to_ascii_lowercase();
        for (teki, treasure) in AssetManager::teki_list().expect("No teki list!")
            .iter()
            .cartesian_product(AssetManager::treasure_list().expect("No treasure list!").iter())
        {
            // Check full string equality rather than prefix/suffix because
            // some treasure names are suffixes of others.
            if format!("{}_{}", teki, treasure.internal_name) == combined_name_lower {
                return (spawn_method, teki.clone(), Some(treasure.clone()));
            }
        }
    }

    (spawn_method, combined_name.to_string(), None)
}
