//! Tier-2 budget (brief B16 §6): 64³ grid, Aliev–Panfilov, dt 0.25 ms → ≤ 8 ms per 16.7 ms
//! frame on 4 cores; plus the default-pack heart (per step and per sinus beat).
//! `cargo bench -p heartsim-sim --bench tier2`.

use criterion::{Criterion, criterion_group, criterion_main};
use glam::Vec3;
use heartsim_core::{Millis, Parameters};
use heartsim_geometry::AnatomyPack;
use heartsim_sim::Engine;
use heartsim_sim::tier1::{EikonalEngine, EikonalParams};
use heartsim_sim::tier2::{
    AlievPanfilov, Compartment, Monodomain, Tier2Engine, Tier2Params, Tissue, VoxelGrid, VoxelSpec,
};
use heartsim_sim::timing::TimingParams;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

fn grid_64(shell: bool) -> VoxelGrid {
    VoxelGrid::from_fn([64, 64, 64], 2.0, Vec3::ZERO, |[i, j, k], _| {
        let r = Vec3::new(i as f32, j as f32, k as f32).distance(Vec3::splat(31.5));
        (!shell || (19.0..=30.0).contains(&r)).then_some(VoxelSpec {
            compartment: Compartment::Ventricular,
            fibre: Vec3::X,
        })
    })
}

fn solver(grid: VoxelGrid) -> Monodomain<AlievPanfilov> {
    let mut sim = Monodomain::uniform(
        grid,
        Tissue {
            model: AlievPanfilov::default(),
            d_long: 2.4,
            d_trans: 0.6,
        },
        0.25,
    );
    let s1: Vec<u32> = (0..2000).collect();
    sim.stimulate(&s1, 0.0, 2.0, 1.0);
    sim.advance_to(50.0);
    sim
}

fn bench(c: &mut Criterion) {
    for (name, shell) in [
        ("tier2 64³ shell frame (67 substeps)", true),
        ("tier2 64³ full box frame (67 substeps)", false),
    ] {
        let base = solver(grid_64(shell));
        eprintln!("{name}: {} active voxels", base.grid().len());
        c.bench_function(name, |b| {
            b.iter_batched_ref(
                || base.clone(),
                |s| {
                    let t = s.now_ms();
                    s.advance_to(t + 16.75);
                },
                criterion::BatchSize::LargeInput,
            );
        });
    }

    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/anatomy/default");
    let pack = AnatomyPack::load(&dir).expect("default pack");
    let eik = EikonalEngine::with_seed_sets(
        Arc::new(pack.cells),
        &Parameters::default(),
        EikonalParams::default(),
        &pack.landmarks.sets,
    );
    let t0 = Instant::now();
    let heart = Tier2Engine::new(
        TimingParams::default(),
        eik,
        &AlievPanfilov::default(),
        Tier2Params::default(),
    );
    eprintln!(
        "tier2 heart: {} voxels, {} classes, built (calibration) in {:.2?}",
        heart.solver().grid().len(),
        heart.solver().tissue().len(),
        t0.elapsed()
    );
    c.bench_function("tier2 heart: one 1 ms engine step", |b| {
        b.iter_batched_ref(
            || heart.clone(),
            |e| e.step(Millis(1.0)),
            criterion::BatchSize::LargeInput,
        );
    });
    c.bench_function("tier2 heart: one sinus beat (857 ms)", |b| {
        b.iter_batched_ref(
            || heart.clone(),
            |e| {
                for _ in 0..857 {
                    e.step(Millis(1.0));
                }
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench
}
criterion_main!(benches);
