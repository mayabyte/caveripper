/// Seed for a sublevel's random generation.
/// Since sublevel generation is deterministic after the first RNG call,
/// one single internal seed state of Pikmin 2's RNG function plus a
/// CaveInfo specification for that sublevel's generation is enough to
/// uniquely specify one of its 2^32 possible layouts.

#[repr(transparent)]
pub struct Seed(pub u32);

impl std::fmt::Display for Seed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:08X}", self.0)
    }
}
