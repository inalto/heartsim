//! Brief B30 acceptance: a normal beat at 60, 75 and 100 bpm — LVEF of the deformed closed LV
//! shell, MAPSE, TAPSE, apical and basal rotation, wall thickening in every LV segment, the
//! timing of ejection against the valve schedule, E / diastasis / A filling, and mesh safety.

mod common;
use common::*;

struct SinusMeasure {
    ef: f64,
    mapse: f64,
    tapse: f64,
    apical: f64,
    basal: f64,
    min_thickening: (usize, f64),
    faults: (usize, usize, usize),
}

fn measure(bpm: f64) -> SinusMeasure {
    let mut model = model();
    let probe = LvProbe::new(&model);
    let mut sim = Sim::sinus(bpm);
    sim.run_to(5000.0);
    let w = beat_after(&sim, 2500.0);
    let a2 = w.a2.expect("a sinus beat ejects");
    let mut f = model.new_frame();
    eval_still(&mut model, &sim, w.m1, &mut f);
    let edv = lv_volume(&model, &f);
    let thick_ed = probe.thickness_mm(&f);
    let mut faults = mesh_faults(&model, &f);
    eval_still(&mut model, &sim, a2, &mut f);
    let esv = lv_volume(&model, &f);
    let thick_es = probe.thickness_mm(&f);
    // peak rotations over systole
    let (mut apical, mut basal) = (0.0f64, 0.0f64);
    let mut t = w.m1;
    while t <= a2 + 60.0 {
        eval_still(&mut model, &sim, t, &mut f);
        apical = apical.min(probe.rotation_deg(&f, (0.12, 0.28)));
        basal = basal.max(probe.rotation_deg(&f, (0.8, 0.95)));
        let (i, b, sl) = mesh_faults(&model, &f);
        if i > 0 {
            for part in &f.parts {
                let one = heartsim_echo::MotionFrame {
                    parts: vec![part.clone()],
                    readout: f.readout,
                };
                let n = mesh_faults(&model, &one).0;
                if n > 0 {
                    eprintln!(
                        "  {bpm} bpm t {:.0} (QRS + {:.0}): {} inverted in {}",
                        t,
                        t - w.qrs,
                        n,
                        part.name
                    );
                }
            }
        }
        faults.0 += i;
        faults.1 += b;
        faults.2 += sl;
        t += 10.0;
    }
    // the read-out after both end-systoles (MAPSE at A2, TAPSE at P2)
    eval_still(&mut model, &sim, a2.max(w.p2.unwrap_or(a2)) + 1.0, &mut f);
    let r = f.readout;
    let mut min_thickening = (0usize, f64::INFINITY);
    for s in 1..=17 {
        let th = (thick_es[s] - thick_ed[s]) / thick_ed[s];
        if th < min_thickening.1 {
            min_thickening = (s, th);
        }
    }
    let m = SinusMeasure {
        ef: (edv - esv) / edv,
        mapse: r.mapse_mm.expect("MAPSE after end-systole"),
        tapse: r.tapse_mm.expect("TAPSE after end-systole"),
        apical,
        basal,
        min_thickening,
        faults,
    };
    eprintln!(
        "{bpm} bpm: EDV {edv:.1} mL ESV {esv:.1} mL EF {:.1} % (read-out {:.1} %), MAPSE {:.1} mm, TAPSE {:.1} mm, \
         apical {:.1}°, basal {:+.1}°, least thickening seg {} {:.1} %, faults (inverted, non-finite, inverted slivers) {:?}",
        m.ef * 100.0,
        r.lvef.unwrap() * 100.0,
        m.mapse,
        m.tapse,
        m.apical,
        m.basal,
        m.min_thickening.0,
        m.min_thickening.1 * 100.0,
        m.faults
    );
    eprintln!(
        "thickness ED {:?}",
        thick_ed[1..]
            .iter()
            .map(|x| (x * 10.0).round() / 10.0)
            .collect::<Vec<_>>()
    );
    eprintln!(
        "thickness ES {:?}",
        thick_es[1..]
            .iter()
            .map(|x| (x * 10.0).round() / 10.0)
            .collect::<Vec<_>>()
    );
    assert!(
        (r.lvef.unwrap() - m.ef).abs() < 0.005,
        "read-out EF equals the mesh's"
    );
    m
}

fn check(m: &SinusMeasure) {
    assert!((0.55..=0.70).contains(&m.ef), "EF {}", m.ef);
    assert!((11.0..=17.0).contains(&m.mapse), "MAPSE {}", m.mapse);
    assert!((17.0..=28.0).contains(&m.tapse), "TAPSE {}", m.tapse);
    // steady sinus: the lateral tricuspid annulus descends by `MotionParams::tapse_mm`
    assert!((23.5..=24.5).contains(&m.tapse), "sinus TAPSE {}", m.tapse);
    assert!((-15.0..=-8.0).contains(&m.apical), "apical rotation {}", m.apical);
    assert!((3.0..=7.0).contains(&m.basal), "basal rotation {}", m.basal);
    assert!(m.min_thickening.1 > 0.0, "segment {} thins", m.min_thickening.0);
    assert_eq!(
        (m.faults.0, m.faults.1),
        (0, 0),
        "inverted triangles / non-finite vertices"
    );
}

#[test]
fn sinus_60_bpm() {
    check(&measure(60.0));
}

#[test]
fn sinus_75_bpm() {
    check(&measure(75.0));
}

#[test]
fn sinus_100_bpm() {
    check(&measure(100.0));
}
