use rand::{Rng, SeedableRng, rngs::SmallRng};
use itertools::Itertools;
use rayon::prelude::{IntoParallelIterator, ParallelIterator};
use crate::{
    assets::AssetManager, 
    sublevel::Sublevel, 
    layout::Layout, 
    render::*
};

#[test]
fn test_render_layouts() {
    AssetManager::init_global("../assets", "..").unwrap();

    let num_layouts = 1_000;
    let mut rng: SmallRng = SeedableRng::seed_from_u64(0x12345678);
    AssetManager::preload_all_caveinfo().expect("Failed to load caveinfo!");
    let all_sublevels = AssetManager::all_sublevels().expect("Failed to get all sublevel caveinfos");

    let tests: Vec<(u32, Sublevel)> = (0..num_layouts).into_iter()
        .map(|_| {
            let seed = rng.gen();
            let sublevel = all_sublevels.iter()
                .map(|e| e.0.clone())
                .sorted()
                .nth(rng.gen_range(0..all_sublevels.len()))
                .unwrap();
            (seed, sublevel)
        })
        .collect();

    let failures = tests.into_par_iter().filter(|(seed, sublevel)| {
        let layout = Layout::generate(*seed, all_sublevels.get(sublevel).unwrap());
        if let Err(cause) = render_layout(&layout, LayoutRenderOptions::default()) {
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

#[test]
fn test_render_caveinfo() {
    AssetManager::init_global("../assets", "..").unwrap();
    AssetManager::preload_all_caveinfo().expect("Failed to load caveinfo!");
    let all_sublevels = AssetManager::all_sublevels().expect("Failed to get all sublevel caveinfos");

    all_sublevels.into_par_iter().panic_fuse().for_each(|(sublevel, caveinfo)| {
        println!("{}", sublevel.long_name());
        render_caveinfo(&caveinfo, CaveinfoRenderOptions::default()).unwrap();
    });
}
