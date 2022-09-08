use criterion::{black_box, criterion_group, criterion_main, Criterion};
use itertools::Itertools;
use rand::{Rng, SeedableRng, rngs::SmallRng};
use caveripper::{
    layout::{Layout, render::{render_layout, LayoutRenderOptions}},
    assets::AssetManager,
};

pub fn benchmark_layout_generation(c: &mut Criterion) {
    AssetManager::init_global("assets", ".").unwrap();
    let mut rng: SmallRng = SeedableRng::seed_from_u64(0x12345678);
    AssetManager::preload_all_caveinfo()
        .expect("Failed to load caveinfo!");
    let manager = AssetManager::all_sublevels().unwrap();
    let all_sublevels = manager.iter().collect_vec();

    c.bench_function("layout generation (reference)", |b| {
        b.iter(|| {
            let seed = rng.gen();
            let caveinfo = &all_sublevels[rng.gen_range(0..all_sublevels.len())].1;
            black_box(Layout::generate(seed, caveinfo));
        })
    });
}

pub fn benchmark_layout_rendering(c: &mut Criterion) {
    AssetManager::init_global("assets", ".").unwrap();
    let mut rng: SmallRng = SeedableRng::seed_from_u64(0x12345678);
    AssetManager::preload_all_caveinfo()
        .expect("Failed to load caveinfo!");
    let manager = AssetManager::all_sublevels().unwrap();
    let all_sublevels = manager.iter().collect_vec();

    c.bench_function("layout generation + rendering", |b| {
        b.iter(|| {
            let seed = rng.gen();
            let caveinfo = &all_sublevels[rng.gen_range(0..all_sublevels.len())].1;
            let layout = Layout::generate(seed, caveinfo);
            black_box(render_layout(&layout, LayoutRenderOptions::default()))
        })
    });
}


criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(1000);
    targets = benchmark_layout_generation, benchmark_layout_rendering
);
criterion_main!(benches);
