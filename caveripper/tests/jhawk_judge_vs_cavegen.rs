//! Compares the jhawk_judge port against scores produced by JHawk's CaveGen.
//!
//! `data/cavegen_judge.txt.gz` was made by running CaveGen
//! (`java -jar CaveGen.jar none <story|cmal> -num 100 -seed 0 -noImages -judge <type>`)
//! for each judge type and collecting its "Judge:" report lines. Each line is
//! `<type> <sublevel> <seed> <score>`, with the score rounded to 2 decimals.
//!
//! Needs the vanilla game's assets extracted with `caveripper extract`. Run with
//! `cargo test --release -p caveripper --test jhawk_judge_vs_cavegen -- --ignored`.

use std::{collections::BTreeMap, fs::File, io::Read, sync::Mutex};

use caveripper::{
    assets::{AssetManager, fs_asset_manager::FsAssetManager},
    jhawk_judge::{JhawkJudge, JudgeType},
    layout::Layout,
    sublevel::Sublevel,
};
use flate2::read::GzDecoder;
use rayon::prelude::*;

const EXPECTED_GZ: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/cavegen_judge.txt.gz");

/// CaveGen prints 2 decimals; allow a little more for f32 rounding differences.
const TOLERANCE: f64 = 0.015;

#[test]
#[ignore = "needs extracted vanilla game assets; run with --ignored"]
fn jhawk_judge_matches_cavegen() {
    let mgr = FsAssetManager::init().expect("Couldn't load assets. Extract the game with `caveripper extract` first.");

    let mut text = String::new();
    GzDecoder::new(File::open(EXPECTED_GZ).unwrap()).read_to_string(&mut text).unwrap();

    // (type, sublevel) -> [(seed, score)]
    let mut cases: BTreeMap<(String, String), Vec<(u32, f64)>> = BTreeMap::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        cases
            .entry((f[0].to_string(), f[1].to_string()))
            .or_default()
            .push((u32::from_str_radix(f[2], 16).unwrap(), f[3].parse().unwrap()));
    }
    let total: usize = cases.values().map(|v| v.len()).sum();
    assert!(total > 0);

    let failures: Mutex<Vec<String>> = Mutex::new(Vec::new());
    cases.par_iter().for_each(|((judge_type, sublevel), seeds)| {
        let judge_type: JudgeType = judge_type.parse().unwrap();
        let sublevel = Sublevel::try_from_str(sublevel, &mgr).unwrap();
        let caveinfo = mgr.load_caveinfo(&sublevel).unwrap();
        let judge = JhawkJudge::new(judge_type, caveinfo);
        for &(seed, expected) in seeds {
            let layout = Layout::generate(seed, caveinfo);
            let actual = judge.judge(&layout);
            if !((actual - expected).abs() <= TOLERANCE) {
                failures.lock().unwrap().push(format!(
                    "{judge_type} {} {seed:08X}: CaveGen {expected:.2}, Caveripper {actual:.2}",
                    sublevel.normalized_name()
                ));
            }
        }
    });

    let mut failures = failures.into_inner().unwrap();
    if failures.is_empty() {
        return;
    }
    failures.sort();
    let mut per_case: BTreeMap<String, usize> = BTreeMap::new();
    for f in failures.iter() {
        let mut it = f.split_whitespace();
        *per_case
            .entry(format!("{} {}", it.next().unwrap(), it.next().unwrap()))
            .or_default() += 1;
    }
    let summary: Vec<String> = per_case.iter().map(|(k, v)| format!("{k}:{v}")).collect();
    panic!(
        "{} of {total} judge scores differ from CaveGen.\nPer case: {}\n{}",
        failures.len(),
        summary.join(", "),
        failures.iter().take(40).cloned().collect::<Vec<_>>().join("\n")
    );
}
