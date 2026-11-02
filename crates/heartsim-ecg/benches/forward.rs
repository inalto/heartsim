//! Forward-model budgets (brief B05): 50 k cells × 9 electrodes — `sample` ≤ 0.3 ms,
//! `move_electrode` ≤ 1 ms. `cargo bench -p heartsim-ecg --bench forward`.

use criterion::{Criterion, criterion_group, criterion_main};
use glam::Vec3;
use heartsim_core::Electrode;
use heartsim_ecg::{ForwardModel, ForwardOptions};
use heartsim_geometry::{ElectrodeSet, Resolution, idealised_heart};
use std::hint::black_box;
use std::path::Path;

fn bench(c: &mut Criterion) {
    // ≈ 50 k cells: the idealised heart on a 1.6 mm lattice (render meshes coarse, unused).
    let heart = idealised_heart(Resolution {
        mesh_voxel_mm: 8.0,
        cell_spacing_mm: 1.6,
    });
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/anatomy/default/electrodes.ron");
    let electrodes =
        ElectrodeSet::from_ron(&std::fs::read_to_string(dir).expect("electrodes.ron")).expect("parses");
    let mut model =
        ForwardModel::build(&heart.cells, &electrodes, ForwardOptions::default()).expect("builds");
    eprintln!("cells {}", model.cell_count());
    let vm: Vec<f32> = heart
        .cells
        .positions
        .iter()
        .map(|p| (0.5 + p.x / 200.0).clamp(0.0, 1.0))
        .collect();
    c.bench_function("forward_sample_50k", |b| {
        b.iter(|| black_box(model.sample(black_box(&vm))));
    });
    let v1 = model.electrode_position(Electrode::V1);
    let mut flip = false;
    c.bench_function("forward_move_electrode_50k", |b| {
        b.iter(|| {
            flip = !flip;
            let dz = if flip { 5.0 } else { 0.0 };
            model.move_electrode(Electrode::V1, v1 + Vec3::new(0.0, 0.0, dz));
        });
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
