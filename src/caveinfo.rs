mod gamedata;
/// CaveInfo is a representation of the generation parameters for a given
/// sublevel.
/// For example, each sublevel's CaveInfo file specifies what treasures to
/// spawn, what map tiles can be used, what enemies can be spawned and where,
/// various parameters to determine the characteristics of the generated
/// layouts, and more. Each sublevel's CaveInfo is what makes it unique.
///
/// For info on the CaveInfo file format, see
/// https://pikmintkb.com/wiki/Cave_generation_parameters
mod parse;

pub use gamedata::*;

use cached::proc_macro::cached;
use encoding_rs::SHIFT_JIS;
use itertools::Itertools;
use once_cell::sync::Lazy;
use parse::parse_caveinfo;
use regex::Regex;
use std::{fs::File, io::Read};

#[derive(Debug, Clone)]
pub struct CaveInfo {
    pub num_floors: u8,
    pub floors: Vec<FloorInfo>,
}

impl From<Vec<[parse::Section<'_>; 5]>> for CaveInfo {
    fn from(raw_sections: Vec<[parse::Section<'_>; 5]>) -> CaveInfo {
        CaveInfo {
            num_floors: raw_sections.len() as u8,
            floors: raw_sections.into_iter().map(Into::into).collect(),
        }
    }
}

/// Corresponds to one "FloorInfo" segment in a CaveInfo file, plus all the
/// TekiInfo, ItemInfo, GateInfo, and CapInfo sections that follow it until
/// the next FloorInfo section begins or the file ends.
#[derive(Debug, Clone)]
pub struct FloorInfo {
    sublevel: u8, // 0-indexed
    max_main_objects: u8,
    max_treasures: u8,
    max_gates: u8,
    num_rooms: u8,             // Excludes corridors and caps/alcoves.
    corridor_probability: f32, // In range [0-1]. Less of a probability and more a relative scale of the floor:room ratio on the sublevel.
    cap_probability: f32, // In range [0-1]. (?) Probability of a cap (no spawn point) being generated instead of an alcove (has one spawn point).
    has_geyser: bool,
    exit_plugged: bool,
    cave_unit_definition_file_name: String,
    teki_info: Vec<TekiInfo>,
    item_info: Vec<ItemInfo>,
    gate_info: Vec<GateInfo>,
    cap_info: Vec<CapInfo>,
}

impl FloorInfo {
    /// This is the 'real' `From` impl, and it's separate for two reasons:
    /// 1. Being able to return an Option makes the code *way* nicer, and TryFrom
    ///    only exists for Result types.
    /// 2. The correct thing to do in the case of most errors this conversion could
    ///    encounter is to panic, so actually spending time fleshing out the errors
    ///    for a TryFrom implementation isn't worth it.
    fn convert_from_sections(raw_sections: [parse::Section<'_>; 5]) -> Option<FloorInfo> {
        let [floorinfo_section, tekiinfo_section, iteminfo_section, gateinfo_section, capinfo_section] =
            raw_sections;

        Some(FloorInfo {
            sublevel: floorinfo_section.get_tag("000")?,
            max_main_objects: floorinfo_section.get_tag("002")?,
            max_treasures: floorinfo_section.get_tag("003")?,
            max_gates: floorinfo_section.get_tag("004")?,
            num_rooms: floorinfo_section.get_tag("005")?,
            corridor_probability: floorinfo_section.get_tag("006")?,
            cap_probability: floorinfo_section.get_tag("014")?,
            has_geyser: floorinfo_section.get_tag::<u8>("007")? > 0,
            exit_plugged: floorinfo_section.get_tag::<u8>("010")? > 0,
            cave_unit_definition_file_name: floorinfo_section.get_tag("008")?,
            teki_info: tekiinfo_section.into(),
            item_info: iteminfo_section.into(),
            gate_info: gateinfo_section.into(),
            cap_info: capinfo_section.into(),
        })
    }
}

impl From<[parse::Section<'_>; 5]> for FloorInfo {
    fn from(raw_sections: [parse::Section<'_>; 5]) -> FloorInfo {
        FloorInfo::convert_from_sections(raw_sections)
            .expect("Failed to fetch all needed info from CaveInfo file.")
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
    internal_name: String,
    carrying: Option<String>, // The object held by this Teki, if any.
    minimum_amount: u8,
    filler_distribution_weight: u8, // https://pikmintkb.com/wiki/Cave_spawning#Weighted_distribution
    group: u8, // A.K.A. "Type" but "group" is used for convenience. https://pikmintkb.com/wiki/Cave_generation_parameters#Type
    spawn_method: Option<String>, // https://pikmintkb.com/wiki/Cave_generation_parameters#Spawn_method
}

impl TekiInfo {
    fn convert_from_section(section: parse::Section) -> Option<Vec<TekiInfo>> {
        section
            .lines
            .iter()
            .skip(1) // First line contains the number of Teki
            .tuples()
            .map(|(item_line, group_line)| -> Option<TekiInfo> {
                let internal_identifier = item_line.items.get(0)?;
                let amount_code = item_line.items.get(1)?;
                let group: u8 = group_line.items.get(0)?.parse().ok()?;

                let (spawn_method, internal_name, carrying) =
                    extract_internal_identifier(internal_identifier);

                // Determine amount and filler_distribution_weight based on teki type
                let minimum_amount: u8;
                let filler_distribution_weight: u8;
                if group == 6 {
                    // 6 is the group number for decorative teki
                    minimum_amount = amount_code.parse().ok()?;
                    filler_distribution_weight = 0;
                } else {
                    let (minimum_amount_str, filler_distribution_weight_str) =
                        amount_code.split_at(amount_code.len() - 1);
                    minimum_amount = minimum_amount_str.parse().ok()?;
                    filler_distribution_weight = filler_distribution_weight_str.parse().ok()?;
                }

                Some(TekiInfo {
                    internal_name,
                    carrying,
                    minimum_amount,
                    filler_distribution_weight,
                    group,
                    spawn_method,
                })
            })
            .collect()
    }
}

impl From<parse::Section<'_>> for Vec<TekiInfo> {
    fn from(section: parse::Section) -> Vec<TekiInfo> {
        TekiInfo::convert_from_section(section).expect("Couldn't decode invalid TekiInfo section.")
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

impl From<parse::Section<'_>> for Vec<ItemInfo> {
    fn from(section: parse::Section) -> Vec<ItemInfo> {
        section
            .lines
            .iter()
            .skip(1)
            .map(|line| -> Option<ItemInfo> {
                let amount_code_str = line.items.get(1)?;
                let (min_amount_str, filler_distribution_weight_str) =
                    amount_code_str.split_at(amount_code_str.len() - 1);
                Some(ItemInfo {
                    internal_name: line.items.get(0)?.to_string(),
                    min_amount: min_amount_str.parse().ok()?,
                    filler_distribution_weight: filler_distribution_weight_str.parse().ok()?,
                })
            })
            .collect::<Option<Vec<ItemInfo>>>()
            .expect("Failed to extract ItemInfo.")
    }
}

#[derive(Debug, Clone)]
pub struct GateInfo {
    health: f32,
    spawn_distribution_weight: u8, // https://pikmintkb.com/wiki/Cave_spawning#Weighted_distribution
}

impl From<parse::Section<'_>> for Vec<GateInfo> {
    fn from(section: parse::Section) -> Vec<GateInfo> {
        section
            .lines
            .iter()
            .skip(1)
            .tuples()
            .map(
                |(health_line, spawn_distribution_weight_line)| -> Option<GateInfo> {
                    Some(GateInfo {
                        health: health_line.items.get(1)?.parse().ok()?,
                        spawn_distribution_weight: spawn_distribution_weight_line
                            .items
                            .get(0)?
                            .chars()
                            .last()?
                            .to_digit(10)? as u8,
                    })
                },
            )
            .collect::<Option<Vec<GateInfo>>>()
            .expect("Failed to extract GateInfo.")
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

impl CapInfo {
    /// Almost an exact duplicate of the code for TekiInfo, which is unfortunately
    /// necessary with how the code is currently structured. May refactor in the future.
    fn convert_from_section(section: parse::Section) -> Option<Vec<CapInfo>> {
        section
            .lines
            .iter()
            .skip(1) // First line contains the number of Teki
            .tuples()
            .map(|(_, item_line, group_line)| -> Option<CapInfo> {
                let internal_identifier = item_line.items.get(0)?;
                let amount_code = item_line.items.get(1)?;
                let group: u8 = group_line.items.get(0)?.parse().ok()?;

                let (spawn_method, internal_name, carrying) =
                    extract_internal_identifier(internal_identifier);

                // Determine amount and filler_distribution_weight based on teki type
                let (minimum_amount_str, filler_distribution_weight_str) =
                    amount_code.split_at(amount_code.len() - 1);
                let minimum_amount = minimum_amount_str.parse().ok()?;
                let filler_distribution_weight = filler_distribution_weight_str.parse().ok()?;

                Some(CapInfo {
                    internal_name,
                    carrying,
                    minimum_amount,
                    filler_distribution_weight,
                    group,
                    spawn_method,
                })
            })
            .collect()
    }
}

impl From<parse::Section<'_>> for Vec<CapInfo> {
    fn from(section: parse::Section) -> Vec<CapInfo> {
        CapInfo::convert_from_section(section).expect("Couldn't decode invalid CapInfo section.")
    }
}

/// Loads the CaveInfo for an entire cave.
/// Should use `get_sublevel_info` in most cases.
#[cached]
pub fn get_caveinfo(cave: String) -> CaveInfo {
    // Load raw text of the caveinfo file
    let filename = cave_name_to_caveinfo_filename(&cave);
    let mut caveinfo_bytes: Vec<u8> = vec![];
    File::open(format!("./caveinfo/{}", filename))
        .expect(&format!(
            "Cannot find caveinfo file '{}' for cave '{}'",
            filename, cave
        ))
        .read_to_end(&mut caveinfo_bytes)
        .expect(&format!("Couldn't read caveinfo file '{}'!", filename));
    let caveinfo_raw: String = SHIFT_JIS.decode(&caveinfo_bytes).0.into_owned();

    // Send it off to the parsing mines
    let floor_chunks = parse_caveinfo(&caveinfo_raw)
        .expect(&format!("Couldn't parse CaveInfo file '{}'", filename))
        .1;

    CaveInfo::from(floor_chunks)
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

    let mut caveinfo = get_caveinfo(cave_name.to_string());

    // Make sure floor is in bounds to avoid panics
    if sublevel_num <= caveinfo.num_floors {
        Ok(caveinfo.floors.swap_remove((sublevel_num - 1u8) as usize))
    } else {
        Err(CaveInfoError::InvalidSublevel(sublevel.to_string()))
    }
}

#[derive(Debug)]
pub enum CaveInfoError {
    InvalidSublevel(String),
}

fn cave_name_to_caveinfo_filename(cave_name: &str) -> &'static str {
    match cave_name.to_ascii_lowercase().as_str() {
        "ec" => "tutorial_1.txt",
        "scx" => "tutorial_2.txt",
        "fc" => "tutorial_3.txt",
        "hob" => "forest_1.txt",
        "wfg" => "forest_2.txt",
        "bk" => "forest_3.txt",
        "sh" => "forest_4.txt",
        "cos" => "yakushima_1.txt",
        "gk" => "yakushima_2.txt",
        "sr" => "yakushima_3.txt",
        "smc" => "yakushima_4.txt",
        // TODO: add Wistful Wilds caves
        _ => panic!("Unrecognized cave name \"{}\"", cave_name),
    }
}

fn is_item_name(name: &str) -> bool {
    TREASURES
        .lock()
        .unwrap()
        .binary_search(&name.trim_start_matches('_'))
        .is_ok()
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
