#![feature(try_blocks)]

pub mod seed;
pub mod caveinfo;
pub mod layout;

use seed::Seed;

fn main() {
    let x = Seed(12);
    println!("{}", x);
}
