//! Per-frame cost of the wall-motion model on the default pack (brief B30; a bench, not a gate):
//! one `MotionModel::evaluate` — contraction of every cell, the LV volume solve, every part's
//! positions and recomputed normals — at times spread over a sinus beat.

use criterion::{Criterion, criterion_group, criterion_main};
use heartsim_core::{Millis, Parameters};
use heartsim_ecg::pcg::PcgParams;
use heartsim_echo::{MotionHistory, MotionInput, MotionModel, MotionParams};
use heartsim_geometry::AnatomyPack;
use heartsim_sim::{EikonalEngine, EikonalParams, Engine, Tier1Engine, TimingParams};
use std::hint::black_box;
use std::path::Path;
use std::sync::Arc;

fn bench(c: &mut Criterion) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/anatomy/default");
    let pack = AnatomyPack::load(&dir).expect("default pack");
    let mut model = MotionModel::new(&pack, MotionParams::default()).expect("model");
    let vertices: usize = model.new_frame().parts.iter().map(|p| p.vertices.len()).sum();
    let eikonal = EikonalEngine::with_seed_sets(
        Arc::new(pack.cells.clone()),
        &Parameters::default(),
        EikonalParams::default(),
        &pack.landmarks.sets,
    );
    let timing = TimingParams::default();
    let rate = timing.sinus.respiration_rate_bpm;
    let mut engine = Tier1Engine::new(timing, eikonal);
    let mut history = MotionHistory::new(PcgParams::default(), rate, pack.cells.len());
    for _ in 0..4000 {
        engine.step(Millis(1.0));
        let ev = engine.drain_events();
        history.on_events(&ev);
        history.record_cells(engine.eikonal().cells());
    }
    eprintln!(
        "{vertices} vertices in {} parts, {} cells",
        model.new_frame().parts.len(),
        pack.cells.len()
    );
    let mut frame = model.new_frame();
    let mut k = 0u32;
    c.bench_function("evaluate (default pack, sinus)", |b| {
        b.iter(|| {
            // 60 Hz frames over a beat
            let t = 2000.0 + f64::from(k % 60) * 16.7;
            k += 1;
            let input = MotionInput {
                history: &history,
                t_ms: t,
                resp_phase: 0.3,
                enabled: true,
                exaggeration: 1.0,
            };
            model.evaluate(black_box(&input), &mut frame);
            black_box(&frame);
        });
    });
}

fn bench_readout(c: &mut Criterion) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/anatomy/default");
    let pack = AnatomyPack::load(&dir).expect("default pack");
    let mut model = MotionModel::new(&pack, MotionParams::default()).expect("model");
    let eikonal = EikonalEngine::with_seed_sets(
        Arc::new(pack.cells.clone()),
        &Parameters::default(),
        EikonalParams::default(),
        &pack.landmarks.sets,
    );
    let timing = TimingParams::default();
    let rate = timing.sinus.respiration_rate_bpm;
    let mut engine = Tier1Engine::new(timing, eikonal);
    let mut history = MotionHistory::new(PcgParams::default(), rate, pack.cells.len());
    for _ in 0..4000 {
        engine.step(Millis(1.0));
        let ev = engine.drain_events();
        history.on_events(&ev);
        history.record_cells(engine.eikonal().cells());
    }
    let mut k = 0u32;
    c.bench_function("readout only (default pack, sinus)", |b| {
        b.iter(|| {
            let t = 2000.0 + f64::from(k % 60) * 16.7;
            k += 1;
            let input = MotionInput {
                history: &history,
                t_ms: t,
                resp_phase: 0.3,
                enabled: false,
                exaggeration: 1.0,
            };
            black_box(model.readout(black_box(&input)));
        });
    });
}

criterion_group!(benches, bench, bench_readout);
criterion_main!(benches);
