//! B13 acceptance, UI side: the pacemaker panel edits the device the timing engine runs, its LEDs
//! follow the marker channel, and the strip's spike marks align with the stimulus events within
//! one sample.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_core::{LEAD_COUNT, Millis};
use heartsim_ecg::{GaussianSynth, LeadRing};
use heartsim_sim::timing::{PacerChannel, PacerConfig, PacerMode, PacerPreset, TimingParams};
use heartsim_sim::{Engine, EventKind, Origin, TimingEngine};
use heartsim_ui::{PaceMarks, PacerPanelResponse, PacerPanelState, pacer_panel};

struct Model {
    panel: PacerPanelState,
    last: PacerPanelResponse,
}

fn harness(device: Option<&PacerConfig>) -> Harness<'static, Model> {
    Harness::new_ui_state(
        |ui, m: &mut Model| {
            let r = pacer_panel(ui, &mut m.panel);
            if r.changed {
                m.last = r;
            }
        },
        Model {
            panel: PacerPanelState::new(device),
            last: PacerPanelResponse::default(),
        },
    )
}

#[test]
fn mode_buttons_and_pause_edit_the_device() {
    let mut h = harness(Some(&PacerConfig::new(PacerMode::Vvi, 60.0)));
    h.get_by_label("DDD").click();
    h.run();
    assert_eq!(h.state().panel.device().unwrap().mode, PacerMode::Ddd);
    assert!(h.state().last.changed);
    h.get_by_label("PAUSE").click();
    h.run();
    assert!(h.state().panel.device().unwrap().paused);
    h.get_by_label("ASYNC").click();
    h.run();
    assert_eq!(h.state().panel.device().unwrap().effective_mode(), PacerMode::Doo);
}

#[test]
fn presets_connect_the_device_and_move_the_leads() {
    let mut h = harness(None);
    assert!(h.state().panel.device().is_none(), "starts disconnected");
    h.get_by_label(PacerPreset::Biv.label()).click();
    h.run();
    let d = h.state().panel.device().expect("connected");
    assert_eq!(d, PacerConfig::preset(PacerPreset::Biv));
    assert!(d.lv_lead.is_some());
    assert!(h.state().last.leads_moved);
}

#[test]
fn leds_flash_with_the_marker_channel() {
    let mut cfg = PacerConfig::new(PacerMode::Vvi, 60.0);
    cfg.lower_rate_bpm = 60.0;
    let mut p = TimingParams::default();
    p.av.complete_block = true;
    p.pacer = Some(cfg.clone());
    let mut e = TimingEngine::new(p);
    let mut panel = PacerPanelState::new(Some(&cfg));
    let mut lit = 0;
    for _ in 0..5000 {
        e.step(Millis(1.0));
        panel.on_markers(&e.drain_pacer_markers(), e.now().0);
        lit += usize::from(panel.pace_lit(PacerChannel::Ventricular));
        assert!(
            !panel.pace_lit(PacerChannel::Atrial),
            "VVI never paces the atrium"
        );
    }
    // 4–5 paces × 120 ms flash
    assert!((400..=700).contains(&lit), "V pace LED lit {lit} ms");
}

/// Run the timing engine and Tier-0 synth exactly as the app does: 1 ms steps, 500 Hz strip
/// samples, pace marks recorded before the step's samples are pushed.
#[test]
fn spike_marks_align_with_stimulus_events_within_one_sample() {
    const FS: f32 = 500.0;
    let period_ms = 1000.0 / f64::from(FS);
    let mut p = TimingParams::default();
    p.sinus.rate_bpm = 50.0;
    p.av.complete_block = true;
    let mut cfg = PacerConfig::preset(PacerPreset::Ddd60130);
    // some paces without capture as well
    cfg.ventricular_lead.threshold_drift_ma_per_min = 25.0;
    p.pacer = Some(cfg);
    let mut e = TimingEngine::new(p);
    let mut synth = GaussianSynth::textbook();
    let mut rings: Vec<LeadRing> = (0..LEAD_COUNT).map(|_| LeadRing::new(20_000, FS)).collect();
    let mut marks = PaceMarks::default();
    let mut stimuli = Vec::new();
    let mut acc = 0.0;
    for _ in 0..20_000 {
        e.step(Millis(1.0));
        let events = e.drain_events();
        stimuli.extend(
            events
                .iter()
                .filter(|ev| {
                    matches!(ev.origin, Origin::Pacer(_))
                        && matches!(ev.kind, EventKind::Stimulus | EventKind::Blocked)
                        && ev.region.is_atrial() == (ev.origin == Origin::Pacer(0))
                        && !ev.region.is_conduction_system()
                })
                .map(|ev| ev.at.0),
        );
        synth.on_events(&events);
        let markers = e.drain_pacer_markers();
        synth.on_pacer_markers(&markers);
        marks.record(&markers, rings[0].pushed());
        acc += 1.0;
        while acc >= period_ms {
            acc -= period_ms;
            let s = synth.sample(e.now());
            for (r, v) in rings.iter_mut().zip(s.0) {
                r.push(v as f32);
            }
        }
    }
    let all: Vec<_> = marks.iter().copied().collect();
    assert!(all.len() >= 30, "{} pace marks", all.len());
    assert!(all.iter().any(|m| !m.captured), "includes non-captured paces");
    stimuli.dedup();
    let mut matched = 0;
    for m in &all {
        // sample k is taken at t = (k + 1) · period
        let t_sample = (m.sample + 1) as f64 * period_ms;
        let nearest = stimuli
            .iter()
            .map(|t| (t_sample - t).abs())
            .fold(f64::INFINITY, f64::min);
        assert!(
            nearest <= period_ms,
            "mark at sample {} ({t_sample} ms) is {nearest} ms from its stimulus",
            m.sample
        );
        matched += 1;
        // the spike is in the trace at that sample (any lead), captured or not
        let peak = rings
            .iter()
            .map(|r| r.at(m.sample).unwrap_or(0.0).abs())
            .fold(0.0, f32::max);
        assert!(peak > 0.3, "spike visible at sample {}: {peak:.2} mV", m.sample);
    }
    assert_eq!(matched, all.len());
}
