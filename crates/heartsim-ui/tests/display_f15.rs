//! F-15 acceptance tests: the filter and artefact controls change only what is drawn. The raw rings,
//! the analysis and the clean exports stay exactly as they were; the artefacts are seeded; the
//! snapshot of a processed trace says so. Headless, no GPU.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_core::{LEAD_COUNT, Millis};
use heartsim_ecg::{ArtefactConfig, DisplayConfig, FilterMode, GaussianSynth, LeadRing, MainsFrequency};
use heartsim_sim::{Engine, TimingEngine, TimingParams};
use heartsim_ui::i18n;
use heartsim_ui::snapshot::{encode_png, render_page, stamp_caption};
use heartsim_ui::{EcgView, TwelveLeadLayout};

const FS: f32 = 500.0;

/// Twelve rings from the real timing engine and the Tier-0 synthesiser (what the app shows).
fn engine_rings(seconds: f64) -> Vec<LeadRing> {
    let mut engine = TimingEngine::new(TimingParams::default());
    let mut synth = GaussianSynth::textbook();
    let mut rings: Vec<LeadRing> = (0..LEAD_COUNT)
        .map(|_| LeadRing::new((60.0 * FS) as usize, FS))
        .collect();
    for k in 0..(seconds * 1000.0) as usize {
        engine.step(Millis(1.0));
        let ev = engine.drain_events();
        synth.on_events(&ev);
        if k % 2 == 1 {
            let s = synth.sample(engine.now());
            for (r, v) in rings.iter_mut().zip(s.0) {
                r.push(v as f32);
            }
        }
    }
    rings
}

fn contents(rings: &[LeadRing]) -> Vec<Vec<f32>> {
    rings.iter().map(|r| r.iter().collect()).collect()
}

fn everything_on(seed: u64) -> DisplayConfig {
    DisplayConfig {
        filter: FilterMode::Monitor,
        artefacts: ArtefactConfig {
            baseline_wander: true,
            respiration: true,
            mains: true,
            mains_hz: MainsFrequency::Hz60,
            muscle: true,
            seed,
            ..ArtefactConfig::default()
        },
    }
}

struct Model {
    view: EcgView,
    rings: Vec<LeadRing>,
}

fn harness() -> Harness<'static, Model> {
    i18n::set_language("en");
    Harness::builder()
        .with_size(egui::vec2(1300.0, 600.0))
        .build_ui_state(
            |ui, m: &mut Model| {
                m.view.update(&m.rings);
                m.view.display_controls(ui);
            },
            Model {
                view: EcgView::new(),
                rings: engine_rings(12.0),
            },
        )
}

#[test]
fn the_controls_set_the_filter_and_the_artefacts() {
    let mut h = harness();
    h.run();
    assert!(h.state().view.display_config().is_identity());

    h.get_by_label("Monitor 0.5–40 Hz").click();
    h.run();
    assert_eq!(h.state().view.display_config().filter, FilterMode::Monitor);
    h.get_by_label("Diagnostic 0.05–150 Hz").click();
    h.run();
    assert_eq!(h.state().view.display_config().filter, FilterMode::Diagnostic);

    // the mains frequency choice only appears with the mains artefact
    assert!(h.query_by_label("60 Hz").is_none());
    h.get_by_label("Mains hum").click();
    h.run();
    let _ = h.get_by_label("50 Hz");
    h.get_by_label("60 Hz").click();
    h.run();
    let a = h.state().view.display_config().artefacts;
    assert!(a.mains && a.mains_hz == MainsFrequency::Hz60);
    assert!(!a.muscle && !a.baseline_wander && !a.respiration);

    for name in ["Baseline wander", "Respiration", "Muscle"] {
        h.get_by_label(name).click();
        h.run();
    }
    let a = h.state().view.display_config().artefacts;
    assert!(a.baseline_wander && a.respiration && a.muscle);
    // an intensity slider appears once something is on
    assert!(h.query_all_by_label_contains("intensity").next().is_some());
}

#[test]
fn the_controls_exist_in_italian() {
    let mut h = harness();
    i18n::set_language("it");
    h.run();
    let _ = h.get_by_label("Filtro");
    let _ = h.get_by_label("Artefatti");
    let _ = h.get_by_label("Ronzio di rete");
    i18n::set_language("en");
}

#[test]
fn the_raw_rings_and_the_analysis_ignore_the_display() {
    let rings = engine_rings(12.0);
    let before = contents(&rings);

    let mut plain = EcgView::new();
    plain.update(&rings);
    let mut processed = EcgView::new();
    processed.set_display_config(everything_on(5));
    processed.update(&rings);

    assert_eq!(
        contents(&rings),
        before,
        "the display stage wrote into the raw rings"
    );
    // what is drawn differs from the raw rings ...
    assert_ne!(contents(processed.display_rings(&rings)), before);
    // ... but the measurements are the same
    let (a, b) = (plain.analysis().unwrap(), processed.analysis().unwrap());
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
    assert!(
        a.hr_bpm.is_some(),
        "the test signal should have a measurable rate"
    );
}

#[test]
fn an_untouched_display_draws_the_raw_rings() {
    let rings = engine_rings(5.0);
    let mut view = EcgView::new();
    view.update(&rings);
    assert!(std::ptr::eq(view.display_rings(&rings).as_ptr(), rings.as_ptr()));
}

#[test]
fn the_artefacts_are_seeded() {
    let rings = engine_rings(8.0);
    let shown = |seed| {
        let mut v = EcgView::new();
        v.set_display_config(everything_on(seed));
        v.update(&rings);
        contents(v.display_rings(&rings))
    };
    assert_eq!(shown(1), shown(1));
    assert_ne!(shown(1), shown(2));
}

#[test]
fn the_snapshot_of_a_processed_trace_carries_the_statement() {
    let rings = engine_rings(12.0);
    let mut view = EcgView::new();
    view.set_display_config(everything_on(9));
    view.update(&rings);
    let note = view
        .display_config()
        .describe()
        .expect("a processed display has a statement");
    assert!(note.contains("monitor 0.5-40 Hz") && note.contains("60 Hz mains") && note.contains("seed 9"));

    let size = [1200, 800];
    let layout = TwelveLeadLayout::Stacked;
    let mut img = render_page(
        view.display_rings(&rings),
        &view.style,
        layout,
        view.order,
        view.head(&rings),
        size,
        1.0,
    );
    let uncaptioned = encode_png(&img).unwrap();
    stamp_caption(&mut img, &view.style, &note);
    let expected = encode_png(&img).unwrap();
    let got = view.page_png(&rings, layout, size, 1.0).unwrap();
    assert_eq!(
        got, expected,
        "page_png must draw the displayed trace and its caption"
    );
    assert_ne!(got, uncaptioned);

    // a clean display has no caption: the snapshot is the plain page
    let mut clean = EcgView::new();
    clean.update(&rings);
    let plain = render_page(
        &rings,
        &clean.style,
        layout,
        clean.order,
        clean.head(&rings),
        size,
        1.0,
    );
    assert_eq!(
        clean.page_png(&rings, layout, size, 1.0).unwrap(),
        encode_png(&plain).unwrap()
    );
}
