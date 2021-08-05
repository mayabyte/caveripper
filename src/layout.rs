use crate::{caveinfo::{CaveUnit, FloorInfo, RoomType}, pikmin_math::PikminRng};

/// Represents a generated sublevel layout.
/// Given a seed and a CaveInfo file, a layout can be generated using a
/// re-implementation of Pikmin 2's internal cave generation function.
/// These layouts are 100% game-accurate (which can be verified using
/// the set-seed mod) and specify exact positions for every tile, teki,
/// and treasure.

pub struct Layout {}

impl Layout {
    /// Cave generation algorithm. Reimplementation of the code in JHawk's
    /// CaveGen (https://github.com/JHaack4/CaveGen/blob/2c99bf010d2f6f80113ed7eaf11d9d79c6cff367/CaveGen.java#L643)
    ///
    /// This implementation follows CaveGen's as closely as possible, even
    /// when that results in non-idiomatic Rust code. It is my 'reference'
    /// implementation; a more optimized one will follow.
    pub fn generate(seed: u32, caveinfo: FloorInfo) -> Layout {
        // Initialize an RNG session with the given seed.
        let rng = PikminRng::new(seed);

        // ** mapUnitsInitialSorting ** //
        // https://github.com/JHaack4/CaveGen/blob/2c99bf010d2f6f80113ed7eaf11d9d79c6cff367/CaveGen.java#L644

        // Separate out different unit types
        let mut cap_queue: Vec<CaveUnit> = Vec::new();
        let mut room_queue: Vec<CaveUnit> = Vec::new();
        let mut corridor_queue: Vec<CaveUnit> = Vec::new();
        for unit in caveinfo.cave_units.clone().into_iter() {
            match unit.room_type {
                RoomType::DeadEnd => cap_queue.push(unit),
                RoomType::Room => room_queue.push(unit),
                RoomType::Hallway => corridor_queue.push(unit),
            }
        }

        // The order of these (and all other RNG calls) is important!
        rng.rand_backs(cap_queue);
        rng.rand_backs(room_queue);
        rng.rand_backs(corridor_queue);

        // ** End mapUnitsInitialSorting ** //

        // ** allocateEnemySlots ** //
        // https://github.com/JHaack4/CaveGen/blob/2c99bf010d2f6f80113ed7eaf11d9d79c6cff367/CaveGen.java#L645

        // Allocate minimum amounts of each enemy type
        let mut allocated_enemy_slots_by_group = [0; 10];
        let mut enemy_weight_sum_by_group = [0; 10];
        let mut num_slots_used_for_min: u16 = 0;
        for enemy_type in [0, 1, 5, 8] {
            for teki in caveinfo.teki_group(enemy_type) {
                allocated_enemy_slots_by_group[enemy_type as usize] += teki.minimum_amount;
                enemy_weight_sum_by_group[enemy_type as usize] += teki.filler_distribution_weight;
                num_slots_used_for_min += teki.minimum_amount as u16;
            }
        }

        // Fill remaining allocation budget randomly according to filler distribution weights
        for _ in 0..(caveinfo.max_main_objects.saturating_sub(num_slots_used_for_min)) {
            if let Some(group) = rng.rand_index_weight(&enemy_weight_sum_by_group) {
                allocated_enemy_slots_by_group[group] += 1;
            }
        }

        // ** End allocateEnemySlots ** //

        // ** Main map unit generation logic ** //

        unimplemented!()
    }
}
