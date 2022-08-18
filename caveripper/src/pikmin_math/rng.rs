use std::cell::Cell;

/// RNG instance for calling Pikmin 2's RNG function.
/// The game's RNG is deterministic pseudo-RNG, which is what allows us to
/// deterministically and reproducibly generate cave layouts from a single
/// seed value.
/// When we say the 'seed' of a layout, what we mean is the RNG function's
/// internal seed at the time the game enters the generation phase.
/// The set-seed Gecko code just replaces this seed with a given value
/// as soon as the game enters the cave generation function, resulting in
/// the same layouts every time.
#[derive(Default, Debug)]
pub struct PikminRng {
    seed: Cell<u32>,
    pub num_rng_calls: Cell<usize>
}

impl PikminRng {
    pub fn new(seed: u32) -> Self {
        Self {
            seed: Cell::new(seed),
            num_rng_calls: Cell::new(0),
        }
    }

    /// The RNG function as implemented by Pikmin 2.
    pub fn rand_raw(&self) -> u32 {
        let old_seed = self.seed.get();
        let new_seed = old_seed.wrapping_mul(0x41C64E6D).wrapping_add(0x3039);
        self.seed.set(new_seed);

        // Update RNG call count
        let old_count = self.num_rng_calls.get();
        self.num_rng_calls.set(old_count + 1);

        (new_seed >> 0x10) & 0x7FFF
    }

    /// Most of the game's internal values are 16-bit integers, so it crunches
    /// the raw RNG results down into 16-bit space via division for compatibility.
    pub fn rand_int(&self, max: u32) -> u32 {
        (self.rand_raw() as f32 * (max as f32 / 32768f32)) as u32
    }

    /// Similar to the above, the game only uses f32s. Since f32 can't represent
    /// all the possible values of a u32, the raw RNG results are crunched down
    /// to the size of a u16 first.
    pub fn rand_f32(&self) -> f32 {
        // process for conversion between u32 and f32 unclear; f32 cannot represent
        // the whole range of u32. what does rust do in this case? what does java
        // (original CaveGen implementation) do in this case?
        // possible alternative: `(self.rand_raw() as f64 / 32768f64) as f32`
        self.rand_raw() as f32 / 32768f32
    }

    /// Shuffles the given list by pushing randomly-chosen elements to the
    /// back of the list. Do this N times.
    pub fn rand_backs_n<T>(&self, list: &mut Vec<T>, n: usize) {
        for _ in 0..n {
            let index = self.rand_int(list.len() as u32);
            let elem = list.remove(index as usize);
            list.push(elem);
        }
    }

    /// Shortcut for rand_backs_n where n = the length of the provided list.
    pub fn rand_backs<T>(&self, list: &mut Vec<T>) {
        self.rand_backs_n(list, list.len())
    }

    /// Takes a list of weights and chooses a random weight. Returns the index of
    /// the chosen weight in the original list rather than the weight itself.
    #[allow(clippy::needless_range_loop)]
    pub fn rand_index_weight(&self, weights: &[u32]) -> Option<usize> {
        let total: u32 = weights.iter().sum();
        let mut cumulative_sum: u32 = 0;
        let threshold = self.rand_int(total);
        for idx in 0..weights.len() {
            cumulative_sum += weights[idx];
            if cumulative_sum > threshold {
                return Some(idx);
            }
        }
        None
    }

    /// For each element of the list, swaps the element there with a random element.
    pub fn rand_swaps<T>(&self, list: &mut Vec<T>) {
        for i in 0..list.len() {
            let swap_to = self.rand_int(list.len() as u32) as usize;
            list.swap(i, swap_to);
        }
    }
}
