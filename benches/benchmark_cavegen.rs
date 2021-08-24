use criterion::{black_box, criterion_group, criterion_main, Criterion};
use cavegen::layout::Layout;
use cavegen::caveinfo::ALL_SUBLEVELS;
use rand::{Rng, SeedableRng, rngs::SmallRng};

pub fn benchmark_layout_generation(c: &mut Criterion) {
    let mut rng: SmallRng = SeedableRng::seed_from_u64(0x12345678);

    c.bench_function("layout generation (reference)", |b| {
        b.iter(|| {
            let seed = rng.gen();
            let caveinfo = ALL_SUBLEVELS[rng.gen_range(0..ALL_SUBLEVELS.len())];
            black_box(Layout::generate(seed, caveinfo));
        })
    });
}


criterion_group!(benches, benchmark_layout_generation);
criterion_main!(benches);
