//! Acceptance tests of brief B2 (`docs/briefs/B02-anatomy-pipeline.md`).

use glam::Vec3;
use heartsim_core::{Electrode, Region};
use heartsim_geometry::procedural::heart::LANDMARKS;
use heartsim_geometry::procedural::{IDEALISED_REGIONS, Resolution, idealised_heart, procedural_electrodes};
use heartsim_geometry::{AnatomyPack, K_NEIGHBOURS, compute_hoods};
use std::path::PathBuf;
use std::sync::OnceLock;

fn default_heart() -> &'static heartsim_geometry::IdealisedHeart {
    static H: OnceLock<heartsim_geometry::IdealisedHeart> = OnceLock::new();
    H.get_or_init(|| idealised_heart(Resolution::DEFAULT))
}

fn heart_bounds() -> (Vec3, Vec3) {
    default_heart().cells.positions.iter().fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(l, h), p| (l.min(*p), h.max(*p)),
    )
}

#[test]
fn cloud_size_depth_fibres_and_spacing() {
    let h = default_heart();
    let n = h.cells.len();
    assert!((15_000..=40_000).contains(&n), "{n} cells");
    for (i, r) in h.cells.regions.iter().enumerate() {
        if r.is_ventricular() {
            let d = h.cells.depth[i];
            assert!((0.0..=1.0).contains(&d), "cell {i} depth {d}");
        }
        let f = h.cells.fibres[i];
        assert!((f.length() - 1.0).abs() < 1e-3, "cell {i} fibre {f:?} not unit");
    }
    let spacing = Resolution::DEFAULT.cell_spacing_mm;
    let mean = h.cells.mean_nearest_distance_mm();
    assert!(
        (mean / spacing - 1.0).abs() < 0.2,
        "mean NN {mean} vs spacing {spacing}"
    );
    for nb in &h.cells.neighbours {
        assert!(nb.iter().all(|e| e.index != u32::MAX && (e.index as usize) < n));
    }
}

#[test]
fn every_defined_region_is_populated_and_landmarks_exist() {
    let h = default_heart();
    for r in IDEALISED_REGIONS {
        let count = h.cells.regions.iter().filter(|x| **x == r).count();
        assert!(count > 0, "{r:?} has no cells");
    }
    for name in [
        "sa_node",
        "av_node",
        "his",
        "rbb",
        "lbb",
        "laf",
        "lpf",
        "rv_apex",
        "lv_lateral",
        "lv_posterior",
    ] {
        let idx = h
            .cells
            .landmarks
            .get(name)
            .unwrap_or_else(|| panic!("landmark {name} missing"));
        assert!((*idx as usize) < h.cells.len());
    }
    for name in LANDMARKS {
        assert!(h.cells.landmarks.contains_key(name), "{name}");
    }
    assert_eq!(
        h.cells.regions[h.cells.landmarks["sa_node"] as usize],
        Region::SinoatrialNode
    );
    assert_eq!(
        h.cells.regions[h.cells.landmarks["av_node"] as usize],
        Region::AvNodeCompact
    );
    assert_eq!(
        h.cells.regions[h.cells.landmarks["his"] as usize],
        Region::HisBundle
    );
    assert_eq!(
        h.cells.regions[h.cells.landmarks["rv_apex"] as usize],
        Region::RightVentricle
    );
    for fascicle in ["rbb", "laf", "lpf"] {
        let seeds = &h.pmj_seeds[fascicle];
        assert!(seeds.len() >= 20, "{fascicle}: {} PMJ seeds", seeds.len());
        assert!(
            seeds
                .iter()
                .all(|s| h.cells.regions[*s as usize].is_ventricular())
        );
    }
    // AHA segments cover 1..=17 on the LV/septum and are 0 elsewhere
    let mut seen = [false; 18];
    for (s, r) in h.aha_segments.iter().zip(&h.cells.regions) {
        if matches!(r, Region::LeftVentricle | Region::VentricularSeptum) {
            assert!((1..=17).contains(s));
            seen[usize::from(*s)] = true;
        } else {
            assert_eq!(*s, 0);
        }
    }
    assert!(seen[1..].iter().all(|b| *b), "AHA segments missing: {seen:?}");
}

#[test]
fn anatomical_orientation() {
    let h = default_heart();
    let at = |name: &str| h.cells.positions[h.cells.landmarks[name] as usize];
    let rv_apex = at("rv_apex");
    let sa = at("sa_node");
    assert!(
        rv_apex.y < -20.0 && rv_apex.x > -10.0 && rv_apex.z > 0.0,
        "apex inferior/left/anterior: {rv_apex:?}"
    );
    assert!(sa.y > 30.0 && sa.x < 0.0, "SA node superior and right: {sa:?}");
    // RV free wall is anterior-right of the LV lateral wall
    assert!(at("rv_free_wall").z > at("lv_lateral").z);
    assert!(at("rv_free_wall").x < at("lv_lateral").x);
    assert!(at("lv_posterior").z < at("lv_anterior").z);
}

#[test]
fn hoods_are_local_and_normalised() {
    let h = default_heart();
    let spacing = Resolution::DEFAULT.cell_spacing_mm;
    for m in h.meshes.iter().filter(|m| m.conducting()) {
        let hoods = compute_hoods(&m.mesh, &h.cells);
        assert_eq!(hoods.len(), m.mesh.vertex_count());
        for (v, hood) in m.mesh.vertices.iter().zip(&hoods) {
            let p = Vec3::from(v.position);
            let sum: f32 = hood.weights.iter().sum();
            assert!((sum - 1.0).abs() < 1e-4, "{}: weights sum {sum}", m.name);
            for k in 0..K_NEIGHBOURS {
                let d = h.cells.positions[hood.cells[k] as usize].distance(p);
                assert!(d <= 3.0 * spacing, "{}: hood cell {d} mm away", m.name);
                assert!(hood.weights[k] >= 0.0);
            }
        }
    }
    assert!(
        h.meshes
            .iter()
            .any(|m| m.name == "lv" && m.mesh.vertex_count() > 5000)
    );
}

#[test]
fn electrodes_are_outside_the_heart_in_the_right_places() {
    let e = procedural_electrodes();
    let (lo, hi) = heart_bounds();
    for el in Electrode::ALL {
        let p = e.standard(el).unwrap_or_else(|| panic!("{el:?} missing"));
        let inside = p.cmpge(lo).all() && p.cmple(hi).all();
        assert!(
            !inside,
            "{el:?} at {p:?} is inside the heart bounds {lo:?}..{hi:?}"
        );
    }
    for name in heartsim_geometry::electrodes::EXTRA_SITES {
        assert!(e.get(name).is_some(), "{name}");
    }
    assert!(e.standard(Electrode::V1).unwrap().z > hi.z);
    assert!(e.standard(Electrode::V2).unwrap().z > hi.z);
    assert!(e.standard(Electrode::LL).unwrap().y < lo.y);
    assert!(e.standard(Electrode::RL).unwrap().y < lo.y);
    assert!(e.standard(Electrode::V6).unwrap().x > hi.x);
    assert!(e.standard(Electrode::RA).unwrap().x < e.standard(Electrode::LA).unwrap().x);
}

#[test]
fn pack_roundtrip_preserves_counts_and_checksums() {
    let dir = std::env::temp_dir().join(format!("heartsim-pack-roundtrip-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut pack = AnatomyPack::procedural(Resolution::COARSE);
    let manifest = pack.save(&dir).expect("save");
    assert!(dir.join("LICENSE").is_file());
    assert!(dir.join("hoods/lv.bin").is_file());
    let loaded = AnatomyPack::load(&dir).expect("load");
    assert_eq!(loaded.manifest, manifest);
    assert_eq!(loaded.cells.len(), pack.cells.len());
    assert_eq!(loaded.cells.positions, pack.cells.positions);
    assert_eq!(loaded.cells.fibres, pack.cells.fibres);
    assert_eq!(loaded.cells.regions, pack.cells.regions);
    assert_eq!(loaded.cells.depth, pack.cells.depth);
    assert_eq!(loaded.cells.neighbours, pack.cells.neighbours);
    assert_eq!(loaded.cells.landmarks, pack.cells.landmarks);
    assert_eq!(loaded.landmarks, pack.landmarks);
    assert_eq!(loaded.electrodes, pack.electrodes);
    assert_eq!(loaded.aha_segments, pack.aha_segments);
    assert_eq!(loaded.torso, pack.torso);
    assert_eq!(loaded.meshes.len(), pack.meshes.len());
    for (a, b) in loaded.meshes.iter().zip(&pack.meshes) {
        assert_eq!(a.name, b.name);
        assert_eq!(a.kind, b.kind);
        assert_eq!(a.mesh, b.mesh);
    }
    assert_eq!(loaded.hoods.len(), pack.hoods.len());
    for (name, h) in &pack.hoods {
        assert_eq!(&loaded.hoods[name], h);
    }
    // saving again yields identical checksums (deterministic serialisation)
    let again = pack.save(&dir).expect("save twice");
    assert_eq!(again.files, manifest.files);

    // a corrupted file is rejected
    let cells = dir.join("cells.bin");
    let mut bytes = std::fs::read(&cells).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    std::fs::write(&cells, bytes).unwrap();
    assert!(matches!(
        AnatomyPack::load(&dir),
        Err(heartsim_geometry::PackError::Checksum { .. })
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

fn default_pack_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/anatomy/default")
}

#[test]
fn default_pack_loads_fast_and_matches_the_generator() {
    let dir = default_pack_dir();
    assert!(
        dir.join("manifest.ron").is_file(),
        "{} missing: run `cargo xtask bake-anatomy`",
        dir.display()
    );
    assert!(
        dir.join("LICENSE").is_file(),
        "licence file missing from the pack"
    );
    // Build the shared fixture first so its CPU time is not charged to the load; time the best of
    // three loads so a parallel test run on a loaded machine does not fail the budget.
    let h = default_heart();
    let mut best = std::time::Duration::MAX;
    let mut pack = None;
    for _ in 0..3 {
        let t = std::time::Instant::now();
        pack = Some(AnatomyPack::load(&dir).expect("default pack loads"));
        best = best.min(t.elapsed());
    }
    let pack = pack.expect("loaded");
    assert!(best.as_millis() < 500, "default pack took {best:?} to load");
    assert_eq!(
        pack.cells.len(),
        h.cells.len(),
        "committed pack is stale: re-run `cargo xtask bake-anatomy`"
    );
    assert_eq!(pack.cells.landmarks, h.cells.landmarks);
    assert_eq!(pack.landmarks.sets, h.pmj_seeds);
    assert_eq!(pack.cells.region_counts(), h.cells.region_counts());
    assert!(pack.electrodes.has_standard_ten());
    assert_eq!(
        pack.manifest.format_version,
        heartsim_geometry::PACK_FORMAT_VERSION
    );
    assert_eq!(
        pack.hoods.len(),
        pack.meshes.iter().filter(|m| m.conducting()).count()
    );
}
