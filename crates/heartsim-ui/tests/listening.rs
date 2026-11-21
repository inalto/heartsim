//! B27 part B acceptance tests of the listening controls: the Display popup's "Listening site" (the
//! six choices and bell / diaphragm, the chest piece unused at the overview), the five clickable
//! markers on the chest drawing, and the heart-sound lane's title naming the site. Headless, no GPU.

use egui::epaint::ClippedShape;
use egui::{Pos2, RawInput, Rect, Shape, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use heartsim_core::{LEAD_COUNT, Lead, Millis};
use heartsim_ecg::pcg::PCG_PERIOD_MS;
use heartsim_ecg::{
    ChestPiece, GaussianSynth, LeadRing, ListeningChoice, ListeningSite, PCG_RATE_HZ, PcgParams, PcgSynth,
};
use heartsim_sim::{Engine, TimingEngine, TimingParams};
use heartsim_ui::auscultation_chest::{marker_position, site_position_mm};
use heartsim_ui::pcg_lane::lane_title;
use heartsim_ui::{EcgView, PcgLane, auscultation_chest, i18n};

const FS: f32 = 500.0;

struct Model {
    view: EcgView,
    rings: Vec<LeadRing>,
}

fn rings_and_pcg(seconds: f64) -> (Vec<LeadRing>, LeadRing) {
    let params = TimingParams::default();
    let mut engine = TimingEngine::new(params.clone());
    let mut synth = GaussianSynth::textbook();
    let mut pcg_synth = PcgSynth::new(PcgParams::default(), 0, params.sinus.respiration_rate_bpm);
    let mut rings: Vec<LeadRing> = (0..LEAD_COUNT)
        .map(|_| LeadRing::new((60.0 * FS) as usize, FS))
        .collect();
    let mut pcg = LeadRing::new((60.0 * PCG_RATE_HZ) as usize, PCG_RATE_HZ);
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
        }
    }
    (rings, pcg)
}

fn popup_harness() -> Harness<'static, Model> {
    let mut view = EcgView::new();
    view.play_heart_sounds = Some(false);
    Harness::builder()
        .with_size(egui::vec2(1300.0, 1000.0))
        .build_ui_state(
            |ui, m: &mut Model| {
                m.view.update(&m.rings);
                m.view.controls(ui, &m.rings, None);
            },
            Model {
                view,
                rings: rings_and_pcg(2.0).0,
            },
        )
}

#[test]
fn the_display_popup_offers_six_sites_and_the_chest_piece() {
    i18n::set_language("en");
    let mut h = popup_harness();
    h.run();
    assert!(h.query_by_label("Listening site").is_none(), "inside the popup");
    h.get_by_label("Display").click();
    h.run();
    let _ = h.get_by_label("Listening site");
    for name in [
        "Overview (all sounds)",
        "Aortic (2nd right intercostal space)",
        "Pulmonic (2nd left intercostal space)",
        "Erb's point (3rd left intercostal space)",
        "Tricuspid (lower left sternal border)",
        "Mitral (apex)",
    ] {
        let _ = h.get_by_label(name);
    }
    assert_eq!(h.state().view.listening, ListeningChoice::default());
    assert!(h.state().view.listening.is_overview(), "the overview by default");
    // the chest piece does nothing at the overview
    assert!(
        h.get_by_label("Bell").accesskit_node().is_disabled(),
        "no chest piece at the overview"
    );
    h.get_by_label("Mitral (apex)").click();
    h.run();
    assert_eq!(h.state().view.listening.site, ListeningSite::Mitral);
    assert!(!h.get_by_label("Bell").accesskit_node().is_disabled());
    h.get_by_label("Bell").click();
    h.run();
    assert_eq!(
        h.state().view.listening,
        ListeningChoice {
            site: ListeningSite::Mitral,
            chest: ChestPiece::Bell
        }
    );
    h.get_by_label("Overview (all sounds)").click();
    h.run();
    assert!(h.state().view.listening.is_overview());
    // in Italian
    i18n::set_language("it");
    h.run();
    let _ = h.get_by_label("Focolaio di auscultazione");
    let _ = h.get_by_label("Mitralico (punta)");
    let _ = h.get_by_label("Campana");
    let _ = h.get_by_label("Membrana");
    i18n::set_language("en");
}

#[test]
fn the_chest_markers_select_their_site() {
    i18n::set_language("en");
    let mut h = Harness::builder()
        .with_size(egui::vec2(300.0, 320.0))
        .build_ui_state(
            |ui, site: &mut ListeningSite| {
                auscultation_chest(ui, egui::vec2(240.0, 260.0), site, 0.7);
            },
            ListeningSite::Overview,
        );
    h.run();
    for (site, name) in [
        (
            ListeningSite::Aortic,
            "Aortic (2nd right intercostal space) on the chest",
        ),
        (
            ListeningSite::Pulmonic,
            "Pulmonic (2nd left intercostal space) on the chest",
        ),
        (
            ListeningSite::Erb,
            "Erb's point (3rd left intercostal space) on the chest",
        ),
        (
            ListeningSite::Tricuspid,
            "Tricuspid (lower left sternal border) on the chest",
        ),
        (ListeningSite::Mitral, "Mitral (apex) on the chest"),
    ] {
        h.get_by_label(name).click();
        h.run();
        assert_eq!(*h.state(), site, "{name}");
    }
    // five markers, none for the overview, laid out as on the chest: aortic to the viewer's left of
    // the pulmonic, the apex lowest and furthest to the viewer's right
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(240.0, 260.0));
    let at = |s| marker_position(rect, s).unwrap_or_else(|| panic!("{s:?}"));
    assert!(marker_position(rect, ListeningSite::Overview).is_none());
    assert!(at(ListeningSite::Aortic).x < at(ListeningSite::Pulmonic).x);
    assert!(at(ListeningSite::Pulmonic).y < at(ListeningSite::Erb).y);
    assert!(at(ListeningSite::Erb).y < at(ListeningSite::Tricuspid).y);
    assert!(at(ListeningSite::Mitral).y > at(ListeningSite::Tricuspid).y);
    assert!(at(ListeningSite::Mitral).x > at(ListeningSite::Tricuspid).x);
    for s in ListeningSite::SITES {
        assert!(rect.contains(at(s)), "{s:?} outside the drawing");
        assert!(site_position_mm(s).is_some());
    }
    i18n::set_language("it");
    h.run();
    let _ = h.get_by_label("Mitralico (punta) sul torace");
    i18n::set_language("en");
}

fn frame(ctx: &egui::Context, ui_fn: impl FnMut(&mut egui::Ui)) -> Vec<ClippedShape> {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 700.0))),
        ..RawInput::default()
    };
    let mut out = ctx.run_ui(input, ui_fn);
    out.textures_delta.clear();
    out.shapes
}

fn has_text(shapes: &[ClippedShape], text: &str) -> bool {
    shapes
        .iter()
        .any(|c| matches!(&c.shape, Shape::Text(t) if t.galley.text() == text))
}

#[test]
fn the_lane_title_names_the_site_and_the_chest_piece() {
    i18n::set_language("en");
    let (rings, pcg) = rings_and_pcg(6.0);
    let lane = PcgLane {
        ring: &pcg,
        marks: &[],
    };
    let strip = Rect::from_min_max(Pos2::new(20.0, 40.0), Pos2::new(1320.0, 300.0));
    let ctx = egui::Context::default();
    let mut view = EcgView::new();
    view.show_heart_sounds = true;
    let shapes = frame(&ctx, |ui| {
        view.strip_with_sounds(ui, strip, &rings, Lead::II, Some(lane));
    });
    assert!(
        has_text(&shapes, "Heart sounds (model)"),
        "the overview keeps today's title"
    );
    view.listening = ListeningChoice {
        site: ListeningSite::Mitral,
        chest: ChestPiece::Bell,
    };
    let shapes = frame(&ctx, |ui| {
        view.strip_with_sounds(ui, strip, &rings, Lead::II, Some(lane));
    });
    assert!(has_text(&shapes, "Heart sounds (model) · Apex · Bell"));
    assert!(!has_text(&shapes, "Heart sounds (model)"));
    assert_eq!(lane_title(view.listening), "Heart sounds (model) · Apex · Bell");
    assert_eq!(
        lane_title(ListeningChoice {
            site: ListeningSite::Aortic,
            chest: ChestPiece::Diaphragm
        }),
        "Heart sounds (model) · Aortic area · Diaphragm"
    );
    i18n::set_language("it");
    assert_eq!(
        lane_title(view.listening),
        "Toni cardiaci (modello) · Punta · Campana"
    );
    i18n::set_language("en");
}
