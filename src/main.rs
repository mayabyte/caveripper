use std::error::Error;
use std::num::ParseIntError;
use cavegen::caveinfo::{FloorInfo, ALL_SUBLEVELS_MAP};
use cavegen::layout::Layout;
use cavegen::layout::render::render_layout;
use once_cell::sync::Lazy;
use simple_logger::SimpleLogger;
use structopt::StructOpt;

fn main() -> Result<(), Box<dyn Error>> {
    if cfg!(debug_assertions) {
        SimpleLogger::new().with_level(log::LevelFilter::max()).init()?;
    }

    let args = Args::from_args();
    let caveinfo = caveinfo_from_str(&args.sublevel).unwrap();
    let seed: u32 = from_hex_str(&args.seed)?;

    let layout = Layout::generate(seed, caveinfo);
    println!("{}", layout.slug());
    //render_layout(&layout);
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

fn caveinfo_from_str(cave: &str) -> Option<&'static Lazy<FloorInfo>> {
    ALL_SUBLEVELS_MAP.get(&cave.to_ascii_lowercase()).cloned()
}
