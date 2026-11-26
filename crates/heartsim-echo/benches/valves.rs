//! The echo gates of brief B36 (designed mitral, aortic and pulmonary leaflets as meshes): a
//! 128 × 512 slice through each valve with every valve half open, one M-mode column through the
//! mitral and the aortic valves, and the cost of re-posing the leaflets when one valve's opening
//! changes and when all four change. Run before and after the change on the same machine:
//! `cargo bench -p heartsim-echo --bench valves`. The gates use only API that existed before B36, so the
//! same file measures both; the per-valve pose and fill costs at the end need B36.

use criterion::{Criterion, criterion_group, criterion_main};
use heartsim_core::{Millis, Parameters};
use heartsim_ecg::Valve;
use heartsim_ecg::pcg::PcgParams;
use heartsim_echo::{
    AcousticTable, DesignedParams, EchoAnatomy, EchoWindow, ImageSettings, LabelImage, MModeScan,
    MotionHistory, MotionInput, MotionModel, MotionParams, ProbePose, ScanContext, SliceState,
};
use heartsim_geometry::{AnatomyPack, Label};
use heartsim_sim::{EikonalEngine, EikonalParams, Engine, Tier1Engine, TimingParams};
use std::hint::black_box;
use std::path::Path;
use std::sync::Arc;

#[allow(clippy::too_many_lines)]
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
    let input = |t: f64| MotionInput {
        history: &history,
        t_ms: t,
        resp_phase: 0.3,
        enabled: true,
        exaggeration: 1.0,
    };
    let mut frame = model.new_frame();
    model.evaluate(&input(3300.0), &mut frame);
    let mut state = SliceState::new(&anatomy);
    state.update(&anatomy, &model, &frame, &input(3300.0));
    state.set_openings(&anatomy, [0.5; 4]);

    let centre = |v: Valve| model.frame().valve_centres_mm[v.index()];
    let d = &anatomy.designed;
    let apical = anatomy.window_pose(EchoWindow::Apical);
    let para = anatomy.window_pose(EchoWindow::Parasternal);
    let aimed = |from: &ProbePose, target, marker| {
        ProbePose::aimed(
            from.skin_mm,
            from.normal,
            target,
            marker,
            d.contact_limit_deg,
            from.depth_mm,
            from.sector_deg,
        )
    };
    let poses = [
        ("mitral (apical window)", apical),
        (
            "aortic (parasternal, aimed at the aortic valve)",
            aimed(
                &para,
                centre(Valve::Aortic),
                centre(Valve::Aortic) - centre(Valve::Mitral),
            ),
        ),
        (
            "pulmonary (parasternal, aimed at the pulmonary valve)",
            aimed(&para, centre(Valve::Pulmonic), glam::Vec3::Y),
        ),
        ("tricuspid (apical window)", apical),
    ];
    let mut image = LabelImage::new(128, 512);
    for (name, pose) in &poses {
        state.slice(&anatomy, pose, &mut image);
        let n = image
            .labels
            .iter()
            .fold(0usize, |n, l| n + usize::from(*l == Label::ValveLeaflet as u8));
        eprintln!("slice through the {name}: {n} leaflet samples");
    }
    for (name, pose) in &poses[..3] {
        c.bench_function(
            &format!("B36 slice 128x512 through the {name}, all valves half open"),
            |b| {
                b.iter(|| {
                    state.slice(&anatomy, black_box(pose), &mut image);
                    black_box(&image);
                });
            },
        );
    }

    // re-posing: one valve's opening changes, then all four
    let mut k = 0u32;
    c.bench_function("B36 repose: the aortic opening changes", |b| {
        b.iter(|| {
            k += 1;
            let mut o = [0.5f32; 4];
            o[Valve::Aortic.index()] = (k % 100) as f32 / 100.0;
            state.set_openings(&anatomy, black_box(o));
        });
    });
    c.bench_function("B36 repose: all four openings change", |b| {
        b.iter(|| {
            k += 1;
            state.set_openings(&anatomy, black_box([(k % 100) as f32 / 100.0; 4]));
        });
    });

    // after B36 only: per mesh valve, the echo pose + grid, the 3-D pose carried by the disc, and
    // the drawn shell's vertices
    for mv in anatomy.mesh_valves() {
        let name = heartsim_echo::mesh_valve::pack_name(mv.valve_id);
        let mut posed = heartsim_echo::PosedMeshValve::default();
        let mut o = 0u32;
        c.bench_function(&format!("B36 {name}: pose + triangle grid (echo)"), |b| {
            b.iter(|| {
                o += 1;
                posed.update(mv, black_box((o % 100) as f32 / 100.0));
            });
        });
        let (mut scratch, mut out) = (Vec::new(), Vec::new());
        c.bench_function(
            &format!("B36 {name}: pose carried by the disc (3-D heart)"),
            |b| {
                b.iter(|| {
                    o += 1;
                    mv.pose(
                        black_box((o % 100) as f32 / 100.0),
                        Some(&frame),
                        &mut scratch,
                        &mut out,
                    );
                    black_box(&out);
                });
            },
        );
        let mut render = heartsim_geometry::ValveRenderMesh::new(
            &mv.data,
            heartsim_echo::mesh_valve::thickness_mm(mv.valve_id, &anatomy.designed),
        );
        let mut verts = Vec::new();
        c.bench_function(
            &format!("B36 {name}: shell and chordae vertices (3-D heart)"),
            |b| {
                b.iter(|| {
                    render.fill(black_box(&out), &mut verts);
                    black_box(&verts);
                });
            },
        );
    }

    // one M-mode column (evaluate + update + thin slice + thin grey), apical and parasternal
    let settings = ImageSettings::default();
    for (name, pose) in [("apical", apical), ("parasternal", para)] {
        let pose = settings.apply_to(&pose);
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
        c.bench_function(&format!("B36 M-mode one column ({name})"), |b| {
            b.iter(|| {
                t += 25.0;
                scan.advance(&mut model, &ctx, black_box(t), 1);
            });
        });
    }
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(30);
    targets = bench
}
criterion_main!(benches);
