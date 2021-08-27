use std::error::Error;
use cavegen::caveinfo;
use cavegen::layout::Layout;
use cavegen::layout::render::render_layout;
use simple_logger::SimpleLogger;

fn main() -> Result<(), Box<dyn Error>> {
    if cfg!(debug_assertions) {
        SimpleLogger::new().with_level(log::LevelFilter::max()).init()?;
    }

    let layout = Layout::generate(0x1DEEDCF3u32, &caveinfo::BK5);
    // println!("{:#?}", &layout);
    render_layout(&layout);
    Ok(())
}
