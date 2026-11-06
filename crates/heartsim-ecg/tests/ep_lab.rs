//! B22 acceptance tests on the electrograms: the coronary-sinus activation sequence in sinus rhythm
//! (proximal to distal) and in left-lateral AVRT (distal first), and AH and HV in the normal ranges.
//!
//! Normal baseline ranges: AH 50–120 ms, HV 35–55 ms (Josephson, *Clinical Cardiac Electrophysiology*;
//! published AH upper limits run from about 105 to 140 ms, HV 35–55 ms is agreed).

use heartsim_core::{Millis, Region};
use heartsim_ecg::{EgmOptions, EgmRecorder, EgmSource, Tier0Scenario};
use heartsim_geometry::{AnatomyPack, Catheter, CatheterAnatomy, CatheterKind};
use heartsim_sim::ep::{AblationTarget, EpLab, EpPreset, Protocol, ProtocolStatus};
use heartsim_sim::{Engine, Event, EventKind, Origin};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn pack() -> &'static AnatomyPack {
    static PACK: OnceLock<AnatomyPack> = OnceLock::new();
    PACK.get_or_init(|| AnatomyPack::load(&assets().join("anatomy/default")).expect("default pack loads"))
}

struct Study {
    lab: EpLab,
    catheters: Vec<Catheter>,
    rec: EgmRecorder,
    events: Vec<Event>,
}

impl Study {
    fn new(id: &str) -> Self {
        let text = std::fs::read_to_string(assets().join(format!("scenarios/ep/{id}.ron"))).unwrap();
        let preset = EpPreset::from_ron(&text).unwrap().expect("ep section");
        let lab = preset.build(
            Arc::new(pack().cells.clone()),
            &pack().landmarks.sets,
            &pack().aha_segments,
        );
        let catheters = CatheterAnatomy::from_cloud(&pack().cells).standard_set();
        let rec = EgmRecorder::new(&pack().cells, &catheters, EgmOptions::default());
        Self {
            lab,
            catheters,
            rec,
            events: Vec::new(),
        }
    }

    fn run(&mut self, ms: f64) {
        let end = self.lab.now().0 + ms;
        while self.lab.now().0 + 0.5 < end {
            self.lab.step(Millis(1.0));
            self.events.extend(self.lab.drain_events());
            self.rec.push(&self.lab.cells().vm, self.lab.now().0);
        }
    }

    fn catheter(&self, kind: CatheterKind) -> usize {
        self.catheters.iter().position(|c| c.kind == kind).unwrap()
    }

    /// CS atrial sequence of the atrial activation starting at `t0`, distal pair first.
    fn cs_sequence(&self, t0: f64, window_ms: f64) -> Vec<f64> {
        let seq = self.rec.activation_sequence(
            self.catheter(CatheterKind::Cs),
            EgmSource::Atrial,
            t0,
            t0 + window_ms,
        );
        assert_eq!(seq.len(), 5);
        seq.iter()
            .enumerate()
            .map(|(k, t)| t.unwrap_or_else(|| panic!("CS pair {k}: no A in {t0}+{window_ms}: {seq:?}")))
            .collect()
    }
}

#[test]
fn the_cs_activates_proximal_to_distal_in_sinus_rhythm() {
    let mut s = Study::new("ep_baseline");
    s.run(3000.0);
    let beats: Vec<f64> = s
        .events
        .iter()
        .filter(|e| {
            e.origin == Origin::Sinus && e.region == Region::RightAtrium && e.kind == EventKind::Automatic
        })
        .map(|e| e.at.0)
        .collect();
    assert!(beats.len() >= 3, "{beats:?}");
    for &t0 in &beats[1..3] {
        let seq = s.cs_sequence(t0, 200.0);
        eprintln!(
            "sinus at {t0}: CS 1-2 … 9-10 at {:?}",
            seq.iter().map(|t| t - t0).collect::<Vec<_>>()
        );
        assert!(seq[4] < seq[0], "proximal {} after distal {}", seq[4], seq[0]);
        for w in seq.windows(2) {
            assert!(w[1] <= w[0] + 2.0, "not proximal to distal: {seq:?}");
        }
    }
}

#[test]
fn the_cs_activates_distal_first_in_left_lateral_avrt() {
    let mut s = Study::new("ep_ap_left_lateral");
    let hra = s
        .lab
        .capture_cell(pack().cells.positions[pack().cells.landmarks["sa_node"] as usize])
        .unwrap();
    s.run(1000.0);
    s.lab.start_protocol(
        hra,
        Protocol::Train {
            s1_cycle_ms: 600.0,
            s1_count: 8,
            s2_ms: Some(280.0),
            s3_ms: None,
        },
    );
    while s.lab.protocol_status() != ProtocolStatus::Finished {
        s.run(1.0);
    }
    s.events.clear();
    s.run(3000.0);
    // retrograde atrial activations: over the accessory pathway, sustained
    let retro: Vec<f64> = s
        .events
        .iter()
        .filter(|e| e.origin == Origin::AccessoryPathway && e.chamber == heartsim_sim::Chamber::Atrium)
        .map(|e| e.at.0)
        .collect();
    assert!(retro.len() >= 5, "AVRT not sustained: {retro:?}");
    for &t0 in &retro[retro.len() - 3..retro.len() - 1] {
        let seq = s.cs_sequence(t0 - 5.0, 120.0);
        eprintln!(
            "AVRT at {t0}: CS 1-2 … 9-10 at {:?}",
            seq.iter().map(|t| t - t0).collect::<Vec<_>>()
        );
        assert!(seq[0] < seq[4], "distal {} after proximal {}", seq[0], seq[4]);
        let earliest = seq
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.total_cmp(b.1))
            .map(|(k, _)| k)
            .unwrap();
        assert!(earliest <= 1, "earliest at CS pair {earliest}: {seq:?}");
    }
}

#[test]
fn after_ablating_the_pathway_the_eccentric_sequence_is_gone() {
    let mut s = Study::new("ep_ap_left_lateral");
    s.lab.ablate_target(AblationTarget::AccessoryPathway);
    s.run(3000.0);
    assert!(
        s.events.iter().all(|e| e.origin != Origin::AccessoryPathway),
        "the pathway still conducts"
    );
}

#[test]
fn ah_and_hv_are_in_the_normal_ranges_in_sinus_rhythm() {
    let mut s = Study::new("ep_baseline");
    s.run(4000.0);
    let i = s
        .rec
        .conduction_intervals()
        .expect("A, H and V on the His catheter");
    eprintln!("{i:?}");
    assert!((50.0..=120.0).contains(&i.ah_ms), "AH {}", i.ah_ms);
    assert!((35.0..=55.0).contains(&i.hv_ms), "HV {}", i.hv_ms);
}

#[test]
fn the_intervals_agree_with_the_timing_engine() {
    let mut s = Study::new("ep_baseline");
    s.run(4000.0);
    let i = s.rec.conduction_intervals().unwrap();
    let his = s
        .events
        .iter()
        .rev()
        .find(|e| e.region == Region::HisBundle && e.kind == EventKind::Conducted)
        .unwrap();
    assert!(
        (i.h_ms - his.at.0).abs() <= 12.0,
        "H on the catheter at {}, His event at {}",
        i.h_ms,
        his.at.0
    );
}

#[test]
fn the_his_catheter_records_a_h_and_v_and_the_hra_only_a() {
    let mut s = Study::new("ep_baseline");
    s.run(3000.0);
    let source_on = |s: &Study, kind: CatheterKind, source: EgmSource| {
        s.rec
            .deflections()
            .filter(|d| d.source == source && s.rec.channels()[d.channel].kind == kind)
            .count()
    };
    assert!(source_on(&s, CatheterKind::His, EgmSource::Atrial) > 0);
    assert!(source_on(&s, CatheterKind::His, EgmSource::His) > 0);
    assert!(source_on(&s, CatheterKind::His, EgmSource::Ventricular) > 0);
    assert!(source_on(&s, CatheterKind::Hra, EgmSource::Atrial) > 0);
    assert_eq!(source_on(&s, CatheterKind::Hra, EgmSource::His), 0);
    assert_eq!(source_on(&s, CatheterKind::Hra, EgmSource::Ventricular), 0);
    assert!(source_on(&s, CatheterKind::Rva, EgmSource::Ventricular) > 0);
}

#[test]
fn bipolar_amplitudes_are_plausible_for_healthy_tissue_and_fall_on_a_lesion() {
    let amplitude = |s: &Study, c: usize| {
        s.rec
            .deflections()
            .filter(|d| d.channel == c && d.source == EgmSource::Atrial)
            .map(|d| d.amplitude_mv)
            .fold(0.0f32, f32::max)
    };
    let mut healthy = Study::new("ep_baseline");
    let hra = healthy
        .rec
        .channels()
        .iter()
        .position(|c| c.kind == CatheterKind::Hra)
        .unwrap();
    healthy.run(3000.0);
    let before = amplitude(&healthy, hra);
    assert!((0.2..=30.0).contains(&before), "HRA bipole {before} mV");

    let mut ablated = Study::new("ep_baseline");
    let site = ablated.catheters[0].pair_midpoint(0);
    ablated.lab.ablate_now(site, 8.0);
    ablated.run(3000.0);
    let after = amplitude(&ablated, hra);
    assert!(
        after < 0.7 * before,
        "{after} mV over a lesion, {before} mV before"
    );
}

#[test]
fn recording_does_not_change_the_rhythm_and_is_deterministic() {
    let mut a = Study::new("ep_baseline");
    a.run(2000.0);
    let mut b = Study::new("ep_baseline");
    b.run(2000.0);
    assert_eq!(a.events, b.events);
    for c in 0..a.rec.channels().len() {
        let n = a.rec.bipolar(c).pushed();
        for k in 0..n {
            assert_eq!(a.rec.bipolar(c).at(k), b.rec.bipolar(c).at(k));
        }
    }
}

#[test]
fn every_procedure_meets_the_content_rules_of_the_scenario_library() {
    let dir = assets().join("scenarios/ep");
    let mut n = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let s = Tier0Scenario::from_ron(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(
            s.scenario.id,
            path.file_stem().unwrap().to_string_lossy(),
            "id and file name"
        );
        let problems = s.content_problems();
        assert!(problems.is_empty(), "{}: {problems:?}", s.scenario.id);
        n += 1;
    }
    assert_eq!(n, 5, "five procedures");
}

/// The resting rhythm of the Tier-1 procedures is what their `expected` blocks say. Those of the
/// flutter and AF procedures describe Tier-2 arrhythmias and are checked in `heartsim-sim`.
#[test]
fn the_tier_1_procedures_rest_in_the_rhythm_they_declare() {
    for id in ["ep_baseline", "ep_avnrt", "ep_ap_left_lateral"] {
        let text = std::fs::read_to_string(assets().join(format!("scenarios/ep/{id}.ron"))).unwrap();
        let s = Tier0Scenario::from_ron(&text).unwrap();
        let (_, violations) = s.validate_run().unwrap();
        assert!(violations.is_empty(), "{id}: {violations:?}");
    }
}

#[test]
fn zz_his_probe() {
    let mut s = Study::new("ep_baseline");
    s.run(3000.0);
    let c = &pack().cells;
    for cat in &s.catheters {
        if cat.kind != CatheterKind::His { continue; }
        for e in cat.electrodes() {
            let mut d = f32::INFINITY;
            for (p, r) in c.positions.iter().zip(&c.regions) {
                if r.is_ventricular() { d = d.min(p.distance(*e)); }
            }
            eprintln!("zz His electrode {e:?} nearest ventricle {d:.1} mm");
        }
    }
    for src in [EgmSource::Atrial, EgmSource::His, EgmSource::Ventricular] {
        let n = s.rec.deflections().filter(|d| d.source == src && s.rec.channels()[d.channel].kind == CatheterKind::His).count();
        eprintln!("zz His {src:?} deflections {n}");
    }
}
