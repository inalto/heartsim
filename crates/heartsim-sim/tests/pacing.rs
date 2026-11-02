//! Acceptance tests for brief B13 (pacing devices), timing side: every NBG mode's timing cycle,
//! the capture-threshold model, amplitude-gated sensing (undersensing, T-wave oversensing),
//! biventricular VV offset, the PMT intervention, the panel buttons and the marker channel.

use heartsim_core::{Millis, Region};
use heartsim_sim::timing::{
    Focus, LeadParams, LeadSite, MarkerKind, PacerChannel, PacerConfig, PacerMarker, PacerMode, PacerPreset,
    T_WAVE_DELAY_MS, TimingParams,
};
use heartsim_sim::{Chamber, Engine, Event, EventKind, Origin, TimingEngine};

const SECONDS: f64 = 20.0;

struct Run {
    events: Vec<Event>,
    markers: Vec<PacerMarker>,
}

fn run(p: TimingParams, seconds: f64) -> Run {
    let mut e = TimingEngine::new(p);
    let (mut events, mut markers) = (Vec::new(), Vec::new());
    for _ in 0..(seconds * 1000.0) as usize {
        e.step(Millis(1.0));
        events.extend(e.drain_events());
        markers.extend(e.drain_pacer_markers());
    }
    Run { events, markers }
}

impl Run {
    fn paces(&self, ch: PacerChannel) -> Vec<f64> {
        self.markers
            .iter()
            .filter(|m| m.channel == ch && m.is_pace())
            .map(|m| m.at.0)
            .collect()
    }

    fn captured(&self, ch: PacerChannel) -> usize {
        self.markers
            .iter()
            .filter(|m| m.channel == ch && m.kind == MarkerKind::Pace { captured: true })
            .count()
    }

    fn senses(&self, ch: PacerChannel) -> Vec<f64> {
        self.markers
            .iter()
            .filter(|m| m.channel == ch && m.kind == MarkerKind::Sense)
            .map(|m| m.at.0)
            .collect()
    }

    /// Ventricular activations (non-blocked, fusion fronts within 150 ms merged).
    fn v_beats(&self) -> Vec<f64> {
        let mut out: Vec<f64> = Vec::new();
        for e in &self.events {
            if e.chamber == Chamber::Ventricle
                && e.kind != EventKind::Blocked
                && out.last().is_none_or(|l| e.at.0 - l > 150.0)
            {
                out.push(e.at.0);
            }
        }
        out
    }
}

fn intervals(t: &[f64]) -> Vec<f64> {
    t.windows(2).map(|w| w[1] - w[0]).collect()
}

fn all_near(x: &[f64], target: f64, tol: f64) -> bool {
    !x.is_empty() && x.iter().all(|v| (v - target).abs() <= tol)
}

fn with_pacer(sinus_bpm: f64, block: bool, cfg: PacerConfig) -> TimingParams {
    let mut p = TimingParams::default();
    p.sinus.rate_bpm = sinus_bpm;
    p.av.complete_block = block;
    p.escape.junctional_bpm = 0.0;
    p.escape.ventricular_bpm = if block { 30.0 } else { 0.0 };
    p.pacer = Some(cfg);
    p
}

// --- timing cycles of every mode -----------------------------------------------------------------

#[test]
fn voo_paces_at_the_lower_rate_whatever_the_rhythm() {
    let r = run(
        with_pacer(80.0, false, PacerConfig::new(PacerMode::Voo, 60.0)),
        SECONDS,
    );
    let v = r.paces(PacerChannel::Ventricular);
    assert!(
        all_near(&intervals(&v), 1000.0, 0.5),
        "VOO intervals {:?}",
        intervals(&v)
    );
    assert!(
        r.senses(PacerChannel::Ventricular).is_empty(),
        "VOO senses nothing"
    );
    assert_eq!(r.paces(PacerChannel::Atrial), Vec::<f64>::new());
    // competition with the faster sinus rhythm: some spikes fall in refractory tissue
    assert!(r.captured(PacerChannel::Ventricular) < v.len());
}

#[test]
fn aoo_paces_the_atrium_at_the_lower_rate() {
    let r = run(
        with_pacer(50.0, false, PacerConfig::new(PacerMode::Aoo, 70.0)),
        SECONDS,
    );
    let a = r.paces(PacerChannel::Atrial);
    assert!(all_near(&intervals(&a), 60_000.0 / 70.0, 1.0));
    assert_eq!(r.paces(PacerChannel::Ventricular), Vec::<f64>::new());
    assert!(r.captured(PacerChannel::Atrial) >= a.len() - 1);
}

#[test]
fn aai_paces_slow_sinus_and_is_inhibited_by_fast_sinus() {
    let r = run(
        with_pacer(40.0, false, PacerConfig::new(PacerMode::Aai, 60.0)),
        SECONDS,
    );
    let a = r.paces(PacerChannel::Atrial);
    assert!(a.len() >= 18, "{} atrial paces at 40 bpm sinus, AAI 60", a.len());
    assert!(all_near(&intervals(&a)[1..], 1000.0, 1.0), "{:?}", intervals(&a));
    // every atrial pace conducts: one ventricular beat per paced P
    assert!(r.v_beats().len().abs_diff(a.len()) <= 1);

    let r = run(
        with_pacer(80.0, false, PacerConfig::new(PacerMode::Aai, 60.0)),
        SECONDS,
    );
    assert!(
        r.paces(PacerChannel::Atrial).is_empty(),
        "inhibited by sinus at 80"
    );
    assert!(r.senses(PacerChannel::Atrial).len() >= 25, "every sinus P sensed");
}

#[test]
fn vvi_inhibits_on_sensed_beats_and_paces_one_lri_later() {
    let r = run(
        with_pacer(40.0, false, PacerConfig::new(PacerMode::Vvi, 60.0)),
        SECONDS,
    );
    let vs = r.senses(PacerChannel::Ventricular);
    let vp = r.paces(PacerChannel::Ventricular);
    assert!(!vp.is_empty() && !vs.is_empty());
    for p in &vp[1..] {
        let prev = vs
            .iter()
            .chain(&vp)
            .copied()
            .filter(|t| t < p)
            .fold(f64::MIN, f64::max);
        assert!(
            (p - prev - 1000.0).abs() <= 1.0,
            "VP {p} is {} after the last V event",
            p - prev
        );
    }
}

#[test]
fn doo_paces_both_chambers_at_fixed_av_delay() {
    let cfg = PacerConfig {
        av_delay_ms: 180.0,
        ..PacerConfig::new(PacerMode::Doo, 60.0)
    };
    let r = run(with_pacer(80.0, true, cfg), SECONDS);
    let a = r.paces(PacerChannel::Atrial);
    let v = r.paces(PacerChannel::Ventricular);
    assert!(all_near(&intervals(&a), 1000.0, 1.0));
    assert_eq!(a.len(), v.len());
    assert!(a.iter().zip(&v).all(|(a, v)| (v - a - 180.0).abs() <= 1.0));
}

#[test]
fn ddd_tracks_sensed_p_with_the_av_delay() {
    let r = run(
        with_pacer(75.0, true, PacerConfig::preset(PacerPreset::Ddd60130)),
        SECONDS,
    );
    let as_ = r.senses(PacerChannel::Atrial);
    let vp = r.paces(PacerChannel::Ventricular);
    // (the first P at 200 ms falls in the PVARP that starts at reset: one start-up AP)
    assert!(
        r.paces(PacerChannel::Atrial).iter().all(|t| *t < 2000.0),
        "sinus above the lower rate"
    );
    assert!(vp.len().abs_diff(as_.len()) <= 1);
    for v in &vp[1..] {
        let p = as_.iter().copied().filter(|t| t < v).fold(f64::MIN, f64::max);
        assert!((v - p - 150.0).abs() <= 1.0, "AS-VP {}", v - p);
    }
}

#[test]
fn ddi_does_not_track_the_atrium() {
    // Sinus at 90 in complete block: DDI senses the P waves (no atrial pace) but paces the
    // ventricle only at the lower rate, never P-synchronously.
    let r = run(
        with_pacer(90.0, true, PacerConfig::new(PacerMode::Ddi, 60.0)),
        SECONDS,
    );
    let vp = r.paces(PacerChannel::Ventricular);
    assert_ne!(r.senses(PacerChannel::Atrial), Vec::<f64>::new());
    assert!(
        (17..=22).contains(&vp.len()),
        "{} ventricular paces: lower rate, not the 90 bpm sinus",
        vp.len()
    );
}

// --- panel buttons ---------------------------------------------------------------------------------

#[test]
fn asynchronous_button_turns_demand_into_fixed_rate() {
    let cfg = PacerConfig {
        asynchronous: true,
        ..PacerConfig::new(PacerMode::Vvi, 60.0)
    };
    assert_eq!(cfg.effective_mode(), PacerMode::Voo);
    let r = run(with_pacer(80.0, false, cfg), SECONDS);
    assert!(all_near(
        &intervals(&r.paces(PacerChannel::Ventricular)),
        1000.0,
        0.5
    ));
    assert_eq!(r.senses(PacerChannel::Ventricular), Vec::<f64>::new());
    assert_eq!(PacerMode::Ddd.asynchronous(), PacerMode::Doo);
    assert_eq!(PacerMode::Aai.asynchronous(), PacerMode::Aoo);
}

#[test]
fn pause_button_stops_output_but_not_sensing() {
    let cfg = PacerConfig {
        paused: true,
        ..PacerConfig::new(PacerMode::Vvi, 60.0)
    };
    let r = run(with_pacer(72.0, true, cfg), SECONDS);
    assert_eq!(r.paces(PacerChannel::Ventricular), Vec::<f64>::new());
    // the underlying 30 bpm escape rhythm shows, and is still sensed
    assert!(r.senses(PacerChannel::Ventricular).len() >= 8);
    let rr = intervals(&r.v_beats());
    assert!(all_near(&rr, 2040.0, 45.0), "escape RR {rr:?}");
}

// --- capture -----------------------------------------------------------------------------------------

#[test]
fn threshold_drifts_and_saturates() {
    let lead = LeadParams {
        threshold_ma: Some(1.0),
        threshold_drift_ma_per_min: 6.0,
        threshold_max_ma: 4.0,
        ..LeadParams::at(LeadSite::RvApex)
    };
    assert!((lead.threshold_at(0.0) - 1.0).abs() < 1e-12);
    assert!((lead.threshold_at(10_000.0) - 2.0).abs() < 1e-12);
    assert!((lead.threshold_at(600_000.0) - 4.0).abs() < 1e-12, "capped");
    let typical = LeadParams::at(LeadSite::LvLateral);
    assert!(typical.threshold_at(0.0) > LeadParams::at(LeadSite::RvApex).threshold_at(0.0));
}

#[test]
fn output_below_threshold_gives_spikes_without_capture() {
    let mut cfg = PacerConfig::new(PacerMode::Vvi, 60.0);
    cfg.ventricular_lead.threshold_ma = Some(2.0);
    for (output, captures) in [(1.9, false), (2.0, true), (5.0, true)] {
        cfg.ventricular_output_ma = output;
        let r = run(with_pacer(72.0, true, cfg.clone()), 10.0);
        let paces = r.paces(PacerChannel::Ventricular);
        assert!(paces.len() >= 4, "{output} mA: {} paces", paces.len());
        assert_eq!(
            r.captured(PacerChannel::Ventricular) > 0,
            captures,
            "{output} mA vs 2.0 mA threshold"
        );
        let paced_beats = r
            .events
            .iter()
            .filter(|e| e.origin == Origin::Pacer(1) && e.kind == EventKind::Stimulus)
            .count();
        assert_eq!(paced_beats, r.captured(PacerChannel::Ventricular));
    }
}

#[test]
fn rising_threshold_loses_capture_and_the_escape_takes_over() {
    let mut cfg = PacerConfig::new(PacerMode::Vvi, 60.0);
    cfg.ventricular_lead.threshold_ma = Some(1.0);
    cfg.ventricular_lead.threshold_drift_ma_per_min = 30.0; // crosses 5 mA at 8 s
    let r = run(with_pacer(72.0, true, cfg), SECONDS);
    let lost: Vec<f64> = r
        .markers
        .iter()
        .filter(|m| m.kind == MarkerKind::Pace { captured: false })
        .map(|m| m.at.0)
        .collect();
    assert!(
        lost.iter().all(|t| *t >= 7_900.0),
        "no loss of capture before 8 s"
    );
    assert!(lost.len() >= 5, "{} spikes without capture", lost.len());
    let late: Vec<f64> = r.v_beats().into_iter().filter(|t| *t > 10_000.0).collect();
    assert!(
        all_near(&intervals(&late), 2040.0, 45.0),
        "30 bpm escape {late:?}"
    );
}

#[test]
fn a_spike_into_refractory_tissue_is_functional_non_capture() {
    // VOO at 60 against conducted sinus at 80: well above threshold, yet spikes just after a
    // native QRS do not capture.
    let r = run(
        with_pacer(80.0, false, PacerConfig::new(PacerMode::Voo, 60.0)),
        SECONDS,
    );
    let missed = r
        .markers
        .iter()
        .filter(|m| m.kind == MarkerKind::Pace { captured: false })
        .count();
    assert!(missed > 0);
    for m in r.markers.iter().filter(|m| m.is_pace()) {
        let blocked = r
            .events
            .iter()
            .any(|e| e.at == m.at && e.origin == Origin::Pacer(1) && e.kind == EventKind::Blocked);
        assert_eq!(blocked, m.kind == MarkerKind::Pace { captured: false });
    }
}

#[test]
fn his_bundle_lead_conducts_over_the_bundle_branches() {
    let mut cfg = PacerConfig::new(PacerMode::Vvi, 70.0);
    cfg.ventricular_lead = LeadParams::at(LeadSite::HisBundle);
    let mut p = with_pacer(72.0, true, cfg);
    // His pacing needs an infranodal-intact conduction system: block above the His (AV node)
    p.av.complete_block = true;
    let r = run(p, 10.0);
    assert!(r.captured(PacerChannel::Ventricular) >= 9);
    let conducted = r
        .events
        .iter()
        .filter(|e| {
            e.origin == Origin::Pacer(1) && e.chamber == Chamber::Ventricle && e.kind == EventKind::Conducted
        })
        .count();
    assert!(
        conducted >= 9,
        "paced beats reach the ventricles over the fascicles: {conducted}"
    );
}

// --- sensing -----------------------------------------------------------------------------------------

#[test]
fn sensitivity_above_the_electrogram_undersenses() {
    let mut cfg = PacerConfig::new(PacerMode::Vvi, 60.0);
    cfg.ventricular_sensitivity_mv = 12.0; // R wave at the RV apex is 10 mV
    let r = run(with_pacer(75.0, false, cfg.clone()), SECONDS);
    assert_eq!(
        r.senses(PacerChannel::Ventricular),
        Vec::<f64>::new(),
        "nothing sensed"
    );
    assert!(
        all_near(&intervals(&r.paces(PacerChannel::Ventricular)), 1000.0, 0.5),
        "competes at the lower rate"
    );
    // same device, sensitivity below the R wave: inhibited
    cfg.ventricular_sensitivity_mv = 2.5;
    let r = run(with_pacer(75.0, false, cfg.clone()), SECONDS);
    assert_eq!(r.paces(PacerChannel::Ventricular), Vec::<f64>::new());
    // or a small electrogram at the lead (scar) with the normal setting
    cfg.ventricular_lead.intrinsic_mv = Some(2.0);
    let r = run(with_pacer(75.0, false, cfg), SECONDS);
    assert_eq!(r.senses(PacerChannel::Ventricular), Vec::<f64>::new());
}

#[test]
fn measured_amplitude_overrides_the_lead_parameter() {
    let mut e = TimingEngine::new(with_pacer(75.0, false, PacerConfig::new(PacerMode::Vvi, 60.0)));
    e.set_measured_amplitude_mv(PacerChannel::Ventricular, Some(1.0));
    let mut paces = 0;
    for _ in 0..10_000 {
        e.step(Millis(1.0));
        paces += e.drain_pacer_markers().iter().filter(|m| m.is_pace()).count();
    }
    assert!(
        paces >= 9,
        "a 1 mV electrogram is undersensed at 2.5 mV: {paces} paces"
    );
}

#[test]
fn t_wave_oversensing_lengthens_the_paced_interval() {
    let mut cfg = PacerConfig::new(PacerMode::Vvi, 60.0);
    cfg.ventricular_sensitivity_mv = 1.0; // T wave on the lead is 1.5 mV
    let r = run(with_pacer(72.0, true, cfg), SECONDS);
    let vp = r.paces(PacerChannel::Ventricular);
    let expected = 1000.0 + T_WAVE_DELAY_MS;
    assert!(
        all_near(&intervals(&vp)[1..], expected, 1.0),
        "{:?}",
        intervals(&vp)
    );
    // every VS is a T wave, T_WAVE_DELAY after a captured pace
    for s in r.senses(PacerChannel::Ventricular) {
        assert!(
            vp.iter().any(|p| (s - p - T_WAVE_DELAY_MS).abs() < 1.0),
            "VS at {s}"
        );
    }
}

// --- biventricular ---------------------------------------------------------------------------------

#[test]
fn biventricular_paces_lv_first_by_the_vv_offset() {
    let mut p = with_pacer(70.0, false, PacerConfig::preset(PacerPreset::Biv));
    p.his_purkinje.lbb.blocked = true;
    let r = run(p, SECONDS);
    let rv = r.paces(PacerChannel::Ventricular);
    let lv = r.paces(PacerChannel::LeftVentricular);
    assert!(
        rv.len() >= 20 && rv.len() == lv.len(),
        "RV {} LV {}",
        rv.len(),
        lv.len()
    );
    assert!(lv.iter().zip(&rv).all(|(l, r)| (r - l - 20.0).abs() <= 1.0));
    // both fronts capture: they fuse into one beat
    assert_eq!(r.captured(PacerChannel::LeftVentricular), lv.len());
    assert_eq!(r.captured(PacerChannel::Ventricular), rv.len());
    assert!(r.events.iter().any(|e| e.origin == Origin::Pacer(2)
        && e.region == Region::LeftVentricle
        && e.kind == EventKind::Stimulus));
}

// --- pacemaker-mediated tachycardia ----------------------------------------------------------------

fn pmt(intervention: bool) -> Run {
    let mut p = TimingParams::default();
    p.sinus.rate_bpm = 70.0;
    p.av.retrograde_fast = true;
    p.av.retrograde_delay_ms = 240.0;
    p.foci.push(Focus {
        max_fires: 1,
        ..Focus::pvc(450.0, 3)
    });
    p.pacer = Some(PacerConfig {
        pmt_intervention: intervention,
        ..PacerConfig::new(PacerMode::Ddd, 60.0)
    });
    run(p, SECONDS)
}

/// Ventricular paces delivered at the upper rate interval (500 ms) after the previous one.
fn upper_rate_paces(r: &Run) -> usize {
    intervals(&r.paces(PacerChannel::Ventricular))
        .iter()
        .filter(|i| (**i - 500.0).abs() <= 1.0)
        .count()
}

#[test]
fn pvc_with_va_conduction_starts_pmt_and_pvarp_extension_ends_it() {
    let endless = pmt(false);
    let n = upper_rate_paces(&endless);
    assert!(
        n >= 25,
        "PMT at the upper rate persists without intervention: {n}"
    );

    let treated = pmt(true);
    let n = upper_rate_paces(&treated);
    assert!(
        (6..=10).contains(&n),
        "detected after 8 upper-rate cycles, then terminated: {n} upper-rate paces"
    );
    // the retrograde P of the terminating cycle is seen inside the extended PVARP (AR)
    assert!(
        treated
            .markers
            .iter()
            .any(|m| m.channel == PacerChannel::Atrial && m.kind == MarkerKind::RefractorySense)
    );
    // afterwards: sinus tracked at 70 bpm again
    let late: Vec<f64> = treated.v_beats().into_iter().filter(|t| *t > 12_000.0).collect();
    let rr = intervals(&late);
    assert!(all_near(&rr, 60_000.0 / 70.0, 15.0), "late RR {rr:?}");
}

// --- marker channel ----------------------------------------------------------------------------------

#[test]
fn every_pace_marker_is_a_stimulus_event_at_the_lead_site() {
    let mut p = with_pacer(50.0, true, PacerConfig::preset(PacerPreset::Ddd60130));
    p.pacer
        .as_mut()
        .unwrap()
        .ventricular_lead
        .threshold_drift_ma_per_min = 20.0;
    let r = run(p, SECONDS);
    let paces: Vec<&PacerMarker> = r.markers.iter().filter(|m| m.is_pace()).collect();
    assert!(paces.len() >= 30);
    for m in paces {
        let site = match m.channel {
            PacerChannel::Atrial => LeadSite::RaAppendage,
            _ => LeadSite::RvApex,
        };
        let ev = r
            .events
            .iter()
            .find(|e| {
                e.at == m.at && e.origin == Origin::Pacer(m.channel.index()) && e.region == site.region()
            })
            .unwrap_or_else(|| panic!("no engine event for {} at {}", m.label(), m.at.0));
        let captured = matches!(m.kind, MarkerKind::Pace { captured: true });
        assert_eq!(
            ev.kind == EventKind::Stimulus,
            captured,
            "{} at {}",
            m.label(),
            m.at.0
        );
    }
    assert!(
        r.markers
            .iter()
            .all(|m| matches!(m.label().as_str(), "AP" | "AS" | "AR" | "VP" | "VS" | "VR"))
    );
}

#[test]
fn deterministic_and_ron_roundtrip() {
    let mut cfg = PacerConfig::preset(PacerPreset::Biv);
    cfg.ventricular_lead.threshold_drift_ma_per_min = 1.5;
    cfg.atrial_sensitivity_mv = 1.0;
    let p = with_pacer(70.0, false, cfg);
    let text = ron::ser::to_string_pretty(&p, ron::ser::PrettyConfig::default().struct_names(true)).unwrap();
    let back: TimingParams = ron::from_str(&text).unwrap();
    assert_eq!(p, back);
    let a = run(p, 8.0);
    let b = run(back, 8.0);
    assert_eq!(a.events, b.events);
    assert_eq!(a.markers, b.markers);
}
