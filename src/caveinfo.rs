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

use cached::proc_macro::cached;
use encoding_rs::SHIFT_JIS;
use parse::parse_caveinfo;
use std::{fs::File, io::Read};

#[derive(Debug, Clone)]
pub struct CaveInfo {
    num_floors: u8,
    floors: Vec<FloorInfo>,
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

        let sublevel: u8 = floorinfo_section.get_tag("000")?;
        let max_main_objects: u8 = floorinfo_section.get_tag("002")?;
        let max_treasures: u8 = floorinfo_section.get_tag("003")?;
        let max_gates: u8 = floorinfo_section.get_tag("004")?;
        let num_rooms: u8 = floorinfo_section.get_tag("005")?;
        let corridor_probability: f32 = floorinfo_section.get_tag("006")?;
        let has_geyser: bool = if floorinfo_section.get_tag::<u8>("007")? > 0 {
            true
        } else {
            false
        };
        let cave_unit_definition_file_name: String = floorinfo_section.get_tag("008")?;
        let exit_plugged: bool = if floorinfo_section.get_tag::<u8>("010")? > 0 {
            true
        } else {
            false
        };
        let cap_probability: f32 = floorinfo_section.get_tag("014")?;

        unimplemented!()
    }
}

impl From<[parse::Section<'_>; 5]> for FloorInfo {
    fn from(raw_sections: [parse::Section<'_>; 5]) -> FloorInfo {
        FloorInfo::convert_from_sections(raw_sections)
            .expect("Failed to fetch all needed info from CaveInfo file.")
    }
}

/// "Teki" is a Japanese word literally meaning "opponent" or "threat". This
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
    spawn_method: String, // https://pikmintkb.com/wiki/Cave_generation_parameters#Spawn_method
}

impl From<parse::Section<'_>> for Vec<TekiInfo> {
    fn from(section: parse::Section) -> Vec<TekiInfo> {
        unimplemented!()
    }
}

/// Defines 'loose' treasures, i.e. those that are not held by an enemy, but
/// rather sitting out in the open or buried.
#[derive(Debug, Clone)]
pub struct ItemInfo {
    internal_name: String,
    amount: u8,
}

impl From<parse::Section<'_>> for Vec<ItemInfo> {
    fn from(section: parse::Section) -> Vec<ItemInfo> {
        unimplemented!()
    }
}

#[derive(Debug, Clone)]
pub struct GateInfo {
    health: f32,
    spawn_distribution_weight: u8, // https://pikmintkb.com/wiki/Cave_spawning#Weighted_distribution
}

impl From<parse::Section<'_>> for Vec<GateInfo> {
    fn from(section: parse::Section) -> Vec<GateInfo> {
        unimplemented!()
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
/// like the ones found on FC1 and SCx2 (among many others).
#[derive(Debug, Clone)]
pub struct CapInfo {
    internal_name: String,
    carrying: Option<String>, // The object held by this Cap Teki, if any.
    minimum_amount: u8,
    filler_distribution_weight: u8, // https://pikmintkb.com/wiki/Cave_spawning#Weighted_distribution
    group: u8,                      // Does not control spawn location like it does in TekiInfo.
    spawn_method: String, // https://pikmintkb.com/wiki/Cave_generation_parameters#Spawn_method
}

impl From<parse::Section<'_>> for Vec<CapInfo> {
    fn from(section: parse::Section) -> Vec<CapInfo> {
        unimplemented!()
    }
}

// The cached macro doesn't work inside impl blocks, so this has to be a
// top-level function.
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

pub fn get_sublevel_info(sublevel: &str) -> FloorInfo {
    unimplemented!()
}

fn cave_name_to_caveinfo_filename(cave_name: &str) -> &'static str {
    match cave_name {
        "EC" => "tutorial_1.txt",
        "SCx" => "tutorial_2.txt",
        "FC" => "tutorial_3.txt",
        "HoB" => "forest_1.txt",
        "WFG" => "forest_2.txt",
        "BK" => "forest_3.txt",
        "SH" => "forest_4.txt",
        "CoS" => "yakushima_1.txt",
        "GK" => "yakushima_2.txt",
        "SR" => "yakushima_3.txt",
        "SmC" => "yakushima_4.txt",
        // TODO: add Wistful Wilds caves
        _ => panic!("Unrecognized cave name \"{}\"", cave_name),
    }
}
