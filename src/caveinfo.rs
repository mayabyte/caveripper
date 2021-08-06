mod caveinfoerror;
mod gamedata;
mod parse;
#[cfg(test)]
mod test;

/// CaveInfo is a representation of the generation parameters for a given
/// sublevel.
/// For example, each sublevel's CaveInfo file specifies what treasures to
/// spawn, what map tiles can be used, what enemies can be spawned and where,
/// various parameters to determine the characteristics of the generated
/// layouts, and more. Each sublevel's CaveInfo is what makes it unique.
///
/// For info on the CaveInfo file format, see
/// https://pikmintkb.com/wiki/Cave_generation_parameters
pub use caveinfoerror::CaveInfoError;
pub use gamedata::*;

use cached::proc_macro::cached;
use encoding_rs::SHIFT_JIS;
use itertools::Itertools;
use once_cell::sync::Lazy;
use parse::{parse_cave_unit_definition, parse_caveinfo, parse_cave_unit_layout_file};
use regex::Regex;
use std::{cmp::Ordering, convert::{TryFrom, TryInto}, fs::File, io::Read};

#[derive(Debug, Clone)]
pub struct CaveInfo {
    pub num_floors: u8,
    pub floors: Vec<FloorInfo>,
}

impl TryFrom<Vec<[parse::Section<'_>; 5]>> for CaveInfo {
    type Error = CaveInfoError;
    fn try_from(raw_sections: Vec<[parse::Section<'_>; 5]>) -> Result<CaveInfo, CaveInfoError> {
        Ok(CaveInfo {
            num_floors: raw_sections.len() as u8,
            floors: raw_sections
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

/// Corresponds to one "FloorInfo" segment in a CaveInfo file, plus all the
/// TekiInfo, ItemInfo, GateInfo, and CapInfo sections that follow it until
/// the next FloorInfo section begins or the file ends.
#[derive(Debug, Clone)]
pub struct FloorInfo {
    pub sublevel: u16, // 0-indexed
    pub max_main_objects: u16,
    pub max_treasures: u16,
    pub max_gates: u16,
    pub num_rooms: u16,             // Excludes corridors and caps/alcoves.
    pub corridor_probability: f32, // In range [0-1]. Less of a probability and more a relative scale of the floor:room ratio on the sublevel.
    pub cap_probability: f32, // In range [0-1]. (?) Probability of a cap (no spawn point) being generated instead of an alcove (has one spawn point).
    pub has_geyser: bool,
    pub exit_plugged: bool,
    pub cave_units: Vec<CaveUnit>,
    pub teki_info: Vec<TekiInfo>,
    pub item_info: Vec<ItemInfo>,
    pub gate_info: Vec<GateInfo>,
    pub cap_info: Vec<CapInfo>,
}

impl FloorInfo {
    pub fn teki_group(&self, group: u16) -> impl Iterator<Item=&TekiInfo> {
        self.teki_info.iter().filter(move |teki| teki.group == group)
    }

    pub fn max_num_doors_single_unit(&self) -> usize {
        self.cave_units.iter().map(|unit| unit.num_doors).max().unwrap_or_default()
    }
}

impl TryFrom<[parse::Section<'_>; 5]> for FloorInfo {
    type Error = CaveInfoError;
    fn try_from(raw_sections: [parse::Section<'_>; 5]) -> Result<FloorInfo, CaveInfoError> {
        let [floorinfo_section, tekiinfo_section, iteminfo_section, gateinfo_section, capinfo_section] =
            raw_sections;

        let cave_unit_definition_file_name: String = floorinfo_section.get_tag("008")?;
        let cave_unit_definition_text = read_file_to_string(format!("./units/{}", &cave_unit_definition_file_name))?;
        let (_, cave_unit_sections) = parse_cave_unit_definition(&cave_unit_definition_text)
            .expect("Couldn't parse Cave Unit Definition file!");

        Ok(FloorInfo {
            sublevel: floorinfo_section.get_tag("000")?,
            max_main_objects: floorinfo_section.get_tag("002")?,
            max_treasures: floorinfo_section.get_tag("003")?,
            max_gates: floorinfo_section.get_tag("004")?,
            num_rooms: floorinfo_section.get_tag("005")?,
            corridor_probability: floorinfo_section.get_tag("006")?,
            cap_probability: floorinfo_section.get_tag("014")?,
            has_geyser: floorinfo_section.get_tag::<u8>("007")? > 0,
            exit_plugged: floorinfo_section.get_tag::<u8>("010")? > 0,
            cave_units: expand_rotations(
                sort_cave_units(
                    cave_unit_sections
                        .into_iter()
                        .map(TryInto::try_into)
                        .collect::<Result<Vec<_>, _>>()?
                )
            ),
            teki_info: tekiinfo_section.try_into()?,
            item_info: iteminfo_section.try_into()?,
            gate_info: gateinfo_section.try_into()?,
            cap_info: capinfo_section.try_into()?,
        })
    }
}

/// "Teki" ("敵") is a Japanese word literally meaning "opponent" or "threat". This
/// is the game's internal name collectively given to enemies (Bulborbs,
/// Sheargrubs, etc.), hazards (poison geysers, electric sparkers, bomb rocks,
/// etc.), plants, and some other objects such as eggs. Most things in caves
/// that aren't either treasures or gates are considered Teki.
/// Treasures held inside enemies *are* defined in TekiInfo, however. See the
/// `carrying` field.
#[derive(Debug, Clone)]
pub struct TekiInfo {
    pub internal_name: String,
    pub carrying: Option<String>, // The object held by this Teki, if any.
    pub minimum_amount: u16,
    pub filler_distribution_weight: u16, // https://pikmintkb.com/wiki/Cave_spawning#Weighted_distribution
    pub group: u16, // A.K.A. "Type" but "group" is used for convenience. https://pikmintkb.com/wiki/Cave_generation_parameters#Type
    pub spawn_method: Option<String>, // https://pikmintkb.com/wiki/Cave_generation_parameters#Spawn_method
}

impl TryFrom<parse::Section<'_>> for Vec<TekiInfo> {
    type Error = CaveInfoError;
    fn try_from(section: parse::Section) -> Result<Vec<TekiInfo>, CaveInfoError> {
        section
            .lines
            .iter()
            .skip(1) // First line contains the number of Teki
            .tuples()
            .map(
                |(item_line, group_line)| -> Result<TekiInfo, CaveInfoError> {
                    let internal_identifier = item_line.get_line_item(0)?;
                    let amount_code = item_line.get_line_item(1)?;
                    let group: u16 = group_line.get_line_item(0)?.parse()?;

                    let (spawn_method, internal_name, carrying) =
                        extract_internal_identifier(internal_identifier);

                    // Determine amount and filler_distribution_weight based on teki type
                    let minimum_amount: u16;
                    let filler_distribution_weight: u16;
                    if group == 6 {
                        // 6 is the group number for decorative teki
                        minimum_amount = amount_code.parse()?;
                        filler_distribution_weight = 0;
                    } else {
                        let (minimum_amount_str, filler_distribution_weight_str) =
                            amount_code.split_at(amount_code.len() - 1);

                        // If there is only one digit, it represents the filler_distribution_weight
                        // and minimum_amount defaults to 0.
                        minimum_amount = minimum_amount_str.parse().unwrap_or(0);
                        filler_distribution_weight = filler_distribution_weight_str.parse()?;
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
            .collect()
    }
}

/// Defines 'loose' treasures, i.e. those that are not held by an enemy, but
/// rather sitting out in the open or buried.
#[derive(Debug, Clone)]
pub struct ItemInfo {
    internal_name: String,
    min_amount: u8,
    filler_distribution_weight: u8,
}

impl TryFrom<parse::Section<'_>> for Vec<ItemInfo> {
    type Error = CaveInfoError;
    fn try_from(section: parse::Section) -> Result<Vec<ItemInfo>, CaveInfoError> {
        section
            .lines
            .iter()
            .skip(1)
            .map(|line| -> Result<ItemInfo, CaveInfoError> {
                let amount_code_str = line.get_line_item(1)?;
                let (min_amount_str, filler_distribution_weight_str) =
                    amount_code_str.split_at(amount_code_str.len() - 1);
                Ok(ItemInfo {
                    internal_name: line.get_line_item(0)?.to_string(),
                    min_amount: min_amount_str.parse()?,
                    filler_distribution_weight: filler_distribution_weight_str.parse()?,
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct GateInfo {
    health: f32,
    spawn_distribution_weight: u8, // https://pikmintkb.com/wiki/Cave_spawning#Weighted_distribution
}

impl TryFrom<parse::Section<'_>> for Vec<GateInfo> {
    type Error = CaveInfoError;
    fn try_from(section: parse::Section) -> Result<Vec<GateInfo>, CaveInfoError> {
        section
            .lines
            .iter()
            .skip(1)
            .tuples()
            .map(
                |(health_line, spawn_distribution_weight_line)| -> Result<GateInfo, CaveInfoError> {
                    Ok(GateInfo {
                        health: health_line.get_line_item(1)?.parse()?,
                        spawn_distribution_weight: spawn_distribution_weight_line
                            .get_line_item(0)?
                            .chars()
                            .last()
                            .ok_or(CaveInfoError::MalformedLine)?
                            .to_digit(10)
                            .ok_or(CaveInfoError::ParseValueError)?
                            as u8,
                    })
                },
            )
            .collect()
    }
}

/// CapInfo specifies what objects will spawn in dedicated Cap/Alcove spawn
/// points. This is similar to TekiInfo, but with a few key differences:
///  1. `group` controls the amount of objects allowed to spawn, not spawn
///     location.
///  2. 'Loose' treasures can be spawned in CapInfo, unlike TekiInfo.
///  3. Objects spawned from CapInfo don't count towards any maximums of their
///     object type, such as `max_main_objects` in FloorInfo.
///
/// CapInfo is most frequently used for falling eggs and falling bomb rocks,
/// like the ones found on FC1 and SCx2 (among many others). However, there
/// are also couple easy-to-spot Cap Enemies such as the second Orange Bulborb
/// on BK1 that faces directly out of its cap.
#[derive(Debug, Clone)]
pub struct CapInfo {
    internal_name: String,
    carrying: Option<String>, // The object held by this Cap Teki, if any.
    minimum_amount: u8,
    filler_distribution_weight: u8, // https://pikmintkb.com/wiki/Cave_spawning#Weighted_distribution
    group: u8,                      // Does not control spawn location like it does in TekiInfo.
    spawn_method: Option<String>, // https://pikmintkb.com/wiki/Cave_generation_parameters#Spawn_method
}

impl TryFrom<parse::Section<'_>> for Vec<CapInfo> {
    /// Almost an exact duplicate of the code for TekiInfo, which is unfortunately
    /// necessary with how the code is currently structured. May refactor in the future.
    type Error = CaveInfoError;
    fn try_from(section: parse::Section) -> Result<Vec<CapInfo>, CaveInfoError> {
        section
            .lines
            .iter()
            .skip(1) // First line contains the number of Teki
            .tuples()
            .map(
                |(_, item_line, group_line)| -> Result<CapInfo, CaveInfoError> {
                    let internal_identifier = item_line.get_line_item(0)?;
                    let amount_code = item_line.get_line_item(1)?;
                    let group: u8 = group_line.get_line_item(0)?.parse()?;

                    let (spawn_method, internal_name, carrying) =
                        extract_internal_identifier(internal_identifier);

                    // Determine amount and filler_distribution_weight based on teki type
                    let (minimum_amount_str, filler_distribution_weight_str) =
                        amount_code.split_at(amount_code.len() - 1);
                    // If there is only one digit, it represents the filler_distribution_weight
                    // and minimum_amount defaults to 0.
                    let minimum_amount = minimum_amount_str.parse().unwrap_or(0);
                    let filler_distribution_weight = filler_distribution_weight_str.parse()?;

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

/// Cave Unit Definition files record info about what map tiles can be
/// generated on a given sublevel. Each CaveUnit represents one possible
/// map tile.
/// https://pikmintkb.com/wiki/Cave_unit_definition_file
#[derive(Debug, Clone)]
pub struct CaveUnit {
    pub unit_folder_name: String,
    pub width: u16,  // In cave grid cells, not in-game coords
    pub height: u16, // In cave grid cells, not in-game coords
    pub room_type: RoomType,
    pub num_doors: usize,
    pub doors: Vec<DoorUnit>,
    pub rotation: u16,
    pub spawn_points: Vec<SpawnPoint>,
}

impl TryFrom<parse::Section<'_>> for CaveUnit {
    type Error = CaveInfoError;
    fn try_from(section: parse::Section) -> Result<CaveUnit, CaveInfoError> {
        let unit_folder_name = section.get_line(1)?.get_line_item(0)?.to_string();
        let width = section.get_line(2)?.get_line_item(0)?.parse()?;
        let height = section.get_line(2)?.get_line_item(1)?.parse()?;
        let room_type = section
            .get_line(3)?
            .get_line_item(0)?
            .parse::<usize>()?
            .into();
        let num_doors = section.get_line(5)?.get_line_item(0)?.parse()?;

        // DoorUnits
        let doors = if num_doors > 0 {
            let num_lines_per_door_unit = (section.lines.len() - 6) / num_doors;
            section.lines[6..]
                .chunks(num_lines_per_door_unit)
                .map(
                    |doorunit_lines: &[parse::InfoLine]| -> Result<DoorUnit, CaveInfoError> {
                        doorunit_lines.try_into()
                    },
                )
                .collect::<Result<Vec<_>, _>>()?
        } else {
            vec![]
        };

        // Cave Unit Layout File (spawn points)
        let spawn_points = match read_file_to_string(format!("./arc/{}/texts.d/layout.txt", unit_folder_name)) {
            Ok(cave_unit_layout_file_txt) => {
                let spawn_points_sections = parse_cave_unit_layout_file(&cave_unit_layout_file_txt)
                    .expect("Couldn't parse cave unit layout file!").1;
                spawn_points_sections.into_iter().map(TryInto::try_into).collect::<Result<Vec<_>, _>>()?
            },
            Err(CaveInfoError::MissingFileError(_)) => Vec::new(),
            Err(e) => return Err(e),
        };

        Ok(CaveUnit {
            unit_folder_name,
            width,
            height,
            room_type,
            num_doors,
            doors,
            rotation: 0,
            spawn_points,
        })
    }
}


/// Implementations for (PartialEq) and (Partial)Ord for CaveUnit.
/// The generation algorithm sorts units by total size (breaking ties with
/// number of doors) as the very first step, so this is important to understand.

impl PartialEq for CaveUnit {
    fn eq(&self, other: &Self) -> bool {
        (self.width * self.height) == (other.width * other.height) && self.num_doors == other.num_doors
    }
}
impl Eq for CaveUnit {}

impl PartialOrd for CaveUnit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for CaveUnit {
    fn cmp(&self, other: &Self) -> Ordering {
        let size_self = self.width * self.height;
        let size_other = other.width * other.height;
        if size_self == size_other {
            self.num_doors.cmp(&other.num_doors)
        } else {
            size_self.cmp(&size_other)
        }
    }
}

impl CaveUnit {
    pub fn copy_and_rotate_to(&self, rotation: u16) -> Self {
        let mut new_unit = self.clone();
        new_unit.rotation = rotation % 4;
        if rotation % 2 == 1 {
            new_unit.width = self.height;
            new_unit.height = self.width;
        }

        new_unit.doors.iter_mut()
            .for_each(|mut door| {
                // I have no idea what this is doing, but I've copied it as faithfully as I can.
                // https://github.com/JHaack4/CaveGen/blob/2c99bf010d2f6f80113ed7eaf11d9d79c6cff367/MapUnit.java#L72
                match door.direction {
                    0 | 2 if rotation == 2 || rotation == 3 => { door.side_lateral_offset = self.width - 1 - door.side_lateral_offset; }
                    1 | 3 if rotation == 1 || rotation == 2 => { door.side_lateral_offset = self.height - 1 - door.side_lateral_offset; },
                    _ => {/* do nothing */}
                }
                door.direction = (door.direction + rotation) % 4;
            });

        new_unit
    }

    pub fn has_start_spawnpoint(&self) -> bool {
        self.spawn_points.iter().any(|spawn_point| spawn_point.group == 7)
    }
}

/// The sorting algorithm required by the generation algorithm for cave units.
/// This sort is unstable! I've implemented it manually here to ensure it
/// exactly matches the one in Pikmin 2.
fn sort_cave_units(mut unsorted: Vec<CaveUnit>) -> Vec<CaveUnit> {
    // This is kinda like Bubble Sort, except it compares the entire
    // remaining list to the current element rather than just the next elem.
    let mut idx = 0;
    while idx < unsorted.len()-1 {
        // SAFETY: idx is always checked to be within [0,unsorted.len()-1), so is
        // always a valid index.
        while unsorted[idx+1..].iter().any(|elem| elem > unsafe{unsorted.get_unchecked(idx)}) {
            let current = unsorted.remove(idx);
            unsorted.push(current);
        }
        idx = idx + 1;
    }
    unsorted
}

/// Takes a Vec of CaveUnits and returns a vec with the same cave units, but
/// duplicated for each possible rotation.
fn expand_rotations(input: Vec<CaveUnit>) -> Vec<CaveUnit> {
    input.into_iter()
        .flat_map(|unit| [unit.clone(), unit.copy_and_rotate_to(1), unit.copy_and_rotate_to(2), unit.copy_and_rotate_to(3)])
        .collect()
}

#[derive(Debug, Clone)]
pub struct DoorUnit {
    pub direction: u16,         // 0, 1, 2, or 3
    pub side_lateral_offset: u16, // Appears to be the offset from center on the side of the room it's facing?
    pub waypoint_index: usize, // Index of the waypoint connected to this door
    pub num_links: usize,
    pub door_links: Vec<DoorLink>,  // Door links are other doors that are reachable through the room that hosts this door.
}

impl TryFrom<&[parse::InfoLine<'_>]> for DoorUnit {
    type Error = CaveInfoError;
    fn try_from(lines: &[parse::InfoLine]) -> Result<DoorUnit, CaveInfoError> {
        let direction = lines[1].get_line_item(0)?.parse()?;
        let side_lateral_offset = lines[1].get_line_item(1)?.parse()?;
        let waypoint_index = lines[1].get_line_item(2)?.parse()?;
        let num_links = lines[2].get_line_item(0)?.parse()?;
        let door_links = lines[3..]
            .into_iter()
            .map(|line| line.try_into())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DoorUnit {
            direction,
            side_lateral_offset,
            waypoint_index,
            num_links,
            door_links,
        })
    }
}

#[derive(Debug, Clone)]
pub struct DoorLink {
    distance: f32,
    door_id: usize,
    tekiflag: bool, // Whether or not a teki should spawn in the seam of this door
}

impl TryFrom<&parse::InfoLine<'_>> for DoorLink {
    type Error = CaveInfoError;
    fn try_from(line: &parse::InfoLine) -> Result<DoorLink, CaveInfoError> {
        let distance = line.get_line_item(0)?.parse()?;
        let door_id = line.get_line_item(1)?.parse()?;
        let tekiflag = line.get_line_item(2)?.parse::<u8>()? > 0;
        Ok(DoorLink {
            distance,
            door_id,
            tekiflag,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoomType {
    Room,
    Hallway,
    DeadEnd,
}

impl From<usize> for RoomType {
    fn from(roomtype: usize) -> RoomType {
        match roomtype {
            0 => RoomType::DeadEnd,
            1 => RoomType::Room,
            2 => RoomType::Hallway,
            _ => panic!("Invalid room type specified"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SpawnPoint {
    pub group: u16,
    pub pos_x: f32,
    pub pos_y: f32,
    pub pos_z: f32,
    pub angle_degrees: f32,
    pub radius: f32,
    pub min_num: u16,
    pub max_num: u16,
}

impl TryFrom<parse::Section<'_>> for SpawnPoint {
    type Error = CaveInfoError;
    fn try_from(section: parse::Section) -> Result<SpawnPoint, Self::Error> {
        Ok(
            SpawnPoint {
                group: section.get_line(0)?.get_line_item(0)?.parse()?,
                pos_x: section.get_line(1)?.get_line_item(0)?.parse()?,
                pos_y: section.get_line(1)?.get_line_item(1)?.parse()?,
                pos_z: section.get_line(1)?.get_line_item(2)?.parse()?,
                angle_degrees: section.get_line(2)?.get_line_item(0)?.parse()?,
                radius: section.get_line(3)?.get_line_item(0)?.parse()?,
                min_num: section.get_line(4)?.get_line_item(0)?.parse()?,
                max_num: section.get_line(5)?.get_line_item(0)?.parse()?,
            }
        )
    }
}

/// Loads the CaveInfo for an entire cave.
/// Should use `get_sublevel_info` in most cases.
#[cached]
pub fn get_caveinfo(cave: String) -> Result<CaveInfo, CaveInfoError> {
    // Load raw text of the caveinfo file
    let caveinfo_filename = cave_name_to_caveinfo_filename(&cave);
    let caveinfo_txt = read_file_to_string(format!("./caveinfo/{}", &caveinfo_filename))?;

    // Send it off to the parsing mines
    let floor_chunks = parse_caveinfo(&caveinfo_txt)
        .expect(&format!("Couldn't parse CaveInfo file '{}'", caveinfo_filename))
        .1;

    CaveInfo::try_from(floor_chunks)
}

/// Gets the CaveInfo for a single sublevel.
/// Argument is a 'qualified sublevel string', such as "FC3", "SCx2", etc.
static SUBLEVEL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\w+)[\s_-]?(\d+)").unwrap());
pub fn get_sublevel_info(sublevel: &str) -> Result<FloorInfo, CaveInfoError> {
    let captures = SUBLEVEL_RE
        .captures(sublevel)
        .ok_or_else(|| CaveInfoError::InvalidSublevel(sublevel.to_string()))?;

    let cave_name = captures.get(1).unwrap().as_str();
    let sublevel_num: u8 = captures.get(2).unwrap().as_str().parse().unwrap();

    let mut caveinfo = get_caveinfo(cave_name.to_string())?;

    // Make sure floor is in bounds to avoid panics
    if sublevel_num <= caveinfo.num_floors {
        Ok(caveinfo.floors.swap_remove((sublevel_num - 1u8) as usize))
    } else {
        Err(CaveInfoError::InvalidSublevel(sublevel.to_string()))
    }
}

/// Retrieves Spawn Method, Internal Name, and Carrying Item from a combined
/// internal identifier as used by TekiInfo and CapInfo.
static INTERNAL_IDENTIFIER_RE: Lazy<Regex> = Lazy::new(|| {
    // Captures an optional Spawn Method and the Internal Name with the
    // Carrying item still attached.
    Regex::new(r"(\$\d?)?([A-Za-z_-]+)").unwrap()
});
fn extract_internal_identifier(
    internal_combined_name: &str,
) -> (Option<String>, String, Option<String>) {
    let captures = INTERNAL_IDENTIFIER_RE
        .captures(internal_combined_name)
        .expect(&format!(
            "Not able to capture info from combined internal identifier {}!",
            internal_combined_name
        ));
    let spawn_method = captures.get(1).map(|s| s.as_str().to_string());
    let internal_combined_name = captures.get(2).unwrap().as_str().to_string();

    // Check if the captured Carried Item candidate is actually a carried item
    let (internal_name, carrying) = match internal_combined_name.rsplit_once('_') {
        Some((name, carrying)) if is_item_name(carrying) => {
            (name.to_string(), Some(carrying.to_string()))
        }
        _ => (internal_combined_name, None),
    };

    (spawn_method, internal_name, carrying)
}

fn read_file_to_string(path: String) -> Result<String, CaveInfoError> {
    let mut file_bytes: Vec<u8> = vec![];
    File::open(&path)
        .map_err(|_| CaveInfoError::MissingFileError(path))?
        .read_to_end(&mut file_bytes)
        .map_err(|err| CaveInfoError::FileReadError(err.to_string()))?;
    let cave_unit_definition_text: String =
        SHIFT_JIS.decode(&file_bytes).0.into_owned();
    Ok(cave_unit_definition_text)
}
