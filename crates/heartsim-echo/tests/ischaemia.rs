//! Brief B30 acceptance: an LAD lesion lowers the excursion of its AHA segments against the same
//! segments without it and leaves a remote segment within 10 %. Excursion is the endocardium's
//! inward motion at end-systole (what regional wall-motion scoring grades).

mod common;
use common::*;
use heartsim_echo::mechanical_injury;

/// The LAD territory of the AHA model (Cerqueira et al. 2002) and a remote (LCx/RCA) segment.
const LAD: [u8; 7] = [1, 2, 7, 8, 13, 14, 17];
const REMOTE: [u8; 2] = [5, 11];

#[test]
fn an_lad_lesion_lowers_its_segments_only() {
    let mut sim = Sim::from_scenario("stemi_anterior");
    sim.run_to(5000.0);
    let injury = mechanical_injury(sim.engine.eikonal(), &pack().aha_segments);
    let injured = injury.iter().filter(|&&m| m > 0.5).count();
    let mut with = model();
    with.set_injury(&injury);
    let mut without = model();
    let probe = LvProbe::new(&with);
    let w = beat_after(&sim, 2500.0);
    let es = w.a2.unwrap();
    let mut fa = with.new_frame();
    let mut fb = without.new_frame();
    eval_still(&mut with, &sim, es, &mut fa);
    eval_still(&mut without, &sim, es, &mut fb);
    let mut lad_ok = true;
    for s in LAD {
        let (a, b) = (probe.inward_mm(&fa, &[s]), probe.inward_mm(&fb, &[s]));
        eprintln!("segment {s:2} (LAD): {a:.2} mm with the lesion, {b:.2} mm without");
        lad_ok &= a < b;
    }
    let (ra, rb) = (probe.inward_mm(&fa, &REMOTE), probe.inward_mm(&fb, &REMOTE));
    let (la, lb) = (probe.inward_mm(&fa, &LAD), probe.inward_mm(&fb, &LAD));
    eprintln!(
        "{injured} cells injured > 0.5; EF factor {:?}; LAD segments {la:.2} vs {lb:.2} mm ({:+.0} %); remote 5+11 {ra:.2} vs {rb:.2} mm ({:+.1} %); EF {:.3} vs {:.3}",
        with.ef_scale(),
        (la / lb - 1.0) * 100.0,
        (ra / rb - 1.0) * 100.0,
        fa.readout.lvef.unwrap_or(0.0),
        fb.readout.lvef.unwrap_or(0.0)
    );
    let (faults, bad, _) = mesh_faults(&with, &fa);
    for part in &fa.parts {
        let one = heartsim_echo::MotionFrame {
            parts: vec![part.clone()],
            readout: fa.readout,
        };
        let n = mesh_faults(&with, &one).0;
        if n > 0 {
            eprintln!("  {n} inverted in {}", part.name);
        }
    }
    assert!(injured > 500, "the LAD lesion injures its territory");
    assert!(lad_ok && la < 0.8 * lb, "LAD segments move less");
    assert!((ra / rb - 1.0).abs() <= 0.10, "remote segment within 10 %");
    assert_eq!((faults, bad), (0, 0));
}
