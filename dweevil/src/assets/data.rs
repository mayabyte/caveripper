/// Some pieces of data required to run Caveripper are small enough to embed within
/// the WASM blob to avoid making requests for them, such as caveinfo and unit files.
use include_dir::{include_dir, Dir};

pub static RESOURCES: Dir = include_dir!("$CARGO_MANIFEST_DIR/../resources");
pub static PIKMIN2: Dir = include_dir!("$CARGO_MANIFEST_DIR/../pikmin2");
