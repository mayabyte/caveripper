use crate::{caveinfo::CaveInfo, seed::Seed};

/// Represents a generated sublevel layout.
/// Given a seed and a CaveInfo file, a layout can be generated using a
/// re-implementation of Pikmin 2's internal cave generation function.
/// These layouts are 100% game-accurate (which can be verified using
/// the set-seed mod) and specify exact positions for every tile, teki,
/// and treasure.

pub struct Layout {}

impl Layout {
    pub fn generate(seed: &Seed, caveinfo: &CaveInfo) -> Layout {
        unimplemented!()
    }
}
