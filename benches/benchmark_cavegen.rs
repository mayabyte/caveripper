use criterion::{black_box, criterion_group, criterion_main, Criterion};
use cavegen::layout::Layout;
use cavegen::assets::ASSETS;
use itertools::Itertools;
use rand::{Rng, SeedableRng, rngs::SmallRng};

pub fn benchmark_layout_generation(c: &mut Criterion) {
    let mut rng: SmallRng = SeedableRng::seed_from_u64(0x12345678);
    ASSETS.preload_vanilla_caveinfo()
        .expect("Failed to load caveinfo!");
    let all_sublevels = ASSETS.all_sublevels().into_iter().collect_vec();

    c.bench_function("layout generation (reference)", |b| {
        b.iter(|| {
            let seed = rng.gen();
            let caveinfo = &all_sublevels[rng.gen_range(0..all_sublevels.len())].1;
            black_box(Layout::generate(seed, &caveinfo));
        })
    });
}


criterion_group!(benches, benchmark_layout_generation);
criterion_main!(benches);
