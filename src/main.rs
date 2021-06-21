#![feature(option_result_contains)]

pub mod caveinfo;
pub mod layout;
pub mod seed;

use caveinfo::get_caveinfo;

fn main() {
    let caveinfo = get_caveinfo("EC".to_string());
    println!("{:?}", caveinfo);
}
