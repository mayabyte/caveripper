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
pub struct PikminRng {
    seed: Cell<u32>,
    pub initial_seed: u32
}

impl PikminRng {
    pub fn new(seed: u32) -> Self {
        Self {
            seed: Cell::new(seed),
            initial_seed: seed
        }
    }

    /// The RNG function as implemented by Pikmin 2.
    pub fn rand_raw(&self) -> u32 {
        let old_seed = self.seed.get();
        let new_seed = old_seed.wrapping_mul(0x41C64E6D).wrapping_add(0x3039);
        self.seed.set(new_seed);

        (new_seed >> 0x10) & 0x7FFF
    }

    /// Most of the game's internal values are 16-bit integers, so it crunches
    /// the raw RNG results down into 16-bit space via division for compatibility.
    pub fn rand_u16(&self, max: u32) -> u16 {
        // unsure if this multiplication is meant to be wrapping?
        ((self.rand_raw() * max) / 32768) as u16
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
}
