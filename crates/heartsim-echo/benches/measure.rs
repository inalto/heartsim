//! Added cost per frame of M-mode and of annular tissue Doppler (brief B34; a bench, not a gate):
//! one M-mode column (a motion evaluation, the grid update, one thin slice and its grey image),
//! the cursor's thin slice and image alone, and one Doppler sample (the ventricles' own motion at
//! the annulus).

use criterion::{Criterion, criterion_group, criterion_main};
use heartsim_core::{Millis, Parameters};
use heartsim_ecg::pcg::PcgParams;
use heartsim_echo::{
    AcousticTable, AnnulusSite, DesignedParams, DopplerContext, DopplerScan, EchoAnatomy, EchoWindow, Gate,
    ImageSettings, MModeSampler, MModeScan, MotionHistory, MotionInput, MotionModel, MotionParams, Plane2,
    ScanContext, SliceState,
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
    let anatomy = EchoAnatomy::new(&pack, &model, DesignedParams::default()).expect("anatomy");
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
    for _ in 0..5000 {
        engine.step(Millis(1.0));
        let ev = engine.drain_events();
        history.on_events(&ev);
        history.record_cells(engine.eikonal().cells());
    }
    let pose = anatomy.window_pose(EchoWindow::Apical);
    let settings = ImageSettings::default();
    let pose = settings.apply_to(&pose);
    let frame_p = pose.frame(anatomy.designed.contact_limit_deg);

    // one M-mode column, whole: evaluation + grid update + thin slice + thin grey image
    let mut scan = MModeScan::new(&anatomy, &model, AcousticTable::builtin());
    let ctx = ScanContext {
        anatomy: &anatomy,
        history: &history,
        pose,
        cursor_deg: 0.0,
        settings: &settings,
        enabled: true,
        exaggeration: 1.0,
    };
    let mut t = 3000.0;
    c.bench_function(
        "M-mode: one column (evaluate + update + thin slice + thin grey)",
        |b| {
            b.iter(|| {
                t += 25.0;
                scan.advance(&mut model, &ctx, black_box(t), 1);
            });
        },
    );

    // the line alone, on a state of one instant
    let mut frame = model.new_frame();
    let mut state = SliceState::new(&anatomy);
    let input = MotionInput {
        history: &history,
        t_ms: 3300.0,
        resp_phase: 0.3,
        enabled: true,
        exaggeration: 1.0,
    };
    model.evaluate(&input, &mut frame);
    state.update(&anatomy, &model, &frame, &input);
    let mut sampler = MModeSampler::new(AcousticTable::builtin());
    c.bench_function(
        "M-mode: the cursor's line alone (17-line thin slice + grey)",
        |b| {
            b.iter(|| {
                black_box(sampler.column(&anatomy, &state, &frame_p, black_box(0.0), &settings, 0));
            });
        },
    );

    // the split of a column: evaluation (with and without normals), grid update, thin line
    let mut tt = 3000.0;
    let mut mf = model.new_frame();
    c.bench_function("split: evaluate (with normals)", |b| {
        b.iter(|| {
            tt += 25.0;
            model.evaluate(black_box(&MotionInput { t_ms: tt, ..input }), &mut mf);
        });
    });
    c.bench_function("split: evaluate_positions (no normals)", |b| {
        b.iter(|| {
            tt += 25.0;
            model.evaluate_positions(black_box(&MotionInput { t_ms: tt, ..input }), &mut mf);
        });
    });
    c.bench_function("split: slice-state update (grid from frame)", |b| {
        b.iter(|| {
            state.update(
                &anatomy,
                &model,
                &mf,
                black_box(&MotionInput { t_ms: tt, ..input }),
            );
        });
    });
    let thin = heartsim_echo::cursor_frame(&frame_p, 0.0);
    let mut lab = heartsim_echo::LabelImage::new(heartsim_echo::MMODE_LINES, settings.samples);
    c.bench_function("split: thin slice (17 lines)", |b| {
        b.iter(|| state.slice_frame(&anatomy, &thin, settings.depth_mm, 17.0 * 0.7, &mut lab));
    });

    // one tissue-Doppler sample
    let site = AnnulusSite::MitralLateral;
    let p = model.annulus_position(&input, site).expect("annulus");
    let (q, _) = Plane2::of_world(&frame_p, p);
    let mut dop = DopplerScan::new();
    dop.set_gate(Some(Gate {
        site,
        beam: (p - frame_p.apex_mm).normalize(),
        at: q,
    }));
    let dctx = DopplerContext {
        history: &history,
        enabled: true,
        exaggeration: 1.0,
    };
    let mut td = 3000.0;
    c.bench_function("Doppler: one sample (the annulus point's motion)", |b| {
        b.iter(|| {
            td += 10.0;
            dop.advance(&mut model, &dctx, black_box(td), 1);
        });
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
