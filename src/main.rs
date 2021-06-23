#![feature(option_result_contains)]
#![feature(slice_as_chunks)]

pub mod caveinfo;
pub mod layout;
pub mod seed;

use caveinfo::get_sublevel_info;

fn main() {
    let caveinfo = get_sublevel_info("SCx1");
    println!("{:#?}", caveinfo);
}
