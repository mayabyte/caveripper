/// CaveInfo is a representation of the generation parameters for a given
/// sublevel.
/// For example, each sublevel's CaveInfo file specifies what treasures to
/// spawn, what map tiles can be used, what enemies can be spawned and where,
/// various parameters to determine the characteristics of the generated
/// layouts, and more. Each sublevel's CaveInfo is what makes it unique.
///
/// For info on the CaveInfo file format, see
/// https://pikmintkb.com/wiki/Cave_generation_parameters

mod util;
mod parse;
mod error;

use std::{cmp::Ordering, fmt::{Display, Formatter}, collections::HashSet};
use parse::parse_caveinfo;
use serde::Serialize;
use error_stack::{Result, Report, report, ResultExt};
use crate::{errors::CaveripperError, assets::CaveConfig, pikmin_math};


/// Corresponds to one "FloorInfo" segment in a CaveInfo file, plus all the
/// TekiInfo, ItemInfo, GateInfo, and CapInfo sections that follow it until
/// the next FloorInfo section begins or the file ends.
/// Essentially, this is the entire collection of information required to
/// generate one sublevel.
#[derive(Debug, Clone, Serialize)]
pub struct CaveInfo {
    pub cave_cfg: CaveConfig,  // Not part of the file format
    pub floor_num: u32, // 0-indexed
    pub max_main_objects: u32,
    pub max_treasures: u32,
    pub max_gates: u32,
    pub num_rooms: u32,             // Excludes corridors and caps/alcoves.
    pub corridor_probability: f32, // In range [0-1]. Less of a probability and more a relative scale of the floor:room ratio on the sublevel.
    pub cap_probability: f32, // In range [0-1]. (?) Probability of a cap (no spawn point) being generated instead of an alcove (has one spawn point).
    pub has_geyser: bool,
    pub exit_plugged: bool,
    pub cave_units: Vec<CaveUnit>,
    pub teki_info: Vec<TekiInfo>,
    pub item_info: Vec<ItemInfo>,
    pub gate_info: Vec<GateInfo>,
    pub cap_info: Vec<CapInfo>,
    pub is_final_floor: bool,
    pub waterwraith_timer: f32,  // How long, in seconds, it'll take for the waterwraith to fall. 0 = never.
}

impl CaveInfo {
    /// Return all teki in a particular spawn group.
    pub fn teki_group(&self, group: u32) -> impl Iterator<Item=&TekiInfo> {
        self.teki_info.iter().filter(move |teki| teki.group == group)
    }

    /// Out of all the possible map tiles on this floor, finds the one with the highest
    /// number of doors and returns that number.
    pub fn max_num_doors_single_unit(&self) -> usize {
        self.cave_units.iter().map(|unit| unit.num_doors).max().unwrap_or_default()
    }

    /// Returns the human-readable sublevel name for this floor, e.g. "SCx6".
    /// Not part of the generation algorithm at all.
    pub fn name(&self) -> String {
        format!("{}{}", self.cave_cfg.shortened_names.first().unwrap(), self.floor_num+1)
    }

    /// Constructs the long name of this sublevel, e.g. "Subterranean Complex 3" with the full cave name.
    pub fn long_name(&self) -> String {
        format!("{} {}", self.cave_cfg.full_name, self.floor_num+1)
    }

    pub fn parse_from(cave: &CaveConfig) -> Result<Vec<CaveInfo>, CaveripperError> {
        parse_caveinfo(cave)
            .change_context(CaveripperError::CaveinfoError)
            .attach_printable_lazy(|| format!("{} ({}/{})", cave.full_name, cave.game, cave.caveinfo_filename))
    }

    pub fn is_challenge_mode(&self) -> bool {
        self.cave_cfg.is_challenge_mode
    }
}

impl Display for CaveInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f, "NumRooms: {}\tNumGates: {}\tCorridorBetweenRoomsProb: {}%\tCapVsHallProb: {}%",
            self.num_rooms, self.max_gates, self.corridor_probability * 100.0, self.cap_probability * 100.0
        )?;

        if !self.is_final_floor {
            if self.exit_plugged {
                write!(f, "Exit plugged. ")?;
            }
            if self.has_geyser {
                write!(f, "Has geyser. ")?;
            }
            if self.exit_plugged || self.has_geyser {
                writeln!(f)?;
            }
        }

        writeln!(f, "Teki (max {}):", self.max_main_objects)?;
        for tekiinfo in self.teki_info.iter() {
            write!(f, "\t{} (group: {}, num: {}", tekiinfo.internal_name, tekiinfo.group, tekiinfo.minimum_amount)?;
            if tekiinfo.filler_distribution_weight > 0 {
                write!(f, ", weight: {}", tekiinfo.filler_distribution_weight)?;
            }
            if let Some(spawn_method) = &tekiinfo.spawn_method {
                write!(f, ", spawn method: {spawn_method}")?;
            }
            write!(f, ")")?;
            if let Some(carrying) = &tekiinfo.carrying {
                write!(f, " Carrying: {carrying}")?;
            }
            writeln!(f)?;
        }

        writeln!(f, "Treasures:")?;
        for (i, iteminfo) in self.item_info.iter().enumerate() {
            writeln!(f, "\t{}: {}", i+1, iteminfo.internal_name)?;
        }

        writeln!(f, "Cap Teki:")?;
        for (i, capinfo) in self.cap_info.iter().enumerate() {
            write!(f, "\t{}: {} (num: {}", i+1, capinfo.internal_name, capinfo.minimum_amount)?;
            if capinfo.filler_distribution_weight > 0 {
                write!(f, ", weight: {}", capinfo.filler_distribution_weight)?;
            }
            if let Some(spawn_method) = &capinfo.spawn_method {
                write!(f, ", spawn method: {spawn_method}")?;
            }
            writeln!(f, ")")?;
        }

        writeln!(f, "Rooms:")?;
        let unique_units: HashSet<&str> = self.cave_units.iter().map(|unit| unit.unit_folder_name.as_ref()).collect();
        for unit in unique_units.iter() {
            writeln!(f, "\t{unit}")?;
        }

        Ok(())
    }
}


/// "Teki" ("敵") is a Japanese word literally meaning "opponent" or "threat". This
/// is the game's internal name collectively given to enemies (Bulborbs,
/// Sheargrubs, etc.), hazards (poison geysers, electric sparkers, bomb rocks,
/// etc.), plants, and some other objects such as eggs. Most things in caves
/// that aren't either treasures or gates are considered Teki.
/// Treasures held inside enemies *are* defined in TekiInfo, however. See the
/// `carrying` field.
#[derive(Debug, Clone, Serialize)]
pub struct TekiInfo {
    pub internal_name: String,
    pub carrying: Option<String>, // The object held by this Teki, if any.
    pub minimum_amount: u32,
    pub filler_distribution_weight: u32, // https://pikmintkb.com/wiki/Cave_spawning#Weighted_distribution
    pub group: u32, // A.K.A. "Type" but "group" is used for convenience. https://pikmintkb.com/wiki/Cave_generation_parameters#Type
    pub spawn_method: Option<String>, // https://pikmintkb.com/wiki/Cave_generation_parameters#Spawn_method
}


/// Defines 'loose' treasures, i.e. those that are not held by an enemy, but
/// rather sitting out in the open or buried.
#[derive(Debug, Clone, Serialize)]
pub struct ItemInfo {
    pub internal_name: String,
    pub min_amount: u8,
    pub filler_distribution_weight: u32,
}


/// Defines gates. Very straightforward.
#[derive(Debug, Clone, Serialize)]
pub struct GateInfo {
    pub health: f32,
    pub spawn_distribution_weight: u32, // https://pikmintkb.com/wiki/Cave_spawning#Weighted_distribution
}


/// CapInfo specifies what objects will spawn in dedicated Alcove spawn points.
/// This is similar to TekiInfo, but with a few key differences:
///  1. `group` controls the amount of objects allowed to spawn, not spawn
///     location. (Spawn location is always a cap.)
///  2. 'Loose' treasures can be spawned in CapInfo, unlike TekiInfo.
///  3. Objects spawned from CapInfo don't count towards any maximums of their
///     object type, such as `max_main_objects` in FloorInfo.
///
/// CapInfo is most frequently used for falling eggs/bomb rocks and Candypop Buds,
/// However, there are also couple easy-to-spot Cap Enemies such as the second
/// Orange Bulborb on BK1 that faces directly out of its cap.
///
/// Re: vocabulary, Pikmin 2's code uses the terms "cap", "alcove", and "dead end"
/// interchangeably, whereas humans usually say "alcove" when they mean 'a dead end
/// with a spawn point' and "cap" when they mean 'a dead end with no spawn point'.
/// CapInfo only applies to the former, 'dead ends with spawn points' A.K.A.
/// "alcoves". Nothing can spawn in "caps" as you might expect.
#[derive(Debug, Clone, Serialize)]
pub struct CapInfo {
    pub internal_name: String,
    pub carrying: Option<String>, // The object held by this Cap Teki, if any.
    pub minimum_amount: u32,
    pub filler_distribution_weight: u32, // https://pikmintkb.com/wiki/Cave_spawning#Weighted_distribution
    pub group: u8,                      // Does not control spawn location like it does in TekiInfo.
    pub spawn_method: Option<String>, // https://pikmintkb.com/wiki/Cave_generation_parameters#Spawn_method
}

impl CapInfo {
    /// Checks the internal name of this Cap Teki to see if it is a Candypop Bud
    /// (or "Pom" internally). This is necessary because Candypop Buds receive
    /// special treatment with regards to falling Cap Teki and Gate spawning.
    pub fn is_candypop(&self) -> bool {
        self.internal_name.to_lowercase().contains("pom")
    }

    /// Returns whether this cap teki will fall, or if it's grounded.
    /// This is just a convenience method to make code intent more clear, since
    /// all spawn methods besides the 'nothing' spawn method are falling.
    pub fn is_falling(&self) -> bool {
        self.spawn_method.is_some()
    }
}


/// Cave Unit Definition files record info about what map tiles can be
/// generated on a given sublevel. Each CaveUnit represents one possible
/// map tile.
/// https://pikmintkb.com/wiki/Cave_unit_definition_file
#[derive(Debug, Clone, Serialize)]
pub struct CaveUnit {
    pub unit_folder_name: String,
    pub width: u16,  // In cave grid cells, not in-game coords
    pub height: u16, // In cave grid cells, not in-game coords
    pub room_type: RoomType,
    pub num_doors: usize,
    pub doors: Vec<DoorUnit>,
    pub rotation: u16,
    pub spawnpoints: Vec<SpawnPoint>,
    pub waterboxes: Vec<Waterbox>,
    pub waypoints: Vec<Waypoint>,  // Points in 3d space that guide carried objects
}


/// Implementations for (Partial)Eq and (Partial)Ord for CaveUnit.
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
    /// Copies this CaveUnit and applies the given rotation to the copy.
    pub fn copy_and_rotate_to(&self, rotation: u16) -> Self {
        let mut new_unit = self.clone();
        new_unit.rotation = (new_unit.rotation + rotation) % 4;
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

        // Rotate waypoints around the *center* of the room
        new_unit.waypoints.iter_mut()
            .for_each(|wp| {
                match rotation {
                    1 => {
                        (wp.x, wp.z) = (wp.z, wp.x);
                        wp.x = -wp.x + new_unit.width as f32 * 170.0;
                    },
                    2 => {
                        wp.z = -wp.z + new_unit.height as f32 * 170.0;
                    },
                    3 => {
                        (wp.x, wp.z) = (wp.z, wp.x);
                        wp.z = -wp.z + new_unit.height as f32 * 170.0;
                    },
                    _ => {}
                }
            });

        new_unit
    }

    pub fn has_start_spawnpoint(&self) -> bool {
        self.spawnpoints.iter().any(|spawnpoint| spawnpoint.group == 7)
    }
}


/// Defines a cuboid of water in a room tile.
#[derive(Debug, Clone, Serialize)]
pub struct Waterbox {
    pub x1: f32,
    pub y1: f32,
    pub z1: f32,
    pub x2: f32,
    pub y2: f32,
    pub z2: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Waypoint {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub r: f32,
    pub index: usize,
    pub links: Vec<usize>,
}

impl Waypoint {
    pub fn dist(&self, other: &Waypoint) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        pikmin_math::sqrt(dx*dx + dy*dy + dz*dz)
    }
}


/// Indicates position and other metadata about doors in each map unit, relative to its
/// origin point. A 'door' is just an open spot in a map unit where other map units get
/// connected. All doors are exactly 170 in-game units wide, i.e. 1 map unit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DoorUnit {
    pub direction: u16,         // 0, 1, 2, or 3
    pub side_lateral_offset: u16, // Appears to be the offset from center on the side of the room it's facing?
    pub waypoint_index: usize, // Index of the waypoint connected to this door
    pub num_links: usize,
    pub door_links: Vec<DoorLink>,  // Door links are other doors that are reachable through the room that hosts this door.
}

impl DoorUnit {
    pub fn facing(&self, other: &DoorUnit) -> bool {
        (self.direction as isize - other.direction as isize).abs() == 2
    }
}


/// DoorLinks are *straight lines* between two doors *in the same room*. There is one
/// DoorLink for every unique pair of doors in a given room tile. These are primarily
/// used for calculating Door Score.
/// To clarify, DoorLinks are NOT links between two doors in separate rooms.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DoorLink {
    pub distance: f32,  // Straight line distance. This can cross out-of-bounds and otherwise uncrossable obstacles.
    pub door_id: usize, // Id of the other door
    pub tekiflag: bool, // Whether or not a teki should spawn in the seam of the origin door
}


#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
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

impl TryFrom<&str> for RoomType {
    type Error = Report<CaveripperError>;
    fn try_from(input: &str) -> std::result::Result<Self, Self::Error> {
        match input.to_ascii_lowercase().as_str() {
            "room" => Ok(RoomType::Room),
            "cap" | "alcove" => Ok(RoomType::DeadEnd),
            "hall" | "hallway" => Ok(RoomType::Hallway),
            _ => Err(report!(CaveripperError::QueryParseError)).attach_printable_lazy(|| input.to_owned())
        }
    }
}

impl Display for RoomType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RoomType::Room => write!(f, "room"),
            RoomType::Hallway => write!(f, "hallway"),
            RoomType::DeadEnd => write!(f, "cap"),
        }
    }
}


/// Spawn Points for everything that gets placed in sublevels, including the Research
/// Pod, the exit hole/geyser, treasures, Teki, etc.
#[derive(Debug, Clone, Serialize)]
pub struct SpawnPoint {
    pub group: u16,
    pub pos_x: f32,  // Positions are all relative to the origin of the unit they belong to, NOT global coords.
    pub pos_y: f32,
    pub pos_z: f32,
    pub angle_degrees: f32,
    pub radius: f32,
    pub min_num: u16,
    pub max_num: u16,
}
