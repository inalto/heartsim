//! B24 acceptance tests of the heart-sound lane: the Display popup's switch adds it, with the switch
//! off the strip is drawn exactly as before, and the `S1` label of a beat sits between its QRS and
//! its T wave. Headless, no GPU.

use egui::epaint::ClippedShape;
use egui::{Pos2, RawInput, Rect, Shape, Ui, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_core::{LEAD_COUNT, Lead, Millis};
use heartsim_ecg::pcg::PCG_PERIOD_MS;
use heartsim_ecg::{Analyzer, GaussianSynth, LeadRing, PCG_RATE_HZ, PcgParams, PcgSynth, SoundLabel};
use heartsim_sim::{Engine, TimingEngine, TimingParams};
use heartsim_ui::annotations::EcgAnnotations;
use heartsim_ui::{EcgView, PcgLane, TwelveLeadLayout, i18n};

const FS: f32 = 500.0;

/// The app's live pipeline: twelve 500 Hz rings, the 4 kHz PCG ring in step with them (eight
/// samples ending with each strip sample) and the labels as PCG ring indices.
struct Signal {
    rings: Vec<LeadRing>,
    pcg: LeadRing,
    marks: Vec<(u64, SoundLabel)>,
}

fn signal(seconds: f64) -> Signal {
    let params = TimingParams::default();
    let mut engine = TimingEngine::new(params.clone());
    let mut synth = GaussianSynth::textbook();
    let mut pcg_synth = PcgSynth::new(PcgParams::default(), 0, params.sinus.respiration_rate_bpm);
    let mut rings: Vec<LeadRing> = (0..LEAD_COUNT)
        .map(|_| LeadRing::new((60.0 * FS) as usize, FS))
        .collect();
    let mut pcg = LeadRing::new((60.0 * PCG_RATE_HZ) as usize, PCG_RATE_HZ);
    let mut marks = Vec::new();
    for k in 0..(seconds * 1000.0) as usize {
        engine.step(Millis(1.0));
        let ev = engine.drain_events();
        synth.on_events(&ev);
        pcg_synth.on_events(&ev);
        if k % 2 == 1 {
            let now = engine.now();
            let s = synth.sample(now);
            for (r, v) in rings.iter_mut().zip(s.0) {
                r.push(v as f32);
            }
            for i in (0..8).rev() {
                pcg.push(pcg_synth.sample(Millis(now.0 - f64::from(i) * PCG_PERIOD_MS)));
            }
            let newest = pcg.pushed() - 1;
            for m in pcg_synth.take_settled_marks(now) {
                let offset = ((m.at.0 - now.0) / PCG_PERIOD_MS).round() as i64;
                marks.push((newest.checked_add_signed(offset).unwrap(), m.label));
            }
        }
    }
    Signal { rings, pcg, marks }
}

/// One headless frame; returns the shapes painted.
fn frame(ctx: &egui::Context, ui_fn: impl FnMut(&mut Ui)) -> Vec<ClippedShape> {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 700.0))),
        ..RawInput::default()
    };
    let mut out = ctx.run_ui(input, ui_fn);
    out.textures_delta.clear();
    out.shapes
}

fn texts(shapes: &[ClippedShape], text: &str) -> Vec<Rect> {
    shapes
        .iter()
        .filter_map(|c| match &c.shape {
            Shape::Text(t) if t.galley.text() == text => Some(t.visual_bounding_rect()),
            _ => None,
        })
        .collect()
}

const STRIP: Rect = Rect::from_min_max(Pos2::new(20.0, 40.0), Pos2::new(1320.0, 300.0));

#[test]
fn with_the_switch_off_the_strip_is_drawn_exactly_as_before() {
    i18n::set_language("en");
    let s = signal(8.0);
    let lane = PcgLane {
        ring: &s.pcg,
        marks: &s.marks,
    };
    let ctx = egui::Context::default();
    let mut view = EcgView::new();
    assert!(!view.show_heart_sounds, "off by default");
    let plain = frame(&ctx, |ui| {
        view.strip(ui, STRIP, &s.rings, Lead::II);
    });
    let mut view = EcgView::new();
    let with_lane_off = frame(&ctx, |ui| {
        view.strip_with_sounds(ui, STRIP, &s.rings, Lead::II, Some(lane));
    });
    assert_eq!(
        format!("{plain:?}"),
        format!("{with_lane_off:?}"),
        "the strip changed"
    );
    // switched on, the lane is drawn under a shorter strip
    view.show_heart_sounds = true;
    let mut win = None;
    let on = frame(&ctx, |ui| {
        win = Some(view.strip_with_sounds(ui, STRIP, &s.rings, Lead::II, Some(lane)));
    });
    assert!(win.unwrap().rect.bottom() < STRIP.bottom() - 30.0);
    assert_eq!(texts(&on, "Heart sounds (model)").len(), 1);
    assert!(!texts(&on, "S1").is_empty() && !texts(&on, "S2").is_empty());
    // and on the 12-lead page
    let page = frame(&ctx, |ui| {
        view.page_with_sounds(ui, STRIP, &s.rings, TwelveLeadLayout::Clinical, Some(lane));
    });
    assert_eq!(texts(&page, "Heart sounds (model)").len(), 1);
    // a ring that does not run in step with the strip (an imported recording) has no lane
    let stray = LeadRing::new(100, PCG_RATE_HZ);
    let omitted = frame(&ctx, |ui| {
        view.strip_with_sounds(
            ui,
            STRIP,
            &s.rings,
            Lead::II,
            Some(PcgLane {
                ring: &stray,
                marks: &[],
            }),
        );
    });
    assert_eq!(texts(&omitted, "Heart sounds (model)").len(), 0, "no lane");
}

#[test]
fn the_s1_label_sits_between_the_qrs_and_the_t_wave_of_its_beat() {
    i18n::set_language("en");
    let s = signal(12.0);
    let lane = PcgLane {
        ring: &s.pcg,
        marks: &s.marks,
    };
    let ctx = egui::Context::default();
    let mut view = EcgView::new();
    view.show_heart_sounds = true;
    let mut win = None;
    let shapes = frame(&ctx, |ui| {
        win = Some(view.strip_with_sounds(ui, STRIP, &s.rings, Lead::II, Some(lane)));
    });
    let win = win.unwrap();
    let ann = EcgAnnotations::compute(&Analyzer::default(), &s.rings, win.head);
    let s1 = texts(&shapes, "S1");
    assert!(s1.len() >= 5, "{} S1 labels", s1.len());
    let mut checked = 0;
    for label in &s1 {
        let x = label.center().x;
        // the beat whose QRS is the last one before the label
        let beat = ann.analysis.beats.iter().rfind(|b| {
            win.x_of_sample(ann.first_sample + b.qrs_onset as u64)
                .is_some_and(|q| q < x)
        });
        let Some(beat) = beat else { continue };
        let (Some(q), Some(t)) = (
            win.x_of_sample(ann.first_sample + beat.qrs_onset as u64),
            beat.t
                .and_then(|t| win.x_of_sample(ann.first_sample + t.peak as u64)),
        ) else {
            continue;
        };
        assert!(q < x && x < t, "S1 at {x:.1} not between QRS {q:.1} and T {t:.1}");
        checked += 1;
    }
    assert!(checked >= 4, "only {checked} beats checked");
}

struct Model {
    view: EcgView,
    rings: Vec<LeadRing>,
}

#[test]
fn the_display_popup_switches_the_heart_sounds_on() {
    i18n::set_language("en");
    let mut h = Harness::builder()
        .with_size(egui::vec2(1300.0, 600.0))
        .build_ui_state(
            |ui, m: &mut Model| {
                m.view.update(&m.rings);
                m.view.controls(ui, &m.rings, None);
            },
            Model {
                view: EcgView::new(),
                rings: signal(3.0).rings,
            },
        );
    h.run();
    assert!(h.query_by_label("Heart sounds").is_none(), "inside the popup");
    h.get_by_label("Display").click();
    h.run();
    h.get_by_label("Heart sounds").click();
    h.run();
    assert!(h.state().view.show_heart_sounds);
    h.get_by_label("Heart sounds").click();
    h.run();
    assert!(!h.state().view.show_heart_sounds);
    i18n::set_language("it");
    h.run();
    let _ = h.get_by_label("Toni cardiaci");
    i18n::set_language("en");
}
