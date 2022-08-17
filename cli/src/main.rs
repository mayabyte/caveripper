mod cli;

use atty::Stream;
use cli::*;
use clap::Parser;
use crossbeam::channel::bounded;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use log::info;
use rand::prelude::*;
use rayon::{self, iter::{IntoParallelIterator, ParallelIterator}};
use std::{error::Error, thread::{scope, available_parallelism}, num::NonZeroUsize, fs::read_to_string, io::stdin, time::{Instant, Duration}};
use caveripper::{assets::ASSETS, layout::{Layout, render::{render_layout, save_image, RenderOptions, render_caveinfo}}};
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
        Commands::Generate { sublevel, seed, render_options } => {
            let caveinfo = ASSETS.get_caveinfo(&sublevel)?;
            let layout = Layout::generate(seed, &caveinfo);
            let filename = save_image(
                &render_layout(&layout, &render_options)?,
                format!("{}_{:#010X}", layout.cave_name, layout.starting_seed)
            )?;
            println!("🍞 Saved layout image as \"{}\"", filename);
        },
        Commands::Caveinfo { sublevel, text } => {
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
        Commands::Search { sublevel, query, timeout_s, num, render, render_options } => {
            let caveinfo = ASSETS.get_caveinfo(&sublevel)?;

            let progress_bar = ProgressBar::new_spinner()
                .with_style(ProgressStyle::default_spinner().template("{spinner} {elapsed_precise} [{per_sec}, {pos} searched]").unwrap());
            progress_bar.enable_steady_tick(Duration::from_secs(2));

            if !atty::is(Stream::Stdout) {
                progress_bar.finish_and_clear();
            }

            let timeout = Duration::from_secs(timeout_s);
            let start_time = Instant::now();

            let parallelism = available_parallelism().unwrap_or(NonZeroUsize::new(8).unwrap()).into();
            info!("Searching with {} threads", parallelism);

            let (sender, results) = bounded(num);
            let (finished_s, finished_r) = bounded::<()>(1);

            scope(|s| -> Result<(), Box<dyn Error>> {
                for _ in 0..parallelism {
                    s.spawn(|| loop {
                        if finished_r.len() > 0 {
                            return;
                        }

                        let seed: u32 = random();
                        let layout = Layout::generate(seed, &caveinfo);
    
                        if query.matches(&layout) {
                            if let Err(_) = sender.try_send(seed) {
                                return;
                            }
                        }

                        progress_bar.inc(1);
                    });
                }

                let mut received = 0;
                while received < num && let Ok(seed) = results.recv_deadline(start_time + timeout) {
                    received += 1;
                    progress_bar.suspend(|| println!("{:#010X}", seed));
                    
                    if render {
                        let layout = Layout::generate(seed, &caveinfo);
                        let filename = save_image(
                            &render_layout(&layout, &render_options)?,
                            format!("{}_{:#010X}", layout.cave_name, layout.starting_seed)
                        )?;
                        if atty::is(Stream::Stdout) {
                            eprintln!("🍞 Saved layout image as \"{}\"", filename);
                        }
                    }
                }

                progress_bar.finish_and_clear();
                finished_s.send(())?;

                if received == 0 {
                    eprintln!("🍞 No matching layouts found.");
                }
                else if atty::is(Stream::Stdout) {
                    eprintln!("🍞 Found {} matching seeds in {}s.", received, start_time.elapsed().as_secs());
                }

                Ok(())
            })?;
        },
        Commands::Stats { sublevel, query, num_to_search } => {
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
        },
        Commands::Filter { sublevel, query, file } => {
            let caveinfo = ASSETS.get_caveinfo(&sublevel)?;

            // Read from a file. In this case, we can check the seeds in parallel.
            if let Some(filename) = file {
                read_to_string(filename)?.lines()
                    .collect::<Vec<_>>()
                    .into_par_iter()
                    .filter_map(|line| parse_seed(line).ok())    
                    .filter(|seed| {
                        let layout = Layout::generate(*seed, &caveinfo);
                        query.matches(&layout)
                    })
                    .for_each(|seed| {
                        println!("{:#010X}", seed);
                    });
            }
            // Read from stdin and print as results become ready
            else {
                stdin().lines()
                    .filter_map(|line| parse_seed(&line.ok()?).ok())
                    .filter(|seed| {
                        let layout = Layout::generate(*seed, &caveinfo);
                        query.matches(&layout)
                    })
                    .for_each(|seed| {
                        println!("{:#010X}", seed);
                    });
            }
        }
    }

    Ok(())
}
