use std::{cell::RefCell, cmp::{max, min}, collections::HashMap, rc::{Rc, Weak}};
use crate::{caveinfo::{CaveUnit, DoorUnit, FloorInfo, RoomType}, pikmin_math::PikminRng};

/// Represents a generated sublevel layout.
/// Given a seed and a CaveInfo file, a layout can be generated using a
/// re-implementation of Pikmin 2's internal cave generation function.
/// These layouts are 100% game-accurate (which can be verified using
/// the set-seed mod) and specify exact positions for every tile, teki,
/// and treasure.

pub struct Layout {
    pub map_units: Vec<PlacedMapUnit>,
}

impl<'token> Layout {
    pub fn generate(seed: u32, caveinfo: FloorInfo) -> Layout {
        let layoutbuilder = LayoutBuilder {
            layout: Layout {
                map_units: Vec::new(),
            },
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
        };
        layoutbuilder.generate(seed, caveinfo)
    }
}

struct LayoutBuilder {
    rng: PikminRng,
    layout: Layout,
    cap_queue: Vec<CaveUnit>,
    room_queue: Vec<CaveUnit>,
    corridor_queue: Vec<CaveUnit>,
    allocated_enemy_slots_by_group: [u16; 10],
    enemy_weight_sum_by_group: [u16; 10],
    num_slots_used_for_min: u16,
    map_min_x: u16,
    map_min_z: u16,
    map_max_x: u16,
    map_max_z: u16,
    map_has_diameter_36: bool,
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
                self.num_slots_used_for_min += teki.minimum_amount as u16;
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
        self.place_map_unit(start_map_unit, 0, 0, true);


        unimplemented!();

        // Done!
        self.layout
    }

    fn place_map_unit(&mut self, unit: CaveUnit, x: u16, z: u16, checks: bool) {
        let unit = PlacedMapUnit::new(unit, x, z);
        self.layout.map_units.push(unit);

        if checks {
            self.attach_close_doors();
            self.shuffle_unit_priority();
            self.recompute_map_size();
        }
    }

    fn recompute_map_size(&mut self) {
        let last_placed_unit = self.layout.map_units.last().unwrap();
        self.map_min_x = min(self.map_min_x, last_placed_unit.x);
        self.map_min_z = min(self.map_min_z, last_placed_unit.z);
        self.map_max_x = max(self.map_max_x, last_placed_unit.x + last_placed_unit.unit.width);
        self.map_max_z = max(self.map_max_z, last_placed_unit.z + last_placed_unit.unit.height);
        self.map_has_diameter_36 = self.map_max_x-self.map_min_x >= 36 || self.map_max_z-self.map_min_z >= 36;
    }

    /// After placing a map unit, a targeted shuffle is performed to increase the chances of
    /// generating other map units that have been seen less often.
    fn shuffle_unit_priority(&mut self) {
        let last_placed_unit = self.layout.map_units.last().unwrap();
        match last_placed_unit.unit.room_type {
            RoomType::DeadEnd => self.rng.rand_backs(&mut self.cap_queue),
            RoomType::Hallway => self.rng.rand_backs(&mut self.corridor_queue),
            RoomType::Room => {
                // Count each type of placed room so far
                let mut room_type_counter: HashMap<&str, usize> = HashMap::new();
                for unit in self.layout.map_units.iter() {
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
        let last_placed_unit = self.layout.map_units.last().unwrap();
        for new_door in last_placed_unit.doors.iter() {
            for open_door in self.open_doors() {
                if new_door.borrow().facing(&open_door.borrow())
                   && new_door.borrow().x == open_door.borrow().x
                   && new_door.borrow().z == open_door.borrow().z {
                    new_door.borrow_mut().adjacent_door = Some(Rc::downgrade(&open_door));
                    open_door.borrow_mut().adjacent_door = Some(Rc::downgrade(new_door));
                }
            }
        }
    }

    fn open_doors(&self) -> Vec<Rc<RefCell<PlacedDoor>>> {
        self.layout.map_units.iter()
            .flat_map(|unit| unit.doors.clone())
            .filter(|door| door.borrow().adjacent_door.is_none())
            .collect()
    }
}

pub struct PlacedMapUnit {
    pub unit: CaveUnit,
    pub x: u16,
    pub z: u16,
    pub doors: Vec<Rc<RefCell<PlacedDoor>>>,
}

impl PlacedMapUnit {
    pub fn new(unit: CaveUnit, x: u16, z: u16) -> PlacedMapUnit {
        let doors = unit.doors.iter()
            .map(|door| {
                let (door_x, door_z) = match door.direction {
                    0 => (x + door.side_lateral_offset, z),
                    1 => (x + unit.width,               z + door.side_lateral_offset),
                    2 => (x + door.side_lateral_offset, z + unit.height),
                    3 => (x,                            z + door.side_lateral_offset),
                    _ => panic!("Invalid door direction")
                };
                Rc::new(RefCell::new(
                    PlacedDoor {
                        x: door_x,
                        z: door_z,
                        door_unit: door.clone(),
                        adjacent_door: None
                    }
                ))
            })
            .collect();
        PlacedMapUnit {
            unit, x, z, doors
        }
    }
}

pub struct PlacedDoor {
    pub x: u16,
    pub z: u16,
    pub door_unit: DoorUnit,
    pub adjacent_door: Option<Weak<RefCell<PlacedDoor>>>,
}

impl PlacedDoor {
    pub fn facing(&self, other: &PlacedDoor) -> bool {
        (self.door_unit.direction as isize - other.door_unit.direction as isize).abs() == 2
    }
}
