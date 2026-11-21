//! A small orientation gizmo for the 3-D views: the patient's three body axes, drawn from the
//! camera's basis, so the learner always knows which way they are looking.
//!
//! The heart frame is +x = the patient's left, +y = cranial (superior), +z = anterior. Each axis
//! has a positive end (L, S, A) drawn as a filled disc and a negative end (R, I, P) drawn as an
//! outlined one. The letters are the anatomical abbreviations and are not translated; only the
//! tooltip is. The gizmo is painted with the egui painter and senses hovering only, so it never
//! takes a click or a drag meant for the 3-D view below it.

use crate::t;
use crate::theme::Palette;
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use glam::Vec3;

/// Where in the 3-D view the gizmo sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoCorner {
    TopRight,
    BottomRight,
    BottomLeft,
}

/// One end of one body axis, projected for the screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GizmoEnd {
    /// L, S, A (positive ends) or R, I, P.
    pub letter: &'static str,
    /// Positive end of its axis (the patient's left, cranial, anterior)?
    pub positive: bool,
    /// Axis index: 0 = left-right, 1 = superior-inferior, 2 = anterior-posterior.
    pub axis: usize,
    /// Screen offset from the gizmo centre in units of the arm length: +x right, +y down.
    pub screen: Vec2,
    /// Distance towards the viewer: > 0 is nearer than the centre.
    pub toward_viewer: f32,
}

/// The six axis ends for a camera with the basis `(right, up, back)` (see
/// `OrbitCamera::basis`; `back` points from the target to the eye), nearest last so the nearer ends
/// are drawn over the farther ones.
#[must_use]
pub fn gizmo_ends(basis: (Vec3, Vec3, Vec3)) -> [GizmoEnd; 6] {
    let (right, up, back) = basis;
    let axes = [(Vec3::X, "L", "R"), (Vec3::Y, "S", "I"), (Vec3::Z, "A", "P")];
    let mut ends = [GizmoEnd {
        letter: "",
        positive: true,
        axis: 0,
        screen: Vec2::ZERO,
        toward_viewer: 0.0,
    }; 6];
    for (i, (dir, pos, neg)) in axes.into_iter().enumerate() {
        for (k, sign) in [1.0_f32, -1.0].into_iter().enumerate() {
            let d = dir * sign;
            ends[2 * i + k] = GizmoEnd {
                letter: if k == 0 { pos } else { neg },
                positive: k == 0,
                axis: i,
                screen: Vec2::new(d.dot(right), -d.dot(up)),
                toward_viewer: d.dot(back),
            };
        }
    }
    ends.sort_by(|a, b| a.toward_viewer.total_cmp(&b.toward_viewer));
    ends
}

/// Size of the gizmo's square, points.
pub const GIZMO_SIZE: f32 = 72.0;

/// Draws the gizmo in a corner of `view` (the 3-D view's rect) for the camera `basis`. Returns
/// the rect it occupies. The tooltip names the axes in full.
pub fn orientation_gizmo(ui: &mut Ui, view: Rect, basis: (Vec3, Vec3, Vec3), corner: GizmoCorner) -> Rect {
    let margin = 8.0;
    let size = Vec2::splat(GIZMO_SIZE);
    let min = match corner {
        GizmoCorner::TopRight => Pos2::new(view.right() - margin - size.x, view.top() + margin),
        GizmoCorner::BottomRight => {
            Pos2::new(view.right() - margin - size.x, view.bottom() - margin - size.y)
        }
        GizmoCorner::BottomLeft => Pos2::new(view.left() + margin, view.bottom() - margin - size.y),
    };
    let rect = Rect::from_min_size(min, size);
    if rect.width() > view.width() || rect.height() > view.height() {
        return rect;
    }
    let p = Palette::of(ui);
    let centre = rect.center();
    let arm = GIZMO_SIZE / 2.0 - 11.0;
    let painter = ui.painter_at(rect.expand(2.0));
    painter.circle_filled(centre, GIZMO_SIZE / 2.0, p.card.gamma_multiply(0.7));
    painter.circle_stroke(centre, GIZMO_SIZE / 2.0, Stroke::new(1.0, p.card_stroke));
    let axis_colour = |axis: usize| -> (Color32, Color32) {
        let c = match axis {
            0 => p.class(heartsim_core::AxisClass::Normal),
            1 => p.class(heartsim_core::AxisClass::RightDeviation),
            _ => p.class(heartsim_core::AxisClass::LeftDeviation),
        };
        (c.strong, c.on_strong)
    };
    for end in gizmo_ends(basis) {
        let to = centre + end.screen * arm;
        let (strong, on_strong) = axis_colour(end.axis);
        // ends pointing away from the viewer are drawn fainter, as depth
        let fade = if end.toward_viewer < -0.15 { 0.6 } else { 1.0 };
        painter.line_segment([centre, to], Stroke::new(1.5, strong.gamma_multiply(fade)));
        let r = if end.positive { 8.5 } else { 7.0 };
        if end.positive {
            painter.circle_filled(to, r, strong.gamma_multiply(fade));
            painter.text(
                to,
                Align2::CENTER_CENTER,
                end.letter,
                FontId::proportional(11.0),
                on_strong,
            );
        } else {
            painter.circle_filled(to, r, p.card.gamma_multiply(fade));
            painter.circle_stroke(to, r, Stroke::new(1.5, strong.gamma_multiply(fade)));
            painter.text(
                to,
                Align2::CENTER_CENTER,
                end.letter,
                FontId::proportional(10.0),
                p.text,
            );
        }
    }
    ui.interact(rect, ui.scope_id().with("orientation_gizmo"), Sense::hover())
        .on_hover_text(t!("orientation-hint"));
    rect
}

#[cfg(test)]
mod tests {
    use super::*;

    fn end(ends: &[GizmoEnd; 6], letter: &str) -> GizmoEnd {
        *ends.iter().find(|e| e.letter == letter).unwrap()
    }

    /// Looking at the patient from the front (camera on +z): the patient's left is on the
    /// viewer's right, cranial is up, and anterior points at the viewer.
    #[test]
    fn from_the_front_the_patients_left_is_on_the_right() {
        let ends = gizmo_ends((Vec3::X, Vec3::Y, Vec3::Z));
        assert!(end(&ends, "L").screen.x > 0.99);
        assert!(end(&ends, "R").screen.x < -0.99);
        assert!(
            end(&ends, "S").screen.y < -0.99,
            "cranial is up (y grows downwards)"
        );
        assert!(end(&ends, "A").toward_viewer > 0.99);
        assert!(end(&ends, "P").toward_viewer < -0.99);
    }

    /// From behind, left and right swap on the screen: the gizmo follows the camera.
    #[test]
    fn from_behind_left_and_right_swap() {
        let ends = gizmo_ends((-Vec3::X, Vec3::Y, -Vec3::Z));
        assert!(end(&ends, "L").screen.x < -0.99);
        assert!(end(&ends, "P").toward_viewer > 0.99);
    }

    #[test]
    fn nearest_ends_are_drawn_last() {
        let ends = gizmo_ends((Vec3::X, Vec3::Y, Vec3::Z));
        assert!(ends.windows(2).all(|w| w[0].toward_viewer <= w[1].toward_viewer));
    }
}
