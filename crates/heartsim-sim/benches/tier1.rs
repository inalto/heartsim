//! Tier-1 budgets (brief B03 §7): 50 k cells — Dijkstra ≤ 15 ms per beat, V_m update ≤ 0.2 ms
//! per step. `cargo bench -p heartsim-sim --bench tier1`.

use criterion::{Criterion, criterion_group, criterion_main};
use heartsim_core::{Millis, Parameters};
use heartsim_geometry::{Resolution, idealised_heart};
use heartsim_sim::tier1::{EikonalEngine, EikonalParams};
use heartsim_sim::{Engine, Origin};
use std::hint::black_box;
use std::sync::Arc;

/// ≈ 50 k cells: the idealised heart on a 1.6 mm lattice (render meshes coarse, unused).
fn engine_50k() -> EikonalEngine {
    let heart = idealised_heart(Resolution {
        mesh_voxel_mm: 8.0,
        cell_spacing_mm: 1.6,
    });
    EikonalEngine::with_seed_sets(
        Arc::new(heart.cells),
        &Parameters::default(),
        EikonalParams::default(),
        &heart.pmj_seeds,
    )
}

fn bench(c: &mut Criterion) {
    let base = engine_50k();
    eprintln!(
        "cells {} edges {} ({:.1}/cell)",
        base.cloud().len(),
        base.graph().edge_count(),
        base.graph().edge_count() as f64 / base.cloud().len() as f64
    );
    let seeds: Vec<u32> = [&base.seeds().laf, &base.seeds().lpf, &base.seeds().rbb]
        .into_iter()
        .flatten()
        .copied()
        .collect();
    let atrial = base.seeds().sinus.clone();

    c.bench_function("tier1/beat_dijkstra_50k", |b| {
        b.iter_batched(
            || {
                let mut e = base.clone();
                e.stimulate_cells(&atrial, Millis(0.0), Origin::Sinus);
                e.stimulate_cells(&seeds, Millis(0.0), Origin::Sinus);
                e
            },
            |mut e| {
                // one 200 ms step settles the whole beat (atria and ventricles, no
                // re-excitation); V_m is updated once
                e.step(Millis(200.0));
                black_box(e.cells().vm[0])
            },
            criterion::BatchSize::LargeInput,
        );
    });

    let mut quiet = base.clone();
    quiet.stimulate_cells(&seeds, Millis(0.0), Origin::Sinus);
    quiet.step(Millis(150.0));
    c.bench_function("tier1/vm_step_50k", |b| {
        b.iter(|| {
            quiet.step(Millis(1.0));
            black_box(quiet.cells().vm[0])
        });
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
