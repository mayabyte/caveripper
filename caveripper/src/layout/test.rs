use itertools::Itertools;
use rand::{Rng, SeedableRng, rngs::SmallRng};
use rayon::prelude::*;
use std::process::Command;

use crate::assets::ASSETS;
use crate::layout::boxes_overlap;
use crate::layout::Layout;
use crate::sublevel::Sublevel;

use super::render::RenderOptions;
use super::render::render_layout;

#[test]
fn test_collision() {
    assert!(!boxes_overlap(0, 0, 5, 7, 5, 5, 5, 5))
}

#[test]
fn test_slugs() {
    let num_layouts = 100;
    let mut rng: SmallRng = SeedableRng::seed_from_u64(0x12345678);
    ASSETS.preload_vanilla_caveinfo()
        .expect("Failed to load caveinfo!");
    let all_sublevels = ASSETS.all_sublevels();

    let tests: Vec<(u32, Sublevel)> = (0..num_layouts).into_iter()
        .map(|_| {
            let seed = rng.gen();
            let sublevel = all_sublevels.iter()
                .map(|e| e.key().clone())
                .sorted()
                .nth(rng.gen_range(0..all_sublevels.len()))
                .unwrap();
            (seed, sublevel)
        })
        .collect();

    let results: Vec<(u32, Sublevel, bool, String, String)> = tests.into_par_iter()
        .map(|(seed, sublevel)| {
            let caveripper_slug: String = Layout::generate(seed, all_sublevels.get(&sublevel).unwrap().value()).slug();

            let jhawk_cavegen_slug: String = Command::new("java")
                .arg("-jar")
                .arg("CaveGen.jar")
                .arg("cave")
                .arg(sublevel.normalized_name())
                .arg("-seed")
                .arg(format!("{:#010X}", seed))
                .arg("-noImages")
                .current_dir("../CaveGen/")
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
            println!("Broken sublevel: {} {:#010X}.\nCaveripper: {}\nJhawk's Cavegen: {}.", sublevel.short_name(), seed, caveripper_slug, jhawk_cavegen_slug);
        }
    }
    println!("Caveripper Accuracy: {:.03}%", accuracy * 100.0);

    assert!(accuracy == 1.0, "Accuracy: {:.03}.", accuracy * 100.0);
}

#[test]
fn test_render() {
    let num_layouts = 1_000;
    let mut rng: SmallRng = SeedableRng::seed_from_u64(0x12345678);
    ASSETS.preload_vanilla_caveinfo().expect("Failed to load caveinfo!");
    let all_sublevels = ASSETS.all_sublevels();

    let tests: Vec<(u32, Sublevel)> = (0..num_layouts).into_iter()
        .map(|_| {
            let seed = rng.gen();
            let sublevel = all_sublevels.iter()
                .map(|e| e.key().clone())
                .sorted()
                .nth(rng.gen_range(0..all_sublevels.len()))
                .unwrap();
            (seed, sublevel)
        })
        .collect();

    let failures = tests.into_par_iter().filter(|(seed, sublevel)| {
        let layout = Layout::generate(*seed, all_sublevels.get(sublevel).as_ref().unwrap());
        if let Err(cause) = render_layout(&layout, &RenderOptions::default()) {
            println!("({}, {:#010X}) {}", sublevel.short_name(), seed, cause);
            true
        }
        else {
            false
        }
    })
    .count();

    assert!(failures == 0);
}
