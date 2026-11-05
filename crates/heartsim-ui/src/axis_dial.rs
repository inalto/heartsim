//! Hexaxial (Cabrera) reference circle: the six frontal-plane leads, the clinical axis zones and
//! the measured or guessed QRS axis as an arrow. Screen frame: 0° = lead I (patient's left, drawn
//! to the right as in a frontal view), +90° = aVF (down), −90° = up.

use crate::t;
use egui::{Align2, Color32, FontId, Pos2, Sense, Shape, Stroke, Ui, Vec2};
use heartsim_core::{AxisClass, FrontalAxis, Lead};

/// Colours and sizes of the dial.
#[derive(Debug, Clone, Copy)]
pub struct AxisDialStyle {
    pub arrow: Color32,
    pub guess: Color32,
    pub text: Color32,
}

impl Default for AxisDialStyle {
    fn default() -> Self {
        Self {
            arrow: Color32::from_rgb(255, 210, 60),
            guess: Color32::from_rgb(90, 200, 255),
            text: Color32::from_gray(210),
        }
    }
}

fn zone_colour(class: AxisClass) -> Color32 {
    match class {
        AxisClass::Normal => Color32::from_rgba_unmultiplied(60, 170, 90, 70),
        AxisClass::LeftDeviation => Color32::from_rgba_unmultiplied(220, 170, 50, 70),
        AxisClass::RightDeviation => Color32::from_rgba_unmultiplied(220, 120, 50, 70),
        AxisClass::ExtremeDeviation => Color32::from_rgba_unmultiplied(200, 60, 60, 70),
    }
}

fn polar(centre: Pos2, radius: f32, deg: f64) -> Pos2 {
    let r = deg.to_radians();
    centre + Vec2::new(r.cos() as f32, r.sin() as f32) * radius
}

/// Draw the dial in a square of side `side`. `axis` is the measured axis, `guess` the user's
/// answer. Returns the angle (hexaxial degrees, −180..180) the user clicked or dragged to, if any.
pub fn axis_dial(
    ui: &mut Ui,
    side: f32,
    axis: Option<f64>,
    guess: Option<f64>,
    interactive: bool,
    style: &AxisDialStyle,
) -> Option<f64> {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::splat(side),
        if interactive {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        },
    );
    let centre = rect.center();
    let radius = side * 0.40;
    let painter = ui.painter_at(rect);

    // clinical zones as pie wedges
    for (from, to, class) in [
        (-30.0, 90.0, AxisClass::Normal),
        (-90.0, -30.0, AxisClass::LeftDeviation),
        (90.0, 180.0, AxisClass::RightDeviation),
        (-180.0, -90.0, AxisClass::ExtremeDeviation),
    ] {
        let mut pts = vec![centre];
        let steps = 24;
        for k in 0..=steps {
            pts.push(polar(
                centre,
                radius,
                from + (to - from) * f64::from(k) / f64::from(steps),
            ));
        }
        painter.add(Shape::convex_polygon(pts, zone_colour(class), Stroke::NONE));
    }
    painter.circle_stroke(centre, radius, Stroke::new(1.5, Color32::from_gray(150)));

    // tick marks every 30° with degree labels
    for k in -5..=6 {
        let deg = f64::from(k) * 30.0;
        painter.line_segment(
            [
                polar(centre, radius * 0.96, deg),
                polar(centre, radius * 1.03, deg),
            ],
            Stroke::new(1.0, Color32::from_gray(150)),
        );
        painter.text(
            polar(centre, radius * 1.17, deg),
            Align2::CENTER_CENTER,
            format!("{}°", deg as i32),
            FontId::proportional(10.0),
            Color32::from_gray(140),
        );
    }

    // lead axes: positive pole labelled
    for lead in Lead::ALL.iter().filter(|l| l.is_limb()) {
        let deg = lead.frontal_angle_deg().unwrap_or(0.0);
        painter.line_segment(
            [polar(centre, radius, deg), polar(centre, radius, deg + 180.0)],
            Stroke::new(1.0, Color32::from_gray(95)),
        );
        painter.text(
            polar(centre, radius * 0.86, deg),
            Align2::CENTER_CENTER,
            lead.label(),
            FontId::proportional(13.0),
            style.text,
        );
    }

    let arrow = |deg: f64, colour: Color32, width: f32| {
        let tip = polar(centre, radius * 0.97, deg);
        painter.line_segment([centre, tip], Stroke::new(width, colour));
        for side in [150.0, -150.0] {
            painter.line_segment([tip, polar(tip, 11.0, deg + side)], Stroke::new(width, colour));
        }
    };
    if let Some(g) = guess {
        arrow(g, style.guess, 2.0);
    }
    if let Some(a) = axis {
        arrow(a, style.arrow, 3.0);
        let class = FrontalAxis { degrees: a }.classify();
        painter.text(
            centre + Vec2::new(0.0, radius * 1.42),
            Align2::CENTER_CENTER,
            format!("{:+.0}°  {}", FrontalAxis::normalise(a), class.label()),
            FontId::proportional(15.0),
            style.arrow,
        );
    }
    painter.add(Shape::circle_filled(centre, 3.5, Color32::WHITE));

    if interactive
        && (response.clicked() || response.dragged())
        && let Some(p) = response.interact_pointer_pos()
    {
        let d = p - centre;
        if d.length() > 6.0 {
            return Some(FrontalAxis::normalise(
                f64::from(d.y).atan2(f64::from(d.x)).to_degrees(),
            ));
        }
    }
    None
}

/// Net QRS polarity a frontal axis produces in a limb lead: `cos(axis − θ_lead)`. A lead nearly
/// perpendicular to the axis (|cos| < 0.2, i.e. within ~12°) is equiphasic: the classic trick for
/// finding the axis is "the equiphasic lead is perpendicular to it".
#[must_use]
pub fn lead_polarity(axis_deg: f64, lead: Lead) -> String {
    match (FrontalAxis { degrees: axis_deg }).projection_onto(lead) {
        Some(c) if c > 0.2 => t!("polarity-positive"),
        Some(c) if c < -0.2 => t!("polarity-negative"),
        Some(_) => t!("polarity-equiphasic"),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polarity_follows_the_lead_axes() {
        assert_eq!(lead_polarity(60.0, Lead::II), "positive");
        assert_eq!(lead_polarity(60.0, Lead::AVL), "equiphasic");
        assert_eq!(lead_polarity(60.0, Lead::AVR), "negative");
        assert_eq!(lead_polarity(-90.0, Lead::AVF), "negative");
        assert_eq!(lead_polarity(0.0, Lead::AVF), "equiphasic");
    }
}
