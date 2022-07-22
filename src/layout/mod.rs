mod generate;
pub mod render;
#[cfg(test)]
mod test;

use std::{cell::RefCell, rc::{Rc, Weak}};
use generate::LayoutBuilder;

use crate::{caveinfo::{CapInfo, CaveUnit, DoorUnit, CaveInfo, GateInfo, ItemInfo, SpawnPoint, TekiInfo}, pikmin_math};

/// Represents a generated sublevel layout.
/// Given a seed and a CaveInfo file, a layout can be generated using a
/// re-implementation of Pikmin 2's internal cave generation function.
/// These layouts are 100% game-accurate (which can be verified using
/// the set-seed mod) and specify exact positions for every tile, teki,
/// and treasure.
#[derive(Debug, Clone)]
pub struct Layout {
    pub starting_seed: u32,
    pub cave_name: String,
    pub map_units: Vec<PlacedMapUnit>,
}

impl Layout {
    pub fn generate(seed: u32, caveinfo: &CaveInfo) -> Layout {
        LayoutBuilder::generate(seed, caveinfo)
    }

    pub fn get_spawn_objects(&self) -> impl Iterator<Item=&SpawnObject> {
        self.map_units.iter()
            .flat_map(|unit| unit.spawnpoints.iter())
            .filter_map(|spawnpoint| spawnpoint.contains.as_ref())
    }

    /// A unique structured string describing this layout.
    /// The general structure is as follows:
    /// <sublevel name>;<0xAAAAAAAA>;<map units list>;<all spawn object list>
    /// This is only used for testing and comparison, so there's no need for this
    /// format to be especially readable.
    pub fn slug(&self) -> String {
        let mut slug = String::new();

        slug.push_str(&format!("{};", self.cave_name));
        slug.push_str(&format!("{:#010X};", self.starting_seed));

        slug.push_str("[");
        for map_unit in self.map_units.iter() {
            slug.push_str(&format!("{},x{}z{}r{};",
                map_unit.unit.unit_folder_name,
                map_unit.x,
                map_unit.z,
                map_unit.unit.rotation
            ));
        }
        slug.push_str("];");

        let mut spawn_object_slugs = Vec::new();
        for map_unit in self.map_units.iter() {
            for spawnpoint in map_unit.spawnpoints.iter() {
                if let Some(spawn_object) = spawnpoint.contains.as_ref() {
                    match &spawn_object {
                        SpawnObject::Teki(tekiinfo) | SpawnObject::PlantTeki(tekiinfo) => {
                            spawn_object_slugs.push(format!("{},carrying:{},spawn_method:{},x{}z{};",
                                tekiinfo.internal_name,
                                tekiinfo.carrying.clone().unwrap_or_else(|| "none".to_string()),
                                tekiinfo.spawn_method.clone().unwrap_or_else(|| "0".to_string()),
                                spawnpoint.x as i32,
                                spawnpoint.z as i32,
                            ));
                        },
                        SpawnObject::TekiBunch(tekiinfo_list) => {
                            for (tekiinfo, (dx, _, dz)) in tekiinfo_list.iter() {
                                spawn_object_slugs.push(format!("{},carrying:{},spawn_method:{},x{}z{};",
                                    tekiinfo.internal_name,
                                    tekiinfo.carrying.clone().unwrap_or_else(|| "none".to_string()),
                                    tekiinfo.spawn_method.clone().unwrap_or_else(|| "0".to_string()),
                                    (spawnpoint.x + dx) as i32,
                                    (spawnpoint.z + dz) as i32,
                                ));
                            }
                        },
                        SpawnObject::CapTeki(capinfo, _) => {
                            spawn_object_slugs.push(format!("{},carrying:{},spawn_method:{},x{}z{};",
                                capinfo.internal_name,
                                capinfo.carrying.clone().unwrap_or_else(|| "none".to_string()),
                                capinfo.spawn_method.clone().unwrap_or_else(|| "0".to_string()),
                                spawnpoint.x as i32,
                                spawnpoint.z as i32,
                            ));
                        },
                        SpawnObject::Item(iteminfo) => {
                            spawn_object_slugs.push(format!("{},x{}z{};",
                                iteminfo.internal_name,
                                spawnpoint.x as i32,
                                spawnpoint.z as i32,
                            ));
                        },
                        SpawnObject::Hole(_) => {
                            spawn_object_slugs.push(format!("hole,x{}z{};",
                                spawnpoint.x as i32,
                                spawnpoint.z as i32,
                            ));
                        },
                        SpawnObject::Geyser => {
                            spawn_object_slugs.push(format!("geyser,x{}z{};",
                                spawnpoint.x as i32,
                                spawnpoint.z as i32,
                            ));
                        },
                        SpawnObject::Ship => {
                            spawn_object_slugs.push(format!("ship,x{}z{};",
                                spawnpoint.x as i32,
                                spawnpoint.z as i32,
                            ));
                        },
                        SpawnObject::Gate(_) => {}, // Does not get placed in this vec.
                    }
                }
                if let Some(SpawnObject::CapTeki(capinfo, _)) = &spawnpoint.falling_cap_teki {
                    spawn_object_slugs.push(format!("{},carrying:{},spawn_method:{},x{}z{};",
                        capinfo.internal_name,
                        capinfo.carrying.clone().unwrap_or_else(|| "none".to_string()),
                        capinfo.spawn_method.clone().unwrap_or_else(|| "0".to_string()),
                        spawnpoint.x as i32,
                        spawnpoint.z as i32,
                    ));
                }
            }

            for door in map_unit.doors.iter() {
                let mut x = (door.borrow().x * 170) as f32;
                let mut z = (door.borrow().z * 170) as f32;
                match door.borrow().door_unit.direction {
                    0 | 2 => x += 85.0,
                    1 | 3 => z += 85.0,
                    _ => panic!("Invalid door direction in slug"),
                }
                match &*door.borrow().seam_spawnpoint {
                    Some(SpawnObject::Teki(tekiinfo)) => {
                        spawn_object_slugs.push(format!("{},carrying:{},spawn_method:{},x{}z{};",
                            tekiinfo.internal_name,
                            tekiinfo.carrying.clone().unwrap_or_else(|| "none".to_string()),
                            tekiinfo.spawn_method.clone().unwrap_or_else(|| "0".to_string()),
                            x as i32, z as i32,
                        ));
                    },
                    Some(SpawnObject::Gate(gateinfo)) => {
                        spawn_object_slugs.push(format!("GATE,hp{},x{}z{};",
                            gateinfo.health, x as i32, z as i32,
                        ));
                    },
                    _ => {}, // Nothing else can spawn in seams.
                }
            }
        }

        slug.push_str("[");
        spawn_object_slugs.sort();
        for so_slug in spawn_object_slugs {
            slug.push_str(&so_slug);
        }
        slug.push_str("];");

        slug
    }
}


#[derive(Debug, Clone)]
pub struct PlacedMapUnit {
    pub unit: CaveUnit,
    pub x: isize,
    pub z: isize,
    pub doors: Vec<Rc<RefCell<PlacedDoor>>>,
    pub spawnpoints: Vec<PlacedSpawnPoint>,
    pub teki_score: u32,
    pub total_score: u32,
}

impl PlacedMapUnit {
    pub fn new(unit: &CaveUnit, x: isize, z: isize) -> PlacedMapUnit {
        let doors = unit.doors.iter()
            .map(|door| {
                // Adjust door positions depending on room rotation
                let (door_x, door_z) = match door.direction {
                    0 => (x + door.side_lateral_offset as isize, z),
                    1 => (x + unit.width as isize,               z + door.side_lateral_offset as isize),
                    2 => (x + door.side_lateral_offset as isize, z + unit.height as isize),
                    3 => (x,                                     z + door.side_lateral_offset as isize),
                    _ => panic!("Invalid door direction")
                };
                Rc::new(RefCell::new(
                    PlacedDoor {
                        x: door_x,
                        z: door_z,
                        door_unit: door.clone(),
                        parent_idx: None,
                        marked_as_cap: false,
                        adjacent_door: None,
                        door_score: Some(0),
                        seam_teki_score: 0,
                        seam_spawnpoint: Rc::new(None),
                    }
                ))
            })
            .collect();

        let spawnpoints = unit.spawnpoints.iter()
            .map(|sp| {
                // Make spawn point coordinates global rather than relative to their parent room
                let base_x = (x as f32 + (unit.width as f32 / 2.0)) * 170.0;
                let base_z = (z as f32 + (unit.height as f32 / 2.0)) * 170.0;
                let (actual_x, actual_z) = match unit.rotation {
                    0 => (base_x + sp.pos_x, base_z + sp.pos_z),
                    1 => (base_x - sp.pos_z, base_z + sp.pos_x),
                    2 => (base_x - sp.pos_x, base_z - sp.pos_z),
                    3 => (base_x + sp.pos_z, base_z - sp.pos_x),
                    _ => panic!("Invalid room rotation")
                };
                let actual_angle = (sp.angle_degrees - unit.rotation as f32 * 90.0) % 360.0;
                PlacedSpawnPoint {
                    x: actual_x,
                    z: actual_z,
                    angle: actual_angle,
                    spawnpoint_unit: sp.clone(),
                    contains: None,
                    falling_cap_teki: None,
                    hole_score: 0,
                    treasure_score: 0,
                }
            })
            .collect();

        PlacedMapUnit {
            unit: unit.clone(),
            x, z,
            doors,
            spawnpoints,
            teki_score: 0,
            total_score: 0,
        }
    }

    pub fn overlaps(&self, other: &PlacedMapUnit) -> bool {
        boxes_overlap(self.x, self.z, self.unit.width, self.unit.height, other.x, other.z, other.unit.width, other.unit.height)
    }
}


#[derive(Debug)]
pub struct PlacedDoor {
    pub x: isize,
    pub z: isize,
    pub door_unit: DoorUnit,
    pub parent_idx: Option<usize>,
    pub marked_as_cap: bool,
    pub adjacent_door: Option<Weak<RefCell<PlacedDoor>>>,
    pub door_score: Option<u32>,
    pub seam_teki_score: u32,
    pub seam_spawnpoint: Rc<Option<SpawnObject>>,
}

impl PlacedDoor {
    pub fn facing(&self, other: &PlacedDoor) -> bool {
        (self.door_unit.direction as isize - other.door_unit.direction as isize).abs() == 2
    }

    pub fn lines_up_with(&self, other: &PlacedDoor) -> bool {
        self.facing(other) && self.x == other.x && self.z == other.z
    }
}


#[derive(Debug, Clone)]
pub struct PlacedSpawnPoint {
    pub spawnpoint_unit: SpawnPoint,
    pub x: f32,
    pub z: f32,
    pub angle: f32,
    pub contains: Option<SpawnObject>,
    pub falling_cap_teki: Option<SpawnObject>,
    pub hole_score: u32,
    pub treasure_score: u32,
}

impl PlacedSpawnPoint {
    fn dist(&self, other: &PlacedSpawnPoint) -> f32 {
        let dx = self.x - other.x;
        let dz = self.z - other.z;
        let dy = self.spawnpoint_unit.pos_y - other.spawnpoint_unit.pos_y;

        pikmin_math::sqrt(dx*dx + dy*dy + dz*dz)
    }
}


#[derive(Debug, Clone)]
pub enum SpawnObject {
    Teki(TekiInfo),
    TekiBunch(Vec<(TekiInfo, (f32, f32, f32))>), // For group 0 enemies. Tuple is displacement from parent spawnpoint.
    PlantTeki(TekiInfo),
    CapTeki(CapInfo, u32), // Cap Teki, num_spawned
    Item(ItemInfo),
    Gate(GateInfo),
    Hole(bool), // Plugged or not
    Geyser,
    Ship
}

impl SpawnObject {
    pub fn name(&self) -> &str {
        match self {
            SpawnObject::Teki(info) => &info.internal_name,
            SpawnObject::TekiBunch(tekis) => &tekis.first().unwrap().0.internal_name,
            SpawnObject::PlantTeki(info) => &info.internal_name,
            SpawnObject::CapTeki(info, _) => &info.internal_name,
            SpawnObject::Item(info) => &info.internal_name,
            SpawnObject::Gate(_) => "gate",
            SpawnObject::Hole(_) => "hole",
            SpawnObject::Geyser => "geyser",
            SpawnObject::Ship => "ship",
        }
    }
}

pub fn boxes_overlap(x1: isize, z1: isize, w1: u16, h1: u16, x2: isize, z2: isize, w2: u16, h2: u16) -> bool {
    !((x1 + w1 as isize <= x2 || x2 + w2 as isize <= x1) || (z1 + h1 as isize <= z2 || z2 + h2 as isize <= z1))
}
