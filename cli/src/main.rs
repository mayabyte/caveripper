mod cli;
mod extract;

use atty::Stream;
use cli::*;
use clap::Parser;
use error_stack::Result;
use extract::extract_iso;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use rand::prelude::*;
use rayon::{self, iter::{IntoParallelIterator, ParallelIterator}};
use std::{fs::read_to_string, io::stdin, time::{Instant, Duration}};
use caveripper::{
    assets::AssetManager,
    layout::Layout,
    render::{
        render_layout,
        save_image,
        render_caveinfo
    },
    query::find_matching_layouts_parallel,
    parse_seed, errors::CaveripperError
};
use simple_logger::SimpleLogger;

fn main() -> Result<(), CaveripperError> {
    // The asset manager has to be initialized as the very first thing because
    // command parsing can involve sublevel string parsing, which requires
    // loading assets.
    AssetManager::init_global("assets", ".")?;

    let args = Cli::parse();
    match args.verbosity {
        0 => SimpleLogger::new().with_level(log::LevelFilter::Warn).init().unwrap(),
        1 => SimpleLogger::new().with_level(log::LevelFilter::Info).init().unwrap(),
        2.. => SimpleLogger::new().with_level(log::LevelFilter::max()).init().unwrap(),
    }

    // Run the desired command.
    match args.subcommand {
        Commands::Generate { sublevel, seed, render_options } => {
            let caveinfo = AssetManager::get_caveinfo(&sublevel)?;
            let layout = Layout::generate(seed, caveinfo);
            let _ = std::fs::create_dir("output");
            save_image(
                &render_layout(&layout, render_options)?,
                format!("output/{}_{:#010X}.png", layout.cave_name, layout.starting_seed)
            )?;
            println!("🍞 Saved layout image as \"output/{}_{:#010X}.png\"", layout.cave_name, layout.starting_seed);
        },
        Commands::Caveinfo { sublevel, text, render_options } => {
            let caveinfo = AssetManager::get_caveinfo(&sublevel)?;
            if text {
                println!("{caveinfo}");
            }
            else {
                let _ = std::fs::create_dir("output");
                save_image(
                    &render_caveinfo(caveinfo, render_options)?,
                    format!("output/{}_Caveinfo.png", caveinfo.name())
                )?;
                println!("🍞 Saved caveinfo image as \"{}_Caveinfo.png\"", caveinfo.name());
            }
        },
        Commands::Search { query, timeout_s, num, start_from } => {
            let start_time = Instant::now();
            let timeout = if timeout_s > 0 { Some(Duration::from_secs(timeout_s)) } else { None };
            let deadline = timeout.map(|t| Instant::now() + t);

            let progress_bar = ProgressBar::new_spinner()
                .with_style(ProgressStyle::default_spinner().template("{spinner} {elapsed_precise} [{per_sec}, {pos} searched]").unwrap());

            if !atty::is(Stream::Stdout) {
                progress_bar.finish_and_clear();
            }

            let result_recv = find_matching_layouts_parallel(
                &query,
                deadline,
                (num > 0).then_some(num),
                start_from,
                Some(&progress_bar)
            );

            let mut num_found = 0;
            for seed in result_recv.iter().take(num) {
                num_found += 1;
                progress_bar.suspend(|| println!("{seed:#010X}"));
            }

            progress_bar.finish_and_clear();
            if atty::is(Stream::Stdout) {
                eprintln!("🍞 Found {} matching seed(s) in {:0.3}s.", num_found, start_time.elapsed().as_secs_f32());
            }
        },
        Commands::Stats { query, num_to_search } => {
            let num_matched = (0..num_to_search).into_par_iter()
                .progress()
                .filter(|_| {
                    let seed: u32 = random();
                    query.matches(seed)
                })
                .count();
            println!(
                "🍞 Searched {} layouts and found {} ({:.03}%) that match the condition '{}'.",
                num_to_search, num_matched, (num_matched as f32 / num_to_search as f32) * 100.0, &query
            );
        },
        Commands::Filter { query, file } => {
            // Read from a file. In this case, we can check the seeds in parallel.
            if let Some(filename) = file {
                read_to_string(filename).unwrap().lines()
                    .collect::<Vec<_>>()
                    .into_par_iter()
                    .filter_map(|line| parse_seed(line).ok())
                    .filter(|seed| {
                        query.matches(*seed)
                    })
                    .for_each(|seed| {
                        println!("{seed:#010X}");
                    });
            }
            // Read from stdin and print as results become ready
            else {
                stdin().lines()
                    .filter_map(|line| parse_seed(&line.ok()?).ok())
                    .filter(|seed| {
                        query.matches(*seed)
                    })
                    .for_each(|seed| {
                        println!("{seed:#010X}");
                    });
            }
        },
        Commands::Extract { iso_path, game_name } => {
            let progress_bar = ProgressBar::new_spinner()
                .with_style(ProgressStyle::default_spinner().template("{spinner} {msg}").unwrap());
            extract_iso(game_name, iso_path, &progress_bar).expect("Failed to extract ISO");
            progress_bar.finish_and_clear();
            println!("🍞 Done extracting ISO.");
        },
    }

    Ok(())
}
