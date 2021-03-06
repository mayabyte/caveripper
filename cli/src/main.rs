mod cli;

use cli::*;
use clap::Parser;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use rand::prelude::*;
use rayon::{self, iter::{IntoParallelIterator, ParallelIterator}};
use std::{error::Error, panic::RefUnwindSafe};
use std::panic::catch_unwind;
use std::time::{SystemTime, Duration};
use caveripper::{assets::ASSETS, layout::{render::{save_image, RenderOptions, render_caveinfo}}};
use caveripper::layout::Layout;
use caveripper::layout::render::render_layout;
use simple_logger::SimpleLogger;

static RAYON_EARLY_EXIT_PAYLOAD: &'static str = "__RAYON_EARLY_EXIT__";

fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();
    match args.verbosity {
        1 => SimpleLogger::new().with_level(log::LevelFilter::Warn).init()?,
        2 => SimpleLogger::new().with_level(log::LevelFilter::Info).init()?,
        3 => SimpleLogger::new().with_level(log::LevelFilter::max()).init()?,
        _ => {/* No higher log levels */},
    }

    // Run the desired command.
    match args.subcommand {
        Commands::Generate{ sublevel, seed, render_options } => {
            let caveinfo = ASSETS.get_caveinfo(&sublevel)?;
            let layout = Layout::generate(seed, &caveinfo);
            save_image(
                &render_layout(&layout, render_options)?,
                format!("{}_{:#010X}", layout.cave_name, layout.starting_seed)
            )?;
        },
        Commands::Caveinfo{ sublevel, text } => {
            let caveinfo = ASSETS.get_caveinfo(&sublevel)?;
            if text {
                println!("{}", caveinfo);
            }
            else {
                save_image(
                    &render_caveinfo(&caveinfo, RenderOptions::default())?,
                    format!("{}_Caveinfo", caveinfo.name())
                )?;
            }
        },
        Commands::Search{ sublevel, query, timeout, render, render_options } => {
            let caveinfo = ASSETS.get_caveinfo(&sublevel)?;

            let result = parallel_search_with_timeout(|| {
                let seed: u32 = random();
                let layout = Layout::generate(seed, &caveinfo);
                query.matches(&layout).then_some(seed)
            }, timeout);
            
            if let Some(seed) = result {
                println!("???? Found matching seed: {:#010X}.", seed);
                if render {
                    let layout = Layout::generate(seed, &caveinfo);
                    save_image(
                        &render_layout(&layout, render_options)?,
                        format!("{}_{:#010X}", layout.cave_name, layout.starting_seed)
                    )?;
                }
            }
            else {
                println!("???? Couldn't find a layout matching the condition '{}' in {}s.", query, timeout);
            }
        },
        Commands::Stats{ sublevel, query, num_to_search } => {
            let caveinfo = ASSETS.get_caveinfo(&sublevel)?;
            let num_matched = (0..num_to_search).into_par_iter()
                .progress()
                .filter(|_| {
                    let seed: u32 = random();
                    let layout = Layout::generate(seed, &caveinfo);
                    query.matches(&layout)
                })
                .count();
            println!(
                "???? Searched {} layouts and found {} ({:.03}%) that match the condition '{}'.", 
                num_to_search, num_matched, (num_matched as f32 / num_to_search as f32) * 100.0, &query
            );
        }
    }

    Ok(())
}

/// Invoke a search function in parallel using Rayon with an unlimited iteration
/// count and a timeout in case the desired condition isn't found.
/// 
/// `f` should return Some(T) when it has found the desired condition.
/// This function returns Some(T) when a value has been found, and None if the
/// timeout was reached or the search function panicked.
fn parallel_search_with_timeout<T, F>(f: F, timeout_secs: u64) -> Option<T> 
where F: Fn() -> Option<T> + Sync + Send + RefUnwindSafe,
      T: Send
{
    // Register a custom pass-through panic handler that suppresses the panic
    // message normally produced by the Rayon early exit hack, and forwards to
    // the default panic handler otherwise.
    let default_panic_handler = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));

    let timeout = Duration::from_secs(timeout_secs);
    let progress_bar = ProgressBar::new_spinner()
        .with_style(ProgressStyle::default_spinner().template("{spinner} {elapsed_precise} [{per_sec}, {pos} searched]"));
    progress_bar.enable_steady_tick(100);
    let start_time = SystemTime::now();
    
    let result = catch_unwind(|| {
        rayon::iter::repeat(())
            .progress_with(progress_bar)
            .panic_fuse()
            .find_map_any(|_| {
                // Check the timeout condition and panic if it's met. This is necessary
                // because Rayon doesn't include functionality to cancel parallel iterators
                // or thread pools manually.
                if timeout_secs > 0 && SystemTime::now().duration_since(start_time).unwrap() > timeout {
                    panic!("{}", RAYON_EARLY_EXIT_PAYLOAD);
                }
                f()
            })
    });

    std::panic::set_hook(default_panic_handler);
    result.ok().flatten()
}
