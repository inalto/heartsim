//! Per-frame cost of the echo cross-section on the default pack (brief B31 item 8; a bench, not a
//! gate): the slice alone (128 × 512 samples from the apical window), the slice-state update (the
//! displacement grid filled from the evaluated frame, the valve openings), and the whole drawn
//! frame (motion evaluation + update + slice); and the grey image of brief B32 on top of that slice,
//! whole and by stage.

use criterion::{Criterion, criterion_group, criterion_main};
use heartsim_core::{Millis, Parameters};
use heartsim_ecg::pcg::PcgParams;
use heartsim_echo::{
    AcousticTable, DesignedParams, EchoAnatomy, EchoImager, EchoWindow, GreyImage, ImageSettings, LabelImage,
    MotionHistory, MotionInput, MotionModel, MotionParams, SliceState,
};
use heartsim_geometry::AnatomyPack;
use heartsim_sim::{EikonalEngine, EikonalParams, Engine, Tier1Engine, TimingParams};
use std::hint::black_box;
use std::path::Path;
use std::sync::Arc;

fn bench(c: &mut Criterion) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/anatomy/default");
    let pack = AnatomyPack::load(&dir).expect("default pack");
    let mut model = MotionModel::new(&pack, MotionParams::default()).expect("model");
    let t0 = std::time::Instant::now();
    let anatomy = EchoAnatomy::new(&pack, &model, DesignedParams::default()).expect("anatomy");
    eprintln!("EchoAnatomy::new: {:.0?}", t0.elapsed());
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
    let pose = anatomy.window_pose(EchoWindow::Apical);
    let mut frame = model.new_frame();
    let mut state = SliceState::new(&anatomy);
    let mut image = LabelImage::new(128, 512);
    let input = |t: f64| MotionInput {
        history: &history,
        t_ms: t,
        resp_phase: 0.3,
        enabled: true,
        exaggeration: 1.0,
    };
    model.evaluate(&input(2300.0), &mut frame);
    state.update(&anatomy, &model, &frame, &input(2300.0));
    c.bench_function("slice alone (128 x 512, apical)", |b| {
        b.iter(|| {
            state.slice(&anatomy, black_box(&pose), &mut image);
            black_box(&image);
        });
    });
    let mut k = 0u32;
    c.bench_function("slice-state update (grid from frame, openings)", |b| {
        b.iter(|| {
            let t = 2000.0 + f64::from(k % 60) * 16.7;
            k += 1;
            state.update(&anatomy, &model, &frame, black_box(&input(t)));
        });
    });
    // the grey image (B32) on top of the slice of the same frame, by stage
    state.slice(&anatomy, &pose, &mut image);
    let settings = ImageSettings::default();
    let frame_p = pose.frame(anatomy.designed.contact_limit_deg);
    let mut imager = EchoImager::new(AcousticTable::builtin());
    let mut grey = GreyImage::new(1, 1);
    let rest = |x, h| state.rest_label(&anatomy, x, h) as u8;
    c.bench_function("grey image (128 x 512, apical, all stages)", |b| {
        b.iter(|| {
            imager.image(&anatomy, &state, black_box(&pose), &image, &settings, &mut grey);
            black_box(&grey);
        });
    });
    c.bench_function("grey 1: line model (attenuation, interfaces, normals)", |b| {
        b.iter(|| imager.stage_line_model(&image, black_box(&frame_p), &settings, rest));
    });
    c.bench_function("grey 2: speckle (rest-position hash)", |b| {
        b.iter(|| imager.stage_speckle(black_box(&image), &settings));
    });
    c.bench_function("grey 3: PSF and envelope", |b| {
        b.iter(|| imager.stage_psf());
    });
    c.bench_function("grey 4: compensation and log compression", |b| {
        b.iter(|| imager.stage_compress(black_box(&image), &settings, &mut grey));
    });
    c.bench_function("evaluate + update + slice (one drawn frame)", |b| {
        b.iter(|| {
            let t = 2000.0 + f64::from(k % 60) * 16.7;
            k += 1;
            let inp = input(t);
            model.evaluate(&inp, &mut frame);
            state.update(&anatomy, &model, &frame, &inp);
            state.slice(&anatomy, &pose, &mut image);
            black_box(&image);
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(20);
    targets = bench
}
criterion_main!(benches);
