//! The frontal QRS-axis panel: the draggable circle shows a hand cursor, brightens and still
//! reports where it was dragged to; the non-draggable one does none of that; the cards and the
//! legend name every class and its range in the words of the catalogue. No GPU.

use egui::{CursorIcon, Modifiers, PointerButton, Pos2, Rect, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_core::AxisClass;
use heartsim_ui::{AxisPanel, Palette, axis_circle, axis_panel, i18n};

struct Dial {
    interactive: bool,
    rect: Rect,
    hovered: bool,
    sector: Option<AxisClass>,
    dragged: Vec<f64>,
}

fn dial(interactive: bool) -> Harness<'static, Dial> {
    Harness::builder()
        .with_size(Vec2::new(500.0, 500.0))
        .build_ui_state(
            |ui, d: &mut Dial| {
                let p = Palette::of(ui);
                let r = axis_circle(ui, 400.0, Some(60.0), None, d.interactive, &p);
                d.rect = r.response.rect;
                d.hovered = r.hovered;
                d.sector = r.hovered_sector;
                if let Some(a) = r.dragged_to {
                    d.dragged.push(a);
                }
            },
            Dial {
                interactive,
                rect: Rect::NOTHING,
                hovered: false,
                sector: None,
                dragged: Vec::new(),
            },
        )
}

/// A point on the arrow at `deg`, a third of the square from the centre (inside the disc).
fn on_arrow(h: &Harness<'_, Dial>, deg: f64) -> Pos2 {
    let rect = h.state().rect;
    let r = rect.width() * 0.3;
    rect.center() + Vec2::new(deg.to_radians().cos() as f32, deg.to_radians().sin() as f32) * r
}

fn cursor(h: &Harness<'_, Dial>) -> CursorIcon {
    h.output().platform_output.cursor_icon
}

#[test]
fn hovering_the_draggable_arrow_shows_a_hand_and_dragging_turns_the_axis() {
    let mut h = dial(true);
    h.run();
    assert_eq!(cursor(&h), CursorIcon::Default, "no pointer yet");
    assert!(!h.state().hovered);

    let tip = on_arrow(&h, 60.0);
    h.hover_at(tip);
    h.run();
    assert_eq!(cursor(&h), CursorIcon::Grab);
    assert!(h.state().hovered, "the arrow is highlighted");
    assert_eq!(
        h.state().sector,
        Some(AxisClass::Normal),
        "the sector under the pointer"
    );

    // outside the disc (a corner of the square) the hand goes away
    let corner = h.state().rect.min + Vec2::splat(4.0);
    h.hover_at(corner);
    h.run();
    assert_eq!(cursor(&h), CursorIcon::Default);
    assert!(!h.state().hovered);
    assert_eq!(h.state().sector, None, "outside the disc no sector is shown");

    // press on the arrow, move: the axis follows the pointer and the hand closes
    h.drag_at(tip);
    h.run();
    let target = on_arrow(&h, -45.0);
    h.hover_at(target);
    h.run();
    assert_eq!(cursor(&h), CursorIcon::Grabbing);
    let last = *h.state().dragged.last().expect("a drag reported an angle");
    assert!((last - -45.0).abs() < 1.0, "{last}");
    h.event(egui::Event::PointerButton {
        pos: target,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    h.run();
    h.hover_at(target);
    h.run();
    assert_eq!(cursor(&h), CursorIcon::Grab, "released: an open hand again");
}

#[test]
fn the_read_only_circle_has_no_hand_and_reports_no_drag() {
    let mut h = dial(false);
    h.run();
    let tip = on_arrow(&h, 60.0);
    h.hover_at(tip);
    h.run();
    assert_eq!(cursor(&h), CursorIcon::Default);
    assert!(!h.state().hovered);
    assert_eq!(
        h.state().sector,
        Some(AxisClass::Normal),
        "the sector hover works read-only too"
    );
    h.hover_at(on_arrow(&h, -60.0));
    h.run();
    assert_eq!(h.state().sector, Some(AxisClass::LeftDeviation));
    h.hover_at(on_arrow(&h, 135.0));
    h.run();
    assert_eq!(h.state().sector, Some(AxisClass::RightDeviation));
    h.hover_at(on_arrow(&h, -135.0));
    h.run();
    assert_eq!(h.state().sector, Some(AxisClass::ExtremeDeviation));
    h.drag_at(tip);
    h.run();
    h.hover_at(on_arrow(&h, -45.0));
    h.run();
    assert_eq!(cursor(&h), CursorIcon::Default);
    assert_eq!(h.state().dragged, Vec::<f64>::new());
}

fn panel(axis: Option<f64>) -> Harness<'static> {
    Harness::builder()
        .with_size(Vec2::new(700.0, 700.0))
        .build_ui(move |ui| {
            let cfg = AxisPanel {
                axis,
                guess: None,
                interactive: false,
                max_side: 360.0,
                show_cards: true,
                show_torso: true,
                note: None,
            };
            axis_panel(ui, &cfg);
        })
}

#[test]
fn the_cards_and_the_legend_name_the_class_the_angle_and_every_range() {
    i18n::set_language("en");
    let mut h = panel(Some(60.0));
    h.run();
    let _ = h.get_by_label("Measured axis");
    let _ = h.get_by_label("+60°");
    let _ = h.get_by_label("Interpretation");
    let _ = h.get_by_label_contains("a normal axis");
    // the legend: the four classes with their ranges, in the words of the catalogue
    for name in [
        "Left axis deviation",
        "Right axis deviation",
        "Extreme axis (indeterminate)",
    ] {
        let _ = h.get_by_label(name);
    }
    assert!(
        h.get_all_by_label("Normal axis").count() >= 2,
        "in the pill and in the legend"
    );
    for range in ["−30° to +90°", "−30° to −90°", "+90° to +180°", "−90° to ±180°"] {
        let _ = h.get_by_label(range);
    }
    // the polarity table: lead II is positive, aVL equiphasic, aVR negative, as glyph and word
    let _ = h.get_by_label("Net QRS polarity");
    assert_eq!(h.get_all_by_label("positive").count(), 4, "I, II, III, aVF");
    let _ = h.get_by_label("equiphasic");
    let _ = h.get_by_label("negative");
    let _ = h.get_by_label("±");
    assert_eq!(h.get_all_by_label("+").count(), 4);
}

#[test]
fn without_a_qrs_the_badge_is_empty_and_in_italian_the_cards_are_translated() {
    i18n::set_language("en");
    let mut h = panel(None);
    h.run();
    let _ = h.get_by_label("—");
    assert!(h.get_all_by_label("No QRS complexes yet").count() >= 1);
    assert!(h.query_by_label("Interpretation").is_some());

    i18n::set_language("it");
    h.run();
    let _ = h.get_by_label("Asse misurato");
    let _ = h.get_by_label("Interpretazione");
    let _ = h.get_by_label("da −30° a +90°");
    i18n::set_language("en");
}
