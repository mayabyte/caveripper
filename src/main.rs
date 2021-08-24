use std::error::Error;
use cavegen::caveinfo;
use cavegen::layout::Layout;
use cavegen::layout::render::render_layout;
use simple_logger::SimpleLogger;

fn main() -> Result<(), Box<dyn Error>> {
    if cfg!(debug_assertions) {
        SimpleLogger::new().with_level(log::LevelFilter::max()).init()?;
    }

    let layout = Layout::generate(0xF59F8835u32, &caveinfo::SH4);
    // println!("{:#?}", &layout);
    render_layout(&layout);
    Ok(())
}
