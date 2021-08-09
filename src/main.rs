#![feature(option_result_contains)]
#![feature(slice_as_chunks)]

pub mod caveinfo;
pub mod layout;
pub mod pikmin_math;

use std::error::Error;
use caveinfo::get_sublevel_info;
use layout::Layout;
use layout::render::render_layout;
use simple_logger::SimpleLogger;

fn main() -> Result<(), Box<dyn Error>> {
    if cfg!(debug_assertions) {
        SimpleLogger::new().with_level(log::LevelFilter::max()).init()?;
    }

    let caveinfo = get_sublevel_info("scx1")?;
    let layout = Layout::generate(0x12345678u32, caveinfo);
    // println!("{:#?}", &layout);
    render_layout(&layout);
    Ok(())
}
