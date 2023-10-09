mod generate;
pub(crate) mod waypoint;

use std::{
    cell::{OnceCell, Ref, RefCell},
    rc::{Rc, Weak},
};

use generate::LayoutBuilder;
use serde::{ser::SerializeStruct, Serialize};
use waypoint::WaypointGraph;

use crate::{
    caveinfo::{CapInfo, CaveInfo, CaveUnit, DoorUnit, GateInfo, ItemInfo, SpawnPoint, TekiInfo},
    point::Point,
    sublevel::Sublevel,
};

/// Represents a generated sublevel layout.
/// Given a seed and a CaveInfo file, a layout can be generated using a
/// re-implementation of Pikmin 2's internal cave generation function.
/// These layouts are 100% game-accurate (which can be verified using
/// the set-seed mod) and specify exact positions for every tile, teki,
/// and treasure.
#[derive(Debug, Clone)]
pub struct Layout<'a> {
    pub sublevel: Sublevel,
    pub starting_seed: u32,
    pub cave_name: String,
    pub map_units: Vec<PlacedMapUnit<'a>>,
    waypoint_graph: OnceCell<WaypointGraph>,
}

impl<'a> Layout<'a> {
    pub fn generate(seed: u32, caveinfo: &CaveInfo) -> Layout {
        LayoutBuilder::generate(seed, caveinfo)
    }

    /// Gets all SpawnObjects in the layout plus their global coordinates
    pub fn get_spawn_objects(&self) -> impl Iterator<Item = (&SpawnObject<'a>, Point<3, f32>)> {
        let room_sps = self.map_units.iter().flat_map(|unit| unit.spawnpoints.iter()).flat_map(|sp| {
            sp.contains.iter().map(|so| match so {
                SpawnObject::Teki(_, pos) => (so, sp.pos + *pos),
                _ => (so, sp.pos),
            })
        });
        let seam_sps = self.map_units.iter().flat_map(|unit| unit.doors.iter()).filter_map(|door| {
            // Doing this means these spawnpoints can never be mutably borrowed again, but that's
            // fine since the layout is already fully generated and shouldn't require modification.
            let door = Ref::leak(door.borrow());
            Option::as_ref(&door.seam_spawnpoint).map(|so| (so, door.center()))
        });
        room_sps.chain(seam_sps)
    }

    pub fn waypoint_graph(&self) -> &WaypointGraph {
        self.waypoint_graph.get_or_init(|| WaypointGraph::build(self))
    }
}

impl Serialize for Layout<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("layout", 9)?;
        state.serialize_field("name", &self.sublevel.short_name())?;
        state.serialize_field("seed", &self.starting_seed)?;
        state.serialize_field(
            "ship",
            &self
                .get_spawn_objects()
                .find(|(so, _)| matches!(so, SpawnObject::Ship))
                .map(|(_, pos)| pos),
        )?;
        state.serialize_field(
            "hole",
            &self
                .get_spawn_objects()
                .find(|(so, _)| matches!(so, SpawnObject::Hole(..)))
                .map(|(_, pos)| pos),
        )?;
        state.serialize_field(
            "geyser",
            &self
                .get_spawn_objects()
                .find(|(so, _)| matches!(so, SpawnObject::Geyser(..)))
                .map(|(_, pos)| pos),
        )?;

        state.serialize_field("map_units", &self.map_units)?;

        #[derive(Serialize)]
        struct PlacedTeki<'a> {
            name: &'a str,
            x: f32,
            z: f32,
            carrying: Option<&'a str>,
        }

        let teki = self
            .get_spawn_objects()
            .filter(|(so, _)| matches!(so, SpawnObject::Teki(..) | SpawnObject::CapTeki(..)))
            .map(|(so, pos)| PlacedTeki {
                name: so.name(),
                x: pos[0],
                z: pos[2],
                carrying: if let SpawnObject::Teki(info, _) = so {
                    info.carrying.as_deref()
                } else {
                    None
                },
            })
            .collect::<Vec<_>>();
        state.serialize_field("teki", &teki)?;

        #[derive(Serialize)]
        struct PlacedObject<'a> {
            name: &'a str,
            x: f32,
            z: f32,
        }

        let treasures = self
            .get_spawn_objects()
            .filter(|(so, _)| matches!(so, SpawnObject::Item(..)))
            .map(|(so, pos)| PlacedObject {
                name: so.name(),
                x: pos[0],
                z: pos[2],
            })
            .collect::<Vec<_>>();
        state.serialize_field("treasures", &treasures)?;

        let gates = self
            .get_spawn_objects()
            .filter(|(so, _)| matches!(so, SpawnObject::Gate(..)))
            .map(|(so, pos)| PlacedObject {
                name: so.name(),
                x: pos[0],
                z: pos[2],
            })
            .collect::<Vec<_>>();
        state.serialize_field("gates", &gates)?;

        state.end()
    }
}

#[derive(Debug, Clone)]
pub struct PlacedMapUnit<'a> {
    pub unit: &'a CaveUnit,
    pub x: i32,
    pub z: i32,
    pub doors: Vec<Rc<RefCell<PlacedDoor<'a>>>>,
    pub spawnpoints: Vec<PlacedSpawnPoint<'a>>,
    pub teki_score: u32,
    pub total_score: u32,
}

impl<'a> PlacedMapUnit<'a> {
    pub fn new(unit: &'a CaveUnit, x: i32, z: i32) -> PlacedMapUnit<'a> {
        let doors = unit
            .doors
            .iter()
            .map(|door| {
                // Adjust door positions depending on room rotation
                let (door_x, door_z) = match door.direction {
                    0 => (x + door.side_lateral_offset as i32, z),
                    1 => (x + unit.width as i32, z + door.side_lateral_offset as i32),
                    2 => (x + door.side_lateral_offset as i32, z + unit.height as i32),
                    3 => (x, z + door.side_lateral_offset as i32),
                    _ => panic!("Invalid door direction"),
                };
                Rc::new(RefCell::new(PlacedDoor {
                    x: door_x,
                    z: door_z,
                    door_unit: door,
                    parent_idx: None,
                    marked_as_cap: false,
                    adjacent_door: None,
                    door_score: Some(0),
                    seam_teki_score: 0,
                    seam_spawnpoint: Rc::new(None),
                }))
            })
            .collect();

        let spawnpoints = unit
            .spawnpoints
            .iter()
            .map(|sp| {
                // Make spawn point coordinates global rather than relative to their parent room
                let base_x = (x as f32 + (unit.width as f32 / 2.0)) * 170.0;
                let base_z = (z as f32 + (unit.height as f32 / 2.0)) * 170.0;
                let (actual_x, actual_z) = match unit.rotation {
                    0 => (base_x + sp.pos[0], base_z + sp.pos[2]),
                    1 => (base_x - sp.pos[2], base_z + sp.pos[0]),
                    2 => (base_x - sp.pos[0], base_z - sp.pos[2]),
                    3 => (base_x + sp.pos[2], base_z - sp.pos[0]),
                    _ => panic!("Invalid room rotation"),
                };
                let actual_angle = (sp.angle_degrees - unit.rotation as f32 * 90.0) % 360.0;
                PlacedSpawnPoint {
                    pos: Point([actual_x, sp.pos[1], actual_z]),
                    angle: actual_angle,
                    spawnpoint_unit: sp,
                    hole_score: 0,
                    treasure_score: 0,
                    contains: vec![],
                }
            })
            .collect();

        PlacedMapUnit {
            unit,
            x,
            z,
            doors,
            spawnpoints,
            teki_score: 0,
            total_score: 0,
        }
    }

    pub fn overlaps(&self, other: &PlacedMapUnit) -> bool {
        boxes_overlap(
            self.x,
            self.z,
            self.unit.width,
            self.unit.height,
            other.x,
            other.z,
            other.unit.width,
            other.unit.height,
        )
    }

    pub fn spawn_objects(&self) -> impl Iterator<Item = &SpawnObject> {
        self.spawnpoints.iter().flat_map(|sp| sp.contains.iter())
    }

    /// Identifier string unique to this unit within a layout.
    pub(crate) fn key(&self) -> String {
        format!("{}/{}/{}", self.x, self.z, self.unit.unit_folder_name)
    }
}

impl Serialize for PlacedMapUnit<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Does not serialize spawn objects; that's left for another step
        let mut state = serializer.serialize_struct("map_unit", 6)?;
        state.serialize_field("name", &self.unit.unit_folder_name)?;
        state.serialize_field("width", &self.unit.width)?;
        state.serialize_field("height", &self.unit.height)?;
        state.serialize_field("x", &self.x)?;
        state.serialize_field("y", &self.z)?;
        state.serialize_field("rotation", &self.unit.rotation)?;
        state.end()
    }
}

#[derive(Debug)]
pub struct PlacedDoor<'a> {
    pub x: i32,
    pub z: i32,
    pub door_unit: &'a DoorUnit,
    pub parent_idx: Option<usize>,
    pub marked_as_cap: bool,
    pub adjacent_door: Option<Weak<RefCell<PlacedDoor<'a>>>>,
    pub door_score: Option<u32>,
    pub seam_teki_score: u32,
    pub seam_spawnpoint: Rc<Option<SpawnObject<'a>>>,
}

impl<'a> PlacedDoor<'a> {
    pub fn facing(&self, other: &PlacedDoor) -> bool {
        (self.door_unit.direction as i32 - other.door_unit.direction as i32).abs() == 2
    }

    pub fn lines_up_with(&self, other: &PlacedDoor) -> bool {
        self.facing(other) && self.x == other.x && self.z == other.z
    }

    pub fn center(&self) -> Point<3, f32> {
        let mut door_pos = Point([self.x as f32 * 170.0, 0.0, self.z as f32 * 170.0]);
        if self.door_unit.direction % 2 == 0 {
            door_pos[0] += 85.0;
        } else {
            door_pos[2] += 85.0;
        }
        door_pos
    }
}

#[derive(Debug, Clone)]
pub struct PlacedSpawnPoint<'a> {
    pub spawnpoint_unit: &'a SpawnPoint,
    pub pos: Point<3, f32>,
    pub angle: f32,
    pub hole_score: u32,
    pub treasure_score: u32,
    pub contains: Vec<SpawnObject<'a>>,
}

/// Any object that can be placed in a SpawnPoint.
#[derive(Debug, Clone)]
pub enum SpawnObject<'a> {
    Teki(&'a TekiInfo, Point<3, f32>), // Teki, offset from spawnpoint
    CapTeki(&'a CapInfo, u32),         // Cap Teki, num_spawned
    Item(&'a ItemInfo),
    Gate(&'a GateInfo, u16), // Rotation
    Hole(bool),              // Plugged or not
    Geyser(bool),            // Plugged or not
    Ship,
}

impl<'a> SpawnObject<'a> {
    pub fn name(&self) -> &str {
        match self {
            SpawnObject::Teki(info, _) => &info.internal_name,
            SpawnObject::CapTeki(info, _) => &info.internal_name,
            SpawnObject::Item(info) => &info.internal_name,
            SpawnObject::Gate(_, _) => "gate",
            SpawnObject::Hole(_) => "hole",
            SpawnObject::Geyser(_) => "geyser",
            SpawnObject::Ship => "ship",
        }
    }

    pub fn amount(&self) -> u32 {
        match self {
            SpawnObject::Teki(info, _) => info.minimum_amount,
            SpawnObject::CapTeki(info, _) => info.minimum_amount,
            SpawnObject::Item(info) => info.min_amount as u32,
            SpawnObject::Gate(_, _) => 0,
            SpawnObject::Hole(_) => 0,
            SpawnObject::Geyser(_) => 0,
            SpawnObject::Ship => 0,
        }
    }

    pub fn weight(&self) -> u32 {
        match self {
            SpawnObject::Teki(info, _) => info.filler_distribution_weight,
            SpawnObject::CapTeki(info, _) => info.filler_distribution_weight,
            SpawnObject::Item(info) => info.filler_distribution_weight,
            SpawnObject::Gate(info, _) => info.spawn_distribution_weight,
            SpawnObject::Hole(_) => 0,
            SpawnObject::Geyser(_) => 0,
            SpawnObject::Ship => 0,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn boxes_overlap(x1: i32, z1: i32, w1: u16, h1: u16, x2: i32, z2: i32, w2: u16, h2: u16) -> bool {
    !((x1 + w1 as i32 <= x2 || x2 + w2 as i32 <= x1) || (z1 + h1 as i32 <= z2 || z2 + h2 as i32 <= z1))
}
