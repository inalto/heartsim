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

/// The default pack ships the human figure for the 3-D views; it must stand around the pack torso:
/// as tall as a person, its chest enclosing the electrode sites' surface.
#[test]
fn default_pack_has_a_registered_human_body() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/anatomy/default");
    let pack = AnatomyPack::load(&dir).expect("default pack");
    let body = pack.body.expect("assets/anatomy/default/human.glb");
    let (lo, hi) = body.bounds();
    assert!((hi.y - lo.y - heartsim_geometry::pack::BODY_HEIGHT_MM).abs() < 1.0);
    assert!(
        lo.y < -900.0 && hi.y > 300.0,
        "{lo} {hi}: heart level near the chest"
    );
    let (tlo, thi) = pack.torso.bounds();
    assert!(lo.x < tlo.x && hi.x > thi.x, "body is wider than the torso block");
}

mod body_regions {
    use super::*;
    use heartsim_geometry::pack::{
        BODY_ANKLES_MM, BODY_MIDLINE_X_MM, BODY_WRISTS_MM, BodyRegion, arm_boundary_mm, body_region,
    };

    fn body() -> heartsim_geometry::TriMesh {
        AnatomyPack::load(&default_pack_dir())
            .expect("default pack")
            .body
            .expect("human.glb")
    }

    /// The wrist and ankle markers sit on the figure's skin and are classified as their own limb.
    #[test]
    fn wrist_and_ankle_constants_are_on_their_limb() {
        let body = body();
        let cases = [
            (BODY_WRISTS_MM[0], BodyRegion::LeftArm),
            (BODY_WRISTS_MM[1], BodyRegion::RightArm),
            (BODY_ANKLES_MM[0], BodyRegion::LeftLeg),
            (BODY_ANKLES_MM[1], BodyRegion::RightLeg),
        ];
        for (p, region) in cases {
            let skin = body.closest_point(p).unwrap();
            assert!(
                skin.distance(p) < 2.0,
                "{p} is {} mm from the skin",
                skin.distance(p)
            );
            assert_eq!(body_region(p), region, "{p}");
        }
    }

    /// Every site of the electrode set, the ones that float off the skin included, is trunk.
    #[test]
    fn all_standard_sites_are_trunk() {
        let pack = AnatomyPack::load(&default_pack_dir()).expect("default pack");
        assert_eq!(pack.electrodes.sites.len(), 15);
        for (name, p) in &pack.electrodes.sites {
            assert_eq!(body_region(Vec3::from(*p)), BodyRegion::Trunk, "{name}");
        }
    }

    /// Between the armpit and the fingertips the boundary runs in the gap between arm and trunk
    /// (no skin within 10 mm of it); the legs start below the groin and keep to their side.
    #[test]
    fn arm_boundary_runs_in_the_gap_and_legs_keep_their_side() {
        let body = body();
        for v in &body.vertices {
            let p = Vec3::from(v.position);
            if (-480.0..=-100.0).contains(&p.y) {
                let d = (p.x - BODY_MIDLINE_X_MM).abs();
                assert!(
                    (d - arm_boundary_mm(p.y)).abs() >= 10.0,
                    "{p}: d {d} vs boundary {}",
                    arm_boundary_mm(p.y)
                );
            }
            if p.y < -560.0 {
                let want = if p.x > BODY_MIDLINE_X_MM {
                    BodyRegion::LeftLeg
                } else {
                    BodyRegion::RightLeg
                };
                assert_eq!(body_region(p), want, "{p}");
            }
        }
    }

    #[test]
    fn spot_checks() {
        let body = body();
        let pos = |v: &heartsim_geometry::Vertex| Vec3::from(v.position);
        let deltoid = body
            .vertices
            .iter()
            .map(pos)
            .filter(|p| (p.y - 100.0).abs() < 20.0)
            .max_by(|a, b| a.x.total_cmp(&b.x))
            .unwrap();
        assert_eq!(body_region(deltoid), BodyRegion::LeftArm, "{deltoid}");
        let thigh = body
            .vertices
            .iter()
            .map(pos)
            .find(|p| (p.y + 700.0).abs() < 3.0 && p.x < BODY_MIDLINE_X_MM)
            .unwrap();
        assert_eq!(body_region(thigh), BodyRegion::RightLeg, "{thigh}");
        let head = body
            .vertices
            .iter()
            .map(pos)
            .max_by(|a, b| a.y.total_cmp(&b.y))
            .unwrap();
        assert_eq!(body_region(head), BodyRegion::Trunk, "{head}");
    }
}

/// Brief B31: pack format 3 carries the two label volumes; their files match the manifest's sizes
/// and CRC-32s, they decode, and they hold what the echo cross-section expects.
#[test]
fn default_pack_has_its_label_volumes() {
    use heartsim_geometry::{Label, LabelVolume};
    let dir = default_pack_dir();
    let pack = AnatomyPack::load(&dir).expect("default pack");
    assert!(pack.manifest.format_version >= 3);
    let vols = pack
        .volumes
        .as_ref()
        .expect("format 3: the default pack has its label volumes");
    for (file, vol, spacing) in [
        ("labels_heart.bin", &vols.heart, 1.0f32),
        ("labels_thorax.bin", &vols.thorax, 2.0f32),
    ] {
        let bytes = std::fs::read(dir.join(file)).expect("volume file");
        let entry = pack.manifest.files[file];
        assert_eq!(bytes.len() as u64, entry.bytes, "{file}: size");
        assert_eq!(crc32fast::hash(&bytes), entry.crc32, "{file}: CRC-32");
        assert_eq!(&LabelVolume::decode(&bytes).expect("decodes"), vol);
        assert_eq!(vol.spacing_mm, spacing);
        eprintln!(
            "{file}: {} bytes, {}×{}×{} voxels of {} mm",
            bytes.len(),
            vol.dims[0],
            vol.dims[1],
            vol.dims[2],
            vol.spacing_mm
        );
    }
    for l in [
        Label::LvMyocardium,
        Label::RvMyocardium,
        Label::LaMyocardium,
        Label::RaMyocardium,
        Label::LvBlood,
        Label::RvBlood,
        Label::LaBlood,
        Label::RaBlood,
        Label::AortaWall,
        Label::AortaLumen,
        Label::PulmonaryArteryWall,
        Label::PulmonaryArteryLumen,
    ] {
        assert!(vols.heart.count(l) > 0, "heart box has no {}", l.name());
    }
    for l in [
        Label::Bone,
        Label::Lung,
        Label::Liver,
        Label::SoftTissue,
        Label::ThoracicBlood,
        Label::Outside,
    ] {
        assert!(vols.thorax.count(l) > 0, "thorax box has no {}", l.name());
    }
    // the designed labels are never stored
    for v in [&vols.heart, &vols.thorax] {
        assert!(
            v.labels
                .iter()
                .all(|b| !Label::from_u8(*b).expect("known").is_designed())
        );
    }
    // the heart sits inside the thorax box and the lookup outside both is "outside"
    let (lo, hi) = (vols.thorax.origin_mm, vols.thorax.max_mm());
    assert!(vols.heart.origin_mm.cmpgt(lo).all() && vols.heart.max_mm().cmplt(hi).all());
    assert_eq!(vols.thorax.label_at(hi + Vec3::splat(10.0)), Label::Outside);
}

/// A version-2 pack is refused with a message that says how to fix it.
#[test]
fn version_2_pack_is_refused_clearly() {
    let dir = std::env::temp_dir().join(format!("heartsim-pack-v2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut pack = AnatomyPack::procedural(Resolution::COARSE);
    pack.save(&dir).expect("save");
    let path = dir.join("manifest.ron");
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("format_version: 4", "format_version: 2");
    std::fs::write(&path, text).unwrap();
    let err = AnatomyPack::load(&dir).expect_err("a version-2 pack");
    let msg = err.to_string();
    assert!(matches!(err, heartsim_geometry::PackError::Version { found: 2 }));
    assert!(msg.contains("bake-anatomy") && msg.contains('3'), "{msg}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Label volumes round-trip through a pack, and a pack listing only one of the two is refused.
#[test]
fn label_volumes_round_trip_through_a_pack() {
    use heartsim_geometry::{Label, LabelVolume, LabelVolumes};
    let dir = std::env::temp_dir().join(format!("heartsim-pack-vol-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut pack = AnatomyPack::procedural(Resolution::COARSE);
    let mut heart = LabelVolume::new(Vec3::splat(-10.0), 1.0, [20, 20, 20], Label::Outside);
    heart.labels[4321] = Label::LvBlood as u8;
    let thorax = LabelVolume::new(Vec3::splat(-50.0), 2.0, [50, 50, 50], Label::SoftTissue);
    pack.volumes = Some(LabelVolumes { heart, thorax });
    pack.save(&dir).expect("save");
    let loaded = AnatomyPack::load(&dir).expect("load");
    assert_eq!(loaded.volumes, pack.volumes);
    // drop one file from the manifest
    let path = dir.join("manifest.ron");
    let manifest: heartsim_geometry::PackManifest =
        ron::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut m = manifest.clone();
    m.files.remove("labels_thorax.bin");
    std::fs::write(&path, ron::to_string(&m).unwrap()).unwrap();
    assert!(matches!(
        AnatomyPack::load(&dir),
        Err(heartsim_geometry::PackError::Format(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Brief B35: a version-3 pack (no valves) still loads, and draws as before (no valve); the
/// default pack is version 4 with the tricuspid valve, whose file matches the manifest, and its
/// LICENSE names the valve's source.
#[test]
fn version_3_packs_still_load_and_the_default_pack_has_its_valve() {
    let dir = std::env::temp_dir().join(format!("heartsim-pack-v3-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut pack = AnatomyPack::procedural(Resolution::COARSE);
    pack.save(&dir).expect("save");
    let path = dir.join("manifest.ron");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("valves:"), "a pack without valves lists none");
    std::fs::write(&path, text.replace("format_version: 4", "format_version: 3")).unwrap();
    let old = AnatomyPack::load(&dir).expect("a version-3 pack loads");
    assert_eq!(old.manifest.format_version, 3);
    assert_eq!(old.valves.len(), 0);
    let _ = std::fs::remove_dir_all(&dir);

    let def = AnatomyPack::load(&default_pack_dir()).expect("default pack");
    assert_eq!(def.manifest.format_version, 4);
    let v = def.valve("tricuspid").expect("the tricuspid valve");
    let e = &def.manifest.valves[0];
    assert_eq!(
        (e.name.as_str(), e.file.as_str()),
        ("tricuspid", "valve_tricuspid.bin")
    );
    let bytes = std::fs::read(default_pack_dir().join(&e.file)).unwrap();
    assert_eq!(def.manifest.files[&e.file].crc32, crc32fast::hash(&bytes));
    assert_eq!(&heartsim_geometry::AnimatedValve::decode(&bytes).unwrap(), v);
    for needle in [
        "Texas TriValve 1.1",
        "Mathur",
        "Rausch",
        "10.1007/s00366-022-01659-w",
        "Creative Commons Attribution 4.0",
        "decimated, fitted to another heart's annulus, re-timed",
    ] {
        assert!(def.licence.contains(needle), "LICENSE lacks `{needle}`");
    }
}

/// A valve round-trips through a pack; a damaged valve file is refused, not half-loaded.
#[test]
fn a_valve_round_trips_through_a_pack_and_a_damaged_one_is_refused() {
    let def = AnatomyPack::load(&default_pack_dir()).expect("default pack");
    let dir = std::env::temp_dir().join(format!("heartsim-pack-valve-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut pack = AnatomyPack::procedural(Resolution::COARSE);
    pack.valves = def.valves.clone();
    pack.save(&dir).expect("save");
    let loaded = AnatomyPack::load(&dir).expect("load");
    assert_eq!(loaded.valves, def.valves);
    // flip a byte and fix the manifest's checksum: the valve's own checks refuse it
    let file = dir.join("valve_tricuspid.bin");
    let mut bytes = std::fs::read(&file).unwrap();
    let n = bytes.len();
    bytes.truncate(n - 3);
    std::fs::write(&file, &bytes).unwrap();
    let path = dir.join("manifest.ron");
    let mut m: heartsim_geometry::PackManifest =
        ron::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    m.files.insert(
        "valve_tricuspid.bin".into(),
        heartsim_geometry::pack::FileEntry {
            bytes: bytes.len() as u64,
            crc32: crc32fast::hash(&bytes),
        },
    );
    std::fs::write(&path, ron::to_string(&m).unwrap()).unwrap();
    assert!(matches!(
        AnatomyPack::load(&dir),
        Err(heartsim_geometry::PackError::Valve { .. })
    ));
    let _ = std::fs::remove_dir_all(&dir);
}
