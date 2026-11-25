//! The Simulator's left panel and its layout (no GPU): the sections of the heart's controls are
//! collapsible and remember being opened or closed in the settings; the three parts (controls, 3-D
//! view, axis panel) sit side by side at 1280 points without touching, and on a phone-narrow
//! window (400 points) are one full-width view at a time behind a segmented control, nothing
//! clipped and nothing overlapping.

use egui::{Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use heartsim_app::heart_view::HeartView;
use heartsim_app::settings::{Settings, SimulatorSections};
use heartsim_app::simulator_layout::{SimulatorParts, SimulatorView, simulator_body};
use heartsim_app::vector_overlay::VectorOverlay;
use heartsim_geometry::AnatomyPack;
use heartsim_sim::TimingParams;
use heartsim_ui::i18n;
use std::path::Path;
use std::sync::OnceLock;

fn pack() -> AnatomyPack {
    static PACK: OnceLock<AnatomyPack> = OnceLock::new();
    PACK.get_or_init(|| {
        AnatomyPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/anatomy/default"))
            .expect("default pack loads")
    })
    .clone()
}

/// The real heart controls, with stand-ins for the axis panel and the 3-D view that record the
/// rectangle each was given in the last frame (`None` when it was not drawn).
struct Parts {
    heart: HeartView,
    vector: VectorOverlay,
    sections: SimulatorSections,
    controls_rect: Option<Rect>,
    axis_rect: Option<Rect>,
    view_rect: Option<Rect>,
}

impl SimulatorParts for Parts {
    fn has_controls(&self) -> bool {
        true
    }
    fn controls(&mut self, ui: &mut egui::Ui) {
        self.controls_rect = Some(ui.max_rect());
        self.heart
            .controls_panel(ui, &mut self.sections, &mut self.vector);
    }
    fn axis(&mut self, ui: &mut egui::Ui) {
        self.axis_rect = Some(ui.max_rect());
        ui.label("Axis stand-in");
    }
    fn viewport(&mut self, ui: &mut egui::Ui) {
        let r = ui.available_rect_before_wrap();
        self.view_rect = Some(r);
        ui.allocate_rect(r, egui::Sense::hover());
    }
}

struct Page {
    parts: Parts,
    view: SimulatorView,
}

fn page(width: f32, height: f32, sections: SimulatorSections) -> Harness<'static, Page> {
    i18n::set_language("en");
    let mut heart = HeartView::from_pack(pack(), &TimingParams::default());
    heart.set_motion_options(false, true, 1.0);
    Harness::builder()
        .with_context_setup(heartsim_ui::theme::apply_to_context)
        .with_size(vec2(width, height))
        .build_ui_state(
            |ui, p: &mut Page| {
                p.parts.controls_rect = None;
                p.parts.axis_rect = None;
                p.parts.view_rect = None;
                simulator_body(ui, &mut p.view, &mut p.parts);
            },
            Page {
                parts: Parts {
                    heart,
                    vector: VectorOverlay::new(),
                    sections,
                    controls_rect: None,
                    axis_rect: None,
                    view_rect: None,
                },
                view: SimulatorView::Heart,
            },
        )
}

fn inside(r: Rect, screen: Rect) -> bool {
    screen.expand(0.5).contains_rect(r)
}

#[test]
fn the_default_sections_are_the_first_three_open() {
    let d = Settings::default().simulator_sections;
    assert!(d.colour && d.tool && d.layers);
    assert!(!d.motion && !d.vector);
}

#[test]
fn every_section_header_toggles_its_section_and_its_flag() {
    // tall, so no header sits below the visible part of the scroll area
    let mut h = page(1280.0, 1800.0, SimulatorSections::default());
    h.run();
    // open: colour map, click tool, anatomy; closed: wall motion, vector overlay
    let _ = h.get_by_label("3-D activation");
    let _ = h.get_by_label("Stimulate");
    assert!(h.query_by_label("Exaggeration").is_none());
    assert!(h.query_by_label("See-through").is_none());

    h.get_by_label("Colour map").click();
    h.run();
    assert!(!h.state().parts.sections.colour);
    assert!(h.query_by_label("3-D activation").is_none(), "the section closed");
    h.get_by_label("Colour map").click();
    h.run();
    assert!(h.state().parts.sections.colour);
    let _ = h.get_by_label("3-D activation");

    // the wall-motion section opens on a click on its header (the switch inside has the same name)
    h.get_by_label("Wall motion").click();
    h.run();
    assert!(h.state().parts.sections.motion);
    let _ = h.get_by_role_and_label(egui::accesskit::Role::Slider, "Exaggeration");

    h.get_by_label("Heart vector").click();
    h.run();
    assert!(h.state().parts.sections.vector);
}

#[test]
fn the_sections_are_named_in_italian_too() {
    i18n::set_language("it");
    for key in [
        "heart-section-colour",
        "heart-section-layers",
        "heart-section-motion",
        "app-sim-view-heart",
        "app-sim-view-controls",
        "app-sim-view-axis",
    ] {
        assert_ne!(i18n::tr(key), key, "it: {key}");
    }
    i18n::set_language("en");
}

#[test]
fn at_1280_the_three_parts_sit_side_by_side_inside_the_window_without_touching() {
    let mut h = page(1280.0, 800.0, SimulatorSections::default());
    h.run();
    let screen = Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 800.0));
    let p = &h.state().parts;
    let (c, v, a) = (
        p.controls_rect.expect("controls"),
        p.view_rect.expect("view"),
        p.axis_rect.expect("axis"),
    );
    for (name, r) in [("controls", c), ("view", v), ("axis", a)] {
        assert!(inside(r, screen), "{name} {r:?} leaves the window");
    }
    assert!(!c.intersects(v) && !v.intersects(a) && !c.intersects(a));
    assert!(c.right() <= v.left() + 0.5 && v.right() <= a.left() + 0.5);
    assert!(v.width() >= 400.0, "the 3-D view keeps room: {v:?}");
    // no segmented control on a wide window
    assert!(h.query_by_label("3-D heart").is_none());
}

#[test]
fn at_400_one_full_width_view_at_a_time_behind_a_segmented_control() {
    let mut h = page(400.0, 800.0, SimulatorSections::default());
    h.run();
    let screen = Rect::from_min_size(egui::Pos2::ZERO, vec2(400.0, 800.0));

    // the 3-D view alone, the full width of the window
    let p = &h.state().parts;
    let v = p.view_rect.expect("the 3-D view is drawn first");
    assert!(p.controls_rect.is_none() && p.axis_rect.is_none());
    assert!(inside(v, screen), "{v:?}");
    assert!(v.width() >= 360.0, "full width less the panel margin: {v:?}");
    for name in ["3-D heart", "Controls", "Axis"] {
        let r = h.get_by_label(name).rect();
        assert!(inside(r, screen), "{name} {r:?}");
    }

    // the controls: full width, nothing else drawn, every header inside the window
    h.get_by_label("Controls").click();
    h.run();
    let p = &h.state().parts;
    let c = p.controls_rect.expect("controls shown");
    assert!(p.view_rect.is_none() && p.axis_rect.is_none());
    assert!(inside(c, screen) && c.width() >= 360.0, "{c:?}");
    for name in ["Colour map", "Click on the heart", "Anatomy and camera"] {
        let r = h.get_by_label(name).rect();
        assert!(inside(r, screen) || r.top() >= screen.bottom(), "{name} {r:?}");
        assert!(r.right() <= 400.5, "{name} {r:?}");
    }
    // every control that is drawn stays within the width of the window
    for node in h.query_all(egui_kittest::kittest::By::new()) {
        if let Some(b) = node.accesskit_node().bounding_box()
            && b.y0 < 800.0
            && b.x1 > b.x0
        {
            assert!(b.x1 <= 400.5, "{:?} {b:?}", node.accesskit_node().label());
        }
    }

    // the axis panel
    h.get_by_label("Axis").click();
    h.run();
    let p = &h.state().parts;
    let a = p.axis_rect.expect("axis shown");
    assert!(p.view_rect.is_none() && p.controls_rect.is_none());
    assert!(inside(a, screen), "{a:?}");

    // and back to the heart
    h.get_by_label("3-D heart").click();
    h.run();
    assert!(h.state().parts.view_rect.is_some());
}

/// The layer rows carry translated part names, not the pack ids, and the opacity rails are named
/// from them; an unknown part keeps its id.
#[test]
fn the_layer_rows_show_translated_part_names_in_en_and_it() {
    use egui::accesskit::Role;
    for (lang, names) in [
        ("en", ["Pulmonary artery", "Superior vena cava", "Left ventricle"]),
        (
            "it",
            ["Arteria polmonare", "Vena cava superiore", "Ventricolo sinistro"],
        ),
    ] {
        let mut h = page(1280.0, 1800.0, SimulatorSections::default());
        i18n::set_language(lang);
        h.run();
        for name in names {
            let _ = h.get_by_label(name);
            let rail = format!("{name} {}", i18n::tr("anatomy-opacity"));
            let _ = h.get_by_role_and_label(Role::Slider, &rail);
        }
        for id in ["pulmonary_artery", "svc", "lv"] {
            assert!(h.query_by_label(id).is_none(), "{lang}: raw id {id} still shown");
        }
    }
    i18n::set_language("en");
}
