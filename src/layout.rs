pub mod render;
#[cfg(test)]
pub mod test;

use std::{cell::RefCell, cmp::{max, min}, collections::HashMap, rc::{Rc, Weak}};
use itertools::Itertools;

use crate::{caveinfo::{CaveUnit, DoorUnit, FloorInfo, RoomType}, pikmin_math::PikminRng};

/// Represents a generated sublevel layout.
/// Given a seed and a CaveInfo file, a layout can be generated using a
/// re-implementation of Pikmin 2's internal cave generation function.
/// These layouts are 100% game-accurate (which can be verified using
/// the set-seed mod) and specify exact positions for every tile, teki,
/// and treasure.

#[derive(Debug)]
pub struct Layout {
    pub map_units: Vec<PlacedMapUnit>,
}

impl<'token> Layout {
    pub fn generate(seed: u32, caveinfo: FloorInfo) -> Layout {
        let layoutbuilder = LayoutBuilder {
            layout: RefCell::new(Layout {
                map_units: Vec::new(),
            }),
            rng: PikminRng::new(seed),
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
        };
        layoutbuilder.generate(seed, caveinfo)
    }
}

struct LayoutBuilder {
    rng: PikminRng,
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
}

impl LayoutBuilder {
    /// Cave generation algorithm. Reimplementation of the code in JHawk's
    /// CaveGen (https://github.com/JHaack4/CaveGen/blob/2c99bf010d2f6f80113ed7eaf11d9d79c6cff367/CaveGen.java#L643)
    ///
    /// This implementation follows CaveGen's as closely as possible, even
    /// when that results in non-idiomatic Rust code. It is my 'reference'
    /// implementation; a more optimized one will follow.
    pub fn generate(mut self, seed: u32, caveinfo: FloorInfo) -> Layout {
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
                        self.place_map_unit(cap_to_place, true);
                    }
                }

                if self.open_doors().len() > 0 { continue; }

                break;
            }
        }

        // Done!
        self.layout.into_inner()
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
                let mut room_type_counter: HashMap<&str, usize> = HashMap::new();
                for unit in placed_units.map_units.iter().filter(|unit| unit.unit.room_type == RoomType::Room) {
                    *room_type_counter.entry(&unit.unit.unit_folder_name).or_default() += 1;
                }

                // Sort the room names by frequency (ascending) using a swapping sort.
                let mut room_types_sorted: Vec<(&str, usize)> = room_type_counter.into_iter().collect();
                for i in 0..room_types_sorted.len() {
                    for j in i+1..room_types_sorted.len() {
                        if room_types_sorted[i].1 > room_types_sorted[j].1 {
                            room_types_sorted.swap(i, j);
                        }
                    }
                }

                // Starting with the least-frequently-placed room types, push all entries
                // for that room type to the end of the room queue. The result is that the
                // *most* frequent room types will be at the end since they're done last,
                // and all the rooms that haven't been used yet will be at the front.
                for room_type in room_types_sorted {
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
            // If every door lines up with an existing door, we can move on.
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
            // If every door lines up with an existing door, we can move on.
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

#[derive(Debug)]
pub struct PlacedMapUnit {
    pub unit: CaveUnit,
    pub x: isize,
    pub z: isize,
    pub doors: Vec<Rc<RefCell<PlacedDoor>>>,
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
                        adjacent_door: None
                    }
                ))
            })
            .collect();
        PlacedMapUnit {
            unit: unit.clone(), x, z, doors
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
