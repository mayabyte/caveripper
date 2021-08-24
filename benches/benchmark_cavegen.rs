use criterion::{black_box, criterion_group, criterion_main, Criterion};
use cavegen::layout::Layout;
use cavegen::caveinfo::{get_sublevel_info, gamedata::ALL_SUBLEVELS_POD};
use rand::{Rng, SeedableRng, rngs::SmallRng};

pub fn benchmark_layout_generation(c: &mut Criterion) {
    let mut rng: SmallRng = SeedableRng::seed_from_u64(0x12345678);
    let all_caveinfo: Vec<_> = ALL_SUBLEVELS_POD.iter()
        .map(|sublevel| get_sublevel_info(sublevel).unwrap())
        .collect();

    c.bench_function("layout generation (reference)", |b| {
        b.iter(|| {
            let seed = rng.gen();
            let caveinfo = all_caveinfo[rng.gen_range(0..all_caveinfo.len())].clone();
            black_box(Layout::generate(seed, caveinfo));
        })
    });
}


criterion_group!(benches, benchmark_layout_generation);
criterion_main!(benches);
