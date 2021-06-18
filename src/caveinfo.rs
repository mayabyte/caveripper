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
use std::{fs::File, io::Read};
use parse::parse_caveinfo;

#[derive(Clone)]
pub struct CaveInfo {
    num_floors: u8,
    floors: Vec<FloorInfo>,
}

/// Corresponds to one "FloorInfo" segment in a CaveInfo file, plus all the
/// TekiInfo, ItemInfo, GateInfo, and CapInfo sections that follow it until
/// the next FloorInfo section begins or the file ends.
#[derive(Clone)]
pub struct FloorInfo {
    sublevel: u8,  // 0-indexed
    max_main_objects: u8,
    max_treasures: u8,
    max_gates: u8,
    num_rooms: u8,  // Excludes corridors and caps/alcoves.
    corridor_probability: f32,  // In range [0-1]. Less of a probability and more a relative scale of the floor:room ratio on the sublevel.
    cap_probability: f32,  // In range [0-1]. (?) Probability of a cap (no spawn point) being generated instead of an alcove (has one spawn point).
    has_geyser: bool,
    exit_plugged: bool,
    // cave_unit_definition_file,
    teki_info: Vec<TekiInfo>,
    item_info: Vec<ItemInfo>,
    gate_info: Vec<GateInfo>,
    cap_info: Vec<CapInfo>,
}

/// "Teki" is a Japanese word literally meaning "opponent" or "threat". This
/// is the game's internal name collectively given to enemies (Bulborbs,
/// Sheargrubs, etc.), hazards (poison geysers, electric sparkers, bomb rocks,
/// etc.), plants, and some other objects such as eggs. Most things in caves
/// that aren't either treasures or gates are considered Teki.
/// Treasures held inside enemies *are* defined in TekiInfo, however. See the
/// `carrying` field.
#[derive(Clone)]
pub struct TekiInfo {
    internal_name: String,
    carrying: Option<String>,  // The object held by this Teki, if any.
    minimum_amount: u8,
    filler_distribution_weight: u8,  // https://pikmintkb.com/wiki/Cave_spawning#Weighted_distribution
    group: u8,  // A.K.A. "Type" but "group" is used for convenience. https://pikmintkb.com/wiki/Cave_generation_parameters#Type
    spawn_method: String,  // https://pikmintkb.com/wiki/Cave_generation_parameters#Spawn_method
}

/// Defines 'loose' treasures, i.e. those that are not held by an enemy, but
/// rather sitting out in the open or buried.
#[derive(Clone)]
pub struct ItemInfo {
    internal_name: String,
    amount: u8,
}

#[derive(Clone)]
pub struct GateInfo {
    health: f32,
    spawn_distribution_weight: u8,  // https://pikmintkb.com/wiki/Cave_spawning#Weighted_distribution
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
#[derive(Clone)]
pub struct CapInfo {
    internal_name: String,
    carrying: Option<String>,  // The object held by this Cap Teki, if any.
    minimum_amount: u8,
    filler_distribution_weight: u8,  // https://pikmintkb.com/wiki/Cave_spawning#Weighted_distribution
    group: u8,  // Does not control spawn location like it does in TekiInfo.
    spawn_method: String,  // https://pikmintkb.com/wiki/Cave_generation_parameters#Spawn_method
}


impl CaveInfo {
}

// The cached macro doesn't work inside impl blocks, so this has to be a
// top-level function.
#[cached]
fn get_caveinfo(cave: String) -> CaveInfo {
    // Load raw text of the caveinfo file
    let filename = cave_name_to_caveinfo_filename(&cave);
    let mut caveinfo_raw = String::new();
    File::open(format!("./caveinfo/{}", filename))
        .expect(&format!("Cannot find caveinfo file '{}' for cave '{}'", filename, cave))
        .read_to_string(&mut caveinfo_raw)
        .expect(&format!("Couldn't read caveinfo file '{}'!", filename));

    // Send it off to the parsing mines
    parse_caveinfo(&caveinfo_raw)
        .expect(&format!("Couldn't parse CaveInfo file '{}'", filename))
        .1
}

pub fn get_sublevel_info(sublevel: &str) -> FloorInfo {
    unimplemented!()
}

fn cave_name_to_caveinfo_filename(cave_name: &str) -> &'static str {
    match cave_name {
        "EC" => "tutorial1.txt",
        "SCx" => "tutorial2.txt",
        "FC" => "tutorial3.txt",
        "HoB" => "forest1.txt",
        "WFG" => "forest2.txt",
        "BK" => "forest3.txt",
        "SH" => "forest4.txt",
        "CoS" => "yakushima1.txt",
        "GK" => "yakushima2.txt",
        "SR" => "yakushima3.txt",
        "SmC" => "yakushima4.txt",
        // TODO: add Wistful Wilds caves
        _ => panic!("Unrecognized cave name \"{}\"", cave_name),
    }
}
