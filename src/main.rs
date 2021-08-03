#![feature(option_result_contains)]
#![feature(slice_as_chunks)]

pub mod caveinfo;
pub mod layout;
pub mod seed;
pub mod pikmin_math;

use std::error::Error;

use caveinfo::get_sublevel_info;

fn main() -> Result<(), Box<dyn Error>> {
    let caveinfo = get_sublevel_info("SCx1")?;
    println!("{:#?}", caveinfo);
    Ok(())
}
