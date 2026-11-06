//! B22 acceptance tests of the catheter models on the default anatomy pack.

use glam::Vec3;
use heartsim_core::Region;
use heartsim_geometry::catheter::CONTACT_MM;
use heartsim_geometry::{AnatomyPack, CardiacChamber, CatheterAnatomy, CatheterKind, CatheterSite};
use std::path::Path;
use std::sync::OnceLock;

fn pack() -> &'static AnatomyPack {
    static PACK: OnceLock<AnatomyPack> = OnceLock::new();
    PACK.get_or_init(|| {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/anatomy/default");
        AnatomyPack::load(&dir).expect("default pack loads")
    })
}

fn anatomy() -> &'static CatheterAnatomy {
    static A: OnceLock<CatheterAnatomy> = OnceLock::new();
    A.get_or_init(|| CatheterAnatomy::from_cloud(&pack().cells))
}

#[test]
fn every_chamber_has_a_cavity_with_its_centre_inside() {
    for chamber in [
        CardiacChamber::RightAtrium,
        CardiacChamber::LeftAtrium,
        CardiacChamber::RightVentricle,
        CardiacChamber::LeftVentricle,
    ] {
        let c = anatomy().cavity(chamber);
        assert!(c.len() > 20, "{chamber:?}: {} voxels", c.len());
        assert!(c.contains(c.centre()), "{chamber:?}");
    }
}

#[test]
fn cavities_do_not_overlap_myocardium_or_each_other() {
    let cells = &pack().cells;
    let ra = anatomy().cavity(CardiacChamber::RightAtrium);
    let rv = anatomy().cavity(CardiacChamber::RightVentricle);
    assert!(!ra.contains(rv.centre()), "RV centre counted as RA");
    assert!(!rv.contains(ra.centre()), "RA centre counted as RV");
    let lv = anatomy().cavity(CardiacChamber::LeftVentricle);
    assert!(
        !rv.contains(lv.centre()),
        "LV cavity counted as RV: the crescent test failed"
    );
    let inside_wall = cells
        .positions
        .iter()
        .zip(&cells.regions)
        .filter(|(p, r)| **r == Region::RightVentricle && rv.contains(**p))
        .count();
    assert_eq!(inside_wall, 0, "RV wall cells inside the RV cavity");
}

#[test]
fn the_standard_set_has_the_usual_bipoles_in_their_chambers() {
    let set = anatomy().standard_set();
    assert_eq!(set.len(), 5);
    let count = |k: CatheterKind| set.iter().find(|c| c.kind == k).unwrap().pair_count();
    assert_eq!(count(CatheterKind::Hra), 2);
    assert_eq!(count(CatheterKind::His), 3, "His 1-3 of F-16");
    assert_eq!(count(CatheterKind::Cs), 5, "CS 1-10 of F-16");
    assert_eq!(count(CatheterKind::Rva), 2);
    assert_eq!(count(CatheterKind::Abl), 2);
    let cs = set.iter().find(|c| c.kind == CatheterKind::Cs).unwrap();
    assert_eq!(cs.pair_label(0), "CS 1-2");
    assert_eq!(cs.pair_label(4), "CS 9-10");

    for (kind, chamber) in [
        (CatheterKind::Hra, CardiacChamber::RightAtrium),
        (CatheterKind::His, CardiacChamber::RightAtrium),
        (CatheterKind::Rva, CardiacChamber::RightVentricle),
    ] {
        let c = set.iter().find(|c| c.kind == kind).unwrap();
        let d = anatomy().cavity(chamber).distance(c.tip());
        assert!(
            d <= CONTACT_MM + 2.0,
            "{kind:?} tip {d} mm from the {chamber:?} cavity"
        );
    }
    // the electrode spacing of the body is kept: 2 mm within a bipole
    let hra = set.iter().find(|c| c.kind == CatheterKind::Hra).unwrap();
    let spacing = hra.electrodes[0].distance(hra.electrodes[1]);
    assert!((spacing - 2.0).abs() < 0.2, "{spacing}");
}

#[test]
fn a_tip_dragged_outside_the_heart_stays_in_its_chamber() {
    let a = anatomy();
    for target in [
        Vec3::new(200.0, 0.0, 0.0),
        Vec3::new(-200.0, 100.0, -50.0),
        Vec3::new(0.0, -300.0, 100.0),
    ] {
        let c = a.place(CatheterKind::Rva, &CatheterSite::Point(target.to_array()));
        let d = a.cavity(CardiacChamber::RightVentricle).distance(c.tip());
        assert!(d <= CONTACT_MM + 1e-3, "{target}: tip {d} mm out");
    }
}

#[test]
fn the_cs_catheter_slides_along_its_vessel_with_the_distal_pair_deepest() {
    let a = anatomy();
    let full = a.cs_length_mm();
    assert!(full > 30.0, "CS {full} mm");
    let deep = a.place(CatheterKind::Cs, &CatheterSite::Default);
    let halfway = a.place(CatheterKind::Cs, &CatheterSite::CsDepth(full * 0.5));
    let cells = &pack().cells;
    let (os, distal) = (
        cells.positions[cells.landmarks["cs_os"] as usize],
        cells.positions[cells.landmarks["cs_distal"] as usize],
    );
    assert!(
        deep.tip().distance(distal) < 1.0,
        "fully in: tip at the distal end"
    );
    assert!(deep.pair_midpoint(4).distance(os) < deep.pair_midpoint(0).distance(os));
    assert!(halfway.tip().distance(distal) > full * 0.3, "withdrawn");
    // dragging: a point near the vessel gives the depth of its nearest rail point
    let depth = a.cs_depth_of(distal + Vec3::X);
    assert!((depth - full).abs() < 3.0, "{depth} of {full}");
}

#[test]
fn the_ablation_catheter_goes_to_the_chamber_of_the_tissue_it_touches() {
    let a = anatomy();
    let cells = &pack().cells;
    for (landmark, chamber) in [
        ("ra_lateral", CardiacChamber::RightAtrium),
        ("rv_free_wall", CardiacChamber::RightVentricle),
        ("la_roof", CardiacChamber::LeftAtrium),
        ("lv_lateral", CardiacChamber::LeftVentricle),
    ] {
        let p = cells.positions[cells.landmarks[landmark] as usize];
        let c = a.place(CatheterKind::Abl, &CatheterSite::Landmark(landmark.into()));
        assert!(
            a.cavity(chamber).distance(c.tip()) <= CONTACT_MM + 1e-3,
            "{landmark}: tip outside the {chamber:?}"
        );
        assert!(
            c.tip().distance(p) < 12.0,
            "{landmark}: tip {} mm away",
            c.tip().distance(p)
        );
    }
}
