pub mod render;
#[cfg(test)]
pub mod test;

use std::{cell::RefCell, cmp::{max, min}, rc::{Rc, Weak}};
use itertools::Itertools;
use log::debug;

use crate::{caveinfo::{CaveUnit, DoorUnit, FloorInfo, GateInfo, ItemInfo, RoomType, SpawnPoint, TekiInfo}, pikmin_math::PikminRng};

/// Represents a generated sublevel layout.
/// Given a seed and a CaveInfo file, a layout can be generated using a
/// re-implementation of Pikmin 2's internal cave generation function.
/// These layouts are 100% game-accurate (which can be verified using
/// the set-seed mod) and specify exact positions for every tile, teki,
/// and treasure.

#[derive(Debug, Clone)]
pub struct Layout {
    pub map_units: Vec<PlacedMapUnit>,
}

impl Layout {
    pub fn generate(seed: u32, caveinfo: &FloorInfo) -> Layout {
        let layoutbuilder = LayoutBuilder {
            layout: RefCell::new(Layout {
                map_units: Vec::new(),
            }),
            rng: PikminRng::new(seed),
            starting_seed: seed,
            cave_name: caveinfo.name(),
            cap_queue: Vec::new(),
            room_queue: Vec::new(),
            corridor_queue: Vec::new(),
            allocated_enemy_slots_by_group: [0; 10],
            enemy_weight_sum_by_group: [0; 10],
            num_slots_used_for_min: 0,
            map_min_x: 0,
            map_min_z: 0,
            map_max_x: 0,
            map_max_z: 0,
            map_has_diameter_36: false,
            marked_open_doors_as_caps: false,
            placed_spawn_point: None,
            placed_exit_hole: None,
            placed_exit_geyser: None,
        };
        layoutbuilder.generate(seed, caveinfo)
    }
}

struct LayoutBuilder {
    rng: PikminRng,
    starting_seed: u32,
    cave_name: String,
    layout: RefCell<Layout>,
    cap_queue: Vec<CaveUnit>,
    room_queue: Vec<CaveUnit>,
    corridor_queue: Vec<CaveUnit>,
    allocated_enemy_slots_by_group: [u32; 10],
    enemy_weight_sum_by_group: [u32; 10],
    num_slots_used_for_min: u32,
    map_min_x: isize,
    map_min_z: isize,
    map_max_x: isize,
    map_max_z: isize,
    map_has_diameter_36: bool,
    marked_open_doors_as_caps: bool,
    placed_spawn_point: Option<PlacedSpawnPoint>,
    placed_exit_hole: Option<PlacedSpawnPoint>,
    placed_exit_geyser: Option<PlacedSpawnPoint>,
}

impl LayoutBuilder {
    /// Cave generation algorithm. Reimplementation of the code in JHawk's
    /// CaveGen (https://github.com/JHaack4/CaveGen/blob/2c99bf010d2f6f80113ed7eaf11d9d79c6cff367/CaveGen.java#L643)
    ///
    /// This implementation follows CaveGen's as closely as possible, even
    /// when that results in non-idiomatic Rust code. It is my 'reference'
    /// implementation; a more optimized one will follow.
    pub fn generate(mut self, seed: u32, caveinfo: &FloorInfo) -> Layout {
        // Initialize an RNG session with the given seed.
        self.rng = PikminRng::new(seed);

        // ** mapUnitsInitialSorting ** //
        // https://github.com/JHaack4/CaveGen/blob/2c99bf010d2f6f80113ed7eaf11d9d79c6cff367/CaveGen.java#L644

        // Separate out different unit types
        for unit in caveinfo.cave_units.clone().into_iter() {
            match unit.room_type {
                RoomType::DeadEnd => self.cap_queue.push(unit),
                RoomType::Room => self.room_queue.push(unit),
                RoomType::Hallway => self.corridor_queue.push(unit),
            }
        }

        // The order of these (and all other RNG calls) is important!
        self.rng.rand_backs(&mut self.cap_queue);
        self.rng.rand_backs(&mut self.room_queue);
        self.rng.rand_backs(&mut self.corridor_queue);

        // ** End mapUnitsInitialSorting ** //

        // ** allocateEnemySlots ** //
        // https://github.com/JHaack4/CaveGen/blob/2c99bf010d2f6f80113ed7eaf11d9d79c6cff367/CaveGen.java#L645

        // Allocate minimum amounts of each enemy type
        for enemy_type in [0, 1, 5, 8] {
            for teki in caveinfo.teki_group(enemy_type) {
                self.allocated_enemy_slots_by_group[enemy_type as usize] += teki.minimum_amount;
                self.enemy_weight_sum_by_group[enemy_type as usize] += teki.filler_distribution_weight;
                self.num_slots_used_for_min += teki.minimum_amount;
            }
        }

        // Fill remaining allocation budget randomly according to filler distribution weights
        for _ in 0..(caveinfo.max_main_objects.saturating_sub(self.num_slots_used_for_min)) {
            if let Some(group) = self.rng.rand_index_weight(&self.enemy_weight_sum_by_group) {
                self.allocated_enemy_slots_by_group[group] += 1;
            }
        }

        // ** End allocateEnemySlots ** //

        // ** Main map unit generation logic ** //

        // Pick the first room in the queue that has a 'start' spawnpoint (for the ship pod)
        // and place it as the first room.
        let start_map_unit = self.room_queue.iter().find(|room| room.has_start_spawnpoint())
            .expect("No room with start spawnpoint found.")
            .clone();
        debug!("Placing starting map unit of type '{}'", start_map_unit.unit_folder_name);
        self.place_map_unit(PlacedMapUnit::new(&start_map_unit, 0, 0), true);


        // Keep placing map units until all doors have been closed
        if self.open_doors().len() > 0 {
            let mut num_loops = 0;
            while num_loops <= 10000 {
                num_loops += 1;
                let mut unit_to_place = None;

                // Check if the number of placed rooms has reached the max, and place one if not
                if self.layout.borrow().map_units.iter()
                    .filter(|unit| unit.unit.room_type == RoomType::Room)
                    .count() < caveinfo.num_rooms as usize
                {
                    // Choose a random door to attempt to add a room onto
                    let open_doors = self.open_doors();
                    let destination_door = open_doors[self.rng.rand_int(open_doors.len() as u32) as usize].clone();

                    // Calculate the corridor probability for this generation step
                    let mut corridor_probability = caveinfo.corridor_probability;
                    if self.map_has_diameter_36 { corridor_probability = 0f32; }
                    if self.layout.borrow().map_units[destination_door.borrow().attached_to.unwrap()].unit.room_type == RoomType::Room { corridor_probability *= 2f32; }

                    let room_type_priority = if self.rng.rand_f32() < corridor_probability {
                        [RoomType::Hallway, RoomType::Room, RoomType::DeadEnd]
                    } else {
                        [RoomType::Room, RoomType::Hallway, RoomType::DeadEnd]
                    };

                    // Try to place a room of each type in the order defined above, only moving on to
                    // the next type if none of the available units fit.
                    'place_room: for room_type in room_type_priority {
                        let unit_queue = match room_type {
                            RoomType::Room => &self.room_queue,
                            RoomType::DeadEnd => &self.cap_queue,
                            RoomType::Hallway => {
                                self.shuffle_corridor_priority(&caveinfo);
                                &self.corridor_queue
                            }
                        };

                        for map_unit in unit_queue.iter() {
                            let mut door_priority = (0..map_unit.num_doors).collect_vec();
                            self.rng.rand_swaps(&mut door_priority);

                            // Try to attach the new room via each of its doors.
                            for door_index in door_priority {
                                if let Some(approved_unit) = self.try_place_unit_at(destination_door.clone(), map_unit, door_index) {
                                    // Have to let the unit escape this context because self can't
                                    // be mutably borrowed here.
                                    unit_to_place = Some(approved_unit);
                                    break 'place_room;
                                }
                            }
                        }
                    }
                }
                // If we've already placed all the rooms we're allowed to, try to place a
                // hallway or cap instead.
                else {
                    self.mark_random_open_doors_as_caps(&caveinfo);

                    // Create a list of 'hallway' units (corridors with exactly 2 doors)
                    let mut hallway_queue: Vec<&CaveUnit> = self.corridor_queue.iter()
                        .filter(|corridor| corridor.width == 1 && corridor.height == 1 && corridor.num_doors == 2)
                        .collect();
                    self.rng.rand_swaps(&mut hallway_queue);

                    // Hallway placement
                    let open_doors = self.open_doors();
                    'place_hallway: for open_door in open_doors.iter() {
                        if open_door.borrow().marked_as_cap {
                            continue;
                        }

                        // Find the closest door the above door can link to.
                        // A door counts as 'linkable' if it's inside a 10x10 rectangle
                        // in front of the starting door.
                        let mut link_door = None;
                        let mut link_door_dist = isize::MAX;
                        for candidate in open_doors.iter() {
                            if open_door.borrow().attached_to == candidate.borrow().attached_to {
                                continue;
                            }

                            let open_door = open_door.borrow();

                            let dx = candidate.borrow().x - open_door.x;
                            let dz = candidate.borrow().z - open_door.z;

                            if dx.abs() >= 10 || dz.abs() >= 10 { continue; }
                            if open_door.door_unit.direction == 0 && dz > 0 { continue; }
                            if open_door.door_unit.direction == 1 && dx < 0 { continue; }
                            if open_door.door_unit.direction == 2 && dz < 0 { continue; }
                            if open_door.door_unit.direction == 3 && dx > 0 { continue; }

                            let distance = dx.abs() + dz.abs();
                            if distance < link_door_dist {
                                link_door = Some(candidate);
                                link_door_dist = distance;
                            }
                        }
                        let link_door = match link_door {
                            None => continue,
                            Some(d) => d
                        };

                        // Temp variables to make the below formula easier to write
                        let dx = link_door.borrow().x - open_door.borrow().x;
                        let dz = link_door.borrow().z - open_door.borrow().z;
                        let link_door_dir = link_door.borrow().door_unit.direction;
                        let open_door_dir = open_door.borrow().door_unit.direction;

                        // Determine the direction priority to try placing this hallway in.
                        // This is the logic responsible for 'snaking' corridors.
                        //
                        // I don't know of a simple way to explain this, but my guess is that
                        // this logic is the result of some kind of compiler optimization and there
                        // exists a smaller formula to describe it.
                        let priority = match open_door_dir {
                            0 => {
                                if dz > -2 { if dx >= 0 { 1 } else { 3 } }
                                else { match dx {
                                    _ if dx < -1 => 3,
                                    -1 => if link_door_dir == 2 || link_door_dir == 3 { 3 } else { 0 },
                                    0  => if link_door_dir == 0 || link_door_dir == 3 { 3 } else { 0 },
                                    1  => if link_door_dir == 1 || link_door_dir == 2 { 1 } else { 0 },
                                    _ if dx > 1 => 1,
                                    _ => unreachable!()
                                }}
                            },
                            1 => {
                                if dx == 0 { if dz > 0 { 2 } else { 0 } }
                                else { match dz {
                                    _ if dz < -1 => 0,
                                    -1 => if link_door_dir == 0 || link_door_dir == 3 { 0 } else { 1 },
                                    0  => if link_door_dir == 0 || link_door_dir == 1 { 0 } else { 1 },
                                    1  => if link_door_dir == 2 || link_door_dir == 3 { 2 } else { 1 },
                                    _ if dz > 1 => 2,
                                    _ => unreachable!()
                                }}
                            },
                            2 => {
                                if dz == 0 { if dx > 0 { 1 } else { 3 } }
                                else { match dx {
                                    _ if dx < -1 => 3,
                                    -1 => if link_door_dir == 0 || link_door_dir == 3 { 3 } else { 2 },
                                    0  => if link_door_dir == 2 || link_door_dir == 3 { 3 } else { 2 },
                                    1  => if link_door_dir == 0 || link_door_dir == 1 { 1 } else { 2 },
                                    _ if dx > 1 => 1,
                                    _ => unreachable!()
                                }}
                            },
                            3 => {
                                if dx > -2 { if dz > 0 { 2 } else { 0 } }
                                else { match dz {
                                    _ if dz < -1 => 0,
                                    -1 => if link_door_dir == 0 || link_door_dir == 1 { 0 } else { 3 },
                                    0  => if link_door_dir == 0 || link_door_dir == 3 { 0 } else { 3 },
                                    1  => if link_door_dir == 1 || link_door_dir == 2 { 2 } else { 3 },
                                    _ if dz > 1 => 2,
                                    _ => unreachable!()
                                }}
                            },
                            _ => panic!("Invalid direction in hallway snaking")
                        };

                        // Try placing a hallway with the desired shape. If that doesn't work,
                        // try placing a straight hallway instead.
                        let dir_hallway_0 = (open_door_dir + 2) % 4;  // Flip the direction 180 degrees
                        for dir_hallway_1 in [priority, open_door_dir] {
                            for hallway_unit in hallway_queue.iter() {
                                let door_dir_0 = hallway_unit.doors[0].direction;
                                let door_dir_1 = hallway_unit.doors[1].direction;
                                if door_dir_0 == dir_hallway_0 && door_dir_1 == dir_hallway_1 {
                                    unit_to_place = self.try_place_unit_at(open_door.clone(), &hallway_unit, 0);
                                }
                                else if door_dir_0 == dir_hallway_1 && door_dir_1 == dir_hallway_0 {
                                    unit_to_place = self.try_place_unit_at(open_door.clone(), &hallway_unit, 1);
                                }
                                if unit_to_place.is_some() {
                                    break 'place_hallway;
                                }
                            }
                        }
                    }
                }

                if let Some(unit_to_place) = unit_to_place {
                    debug!("Placing unit of type '{}' at ({}, {})",
                            unit_to_place.unit.unit_folder_name, unit_to_place.x, unit_to_place.z);
                    self.place_map_unit(unit_to_place, true);
                }
                // If neither a room nor a hallway can be placed via the 'normal' logic above,
                // try to cap off any remaining open doors using caps or open hallways (or rooms,
                // but in reality this is very rare).
                else {
                    let mut cap_to_place = None;
                    'place_cap: for open_door in self.open_doors() {
                        for room_type in [RoomType::DeadEnd, RoomType::Hallway, RoomType::Room] {
                            let unit_queue = match room_type {
                                RoomType::Room => &self.room_queue,
                                RoomType::DeadEnd => &self.cap_queue,
                                RoomType::Hallway => {
                                    self.shuffle_corridor_priority(&caveinfo);
                                    &self.corridor_queue
                                }
                            };
                            for num_doors in 1..=caveinfo.max_num_doors_single_unit() {
                                for map_unit in unit_queue {
                                    if map_unit.num_doors != num_doors { continue; }

                                    let mut door_priority = (0..num_doors).collect_vec();
                                    self.rng.rand_swaps(&mut door_priority);

                                    for door_index in door_priority {
                                        if let Some(approved_unit) = self.try_place_unit_at(open_door.clone(), map_unit, door_index) {
                                            cap_to_place = Some(approved_unit);
                                            break 'place_cap;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if let Some(cap_to_place) = cap_to_place {
                        debug!("Placing cap of type '{}' at ({}, {})",
                            cap_to_place.unit.unit_folder_name, cap_to_place.x, cap_to_place.z);
                        self.place_map_unit(cap_to_place, true);
                    }
                }

                if self.open_doors().len() > 0 { continue; }
                let mut cap_to_replace = None;

                // changeCapToHallMapUnit //
                // Change all alcoves with a corridor directly behind them into a corridor unit.
                let hallway_unit_names: Vec<&str> = self.corridor_queue.iter()
                    .filter(|unit| unit.width == 1 && unit.height == 1 && unit.num_doors == 2)
                    // Filter out east-to-west hallways. Not sure why this is done.
                    .filter(|unit| unit.doors[0].direction == 0 && unit.doors[1].direction == 2)
                    .map(|unit| unit.unit_folder_name.as_ref())
                    .collect();

                if hallway_unit_names.len() > 0 {
                    'change_cap_to_hallway: for i in 0..self.layout.borrow().map_units.len() {
                        let placed_unit = &self.layout.borrow().map_units[i];
                        if placed_unit.unit.room_type != RoomType::DeadEnd { continue; }

                        // Compute space behind alcove
                        let (space_x, space_z) = match placed_unit.doors[0].borrow().door_unit.direction {
                            0 => (placed_unit.x, placed_unit.z + 1),
                            1 => (placed_unit.x - 1, placed_unit.z),
                            2 => (placed_unit.x, placed_unit.z - 1),
                            3 => (placed_unit.x + 1, placed_unit.z),
                            _ => panic!("Invalid door direction in changeCapToHallMapUnit")
                        };

                        // Check for a corridor in the space behind this alcove
                        let corridor_behind_idx = self.layout.borrow().map_units.iter()
                            .filter(|unit| unit.unit.room_type == RoomType::Hallway)
                            .filter(|unit| unit.x != placed_unit.x && unit.z != placed_unit.z) // Don't check self
                            .enumerate()
                            .find_map(|(idx, unit)| {
                                if unit.x == space_x && unit.z == space_z {
                                    Some(idx)
                                }
                                else {
                                    None
                                }
                            });

                        if let Some(corridor_behind_idx) = corridor_behind_idx {
                            // Set reflexive adjacent_door pointers to None before deletion
                            if let Some(adjacent_door) = &placed_unit.doors[0].borrow().adjacent_door {
                                adjacent_door.upgrade().unwrap().borrow_mut().adjacent_door = None;
                            }
                            let corridor_behind = &self.layout.borrow().map_units[corridor_behind_idx];
                            for door in corridor_behind.doors.iter() {
                                if let Some(adjacent_door) = &door.borrow().adjacent_door {
                                    adjacent_door.upgrade().unwrap().borrow_mut().adjacent_door = None;
                                }
                            }

                            // Store this for later
                            let cap_door_dir = placed_unit.doors[0].borrow().door_unit.direction.clone();
                            let attach_to = placed_unit.doors[0].borrow().adjacent_door.as_ref().unwrap().upgrade().unwrap();

                            // Remove the one with the greater index first so we don't have to re-find
                            // the other one after shifting.
                            if i > corridor_behind_idx {
                                self.layout.borrow_mut().map_units.remove(i);
                                self.layout.borrow_mut().map_units.remove(corridor_behind_idx);
                            }
                            else {
                                self.layout.borrow_mut().map_units.remove(corridor_behind_idx);
                                self.layout.borrow_mut().map_units.remove(i);
                            }

                            // Add a hallway unit in the cap's place. Note that another hallway unit
                            // isn't added in place of the deleted hallway behind the cap; it will be
                            // added in a normal hallway pass after this.
                            let chosen_hallway = hallway_unit_names[self.rng.rand_int(hallway_unit_names.len() as u32) as usize];
                            for unit in self.corridor_queue.iter() {
                                if unit.unit_folder_name == chosen_hallway && unit.doors[0].direction == cap_door_dir {
                                    if let Some(approved_unit) = self.try_place_unit_at(attach_to.clone(), unit, 0) {
                                        cap_to_replace = Some(approved_unit);
                                        break 'change_cap_to_hallway;
                                    }
                                }
                            }
                            panic!("Deleted cap in cap-to-hallway replacement step but couldn't replace it with a hallway!");
                        }
                    }
                    if let Some(cap_to_replace) = cap_to_replace {
                        debug!("Replacing cap at ({}, {}) with hallway unit of type '{}'",
                            cap_to_replace.x, cap_to_replace.z, cap_to_replace.unit.unit_folder_name);
                        self.place_map_unit(cap_to_replace, true);
                    }
                }

                if self.open_doors().len() > 0 { continue; }

                // Look for instances of two 1x1 hallway units in a row and change them to
                // single 2x1 hallway units.
                // This section is easily the worst piece of code in this whole file.

                // Create list of 1x1 and 2x1 hallway unit names
                let hallway_unit_names_1x1: Vec<String> = self.corridor_queue.iter()
                    .filter(|unit| unit.width == 1 && unit.height == 1 && unit.num_doors == 2)
                    .filter(|unit| unit.doors[0].direction == 0 && unit.doors[1].direction == 2)
                    .map(|unit| unit.unit_folder_name.clone())
                    .collect();
                let hallway_unit_names_2x1: Vec<String> = self.corridor_queue.iter()
                    .filter(|unit| unit.width == 1 && unit.height == 2 && unit.num_doors == 2)
                    // Filter out east-to-west hallways. Not sure why this is done.
                    .filter(|unit| unit.doors[0].direction == 0 && unit.doors[1].direction == 2)
                    .map(|unit| unit.unit_folder_name.clone())
                    .collect();

                if hallway_unit_names_1x1.is_empty() || hallway_unit_names_2x1.is_empty() {
                    continue;
                }

                // Required to avoid panics with RefCell
                let mut num_placed_units = self.layout.borrow().map_units.len();
                let mut unit_1_idx = 0;
                while unit_1_idx < num_placed_units {
                    unit_1_idx += 1;
                    if !hallway_unit_names_1x1.contains(&self.layout.borrow().map_units[unit_1_idx-1].unit.unit_folder_name) {
                        continue;
                    }

                    // Check for another 1x1 hallway next to this one
                    let mut md: Option<Rc<RefCell<PlacedDoor>>> = None;
                    let mut od: Option<Rc<RefCell<PlacedDoor>>> = None;
                    let mut unit_2_idx = 99999999;
                    for j in 0..2 {
                        md = Some(self.layout.borrow().map_units[unit_1_idx-1].doors[j].clone());
                        unit_2_idx = md.as_ref().unwrap().borrow().adjacent_door.as_ref().unwrap().upgrade().unwrap().borrow().attached_to.unwrap();
                        if hallway_unit_names_1x1.contains(&self.layout.borrow().map_units[unit_2_idx].unit.unit_folder_name) {
                            od = md.as_ref().unwrap().borrow().adjacent_door.as_ref().unwrap().upgrade();
                            break;
                        }
                    }
                    if od.is_none() {
                        continue;
                    }

                    let expand_from;
                    let desired_direction;
                    // Create a sub-scope to avoid conflicting borrows of self.layout
                    {
                        let unit_1 = &self.layout.borrow().map_units[unit_1_idx-1];
                        let unit_2 = &self.layout.borrow().map_units[unit_2_idx];

                        // Find which door to expand from
                        expand_from = if unit_1.x > unit_2.x || unit_1.z < unit_2.z {
                            unit_1.doors[
                                md.unwrap().borrow().door_unit.door_links[0].door_id
                            ]
                            .borrow().adjacent_door
                            .as_ref().unwrap().upgrade().unwrap()
                        }
                        else {
                            unit_2.doors[
                                od.unwrap().borrow().door_unit.door_links[0].door_id
                            ]
                            .borrow().adjacent_door
                            .as_ref().unwrap().upgrade().unwrap()
                        };

                        // Set reflexive adjacent_door pointers to None before deletion
                        for door in unit_1.doors.iter() {
                            if let Some(adjacent_door) = &door.borrow().adjacent_door {
                                adjacent_door.upgrade().unwrap().borrow_mut().adjacent_door = None;
                            }
                        }
                        for door in unit_2.doors.iter() {
                            if let Some(adjacent_door) = &door.borrow().adjacent_door {
                                adjacent_door.upgrade().unwrap().borrow_mut().adjacent_door = None;
                            }
                        }

                        // Store this for later
                        desired_direction = if unit_1.x == unit_2.x { 0 } else { 1 };
                    };

                    // Delete the 1x1 hallway units
                    if unit_1_idx-1 > unit_2_idx {
                        self.layout.borrow_mut().map_units.remove(unit_1_idx-1);
                        self.layout.borrow_mut().map_units.remove(unit_2_idx);
                    }
                    else {
                        self.layout.borrow_mut().map_units.remove(unit_2_idx);
                        self.layout.borrow_mut().map_units.remove(unit_1_idx-1);
                    }
                    self.recalculate_door_attachments();
                    num_placed_units -= 2;

                    // Choose a 2x1 hallway unit to add in their place
                    let mut placed = false;
                    let name_chosen_2x1 = &hallway_unit_names_2x1[self.rng.rand_int(hallway_unit_names_2x1.len() as u32) as usize];
                    for new_unit in self.corridor_queue.iter() {
                        if &new_unit.unit_folder_name == name_chosen_2x1 && new_unit.doors[0].direction == desired_direction {
                            if let Some(approved_unit) = self.try_place_unit_at(expand_from.clone(), new_unit, 0) {
                                debug!("Combining hallway units into type '{}' at ({}, {})",
                                    new_unit.unit_folder_name, expand_from.borrow().x, expand_from.borrow().z);
                                self.place_map_unit(approved_unit, true);
                                num_placed_units += 1;
                                placed = true;
                                break;
                            }
                        }
                    }
                    assert!(placed, "Deleted hallway units to combine but couldn't place a new hallway unit in their place! Seed: {:#X}, Sublevel: {}", seed, caveinfo.name());
                }

                // After this, we're finished setting room tiles.
                break;
            }
        }

        // Recenter the map such that all positions are >= 0
        // {
        //     let min_x = self.layout.borrow().map_units.iter().map(|unit| unit.x).min().unwrap();
        //     let min_z = self.layout.borrow().map_units.iter().map(|unit| unit.z).min().unwrap();
        //     for map_unit in self.layout.borrow_mut().map_units.iter_mut() {
        //         map_unit.x = map_unit.x - min_x;
        //         map_unit.z = map_unit.z - min_z;
        //     }
        //     debug!("Recentered map.");
        // }

        // Set the start point, a.k.a. the Research Pod
        {
            let mut layout_mut = self.layout.borrow_mut();
            let mut candidates: Vec<&mut PlacedSpawnPoint> = layout_mut.map_units[0]
                .spawnpoints.iter_mut()
                .filter(|sp| sp.spawnpoint_unit.group == 7)
                .collect();
            let chosen = self.rng.rand_int(candidates.len() as u32) as usize;
            candidates[chosen].contains = RefCell::new(Some(SpawnObject::Ship));
            self.placed_spawn_point = Some(candidates[chosen].clone());
            debug!("Placed ship pod at ({}, {}).", candidates[chosen].x, candidates[chosen].z);
        }

        // Calculate Distance Score.
        // Distance Score (a.k.a. Door Score) is based on the straight-line distance
        // between doors. This is NOT dependent on enemies or anything else; it is
        // added to Enemy Score and other score types later on to form the total Unit Score.
        {
            // Initialize the starting Distance Scores for each door in the starting room to 1.
            for door in self.layout.borrow().map_units[0].doors.iter() {
                door.borrow_mut().door_score = Some(1);
                self.get_adjacent_door(door.clone()).borrow_mut().door_score = Some(1);
                debug!("Set Distance Score for starting room door at ({}, {}) to 1.", door.borrow().x, door.borrow().z);
            }

            // Set door scores in a roughly breadth-first fashion by finding the smallest
            // new door score that can be set from the doors that have already had their
            // score calculated.
            loop {
                if let Some((end_door, score)) = self.layout.borrow().map_units.iter()
                    .flat_map(|unit| unit.doors.iter())
                    .filter(|door| door.borrow().door_score.is_some())
                    .flat_map(|door| {
                        door.borrow().door_unit.door_links.iter()
                            .map(|door_link| {
                                let map_unit = &self.layout.borrow().map_units[door.borrow().attached_to.unwrap()];
                                let other_door = map_unit.doors[door_link.door_id].clone();
                                let potential_score = door.borrow().door_score.unwrap() + (door_link.distance / 10.0) as u32;
                                (other_door, potential_score)
                            })
                            // Only link to doors that haven't had their score set yet.
                            .filter(|(other_door, _)| other_door.borrow().door_score.is_none())
                            .collect::<Vec<(Rc<RefCell<PlacedDoor>>, u32)>>()
                    })
                    .min_by_key(|(_, potential_score)| *potential_score)
                {
                    end_door.borrow_mut().door_score = Some(score);
                    self.get_adjacent_door(end_door.clone()).borrow_mut().door_score = Some(score);
                    debug!("Set Distance Score for door at ({}, {}) to {}.", end_door.borrow().x, end_door.borrow().z, score);
                }
                else {
                    // When there are no doors with unset score, we are finished.
                    break;
                }
            }
        }

        // Place the exit hole and/or geyser, as applicable.
        if !caveinfo.is_final_floor {
            self.place_hole(SpawnObject::Hole);
        }
        if caveinfo.is_final_floor || caveinfo.has_geyser {
            self.place_hole(SpawnObject::Geyser);
        }

        // Done!
        self.layout.into_inner()
    }

    fn place_hole(&mut self, to_place: SpawnObject) {
        let layout = self.layout.borrow();

        // Get a list of applicable spawn points (group 4 or 9)
        let mut hole_spawn_points = Vec::new();
        for unit_type in [RoomType::Room, RoomType::DeadEnd, RoomType::Hallway] {
            // Only use hallway spawn points if there are zero other available locations.
            if unit_type == RoomType::Hallway && hole_spawn_points.len() > 0 {
                continue;
            }

            for unit in layout.map_units.iter() {
                if unit.unit.room_type != unit_type {
                    continue;
                }
                // Hole Score of this unit is the smallest of its Door Scores.
                let score = unit.doors.iter()
                    .map(|door| door.borrow().door_score.unwrap())
                    .min()
                    // Some units have zero doors, so we default to 0 if that's the case.
                    .unwrap_or_default();

                for spawn_point in unit.spawnpoints.iter() {
                    if spawn_point.contains.borrow().is_some() {
                        continue;
                    }

                    let dist_to_start = spawn_point_dist(&self.placed_spawn_point.as_ref().unwrap(), spawn_point);
                    if (spawn_point.spawnpoint_unit.group == 4 && dist_to_start >= 150.0) || (spawn_point.spawnpoint_unit.group == 9) {
                        *spawn_point.hole_score.borrow_mut() = Some(score);
                        hole_spawn_points.push(spawn_point);
                    }
                }
            }
        }

        // Only consider the spots with the highest score
        let max_hole_score = hole_spawn_points.iter()
            .filter(|sp| sp.contains.borrow().is_none())
            .map(|sp| sp.hole_score.borrow().unwrap())
            .max()
            .expect(&format!("{} {:#X}", self.cave_name, self.starting_seed));

        let candidate_spawnpoints = hole_spawn_points.iter()
            .filter(|sp| sp.hole_score.borrow().unwrap() == max_hole_score)
            .filter(|sp| sp.contains.borrow().is_none())
            .collect::<Vec<_>>();

        let hole_location = candidate_spawnpoints[self.rng.rand_int(candidate_spawnpoints.len() as u32) as usize];
        *hole_location.contains.borrow_mut() = Some(to_place.clone());

        match to_place {
            SpawnObject::Hole => {
                self.placed_exit_hole = Some(hole_location.clone().clone());
                debug!("Placed Exit Hole at ({}, {}).", hole_location.x, hole_location.z);
            },
            SpawnObject::Geyser => {
                self.placed_exit_geyser = Some(hole_location.clone().clone());
                debug!("Placed Exit Geyser at ({}, {}).", hole_location.x, hole_location.z);
            },
            _ => panic!("Tried to place an object other than Hole or Geyser in place_hole"),
        }
    }

    fn get_adjacent_door(&self, door: Rc<RefCell<PlacedDoor>>) -> Rc<RefCell<PlacedDoor>> {
        door.borrow().adjacent_door.as_ref().unwrap().upgrade().unwrap()
    }

    fn recalculate_door_attachments(&mut self) {
        for (i, unit) in self.layout.borrow().map_units.iter().enumerate() {
            for door in unit.doors.iter() {
                door.borrow_mut().attached_to = Some(i);
            }
        }
    }

    fn place_map_unit(&mut self, unit: PlacedMapUnit, checks: bool) {
        for door in unit.doors.iter() {
            door.borrow_mut().attached_to = Some(self.layout.borrow().map_units.len());
        }
        self.layout.borrow_mut().map_units.push(unit);

        if checks {
            self.attach_close_doors();
            self.shuffle_unit_priority();
            self.recompute_map_size();
        }
    }

    fn recompute_map_size(&mut self) {
        let placed_units = self.layout.borrow();
        let last_placed_unit = placed_units.map_units.last().unwrap();
        self.map_min_x = min(self.map_min_x, last_placed_unit.x);
        self.map_min_z = min(self.map_min_z, last_placed_unit.z);
        self.map_max_x = max(self.map_max_x, last_placed_unit.x + last_placed_unit.unit.width as isize);
        self.map_max_z = max(self.map_max_z, last_placed_unit.z + last_placed_unit.unit.height as isize);
        self.map_has_diameter_36 = self.map_max_x-self.map_min_x >= 36 || self.map_max_z-self.map_min_z >= 36;
    }

    /// After placing a map unit, a targeted shuffle is performed to increase the chances of
    /// generating other map units that have been seen less often.
    fn shuffle_unit_priority(&mut self) {
        let placed_units = self.layout.borrow();
        let last_placed_unit = placed_units.map_units.last().unwrap();
        match last_placed_unit.unit.room_type {
            RoomType::DeadEnd => self.rng.rand_backs(&mut self.cap_queue),
            RoomType::Hallway => self.rng.rand_backs(&mut self.corridor_queue),
            RoomType::Room => {
                // Count each type of placed room so far
                let mut room_type_counter: Vec<(&str, usize)> = Vec::new();
                for unit in placed_units.map_units.iter().filter(|unit| unit.unit.room_type == RoomType::Room) {
                    if let Some(entry) = room_type_counter.iter_mut().find(|(name, _)| name == &unit.unit.unit_folder_name) {
                        entry.1 += 1;
                    }
                    else {
                        room_type_counter.push((&unit.unit.unit_folder_name, 1));
                    }
                }

                // Sort the room names by frequency (ascending) using a swapping sort.
                for i in 0..room_type_counter.len() {
                    for j in i+1..room_type_counter.len() {
                        if room_type_counter[i].1 > room_type_counter[j].1 {
                            room_type_counter.swap(i, j);
                        }
                    }
                }

                // Starting with the least-frequently-placed room types, push all entries
                // for that room type to the end of the room queue. The result is that the
                // *most* frequent room types will be at the end since they're done last,
                // and all the rooms that haven't been used yet will be at the front.
                for room_type in room_type_counter {
                    let room_type = room_type.0;  // Don't need the frequency anymore
                    let mut idx = 0;
                    let mut matching_rooms = Vec::new();
                    while idx < self.room_queue.len() {
                        if room_type == self.room_queue[idx].unit_folder_name {
                            matching_rooms.push(self.room_queue.remove(idx));
                        } else {
                            idx += 1;
                        }
                    }

                    // matching_rooms should always have length 4 (one entry per facing direction)
                    // but I choose to explicitly pass 4 in case there are exceptions to this for
                    // some reason.
                    self.rng.rand_backs_n(&mut matching_rooms, 4);

                    self.room_queue.append(&mut matching_rooms);
                }
            }
        }
    }

    /// Looks for 'close' doors that are directly facing each other and attaches
    /// them together.
    fn attach_close_doors(&self) {
        let placed_units = self.layout.borrow();
        let last_placed_unit = placed_units.map_units.last().unwrap();
        for new_door in last_placed_unit.doors.iter() {
            for open_door in self.open_doors() {
                if new_door.borrow().lines_up_with(&open_door.borrow()) {
                    new_door.borrow_mut().adjacent_door = Some(Rc::downgrade(&open_door));
                    open_door.borrow_mut().adjacent_door = Some(Rc::downgrade(new_door));
                }
            }
        }
    }

    fn open_doors(&self) -> Vec<Rc<RefCell<PlacedDoor>>> {
        self.layout.borrow().map_units.iter()
            .flat_map(|unit| unit.doors.iter().map(move |door| door.clone()))
            .filter(|door| door.borrow().adjacent_door.is_none())
            .collect()
    }

    fn shuffle_corridor_priority(&mut self, caveinfo: &FloorInfo) {
        let max_num_doors_single_unit = caveinfo.max_num_doors_single_unit();
        let num_open_doors = self.open_doors().len();
        let mut corridor_priority = Vec::new();

        // If few open doors, prioritize corridor units with many doors
        if num_open_doors < 4 {
            for i in 0..max_num_doors_single_unit {
                corridor_priority.push(max_num_doors_single_unit-i);
            }
        }
        // If many open doors, prioritize hallways
        else if num_open_doors >= 10 {
            for i in 0..max_num_doors_single_unit {
                corridor_priority.push(i+1);
            }
        }
        // Otherwise prioritize randomly
        else {
            for i in 0..max_num_doors_single_unit {
                corridor_priority.push(i+1);
            }
            self.rng.rand_swaps(&mut corridor_priority);
        }

        // Sort the corridor queue by the priority determined above
        let mut new_corridor_queue = Vec::new();
        for num_doors in corridor_priority {
            let mut i = 0;
            while i < self.corridor_queue.len() {
                if self.corridor_queue[i].num_doors == num_doors {
                    new_corridor_queue.push(self.corridor_queue.remove(i));
                }
                else { i += 1; }
            }
        }
        self.corridor_queue = new_corridor_queue;
    }

    /// Attempts to place a new map unit connected to destination_door, if it fits.
    /// Returns true if the map unit was successfully placed, otherwise returns false.
    fn try_place_unit_at(&self, destination_door: Rc<RefCell<PlacedDoor>>, new_unit: &CaveUnit, door_index: usize) -> Option<PlacedMapUnit> {
        // Ensure doors are facing each other
        if !destination_door.borrow().door_unit.facing(&new_unit.doors[door_index]) {
            return None;
        }

        let new_unit_door = &new_unit.doors[door_index];
        let (candidate_unit_x, candidate_unit_z) = match new_unit_door.direction {
            0 => (destination_door.borrow().x - new_unit_door.side_lateral_offset as isize, destination_door.borrow().z),
            1 => (destination_door.borrow().x - new_unit.width as isize, destination_door.borrow().z - new_unit_door.side_lateral_offset as isize),
            2 => (destination_door.borrow().x - new_unit_door.side_lateral_offset as isize, destination_door.borrow().z - new_unit.height as isize),
            3 => (destination_door.borrow().x, destination_door.borrow().z - new_unit_door.side_lateral_offset as isize),
            _ => panic!("Invalid door direction")
        };
        let candidate_unit = PlacedMapUnit::new(new_unit, candidate_unit_x, candidate_unit_z);

        // Make sure the new unit wouldn't overlap any already placed units
        for placed_unit in self.layout.borrow().map_units.iter() {
            if placed_unit.overlaps(&candidate_unit) {
                return None;
            }
        }

        // Check the space in front of each door in the candidate unit. That space must either
        // line up with an existing door, or be completely empty. Otherwise that means it's
        // facing straight into the outer wall of a placed room, which we don't want.
        for new_door in candidate_unit.doors.iter() {
            // If the door lines up with an existing door, we can move on.
            if self.open_doors().iter().any(|open_door| new_door.borrow().lines_up_with(&open_door.borrow())) {
                continue;
            }

            // However if there are any that don't line up, we need to check the space in front.
            let open_space_x = new_door.borrow().x - (if new_door.borrow().door_unit.direction == 3 { 1 } else { 0 });
            let open_space_z = new_door.borrow().z - (if new_door.borrow().door_unit.direction == 0 { 1 } else { 0 });
            if self.layout.borrow().map_units.iter()
                .any(|placed_unit| {
                    boxes_overlap(
                        open_space_x, open_space_z, 1, 1,
                        placed_unit.x, placed_unit.z, placed_unit.unit.width, placed_unit.unit.height
                    )
                })
            {
                return None;
            }
        }

        // Same thing again, but this time checking existing doors against the new map unit
        for open_door in self.open_doors() {
            // If the door lines up with an existing door, we can move on.
            if candidate_unit.doors.iter().any(|new_door| open_door.borrow().lines_up_with(&new_door.borrow())) {
                continue;
            }

            // However if there are any that don't line up, we need to check the space in front.
            let open_space_x = open_door.borrow().x - (if open_door.borrow().door_unit.direction == 3 { 1 } else { 0 });
            let open_space_z = open_door.borrow().z - (if open_door.borrow().door_unit.direction == 0 { 1 } else { 0 });
            if boxes_overlap(
                open_space_x, open_space_z, 1, 1,
                candidate_unit.x, candidate_unit.z, candidate_unit.unit.width, candidate_unit.unit.height
            ) {
                return None;
            }
        }

        Some(candidate_unit)
    }

    /// Choose some random open doors to mark as 'capped'.
    /// This means they won't be used as starting points to generate new hallways,
    /// however they can still be attached if hallways stemming from elsewhere
    /// line up by chance.
    fn mark_random_open_doors_as_caps(&mut self, caveinfo: &FloorInfo) {
        if self.marked_open_doors_as_caps {
            return;
        }
        self.marked_open_doors_as_caps = true;

        let mut num_marked = 0;  // We'll stop after 16 maximum.
        for open_door in self.open_doors() {
            if self.rng.rand_f32() < caveinfo.cap_probability {
                open_door.borrow_mut().marked_as_cap = true;
                num_marked += 1;
                if num_marked >= 16 {
                    break;
                }
            }
        }
    }
}


#[derive(Debug, Clone)]
pub struct PlacedMapUnit {
    pub unit: CaveUnit,
    pub x: isize,
    pub z: isize,
    pub doors: Vec<Rc<RefCell<PlacedDoor>>>,
    pub spawnpoints: Vec<PlacedSpawnPoint>,
}

impl PlacedMapUnit {
    pub fn new(unit: &CaveUnit, x: isize, z: isize) -> PlacedMapUnit {
        let doors = unit.doors.iter()
            .map(|door| {
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
                        attached_to: None,
                        marked_as_cap: false,
                        adjacent_door: None,
                        door_score: None,
                    }
                ))
            })
            .collect();

        let spawnpoints = unit.spawn_points.iter()
            .map(|sp| {
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
                    contains: RefCell::new(None),
                    hole_score: RefCell::new(None),
                }
            })
            .collect();

        PlacedMapUnit {
            unit: unit.clone(),
            x, z,
            doors,
            spawnpoints,
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
    pub attached_to: Option<usize>,
    pub marked_as_cap: bool,
    pub adjacent_door: Option<Weak<RefCell<PlacedDoor>>>,
    pub door_score: Option<u32>,
}

impl PlacedDoor {
    pub fn facing(&self, other: &PlacedDoor) -> bool {
        (self.door_unit.direction as isize - other.door_unit.direction as isize).abs() == 2
    }

    pub fn lines_up_with(&self, other: &PlacedDoor) -> bool {
        self.facing(other) && self.x == other.x && self.z == other.z
    }
}

pub fn boxes_overlap(x1: isize, z1: isize, w1: u16, h1: u16, x2: isize, z2: isize, w2: u16, h2: u16) -> bool {
    !((x1 + w1 as isize <= x2 || x2 + w2 as isize <= x1) || (z1 + h1 as isize <= z2 || z2 + h2 as isize <= z1))
}


#[derive(Debug, Clone)]
pub struct PlacedSpawnPoint {
    pub spawnpoint_unit: SpawnPoint,
    pub x: f32,
    pub z: f32,
    pub angle: f32,
    pub contains: RefCell<Option<SpawnObject>>,
    pub hole_score: RefCell<Option<u32>>,
}

fn spawn_point_dist(a: &PlacedSpawnPoint, b: &PlacedSpawnPoint) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    let dy = a.spawnpoint_unit.pos_y - b.spawnpoint_unit.pos_y;
    crate::pikmin_math::sqrt(dx*dx + dy*dy + dz*dz)
}


#[derive(Debug, Clone)]
pub enum SpawnObject {
    Teki(TekiInfo),
    Item(ItemInfo),
    Gate(GateInfo),
    Hole,
    Geyser,
    Ship
}
