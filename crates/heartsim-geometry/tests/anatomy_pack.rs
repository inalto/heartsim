//! Acceptance tests of brief B2 (`docs/briefs/B02-anatomy-pipeline.md`).

use glam::Vec3;
use heartsim_core::{Electrode, Region};
use heartsim_geometry::procedural::heart::LANDMARKS;
use heartsim_geometry::procedural::{IDEALISED_REGIONS, Resolution, idealised_heart, procedural_electrodes};
use heartsim_geometry::territory::RV_ANTERIOR_SEGMENT;
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
    // AHA segments cover 1..=17 on the LV/septum; the RV free wall has 0 or the LAD's zone, about a
    // third of it; other tissue has 0
    let mut seen = [false; 18];
    for (s, r) in h.aha_segments.iter().zip(&h.cells.regions) {
        if matches!(r, Region::LeftVentricle | Region::VentricularSeptum) {
            assert!((1..=17).contains(s));
            seen[usize::from(*s)] = true;
        } else if *r == Region::RightVentricle {
            assert!(*s == 0 || *s == RV_ANTERIOR_SEGMENT);
        } else {
            assert_eq!(*s, 0);
        }
    }
    let rv = h
        .cells
        .regions
        .iter()
        .filter(|r| **r == Region::RightVentricle)
        .count();
    // a test on a few thousand cells: not worth the `bytecount` dependency
    #[allow(clippy::naive_bytecount)]
    let lad = h
        .aha_segments
        .iter()
        .filter(|s| **s == RV_ANTERIOR_SEGMENT)
        .count();
    assert!(
        (0.3..0.45).contains(&(lad as f64 / rv as f64)),
        "LAD zone: {lad} of {rv} RV cells"
    );
    assert!(seen[1..].iter().all(|b| *b), "AHA segments missing: {seen:?}");
}

#[test]
fn anatomical_orientation() {
    let h = default_heart();
    let at = |name: &str| h.cells.positions[h.cells.landmarks[name] as usize];
    let rv_apex = at("rv_apex");
    let lv_apex = at("lv_apex");
    let sa = at("sa_node");
    assert!(
        lv_apex.y < -15.0 && lv_apex.x > 20.0 && lv_apex.z > 10.0,
        "LV apex inferior/left/anterior: {lv_apex:?}"
    );
    // The long axis is 27° below the horizontal (Engblom 2005 mean, see `procedural::heart`), not the
    // 45° of the first version of the pack, so the RV apex is anterior and well below the outflow
    // tract rather than far below the centroid.
    assert!(
        rv_apex.y < at("rvot").y - 20.0 && rv_apex.x > -10.0 && rv_apex.z > 0.0,
        "RV apex below the outflow tract, left, anterior: {rv_apex:?}"
    );
    assert!(sa.y > 25.0 && sa.x < 0.0, "SA node superior and right: {sa:?}");
    assert!(sa.z < at("rvot").z, "atria behind the ventricles: {sa:?}");
    // all four chambers share an axial slice through the middle of the ventricles, the veins enter
    // the left atrium at or above it, and the left atrium is the most posterior chamber
    let antrum = at("la_pv_antrum");
    assert!(
        antrum.y > 0.0,
        "pulmonary veins not below mid-ventricle: {antrum:?}"
    );
    assert!(
        antrum.y > lv_apex.y + 25.0,
        "pulmonary veins above the apex: {antrum:?}"
    );
    assert!(
        antrum.z < at("lv_lateral").z && antrum.z < sa.z,
        "left atrium posterior: {antrum:?}"
    );
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
fn default_pack_loads_fast_and_is_complete() {
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
    // Time the best of three loads so a parallel test run on a loaded machine does not fail the
    // budget.
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
        pack.manifest.format_version,
        heartsim_geometry::PACK_FORMAT_VERSION
    );
    // The default pack is baked from the Rodero 2021 heart and the KCL torso (CC BY 4.0): the
    // attribution their licences require must ship in the pack.
    assert!(
        pack.licence.contains("CC BY 4.0") || pack.licence.contains("Creative Commons Attribution 4.0"),
        "pack LICENSE lacks the CC BY 4.0 attribution"
    );
    assert!(pack.licence.contains("zenodo.4593739") && pack.licence.contains("zenodo.7253863"));
    assert!(
        pack.manifest.source.contains("Rodero"),
        "{}",
        pack.manifest.source
    );

    // The same contract as the procedural generator: every region and landmark the engines use.
    let n = pack.cells.len();
    assert!((15_000..=40_000).contains(&n), "{n} cells");
    for r in IDEALISED_REGIONS {
        let count = pack.cells.regions.iter().filter(|x| **x == r).count();
        assert!(count > 0, "{r:?} has no cells");
    }
    for name in LANDMARKS {
        let idx = pack
            .cells
            .landmarks
            .get(name)
            .unwrap_or_else(|| panic!("landmark {name} missing"));
        assert!((*idx as usize) < n);
    }
    assert_eq!(pack.cells.landmarks, pack.landmarks.points);
    assert_eq!(
        pack.cells.regions[pack.cells.landmarks["sa_node"] as usize],
        Region::SinoatrialNode
    );
    assert_eq!(
        pack.cells.regions[pack.cells.landmarks["av_node"] as usize],
        Region::AvNodeCompact
    );
    assert_eq!(
        pack.cells.regions[pack.cells.landmarks["his"] as usize],
        Region::HisBundle
    );
    assert_eq!(
        pack.cells.regions[pack.cells.landmarks["rv_apex"] as usize],
        Region::RightVentricle
    );
    assert_eq!(
        pack.cells.regions[pack.cells.landmarks["lv_septal"] as usize],
        Region::VentricularSeptum
    );
    for fascicle in ["rbb", "laf", "lpf"] {
        let seeds = &pack.landmarks.sets[fascicle];
        assert!(seeds.len() >= 20, "{fascicle}: {} PMJ seeds", seeds.len());
        assert!(
            seeds
                .iter()
                .all(|s| pack.cells.regions[*s as usize].is_ventricular())
        );
    }
    for (i, r) in pack.cells.regions.iter().enumerate() {
        if r.is_ventricular() {
            let d = pack.cells.depth[i];
            assert!((0.0..=1.0).contains(&d), "cell {i} depth {d}");
        }
        let f = pack.cells.fibres[i];
        assert!((f.length() - 1.0).abs() < 1e-3, "cell {i} fibre {f:?} not unit");
    }
    let spacing = pack.manifest.cell_spacing_mm;
    let mean = pack.cells.mean_nearest_distance_mm();
    assert!(
        (mean / spacing - 1.0).abs() < 0.25,
        "mean NN {mean} vs spacing {spacing}"
    );
    let mut seen = [false; 18];
    for (s, r) in pack.aha_segments.iter().zip(&pack.cells.regions) {
        if matches!(r, Region::LeftVentricle | Region::VentricularSeptum) {
            assert!((1..=17).contains(s));
            seen[usize::from(*s)] = true;
        } else if *r == Region::RightVentricle {
            assert!(*s == 0 || *s == RV_ANTERIOR_SEGMENT);
        } else {
            assert_eq!(*s, 0);
        }
    }
    assert!(seen[1..].iter().all(|b| *b), "AHA segments missing: {seen:?}");

    // orientation in the thorax: the measured pose (`procedural::heart`), real chambers
    let at = |name: &str| pack.cells.positions[pack.cells.landmarks[name] as usize];
    let lv_apex = at("lv_apex");
    assert!(
        lv_apex.y < -15.0 && lv_apex.x > 20.0 && lv_apex.z > 10.0,
        "LV apex inferior/left/anterior: {lv_apex:?}"
    );
    let sa = at("sa_node");
    assert!(sa.y > 20.0 && sa.x < 0.0, "SA node superior and right: {sa:?}");
    let antrum = at("la_pv_antrum");
    assert!(
        antrum.z < at("lv_lateral").z && antrum.z < sa.z,
        "left atrium posterior: {antrum:?}"
    );
    assert!(at("rv_free_wall").z > at("lv_lateral").z);
    assert!(at("rv_free_wall").x < at("lv_lateral").x);
    assert!(at("lv_posterior").z < at("lv_anterior").z);

    // hoods for every myocardial mesh, local and normalised
    assert_eq!(
        pack.hoods.len(),
        pack.meshes.iter().filter(|m| m.conducting()).count()
    );
    for m in pack.meshes.iter().filter(|m| m.conducting()) {
        let hoods = &pack.hoods[&m.name];
        assert_eq!(hoods.len(), m.mesh.vertex_count());
        for (v, hood) in m.mesh.vertices.iter().zip(hoods) {
            let p = Vec3::from(v.position);
            let sum: f32 = hood.weights.iter().sum();
            assert!((sum - 1.0).abs() < 1e-4, "{}: weights sum {sum}", m.name);
            let d = pack.cells.positions[hood.cells[0] as usize].distance(p);
            assert!(d <= 4.0 * spacing, "{}: nearest hood cell {d} mm away", m.name);
        }
        assert!(
            m.mesh.vertex_count() > 5000,
            "{} has {} vertices",
            m.name,
            m.mesh.vertex_count()
        );
    }

    // electrodes: the standard ten and the extra sites, outside the heart, in the right places
    assert!(pack.electrodes.has_standard_ten());
    for name in heartsim_geometry::electrodes::EXTRA_SITES {
        assert!(pack.electrodes.get(name).is_some(), "{name}");
    }
    let (lo, hi) = pack.cells.positions.iter().fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(l, h), p| (l.min(*p), h.max(*p)),
    );
    let e = &pack.electrodes;
    for el in Electrode::ALL {
        let p = e.standard(el).unwrap_or_else(|| panic!("{el:?} missing"));
        let inside = p.cmpge(lo).all() && p.cmple(hi).all();
        assert!(
            !inside,
            "{el:?} at {p:?} is inside the heart bounds {lo:?}..{hi:?}"
        );
    }
    assert!(e.standard(Electrode::V1).unwrap().z > hi.z);
    assert!(e.standard(Electrode::V2).unwrap().z > hi.z);
    assert!(e.standard(Electrode::LL).unwrap().y < lo.y);
    assert!(e.standard(Electrode::RL).unwrap().y < lo.y);
    assert!(e.standard(Electrode::V6).unwrap().x > hi.x);
    assert!(e.standard(Electrode::RA).unwrap().x < e.standard(Electrode::LA).unwrap().x);
}
