//! Expectation tests against JHawk's CaveGen.
//!
//! `data/cavegen_expected_output.txt.gz` is CaveGen's `files/expected_output.txt`
//! (gzipped), produced by `java -jar CaveGen.jar -expectTest`. It covers seeds
//! 0-99 for every story mode and challenge mode sublevel in the vanilla game and
//! records, for each layout:
//!
//! ```text
//! Generating <cave> <floor> on seed <SEED>
//! Placed: <unit name> <rotation> <x>,<z>               (map units, in placement order)
//! Spawned: <teki name> <group>[f] <x>,<z>              (teki; 'f' marks a falling teki)
//! Spawned: <treasure name> 2 <x>,<z>                   (loose treasures)
//! Spawned: gate<health> 5g <x>,<z>                     (gates)
//! ```
//!
//! Map units are compared in order. Spawned objects are compared as a multiset
//! per layout, since Caveripper doesn't record the order objects were spawned in.
//! Numbers are compared as f32 values so that formatting differences between
//! Java's `Float.toString` and Rust don't matter.
//!
//! These tests need the vanilla game's assets extracted with `caveripper extract`.
//! Run them with `cargo test --release -p caveripper --test cavegen_expect -- --ignored`.
//! Set `CAVEGEN_EXPECTED` to a plain-text expected_output.txt to test against a
//! different file.

use std::{collections::BTreeMap, fs::File, io::Read, sync::Mutex};

use caveripper::{
    assets::{fs_asset_manager::FsAssetManager, AssetManager},
    layout::{Layout, SpawnObject},
    sublevel::Sublevel,
};
use flate2::read::GzDecoder;
use rayon::prelude::*;

const EXPECTED_GZ: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/cavegen_expected_output.txt.gz");

/// Maximum number of mismatched layouts to print in full.
const MAX_REPORTED: usize = 20;

struct ExpectedLayout {
    cave: String,
    floor: usize,
    seed: u32,
    lines: Vec<String>,
}

fn load_expected() -> Vec<ExpectedLayout> {
    let mut text = String::new();
    if let Ok(path) = std::env::var("CAVEGEN_EXPECTED") {
        File::open(path).unwrap().read_to_string(&mut text).unwrap();
    } else {
        GzDecoder::new(File::open(EXPECTED_GZ).unwrap()).read_to_string(&mut text).unwrap();
    }

    let mut layouts: Vec<ExpectedLayout> = Vec::new();
    for line in text.lines() {
        if let Some(header) = line.strip_prefix("Generating ") {
            // "<cave> <floor> on seed <SEED>"
            let parts: Vec<&str> = header.split_whitespace().collect();
            layouts.push(ExpectedLayout {
                cave: parts[0].to_string(),
                floor: parts[1].parse().unwrap(),
                seed: u32::from_str_radix(parts[4], 16).unwrap(),
                lines: Vec::new(),
            });
        } else if !line.is_empty() {
            layouts.last_mut().expect("line before first header").lines.push(line.to_string());
        }
    }
    layouts
}

/// Produces the same lines CaveGen's `Memo.outputSublevelForExpect` writes.
pub fn cavegen_expect_lines(layout: &Layout) -> Vec<String> {
    let mut lines = Vec::new();
    for unit in layout.map_units.iter() {
        lines.push(format!("Placed: {} {} {},{}", unit.unit.unit_folder_name, unit.unit.rotation, unit.x, unit.z));
    }

    let mut teki = Vec::new();
    let mut items = Vec::new();
    let mut gates = Vec::new();
    for (so, pos) in layout.get_spawn_objects() {
        let (x, z) = (pos[0], pos[2]);
        match so {
            SpawnObject::Teki(info, _) => {
                let fall = if info.spawn_method.is_some() { "f" } else { "" };
                teki.push(format!("Spawned: {} {}{} {:?},{:?}", info.internal_name, info.group, fall, x, z));
            }
            SpawnObject::CapTeki(info, num_spawned) => {
                let fall = if info.is_falling() { "f" } else { "" };
                for _ in 0..*num_spawned {
                    teki.push(format!("Spawned: {} {}{} {:?},{:?}", info.internal_name, info.group, fall, x, z));
                }
            }
            SpawnObject::Item(info) => items.push(format!("Spawned: {} 2 {:?},{:?}", info.internal_name, x, z)),
            SpawnObject::Gate(info, _) => gates.push(format!("Spawned: gate{:?} 5g {:?},{:?}", info.health, x, z)),
            SpawnObject::Hole(_) | SpawnObject::Geyser(_) | SpawnObject::Ship | SpawnObject::Onion(_) => {}
        }
    }
    lines.extend(teki);
    lines.extend(items);
    lines.extend(gates);
    lines
}

/// Rewrites every number in a line through f32 so Java and Rust float formatting compare equal.
fn canonicalize(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut token = String::new();
    let flush = |token: &mut String, out: &mut String| {
        // Split off a non-numeric prefix such as "gate" in "gate4000.0".
        let split = token.find(|c: char| c.is_ascii_digit() || c == '-').unwrap_or(token.len());
        let (prefix, num) = token.split_at(split);
        match num.parse::<f32>() {
            Ok(v) if !num.is_empty() && !num.ends_with('f') && !num.ends_with('g') => {
                out.push_str(prefix);
                out.push_str(&format!("{v:?}"));
            }
            _ => out.push_str(token),
        }
        token.clear();
    };
    for c in line.chars() {
        if c == ' ' || c == ',' {
            flush(&mut token, &mut out);
            out.push(c);
        } else {
            token.push(c);
        }
    }
    flush(&mut token, &mut out);
    out
}

/// Whether two positions are equal up to f32 rounding error.
fn close(a: (f32, f32), b: (f32, f32)) -> bool {
    const TOLERANCE: f32 = 0.01;
    (a.0 - b.0).abs() <= TOLERANCE && (a.1 - b.1).abs() <= TOLERANCE
}

/// Returns a description of the differences, or None if the layouts match.
fn compare(expected: &[String], actual: &[String]) -> Option<String> {
    // Spawned lines are grouped by "<name> <group>" with their positions.
    let split = |lines: &[String]| -> (Vec<String>, BTreeMap<String, Vec<(f32, f32)>>) {
        let mut placed = Vec::new();
        let mut spawned: BTreeMap<String, Vec<(f32, f32)>> = BTreeMap::new();
        for line in lines {
            // Caveripper's caveinfo parser renames the stationary "F" teki variants.
            let line = canonicalize(&line.replace("Spawned: FminiHoudai ", "Spawned: MiniHoudai ").replace("Spawned: Fkabuto ", "Spawned: Kabuto "));
            if line.starts_with("Placed:") {
                placed.push(line);
            } else {
                let (label, pos) = line.rsplit_once(' ').unwrap();
                let (x, z) = pos.split_once(',').unwrap();
                spawned.entry(label.to_string()).or_default().push((x.parse().unwrap(), z.parse().unwrap()));
            }
        }
        (placed, spawned)
    };
    let (exp_placed, exp_spawned) = split(expected);
    let (act_placed, act_spawned) = split(actual);

    let mut report = String::new();
    if exp_placed != act_placed {
        let i = exp_placed.iter().zip(act_placed.iter()).position(|(a, b)| a != b).unwrap_or(exp_placed.len().min(act_placed.len()));
        report.push_str(&format!(
            "  map units differ at #{i} (expected {}, got {})\n    expected: {}\n    got:      {}\n",
            exp_placed.len(),
            act_placed.len(),
            exp_placed.get(i).map_or("<none>", |s| s.as_str()),
            act_placed.get(i).map_or("<none>", |s| s.as_str()),
        ));
    }
    let mut keys: Vec<&String> = exp_spawned.keys().chain(act_spawned.keys()).collect();
    keys.sort();
    keys.dedup();
    for key in keys {
        let mut exp = exp_spawned.get(key).cloned().unwrap_or_default();
        let mut act = act_spawned.get(key).cloned().unwrap_or_default();
        // Pair up positions that agree to within float rounding. CaveGen does some of
        // its math in a different order, so the last digit or two can differ.
        exp.retain(|e| match act.iter().position(|a| close(*e, *a)) {
            Some(i) => {
                act.swap_remove(i);
                false
            }
            None => true,
        });
        for (x, z) in exp {
            report.push_str(&format!("    missing: {key} {x:?},{z:?}\n"));
        }
        for (x, z) in act {
            report.push_str(&format!("    extra:   {key} {x:?},{z:?}\n"));
        }
    }
    if report.is_empty() { None } else { Some(report) }
}

#[test]
#[ignore = "needs extracted vanilla game assets; run with --ignored"]
fn matches_cavegen_expected_output() {
    let mgr = FsAssetManager::init().expect("Couldn't load assets. Extract the game with `caveripper extract` first.");
    let expected = load_expected();
    assert!(!expected.is_empty());

    let failures: Mutex<Vec<(String, u32, String)>> = Mutex::new(Vec::new());
    let mut by_sublevel: BTreeMap<(String, usize), Vec<&ExpectedLayout>> = BTreeMap::new();
    for e in expected.iter() {
        by_sublevel.entry((e.cave.clone(), e.floor)).or_default().push(e);
    }

    by_sublevel.par_iter().for_each(|((cave, floor), layouts)| {
        let sublevel = Sublevel::try_from_str(&format!("{cave}-{floor}"), &mgr).unwrap_or_else(|e| panic!("{cave}-{floor}: {e}"));
        let caveinfo = mgr.load_caveinfo(&sublevel).unwrap();
        for e in layouts {
            let layout = Layout::generate(e.seed, caveinfo);
            if let Some(diff) = compare(&e.lines, &cavegen_expect_lines(&layout)) {
                failures.lock().unwrap().push((format!("{cave}-{floor}"), e.seed, diff));
            }
        }
    });

    let mut failures = failures.into_inner().unwrap();
    failures.sort_by(|a, b| (&a.0, a.1).cmp(&(&b.0, b.1)));
    if failures.is_empty() {
        return;
    }

    let mut per_sublevel: BTreeMap<&str, usize> = BTreeMap::new();
    for (name, _, _) in failures.iter() {
        *per_sublevel.entry(name).or_default() += 1;
    }
    let mut msg = format!("{} of {} layouts differ from CaveGen.\nPer sublevel:", failures.len(), expected.len());
    for (name, n) in per_sublevel.iter() {
        msg.push_str(&format!(" {name}:{n}"));
    }
    msg.push('\n');
    for (name, seed, diff) in failures.iter().take(MAX_REPORTED) {
        msg.push_str(&format!("{name} seed {seed:#010X}:\n{diff}"));
    }
    panic!("{msg}");
}
