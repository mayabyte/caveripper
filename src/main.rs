use std::error::Error;
use std::num::ParseIntError;
use cavegen::assets::ASSETS;
use cavegen::caveinfo::{FloorInfo, RoomType};
use cavegen::layout::{Layout, SpawnObject};
use cavegen::layout::render::render_layout;
use cavegen::sublevel::Sublevel;
use once_cell::sync::Lazy;
use simple_logger::SimpleLogger;
use structopt::StructOpt;
use rand::Rng;
use rayon::prelude::*;
use indicatif::{ProgressBar, ProgressStyle, ParallelProgressIterator};

fn main() -> Result<(), Box<dyn Error>> {
    if cfg!(debug_assertions) {
        SimpleLogger::new().with_level(log::LevelFilter::max()).init()?;
    }

    let args = Args::from_args();
    let caveinfo = ASSETS.get_caveinfo(&args.sublevel).unwrap();
    let seed: u32 = from_hex_str(&args.seed)?;

    let layout = Layout::generate(seed, &caveinfo);
    render_layout(&layout);

    // let mut bar = ProgressBar::new_spinner();
    // bar.set_style(ProgressStyle::default_bar().template("[{elapsed}] [Searched: {pos} ({per_sec})]"));
    // let sr7 = ASSETS.get_caveinfo("sr7").unwrap();

    // let bloysterless_seed: u32 = (330_000_000..0xFFFFFFFF).into_par_iter()
    //     .progress_with(bar)
    //     .find_first(|seed| {
    //         let layout = Layout::generate(*seed, &sr7);
    //         layout.map_units.iter().filter(|unit| unit.unit.room_type == RoomType::Room).count() == 1
    //     }).unwrap();
    // println!("{}", bloysterless_seed);

    Ok(())
}


#[derive(StructOpt)]
struct Args {
    #[structopt()]
    sublevel: String,

    #[structopt()]
    seed: String,
}

fn from_hex_str(src: &str) -> Result<u32, ParseIntError> {
    u32::from_str_radix(src.strip_prefix("0x").unwrap_or(src), 16)
}
