//! Tier-2 wall-clock budget (brief B16 §6). A binary of its own: cargo runs test binaries one at
//! a time, so the measurement does not compete with the other tests for the rayon pool.

use glam::Vec3;
use heartsim_sim::tier2::{AlievPanfilov, Compartment, Monodomain, Tissue, VoxelGrid, VoxelSpec};
use std::time::Instant;

/// Brief budget: 64³ grid, Aliev–Panfilov, dt 0.25 ms → ≤ 8 ms per 16.7 ms frame on 4 cores in
/// release builds. **This test does not check that budget**, which is not met yet: it runs at
/// opt-level 1 on shared CI machines and only catches a kernel some 20× slower than today's. Wall
/// time here is mostly a measure of the machine's load: the fastest frame has been seen from 75 ms
/// to 480 ms on 8 threads for the same CPU time (≈ 5.5 s for the run at any thread count). It also
/// cannot catch lost parallelism, since one thread takes 300–330 ms. The honest numbers come from
/// `cargo bench -p heartsim-sim --bench tier2`.
#[test]
fn budget_64_cubed_loose() {
    // a thick spherical shell: ≈ 84 k active voxels of the 262 k box
    let grid = VoxelGrid::from_fn([64, 64, 64], 2.0, Vec3::ZERO, |[i, j, k], _| {
        let r = Vec3::new(i as f32, j as f32, k as f32).distance(Vec3::splat(31.5));
        (19.0..=30.0).contains(&r).then_some(VoxelSpec {
            compartment: Compartment::Ventricular,
            fibre: Vec3::X,
        })
    });
    let active = grid.len();
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
    let frames = 12;
    let (mut total_ms, mut fastest_ms) = (0.0f64, f64::INFINITY);
    for f in 1..=frames {
        let t0 = Instant::now();
        sim.advance_to(50.0 + 16.7 * f64::from(f));
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        total_ms += ms;
        fastest_ms = fastest_ms.min(ms);
    }
    eprintln!(
        "64³ budget: {active} active voxels, per 16.7 ms frame {fastest_ms:.2} ms fastest, {:.2} ms mean, {} threads",
        total_ms / f64::from(frames),
        rayon::current_num_threads()
    );
    assert!(fastest_ms < 1500.0, "fastest frame {fastest_ms:.1} ms");
}
