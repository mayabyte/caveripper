mod cli;

use cli::*;
use clap::Parser;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use rand::prelude::*;
use rayon::{self, iter::{IntoParallelIterator, ParallelIterator}};
use std::{error::Error, panic::RefUnwindSafe};
use std::panic::catch_unwind;
use std::time::{SystemTime, Duration};
use cavegen::assets::ASSETS;
use cavegen::layout::Layout;
use cavegen::layout::render::render_layout;
use simple_logger::SimpleLogger;

static RAYON_EARLY_EXIT_PAYLOAD: &'static str = "__RAYON_EARLY_EXIT__";

fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();
    if args.debug_logging {
        SimpleLogger::new().with_level(log::LevelFilter::max()).init()?;
    }

    // Register a custom pass-through panic handler that suppresses the panic
    // message normally produced by the Rayon early exit hack, and forwards to
    // the default panic handler otherwise.
    let default_panic_handler = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        if let Some(msg) = panic_info.payload().downcast_ref::<&str>() {
            if *msg != RAYON_EARLY_EXIT_PAYLOAD {
                default_panic_handler(panic_info);
            }
        }
    }));

    // Run the desired command.
    match args.subcommand {
        Commands::Generate{ sublevel, seed } => {
            let caveinfo = ASSETS.get_caveinfo(&sublevel)?;
            let layout = Layout::generate(seed, &caveinfo);
            render_layout(&layout);
        },
        Commands::Search{ sublevel, condition, timeout } => {
            let caveinfo = ASSETS.get_caveinfo(&sublevel)?;

            let result = parallel_search_with_timeout(|| {
                let seed: u32 = random();
                let layout = Layout::generate(seed, &caveinfo);
                condition.matches(&layout).then_some(seed)
            }, timeout);
            
            if let Some(seed) = result {
                println!("🍞 Found matching seed: {:#10X}.", seed);
            }
            else {
                println!("🍞 Couldn't find a layout matching the condition '{}' in {}s.", condition, timeout);
            }
        },
        Commands::Stats { sublevel, condition, num_to_search } => {
            let caveinfo = ASSETS.get_caveinfo(&sublevel)?;
            let num_matched = (0..num_to_search).into_par_iter()
                .progress()
                .filter(|_| {
                    let seed: u32 = random();
                    let layout = Layout::generate(seed, &caveinfo);
                    condition.matches(&layout)
                })
                .count();
            println!(
                "🍞 Searched {} layouts and found {} ({:.03}%) that match the condition '{}'.", 
                num_to_search, num_matched, (num_matched as f32 / num_to_search as f32) * 100.0, &condition
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
                if SystemTime::now().duration_since(start_time).unwrap() > timeout {
                    panic!("{}", RAYON_EARLY_EXIT_PAYLOAD);
                }
                f()
            })
    });
    result.ok().flatten()
}
