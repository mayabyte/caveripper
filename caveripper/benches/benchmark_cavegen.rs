use criterion::{black_box, criterion_group, criterion_main, Criterion};
use itertools::Itertools;
use rand::{Rng, SeedableRng, rngs::SmallRng};
use caveripper::{
    layout::Layout,
    assets::AssetManager,
};

pub fn benchmark_layout_generation(c: &mut Criterion) {
    AssetManager::init("assets", ".");
    let mut rng: SmallRng = SeedableRng::seed_from_u64(0x12345678);
    AssetManager::preload_vanilla_caveinfo()
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


criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(10000);
    targets = benchmark_layout_generation
);
criterion_main!(benches);
