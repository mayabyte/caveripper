mod cli;

use cli::*;
use clap::Parser;
use crossbeam::channel::bounded;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use rand::prelude::*;
use rayon::{self, iter::{IntoParallelIterator, ParallelIterator}};
use std::error::Error;
use std::time::{SystemTime, Duration};
use caveripper::{assets::ASSETS, layout::{render::{save_image, RenderOptions, render_caveinfo}}};
use caveripper::layout::Layout;
use caveripper::layout::render::render_layout;
use simple_logger::SimpleLogger;

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
            let filename = save_image(
                &render_layout(&layout, &render_options)?,
                format!("{}_{:#010X}", layout.cave_name, layout.starting_seed)
            )?;
            println!("🍞 Saved layout image as \"{}\"", filename);
        },
        Commands::Caveinfo{ sublevel, text } => {
            let caveinfo = ASSETS.get_caveinfo(&sublevel)?;
            if text {
                println!("{}", caveinfo);
            }
            else {
                let filename = save_image(
                    &render_caveinfo(&caveinfo, RenderOptions::default())?,
                    format!("{}_Caveinfo", caveinfo.name())
                )?;
                println!("🍞 Saved caveinfo image as \"{}\"", filename);
            }
        },
        Commands::Search{ sublevel, query, timeout, quiet, num, render, render_options } => {
            let caveinfo = ASSETS.get_caveinfo(&sublevel)?;
            
            let results = parallel_search_with_timeout(
                || {
                    let seed: u32 = random();
                    let layout = Layout::generate(seed, &caveinfo);
                    query.matches(&layout).then_some(seed)
                }, 
                timeout,
                num
            );
            
            if results.len() > 0 {
                if !quiet {
                    print!("🍞 Found matching seed(s):");
                    for seed in results.iter() {
                        print!(" {:#010X}", seed);
                    }
                    println!();
                }
                else {
                    for seed in results.iter() {
                        println!("{:#010X}", seed);
                    }
                }

                if render {
                    for seed in results.iter() {
                        let layout = Layout::generate(*seed, &caveinfo);
                        let filename = save_image(
                            &render_layout(&layout, &render_options)?,
                            format!("{}_{:#010X}", layout.cave_name, layout.starting_seed)
                        )?;
                        if !quiet {
                            println!("🍞 Saved layout image as \"{}\"", filename);
                        }
                    }
                }
            }
            else {
                println!("🍞 Couldn't find a layout matching the condition '{}' in {}s.", query, timeout);
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
                "🍞 Searched {} layouts and found {} ({:.03}%) that match the condition '{}'.", 
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
fn parallel_search_with_timeout<T, F>(f: F, timeout_secs: u64, num: usize) -> Vec<T> 
where F: Fn() -> Option<T> + Sync + Send,
      T: Send
{
    let timeout = Duration::from_secs(timeout_secs);
    let progress_bar = ProgressBar::new_spinner()
        .with_style(ProgressStyle::default_spinner().template("{spinner} {elapsed_precise} [{per_sec}, {pos} searched]"));
    progress_bar.enable_steady_tick(100);
    let start_time = SystemTime::now();

    let (sender, results) = bounded(num);
    
    rayon::iter::repeat(())
        .progress_with(progress_bar)
        .map(|_| {
            if timeout_secs > 0 && SystemTime::now().duration_since(start_time).unwrap() > timeout {
                return None;
            }

            if sender.is_full() {
                return None;
            }

            if let Some(result) = f() {
                return sender.try_send(result).ok();
            }

            Some(())
        })
        .while_some()
        .collect::<Vec<()>>();

    results.iter().take(num).collect()
}
