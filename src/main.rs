mod cli;

use cli::Cli;
use clap::Parser;
use std::error::Error;
use cavegen::assets::ASSETS;
use cavegen::layout::Layout;
use cavegen::layout::render::render_layout;
use simple_logger::SimpleLogger;

fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();
    if cfg!(debug_assertions) || args.debug_logging {
        SimpleLogger::new().with_level(log::LevelFilter::max()).init()?;
    }

    let caveinfo = ASSETS.get_caveinfo(&args.sublevel)?;

    let layout = Layout::generate(args.seed, &caveinfo);
    render_layout(&layout);

    Ok(())
}
