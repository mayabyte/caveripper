use caveripper::{
    assets::fs_asset_manager::FsAssetManager,
    caveinfo::CaveInfo,
    layout::Layout,
    render::{render_layout, LayoutRenderOptions, RenderHelper},
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::{rngs::SmallRng, Rng, SeedableRng};

fn preload_caveinfo(mgr: &FsAssetManager) -> Vec<CaveInfo> {
    let mut caveinfo = Vec::new();
    for cfg in mgr.cave_cfg.iter() {
        caveinfo.extend(
            mgr.caveinfos_from_cave(&format!("{}:{}", cfg.game, cfg.shortened_names.first().unwrap()))
                .unwrap()
                .into_iter()
                .cloned(),
        );
    }
    caveinfo
}

pub fn benchmark_layout_generation(c: &mut Criterion) {
    let mgr = FsAssetManager::init().unwrap();
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
    let mgr = FsAssetManager::init().unwrap();
    let mut rng: SmallRng = SeedableRng::seed_from_u64(0x12345678);
    let all_sublevels = preload_caveinfo(&mgr);
    let helper = RenderHelper::new(&mgr);

    c.bench_function("layout generation + rendering", |b| {
        b.iter(|| {
            let seed = rng.gen();
            let caveinfo = &all_sublevels[rng.gen_range(0..all_sublevels.len())];
            let layout = Layout::generate(seed, caveinfo);
            black_box(render_layout(&layout, &helper, LayoutRenderOptions::default()))
        })
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(1000);
    targets = benchmark_layout_generation, benchmark_layout_rendering
);
criterion_main!(benches);
