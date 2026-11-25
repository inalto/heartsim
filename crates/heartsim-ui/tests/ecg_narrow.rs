//! The strip toolbar on a phone-narrow window (400 points): one row (the lead as a combo box and
//! the Display button), everything else behind Display, so the trace keeps a useful height; the
//! wide toolbar is unchanged. Headless, no GPU.

use egui::{Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, NodeT, Queryable};
use heartsim_core::{LEAD_COUNT, Lead, Millis};
use heartsim_ecg::{GaussianSynth, LeadRing};
use heartsim_sim::{Engine, TimingEngine, TimingParams};
use heartsim_ui::{EcgView, i18n};

const FS: f32 = 500.0;
/// The least height the trace may have under the toolbar, points.
const TRACE_MIN_PX: f32 = 120.0;

fn rings() -> Vec<LeadRing> {
    let mut engine = TimingEngine::new(TimingParams::default());
    let mut synth = GaussianSynth::textbook();
    let mut rings: Vec<LeadRing> = (0..LEAD_COUNT)
        .map(|_| LeadRing::new((20.0 * FS) as usize, FS))
        .collect();
    for k in 0..2000 {
        engine.step(Millis(1.0));
        synth.on_events(&engine.drain_events());
        if k % 2 == 1 {
            let s = synth.sample(engine.now());
            for (r, v) in rings.iter_mut().zip(s.0) {
                r.push(v as f32);
            }
        }
    }
    rings
}

struct Model {
    view: EcgView,
    rings: Vec<LeadRing>,
    lead: Lead,
    trace: Rect,
}

/// The toolbar in a bottom panel of the size the app gives the Simulator's strip (200 points).
fn harness(width: f32) -> Harness<'static, Model> {
    i18n::set_language("en");
    Harness::builder()
        .with_context_setup(heartsim_ui::theme::apply_to_context)
        .with_size(vec2(width, 800.0))
        .build_ui_state(
            |ui, m: &mut Model| {
                egui::Panel::bottom("strip")
                    .resizable(true)
                    .default_size(200.0)
                    .show(ui, |ui| {
                        m.view.update(&m.rings);
                        m.view.controls(ui, &m.rings, Some(&mut m.lead));
                        m.trace = Rect::from_min_size(ui.cursor().min, ui.available_size());
                        ui.allocate_rect(m.trace, egui::Sense::hover());
                    });
            },
            Model {
                view: EcgView::new(),
                rings: rings(),
                lead: Lead::II,
                trace: Rect::NOTHING,
            },
        )
}

#[test]
fn at_400_the_toolbar_is_one_compact_row_and_the_trace_keeps_its_height() {
    let mut h = harness(400.0);
    h.run();
    assert!(
        h.state().trace.height() >= TRACE_MIN_PX,
        "the trace has {} points",
        h.state().trace.height()
    );
    assert!(h.state().trace.width() >= 360.0);
    // lead combo and Display on one row; the rest is folded away
    let lead = h
        .get_by_role_and_label(egui::accesskit::Role::ComboBox, "Lead")
        .rect();
    let display = h.get_by_label_contains("Display").rect();
    assert!(
        (lead.center().y - display.center().y).abs() < 2.0,
        "{lead:?} {display:?}"
    );
    assert!(display.right() <= 400.5);
    for name in ["Clear calipers", "Scroll", "Sweep"] {
        assert!(h.query_by_label(name).is_none(), "{name} is behind Display");
    }
    // opening Display brings them
    h.get_by_label_contains("Display").click();
    h.run();
    for name in ["Clear calipers", "Scroll", "Sweep"] {
        let _ = h.get_by_label(name);
    }
}

#[test]
fn at_400_no_toolbar_control_is_cut_by_the_right_edge() {
    let mut h = harness(400.0);
    h.run();
    for node in h.query_all(By::new()) {
        if let Some(b) = node.accesskit_node().bounding_box()
            && b.x1 > b.x0
        {
            assert!(b.x1 <= 400.5, "{:?} {b:?}", node.accesskit_node().label());
        }
    }
}

#[test]
fn when_wide_the_toolbar_keeps_every_control_in_the_row() {
    let mut h = harness(1280.0);
    h.run();
    for name in ["Clear calipers", "Scroll", "Sweep", "II"] {
        let _ = h.get_by_label(name);
    }
    assert!(
        h.query_by_role_and_label(egui::accesskit::Role::ComboBox, "Lead")
            .is_none()
    );
}
