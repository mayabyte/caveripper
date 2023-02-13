use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::{Rng, SeedableRng, rngs::SmallRng};
use caveripper::{
    layout::Layout,
    render::{Renderer, LayoutRenderOptions},
    assets::AssetManager, caveinfo::CaveInfo,
};

fn preload_caveinfo(mgr: &AssetManager) -> Vec<CaveInfo> {
    let mut caveinfo = Vec::new();
    for cfg in mgr.cave_cfg.iter() {
        caveinfo.extend(
            mgr.caveinfos_from_cave(&format!("{}:{}", cfg.game, cfg.shortened_names.first().unwrap()))
                .unwrap()
                .into_iter()
                .cloned()
        );
    }
    caveinfo
}

pub fn benchmark_layout_generation(c: &mut Criterion) {
    let mgr = AssetManager::init().unwrap();
    let mut rng: SmallRng = SeedableRng::seed_from_u64(0x12345678);
    let all_sublevels = preload_caveinfo(&mgr);

    c.bench_function("layout generation (reference)", |b| {
        b.iter(|| {
            let seed = rng.gen();
            let caveinfo = &all_sublevels[rng.gen_range(0..all_sublevels.len())];
            black_box(Layout::generate(seed, caveinfo));
        })
    });
}

pub fn benchmark_layout_rendering(c: &mut Criterion) {
    let mgr = AssetManager::init().unwrap();
    let mut rng: SmallRng = SeedableRng::seed_from_u64(0x12345678);
    let all_sublevels = preload_caveinfo(&mgr);
    let renderer = Renderer::new(&mgr);

    c.bench_function("layout generation + rendering", |b| {
        b.iter(|| {
            let seed = rng.gen();
            let caveinfo = &all_sublevels[rng.gen_range(0..all_sublevels.len())];
            let layout = Layout::generate(seed, caveinfo);
            black_box(renderer.render_layout(&layout, LayoutRenderOptions::default()))
        })
    });
}


criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(1000);
    targets = benchmark_layout_generation, benchmark_layout_rendering
);
criterion_main!(benches);
