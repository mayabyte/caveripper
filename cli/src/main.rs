mod cli;
mod extract;

use std::{
    fs::read_to_string,
    io::stdin,
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::anyhow;
use atty::Stream;
use caveripper::{
    assets::{fs_asset_manager::FsAssetManager, AssetManager},
    errors::CaveripperError,
    layout::Layout,
    parse_seed,
    pikmin_math::PikminRng,
    query::{find_matching_layouts_parallel, special::ConsecutiveIdenticalSeedsQuery, Query, StructuralQuery},
    render::{render_caveinfo, render_layout, save_image, RenderHelper},
    sublevel::Sublevel,
};
use clap::Parser;
use cli::*;
use error_stack::Result;
use extract::{bti::BtiImage, extract_iso, extract_szs};
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressIterator, ProgressStyle};
use rand::prelude::*;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use simple_logger::SimpleLogger;

fn main() -> Result<(), CaveripperError> {
    // The asset manager has to be initialized as the very first thing because
    // command parsing can involve sublevel string parsing, which requires
    // loading assets.
    let mgr = FsAssetManager::init()?;
    let helper = RenderHelper::new(&mgr);

    let args = Cli::parse();
    match args.verbosity {
        0 => SimpleLogger::new().with_level(log::LevelFilter::Warn).init().unwrap(),
        1 => SimpleLogger::new().with_level(log::LevelFilter::Info).init().unwrap(),
        2.. => SimpleLogger::new().with_level(log::LevelFilter::max()).init().unwrap(),
    }

    // Run the desired command.
    match args.subcommand {
        Commands::Generate {
            sublevel,
            seed,
            render_options,
        } => {
            let sublevel = Sublevel::try_from_str(&sublevel, &mgr)?;
            let caveinfo = mgr.load_caveinfo(&sublevel)?;
            let layout = Layout::generate(seed, caveinfo);
            let _ = std::fs::create_dir("output");
            save_image(
                &render_layout(&layout, &helper, render_options)?,
                format!("output/{}_{:#010X}.png", layout.cave_name, layout.starting_seed),
            )?;
            println!(
                "🍞 Saved layout image as \"output/{}_{:#010X}.png\"",
                layout.cave_name, layout.starting_seed
            );
        }
        Commands::Caveinfo {
            sublevel,
            text,
            render_options,
        } => {
            let sublevel = Sublevel::try_from_str(&sublevel, &mgr)?;
            let caveinfo = mgr.load_caveinfo(&sublevel)?;
            if text {
                println!("{caveinfo}");
            } else {
                let _ = std::fs::create_dir("output");
                save_image(
                    &render_caveinfo(caveinfo, &helper, render_options)?,
                    format!("output/{}_Caveinfo.png", caveinfo.name()),
                )?;
                println!("🍞 Saved caveinfo image as \"{}_Caveinfo.png\"", caveinfo.name());
            }
        }
        Commands::Search { query, timeout_s, num } => {
            let query = StructuralQuery::try_parse(&query, &mgr)?;
            let timeout = if timeout_s > 0 {
                Some(Duration::from_secs(timeout_s))
            } else {
                None
            };
            search(query, &mgr, timeout, num);
        }
        Commands::SearchSpecial { name, args } => {
            let query = match name.to_ascii_lowercase().as_str() {
                "consecutive_identical_seeds" => {
                    let (sublevel_arg, num_consecutive_arg) = args.split_once(' ').unwrap_or((&args, "2"));

                    ConsecutiveIdenticalSeedsQuery {
                        sublevel: Sublevel::try_from_str(&sublevel_arg, &mgr)?,
                        num_consecutive: num_consecutive_arg.parse().expect("Second argument must be a number"),
                    }
                }
                _ => {
                    eprintln!("Unrecognized special query!");
                    return Ok(());
                }
            };

            search(query, &mgr, None, 1);
        }
        Commands::SearchFrom { start_from, query, max } => {
            let query = StructuralQuery::try_parse(&query, &mgr)?;
            let rng = PikminRng::new(start_from);
            let progress_bar = ProgressBar::new(max as u64);

            rng.take(max)
                .enumerate()
                .progress_with(progress_bar.clone())
                .filter(|(_, seed)| query.matches(*seed, &mgr))
                .for_each(|(offset, seed)| {
                    progress_bar.suspend(|| println!("{seed:#010X}\tOffset: {} ({:#0X})", offset + 1, offset + 1));
                });
        }
        Commands::Stats { query, num_to_search } => {
            let query = StructuralQuery::try_parse(&query, &mgr)?;
            let num_matched = (0..num_to_search)
                .into_par_iter()
                .progress()
                .filter(|_| {
                    let seed: u32 = random();
                    query.matches(seed, &mgr)
                })
                .count();
            println!(
                "🍞 {num_matched} out of {num_to_search} ({:.03}%) match the condition '{query}'.",
                (num_matched as f32 / num_to_search as f32) * 100.0
            );
        }
        Commands::Filter { query, file } => {
            let query = StructuralQuery::try_parse(&query, &mgr)?;
            // Read from a file. In this case, we can check the seeds in parallel.
            if let Some(filename) = file {
                read_to_string(filename)
                    .unwrap()
                    .lines()
                    .collect::<Vec<_>>()
                    .into_par_iter()
                    .filter_map(|line| parse_seed(line).ok())
                    .filter(|seed| query.matches(*seed, &mgr))
                    .for_each(|seed| {
                        println!("{seed:#010X}");
                    });
            }
            // Read from stdin and print as results become ready
            else {
                stdin()
                    .lines()
                    .filter_map(|line| parse_seed(&line.ok()?).ok())
                    .filter(|seed| query.matches(*seed, &mgr))
                    .for_each(|seed| {
                        println!("{seed:#010X}");
                    });
            }
        }
        Commands::Extract {
            iso_path,
            game_name,
            out_dir,
        } => {
            let progress_bar = ProgressBar::new_spinner().with_style(ProgressStyle::default_spinner().template("{spinner} {msg}").unwrap());
            let output_directory = out_dir.unwrap_or_else(|| {
                let home_dir = dirs::home_dir()
                    .ok_or(anyhow!("Couldn't locate home directory!"))
                    .expect("Couldn't locate home directory!");
                let out_path = home_dir.join(".config/caveripper/assets");
                out_path.to_string_lossy().into_owned()
            });

            extract_iso(game_name, iso_path, &progress_bar, &output_directory).expect("Failed to extract ISO");
            progress_bar.finish_and_clear();
            println!("🍞 Done extracting ISO.");
        }
        Commands::ExtractSzs { file_path } => {
            let data = std::fs::read(&file_path).expect("Couldn't read provided file path!");
            let extracted = extract_szs(data).expect("Couldn't decompress file as SZS!");
            let filename = file_path.file_name().unwrap().to_string_lossy();
            let folder_name = filename.strip_suffix(".szs").unwrap_or(&filename);
            for (path, bytes) in extracted.into_iter() {
                let mut full_path = PathBuf::new();
                full_path.push(folder_name);
                full_path.push(path);
                std::fs::create_dir_all(&full_path.parent().unwrap_or(&full_path)).expect(&format!(
                    "Couldn't create subdirectory {} for extracted files!",
                    full_path.to_string_lossy()
                ));
                std::fs::write(full_path, bytes).expect("Failed to write file data!");
            }
        }
        Commands::ExtractBti { file_path } => {
            let data = std::fs::read(&file_path).expect("Couldn't read provided file path!");
            let bti = BtiImage::decode(&data);
            let dest_path = file_path.with_file_name(format!(
                "{}png",
                file_path.file_name().unwrap().to_string_lossy().strip_suffix("bti").unwrap(),
            ));
            image::save_buffer_with_format(
                dest_path,
                bti.pixels().flatten().cloned().collect::<Vec<_>>().as_slice(),
                bti.width as u32,
                bti.height as u32,
                image::ColorType::Rgba8,
                image::ImageFormat::Png,
            )
            .unwrap();
        }
    }

    Ok(())
}

fn search(query: impl Query + Send + Sync, mgr: &FsAssetManager, timeout: Option<Duration>, num: usize) {
    let start_time = Instant::now();
    let deadline = timeout.map(|t| Instant::now() + t);

    let progress_bar = ProgressBar::new_spinner().with_style(
        ProgressStyle::default_spinner()
            .template("{spinner} {elapsed_precise} [{per_sec}, {pos} searched]")
            .unwrap(),
    );

    if !atty::is(Stream::Stdout) {
        progress_bar.finish_and_clear();
    }

    find_matching_layouts_parallel(
        &query,
        mgr,
        deadline,
        (num > 0).then_some(num),
        Some(|| {
            progress_bar.inc(1);
        }),
        |seed| {
            progress_bar.suspend(|| println!("{seed:#010X}"));
        },
    );

    progress_bar.finish_and_clear();
    if atty::is(Stream::Stdout) {
        eprintln!("🍞 Finished in {:0.3}s.", start_time.elapsed().as_secs_f32());
    }
}
