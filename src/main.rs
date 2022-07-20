mod cli;

use cavegen::search::SearchCondition;
use cli::{Cli, Commands};
use clap::Parser;
use rand::prelude::*;
use std::error::Error;
use cavegen::assets::ASSETS;
use cavegen::layout::Layout;
use cavegen::layout::render::render_layout;
use simple_logger::SimpleLogger;

fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();
    if args.debug_logging {
        SimpleLogger::new().with_level(log::LevelFilter::max()).init()?;
    }

    match args.subcommand {
        Commands::Generate{ sublevel, seed } => {
            let caveinfo = ASSETS.get_caveinfo(&sublevel)?;

            let layout = Layout::generate(seed, &caveinfo);
            render_layout(&layout);
        },
        Commands::Search{ sublevel, condition } => {
            let caveinfo = ASSETS.get_caveinfo(&sublevel)?;
            let mut seeds_searched = 0;
            loop {
                seeds_searched += 1;
                let seed: u32 = random();
                let layout = Layout::generate(seed, &caveinfo);
                match &condition { 
                    SearchCondition::CountEntity{ name, relationship, amount } => {
                        let entity_count = layout.map_units.iter()
                            .flat_map(|unit| unit.spawnpoints.iter().filter_map(|sp| sp.contains.as_ref()))
                            .filter(|entity| entity.name() == name)
                            .count();
                        if &entity_count.cmp(&amount) == relationship {
                            println!("Found matching seed: {:#10X}. Searched {} seeds.", seed, seeds_searched);
                            render_layout(&layout);
                            break;
                        }
                    }
                }
            }
        },
        Commands::Stats { sublevel, condition, num_to_search } => {
            let caveinfo = ASSETS.get_caveinfo(&sublevel)?;
            let mut num_matched = 0;
            for _ in 0..num_to_search {
                let seed: u32 = random();
                let layout = Layout::generate(seed, &caveinfo);
                match &condition { 
                    SearchCondition::CountEntity{ name, relationship, amount } => {
                        let entity_count = layout.map_units.iter()
                            .flat_map(|unit| unit.spawnpoints.iter().filter_map(|sp| sp.contains.as_ref()))
                            .filter(|entity| entity.name() == name)
                            .count();
                        if &entity_count.cmp(&amount) == relationship {
                            num_matched += 1;
                        }
                    }
                }
            }
            println!(
                "Searched {} layouts and found {} ({:.05}%) that match the condition.", 
                num_to_search, num_matched, num_matched as f32 / num_to_search as f32
            );
        }
    }

    Ok(())
}
