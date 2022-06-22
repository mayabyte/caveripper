use itertools::Itertools;
use rand::{Rng, SeedableRng, rngs::SmallRng};
use regex::Regex;
use lazy_static::lazy_static;
use rayon::prelude::*;
use std::process::Command;

use crate::{caveinfo::force_load_all, layout::boxes_overlap};
use crate::caveinfo::ALL_SUBLEVELS_MAP;
use crate::layout::Layout;

#[test]
fn test_collision() {
    assert!(!boxes_overlap(0, 0, 5, 7, 5, 5, 5, 5))
}

#[test]
fn test_slugs() {
    let num_layouts = 100;
    let mut rng: SmallRng = SeedableRng::seed_from_u64(0x12345678);
    force_load_all();

    let tests: Vec<(u32, String)> = (0..num_layouts).into_iter()
        .map(|_| {
            let seed = rng.gen();
            let sublevel = ALL_SUBLEVELS_MAP.keys()
                .sorted()
                .nth(rng.gen_range(0..ALL_SUBLEVELS_MAP.len()))
                .unwrap()
                .to_owned();
            (seed, sublevel)
        })
        .collect();

    let results: Vec<(u32, String, bool, String, String)> = tests.into_par_iter()
        .map(|(seed, sublevel)| {
            let caveripper_slug: String = Layout::generate(seed, ALL_SUBLEVELS_MAP[&sublevel]).slug();

            let jhawk_cavegen_slug: String = Command::new("java")
                .arg("-jar")
                .arg("CaveGen.jar")
                .arg("cave")
                .arg(normalize_sublevel(sublevel.as_str()).unwrap_or_else(|| sublevel.to_string()))
                .arg("-seed")
                .arg(format!("{:#010X}", seed))
                .arg("-noImages")
                .current_dir("./CaveGen/")
                .output()
                .map(|output| String::from_utf8(output.stdout).unwrap())
                .expect("Failed to invoke CaveGen in test")
                .trim()
                .to_string();

            (seed, sublevel, caveripper_slug == jhawk_cavegen_slug, caveripper_slug, jhawk_cavegen_slug)
        })
        .collect();

    let accuracy = (results.iter().filter(|(_, _, accurate, _, _)| *accurate).count() as f32) / (results.len() as f32);
    if accuracy < 1.0 {
        let num_samples = 5;
        let inaccurate_samples = results.iter()
            .filter(|(_, _, accurate, _, _)| !*accurate)
            .take(num_samples);
        for (seed, sublevel, _, caveripper_slug, jhawk_cavegen_slug) in inaccurate_samples {
            println!("Broken sublevel: {} {:#010X}.\nCaveripper: {}\nJhawk's Cavegen: {}.", sublevel, seed, caveripper_slug, jhawk_cavegen_slug);
        }
    }
    println!("Caveripper Accuracy: {:.03}%", accuracy * 100.0);

    assert!(accuracy == 1.0, "Accuracy: {:.03}.", accuracy * 100.0);
}

lazy_static! {
    static ref SUBLEVEL_ID_RE: Regex = Regex::new(r"([[:alpha:]]{2,5})[_-]?(\d+)").unwrap();
    static ref CAVES: [&'static str; 42] = [
        "EC", "SCx", "FC", "HoB", "WFG", "SH", "BK", "CoS", "GK", "SC", "SmC", "SR", "CoC", "DD", "HoH",
        "AT", "IM", "AD", "GD", "FT", "WF", "GdD", "AS", "SS", "CK", "PoW", "PoM", "EA", "DD",
        "PP", "BG", "SK", "CwNN", "SnD", "CH", "RH", "SA", "AA", "TC", "ER", "CG", "SD"
    ];
}

/// Transforms a non-strict sublevel specifier (e.g. scx1) into the format Cavegen
/// expects (SCx-1).
fn normalize_sublevel(raw: &str) -> Option<String> {
    let captures = SUBLEVEL_ID_RE.captures(raw)?;
    let cave_name = captures.get(1)?.as_str();
    let sublevel = captures.get(2)?.as_str();

    let cave_name_normalized = *CAVES
        .iter()
        .find(|cave| cave_name.eq_ignore_ascii_case(cave))?;

    // Handle alts
    let cave_name_normalized = match cave_name_normalized {
        "SmC" => "SC",
        _ => cave_name_normalized,
    };

    Some(format!("{}-{}", cave_name_normalized, sublevel))
}