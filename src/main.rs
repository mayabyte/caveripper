#![feature(option_result_contains)]
#![feature(slice_as_chunks)]

pub mod caveinfo;
pub mod layout;
pub mod pikmin_math;

use std::error::Error;

use caveinfo::get_sublevel_info;
use layout::Layout;

fn main() -> Result<(), Box<dyn Error>> {
    let caveinfo = get_sublevel_info("SCx1")?;
    Layout::generate(12345678u32, caveinfo);
    Ok(())
}
