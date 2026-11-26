//! Brief B36, the generator on synthetic discs (no pack): the designed mitral valve and the
//! designed semilunar cusps keep their designed dimensions, close competently, open to their
//! stated orifice, move without a snap and never cross; the same input gives the same bytes.

use glam::Vec3;
use heartsim_geometry::AnimatedValve;
use heartsim_geometry::valve_design::{
    CuspDesign, DesignedValve, DiscFrame, MitralDesign, SHAPES, checks, mitral, semilunar,
};

/// A flat circular disc of radius `r` in the plane z = 0, its normal toward `downstream`.
fn disc(r: f32, downstream: Vec3) -> DiscFrame {
    let mut pts = Vec::new();
    for ring in 1..=4 {
        let rr = r * ring as f32 / 4.0;
        for k in 0..180 {
            let a = std::f32::consts::TAU * k as f32 / 180.0;
            pts.push(Vec3::new(rr * a.cos(), rr * a.sin(), 0.0));
        }
    }
    DiscFrame::new(&pts, downstream).expect("disc")
}

fn mitral_disc() -> DiscFrame {
    disc(14.0, Vec3::new(0.0, 0.0, -10.0))
}

fn make_mitral() -> DesignedValve {
    mitral(
        &mitral_disc(),
        Vec3::X,
        Vec3::Y,
        [Vec3::new(2.0, 9.0, -21.0), Vec3::new(2.0, -9.0, -23.0)],
        &MitralDesign::default(),
        None,
    )
    .expect("mitral")
}

fn aortic_disc() -> DiscFrame {
    disc(12.1, Vec3::new(0.0, 0.0, 10.0))
}

fn make_aortic() -> DesignedValve {
    semilunar(
        "aortic",
        ["non-coronary", "left coronary", "right coronary"],
        &aortic_disc(),
        Vec3::X,
        &CuspDesign::default(),
        None,
    )
    .expect("aortic")
}

fn shape(v: &AnimatedValve, k: usize) -> Vec<Vec3> {
    let mut p = Vec::new();
    v.keyframe_positions(k, &mut p);
    p
}

fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
    (a + ab * t).distance(p)
}

fn rim_distance(rim: &[Vec3], p: Vec3) -> f32 {
    (0..rim.len())
        .map(|b| segment_distance(p, rim[b], rim[(b + 1) % rim.len()]))
        .fold(f32::INFINITY, f32::min)
}

/// Along-surface length of a column (annulus to free edge) in `pos`.
fn column_length(pos: &[Vec3], col: usize, rows: usize) -> f32 {
    let per = rows + 1;
    (0..rows)
        .map(|r| pos[col * per + r].distance(pos[col * per + r + 1]))
        .sum()
}

#[test]
fn the_same_input_gives_the_same_bytes() {
    assert_eq!(make_mitral().valve.encode(), make_mitral().valve.encode());
    assert_eq!(make_aortic().valve.encode(), make_aortic().valve.encode());
    // and the record round-trips
    let v = make_mitral().valve;
    assert_eq!(AnimatedValve::decode(&v.encode()).expect("decodes"), v);
}

#[test]
fn the_mitral_leaflets_have_their_designed_lengths_ids_and_annulus() {
    let d = make_mitral();
    for l in &d.report {
        eprintln!("{l}");
    }
    let v = &d.valve;
    let p = MitralDesign::default();
    assert_eq!(v.leaflet_names, ["anterior", "posterior"]);
    assert_eq!(v.triangle_leaflet.len(), v.triangles.len());
    assert!(v.triangle_leaflet.iter().all(|l| *l < 2));
    assert!(
        (2000..=3000).contains(&v.triangles.len()),
        "{} triangles",
        v.triangles.len()
    );
    assert!(v.keyframe_count() <= 8 && v.keyframe_count() == SHAPES);
    let disc = mitral_disc();
    // the anterior leaflet's centre column (toward +x) and the posterior's (toward −x)
    let col_toward = |dir: Vec3| -> usize {
        (0..p.columns)
            .max_by(|a, b| {
                let pa = v.rest_mm[a * (p.rows + 1)];
                let pb = v.rest_mm[b * (p.rows + 1)];
                pa.dot(dir).total_cmp(&pb.dot(dir))
            })
            .unwrap()
    };
    let (ca, cp) = (col_toward(Vec3::X), col_toward(-Vec3::X));
    for k in [0, SHAPES - 1] {
        let pos = shape(v, k);
        let (la, lp) = (column_length(&pos, ca, p.rows), column_length(&pos, cp, p.rows));
        eprintln!("shape {k}: anterior {la:.2} mm, posterior (P2) {lp:.2} mm");
        assert!((la - p.anterior_length_mm).abs() <= 0.5, "anterior {la}");
        assert!((lp - p.posterior_length_mm).abs() <= 0.5, "posterior {lp}");
    }
    // the annulus on the rim, at every shape
    for k in 0..v.keyframe_count() {
        let pos = shape(v, k);
        let w = v
            .annulus
            .iter()
            .map(|&a| rim_distance(&disc.rim_mm, pos[a as usize]))
            .fold(0.0f32, f32::max);
        assert!(w <= 0.1, "shape {k}: annulus {w} mm off the rim");
    }
    let areas = v.leaflet_areas_mm2(&shape(v, SHAPES - 1));
    eprintln!(
        "shut leaflet areas: anterior {:.0} mm², posterior {:.0} mm², annulus {:.0} mm²",
        areas[0],
        areas[1],
        disc.area_mm2()
    );
    assert_eq!(v.chordae.len(), 12);
    assert!(
        v.chordae
            .iter()
            .all(|c| c[1] < v.leaflet_vertices && c[0] >= v.leaflet_vertices)
    );
}

#[test]
fn the_shut_mitral_leaflets_never_rise_above_the_annulus_plane() {
    let v = make_mitral().valve;
    let n = mitral_disc().normal();
    let pos = shape(&v, SHAPES - 1);
    let per = MitralDesign::default().rows + 1;
    let worst = (0..v.leaflet_vertices as usize)
        .map(|i| -(pos[i] - pos[i / per * per]).dot(n))
        .fold(f32::NEG_INFINITY, f32::max);
    eprintln!("highest shut vertex above its hinge: {worst:.3} mm");
    assert!(worst <= 1e-3, "{worst}");
}

#[test]
fn the_aortic_cusps_have_their_designed_heights_ids_and_crown() {
    let d = make_aortic();
    for l in &d.report {
        eprintln!("{l}");
    }
    let v = &d.valve;
    let p = CuspDesign::default();
    let disc = aortic_disc();
    let n = disc.normal();
    let r = disc.mean_radius_mm();
    assert!(v.triangle_leaflet.iter().all(|l| *l < 3));
    // the cusps are coarser than the mitral leaflets (the echo's cost gate; CuspDesign::columns)
    assert!(
        (1000..=3000).contains(&v.triangles.len()),
        "{} triangles",
        v.triangles.len()
    );
    let per = p.rows + 1;
    let shut = shape(v, SHAPES - 1);
    let gh = (r * r + p.effective_height_mm * p.effective_height_mm).sqrt();
    for k in 0..3 {
        let col = k * p.columns + p.columns / 2;
        let nadir = shut[col * per];
        let mid = shut[col * per + p.rows];
        let eff = (mid - disc.centre()).dot(n);
        let geo = nadir.distance(mid);
        eprintln!(
            "cusp {k}: effective height {eff:.2} mm, geometric height {geo:.2} mm (straight; along the cusp {:.2} mm)",
            column_length(&shut, col, p.rows)
        );
        assert!((eff - p.effective_height_mm).abs() <= 0.5, "effective {eff}");
        assert!((geo - gh).abs() <= 0.5, "geometric {geo} vs {gh}");
    }
    // the attachment: on the rim in projection, at the crown's height
    let hc = p.commissure_ratio * gh;
    let mut worst = 0.0f32;
    for &a in &v.annulus {
        let q = v.rest_mm[a as usize];
        let h = (q - disc.centre()).dot(n);
        let flat = q - n * h;
        worst = worst.max(rim_distance(&disc.rim_mm, flat));
        assert!(h >= -1e-4 && h <= hc + 1e-3, "attachment height {h}");
    }
    assert!(worst <= 0.1, "attachment {worst} mm off the rim in projection");
    let areas = v.leaflet_areas_mm2(&shut);
    eprintln!("shut cusp areas {areas:.0?} mm²");
    assert_eq!(v.chordae, Vec::<[u32; 2]>::new(), "cusps have no chordae");
}

fn motion_checks(v: &AnimatedValve, thickness: f32, label: &str) {
    // closures strictly increasing, the last one shut
    let mut prev = 0.0;
    for k in &v.keyframes {
        assert!(k.closure > prev);
        prev = k.closure;
    }
    assert!((prev - 1.0).abs() < 1e-6);
    // the ends bit for bit
    let mut a = Vec::new();
    v.pose_rest(1.0, &mut a);
    assert_eq!(a, shape(v, 0), "{label}: opening 1");
    v.pose_rest(0.0, &mut a);
    assert_eq!(a, shape(v, v.keyframe_count() - 1), "{label}: opening 0");
    let (fall, fall_at) = checks::closure_metric_largest_fall(v, 1000);
    let (share, at) = checks::largest_step_share(v);
    eprintln!(
        "{label}: closure metric largest fall {fall:.5} mm (at opening {fall_at:.3}; quantum {:.5} mm); largest \
         step share {share:.3} (vertex {at})",
        v.quantum_mm
    );
    assert!(fall <= 1e-4, "{label}: the closure is not monotone");
    assert!(
        share <= 0.25,
        "{label}: a keyframe step takes {share} of a vertex's path"
    );
    let mut worst = (0.0f32, 0.0f32);
    for k in 0..=1000 {
        let o = k as f32 / 1000.0;
        v.pose_rest(o, &mut a);
        let d = checks::deepest_crossing_mm(v, &a);
        if d > worst.0 {
            worst = (d, o);
        }
    }
    eprintln!(
        "{label}: deepest crossing {:.3} mm at opening {:.3} (thickness {thickness} mm)",
        worst.0, worst.1
    );
    assert!(worst.0 <= thickness);
}

#[test]
fn the_mitral_valve_moves_without_a_snap_and_never_crosses() {
    motion_checks(&make_mitral().valve, 1.6, "mitral");
}

#[test]
fn the_aortic_valve_moves_without_a_snap_and_never_crosses() {
    motion_checks(&make_aortic().valve, 1.0, "aortic");
}

#[test]
fn shut_the_valves_are_competent_and_open_they_leave_their_orifice() {
    let m = make_mitral().valve;
    let (gap, at) = checks::free_edge_gap_mm(&m, &shape(&m, SHAPES - 1));
    let open = checks::orifice_fraction(&m, &shape(&m, 0), &mitral_disc());
    eprintln!(
        "mitral: largest shut free-edge gap {gap:.3} mm (vertex {at}); open orifice {open:.3} of the disc"
    );
    assert!(gap < 1.0);
    // stated fraction: at least 0.5 of the disc (Designed; no source for the open mitral orifice
    // against the annulus was read)
    assert!(open >= 0.5, "{open}");
    let a = make_aortic().valve;
    let (gap, at) = checks::free_edge_gap_mm(&a, &shape(&a, SHAPES - 1));
    let open = checks::orifice_fraction(&a, &shape(&a, 0), &aortic_disc());
    eprintln!(
        "aortic: largest shut free-edge gap {gap:.3} mm (vertex {at}); open orifice {open:.3} of the disc"
    );
    assert!(gap < 1.0);
    // stated fraction: CuspDesign::open_orifice_fraction, 0.75 (Designed, research note A14)
    assert!(
        open >= CuspDesign::default().open_orifice_fraction - 0.01,
        "{open}"
    );
}
