//! Vectorcardiogram planes (brief B8 §2): the heart-vector loop of the last beat and the
//! instantaneous heart vector, projected on the frontal and horizontal planes with the lead axes
//! drawn for reference.
//!
//! Screen conventions follow the clinical diagrams so the loop reads like the axis circle:
//! frontal plane seen from the front with the patient's left drawn to the right and the feet
//! down (0° = lead I, +90° = aVF, as [`crate::axis_dial`]); horizontal plane seen from above with
//! the patient's left to the right and anterior down (V1–V2 point down, V6 to the right).

use egui::{Align2, Color32, FontId, Pos2, Sense, Shape, Stroke, Ui, Vec2};
use glam::DVec3;
use heartsim_core::Lead;
use heartsim_ecg::LeadVectors;

/// A projection plane of the vectorcardiogram.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorPlane {
    /// x–y: patient's left → right, cranial → up.
    Frontal,
    /// x–z: patient's left → right, anterior → down (viewed from the head).
    Horizontal,
}

impl VectorPlane {
    pub const ALL: [VectorPlane; 2] = [VectorPlane::Frontal, VectorPlane::Horizontal];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            VectorPlane::Frontal => "Frontal plane",
            VectorPlane::Horizontal => "Horizontal plane",
        }
    }

    /// Screen offset of a heart-frame vector (egui frame: +x right, +y down), before scaling.
    #[must_use]
    pub fn project(self, v: DVec3) -> Vec2 {
        match self {
            VectorPlane::Frontal => Vec2::new(v.x as f32, -v.y as f32),
            VectorPlane::Horizontal => Vec2::new(v.x as f32, v.z as f32),
        }
    }

    /// Leads whose axes are drawn on this plane.
    #[must_use]
    pub fn reference_leads(self) -> &'static [Lead] {
        match self {
            VectorPlane::Frontal => &[Lead::I, Lead::II, Lead::III, Lead::AVR, Lead::AVL, Lead::AVF],
            VectorPlane::Horizontal => &[Lead::V1, Lead::V2, Lead::V3, Lead::V4, Lead::V5, Lead::V6],
        }
    }
}

/// Colours of the vector planes.
#[derive(Debug, Clone, Copy)]
pub struct VectorViewStyle {
    pub background: Color32,
    pub axes: Color32,
    pub text: Color32,
    /// Loop colour at the start of the beat; it fades to `loop_end` along the loop.
    pub loop_start: Color32,
    pub loop_end: Color32,
    pub arrow: Color32,
}

impl Default for VectorViewStyle {
    fn default() -> Self {
        Self {
            background: Color32::from_gray(18),
            axes: Color32::from_gray(70),
            text: Color32::from_gray(200),
            loop_start: Color32::from_rgb(90, 200, 255),
            loop_end: Color32::from_rgb(230, 90, 200),
            arrow: Color32::from_rgb(255, 210, 60),
        }
    }
}

/// Smallest full-scale radius, mV, so a flat line does not blow up to fill the plane.
pub const MIN_FULL_SCALE_MV: f64 = 0.5;

/// Full-scale radius (mV) for showing `loop_mv` and `now`: the largest magnitude, rounded up to
/// the next 0.5 mV.
#[must_use]
pub fn full_scale_mv(loop_mv: &[DVec3], now: Option<DVec3>) -> f64 {
    let m = loop_mv
        .iter()
        .chain(now.iter())
        .map(|v| v.length())
        .fold(0.0_f64, f64::max);
    ((m / 0.5).ceil() * 0.5).max(MIN_FULL_SCALE_MV)
}

fn lerp_colour(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Draw one plane in a square of side `side`: lead axes, the loop (`loop_mv`, oldest first) and
/// the instantaneous vector `now` as an arrow. `full_scale_mv` is the radius of the outer ring.
pub fn vector_plane(
    ui: &mut Ui,
    side: f32,
    plane: VectorPlane,
    loop_mv: &[DVec3],
    now: Option<DVec3>,
    full_scale_mv: f64,
    style: &VectorViewStyle,
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, style.background);
    let centre = rect.center();
    let radius = side * 0.40;
    let scale = radius / full_scale_mv.max(1e-6) as f32;
    let axis_stroke = Stroke::new(1.0, style.axes);
    painter.circle_stroke(centre, radius, axis_stroke);
    painter.circle_stroke(centre, radius * 0.5, Stroke::new(0.5, style.axes));
    let lv = LeadVectors::standard();
    for &lead in plane.reference_leads() {
        let d = plane.project(lv.vectors[lead as usize]);
        let d = if d.length() > 1e-6 {
            d.normalized()
        } else {
            continue;
        };
        let tip = centre + d * radius;
        painter.line_segment([centre - d * radius, tip], axis_stroke);
        painter.text(
            centre + d * (radius + 14.0),
            Align2::CENTER_CENTER,
            lead.label(),
            FontId::proportional(12.0),
            style.text,
        );
    }
    painter.text(
        rect.left_top() + Vec2::new(6.0, 4.0),
        Align2::LEFT_TOP,
        plane.label(),
        FontId::proportional(13.0),
        style.text,
    );
    painter.text(
        rect.right_bottom() - Vec2::new(6.0, 4.0),
        Align2::RIGHT_BOTTOM,
        format!("ring = {full_scale_mv:.1} mV"),
        FontId::proportional(11.0),
        style.text,
    );
    let at = |v: DVec3| -> Pos2 { centre + plane.project(v) * scale };
    let n = loop_mv.len();
    for (i, w) in loop_mv.windows(2).enumerate() {
        let t = i as f32 / n.max(2) as f32;
        painter.line_segment(
            [at(w[0]), at(w[1])],
            Stroke::new(2.0, lerp_colour(style.loop_start, style.loop_end, t)),
        );
    }
    if let Some(v) = now {
        let tip = at(v);
        let d = tip - centre;
        if d.length() > 2.0 {
            let u = d.normalized();
            let head = 9.0_f32.min(d.length() * 0.5);
            let side_v = Vec2::new(-u.y, u.x) * head * 0.45;
            let neck = tip - u * head;
            painter.line_segment([centre, neck], Stroke::new(3.0, style.arrow));
            painter.add(Shape::convex_polygon(
                vec![tip, neck + side_v, neck - side_v],
                style.arrow,
                Stroke::NONE,
            ));
        }
        painter.circle_filled(centre, 3.0, style.arrow);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontal_plane_matches_the_hexaxial_screen_angle() {
        // hexaxial θ is drawn clockwise from the right (axis_dial): screen angle = θ
        for deg in [-90.0_f64, -30.0, 0.0, 60.0, 90.0, 150.0] {
            let p = VectorPlane::Frontal.project(LeadVectors::frontal(deg));
            let screen = f64::from(p.y).atan2(f64::from(p.x)).to_degrees();
            assert!((screen - deg).abs() < 1e-4, "{deg}° drawn at {screen}°");
        }
    }

    #[test]
    fn horizontal_plane_puts_anterior_down_and_left_right() {
        let p = VectorPlane::Horizontal;
        assert_eq!(p.project(DVec3::Z), Vec2::new(0.0, 1.0));
        assert_eq!(p.project(DVec3::X), Vec2::new(1.0, 0.0));
        assert_eq!(p.project(DVec3::Y), Vec2::ZERO, "cranial is out of plane");
        let v6 = p.project(LeadVectors::standard().vectors[Lead::V6 as usize]);
        assert!(v6.x > 0.9, "V6 points to the patient's left: {v6:?}");
    }

    #[test]
    fn full_scale_rounds_up_to_half_millivolts() {
        assert!((full_scale_mv(&[], None) - MIN_FULL_SCALE_MV).abs() < 1e-12);
        let l = [DVec3::new(1.2, 0.0, 0.0), DVec3::new(0.0, -0.3, 0.0)];
        assert!((full_scale_mv(&l, None) - 1.5).abs() < 1e-12);
        assert!((full_scale_mv(&l, Some(DVec3::new(0.0, 2.1, 0.0))) - 2.5).abs() < 1e-12);
    }
}
