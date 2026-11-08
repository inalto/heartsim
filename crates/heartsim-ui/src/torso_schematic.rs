//! A small frontal schematic of the torso: the three limb electrodes where the procedural torso
//! puts them, the Einthoven triangle between them, and the six limb leads as pills at their
//! hexaxial angles around the heart. The outline is sampled from the torso's implicit surface and
//! the angles come from [`Lead::frontal_angle_deg`], so nothing here is drawn by hand with numbers
//! that could disagree with the circle.

use crate::axis_dial::degrees_text;
use crate::cards::paint_pill;
use crate::theme::Palette;
use egui::{Color32, FontId, Pos2, Response, Sense, Shape, Stroke, Ui, Vec2};
use glam::Vec3;
use heartsim_core::{Electrode, Lead};
use heartsim_geometry::procedural::torso::{
    TORSO_CENTRE_MM, TORSO_HALF_EXTENTS_MM, procedural_electrodes, torso_implicit,
};
use std::sync::OnceLock;

/// The frontal silhouette and the limb electrodes, heart-frame millimetres `[x, y]`.
struct Silhouette {
    /// Closed outline, down the patient's left side then up the right.
    outline_mm: Vec<[f32; 2]>,
    electrodes_mm: [(Electrode, [f32; 2]); 3],
}

fn silhouette() -> &'static Silhouette {
    static CACHE: OnceLock<Silhouette> = OnceLock::new();
    CACHE.get_or_init(|| {
        let c = TORSO_CENTRE_MM;
        let h = TORSO_HALF_EXTENTS_MM;
        let rows = 40;
        let mut left_side = Vec::new();
        for k in 0..=rows {
            let y = c.y + h.y - 2.0 * h.y * (k as f32) / (rows as f32);
            if torso_implicit(Vec3::new(c.x, y, c.z)) >= 0.0 {
                continue;
            }
            // bisection on the half-width at this height, in the frontal mid-plane
            let (mut lo, mut hi) = (0.0f32, 1.5 * h.x);
            for _ in 0..30 {
                let mid = f32::midpoint(lo, hi);
                if torso_implicit(Vec3::new(c.x + mid, y, c.z)) < 0.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            left_side.push([c.x + f32::midpoint(lo, hi), y]);
        }
        let mut outline_mm = left_side.clone();
        outline_mm.extend(left_side.iter().rev().map(|[x, y]| [2.0 * c.x - x, *y]));
        let set = procedural_electrodes();
        let site = |e: Electrode| {
            let p = set.standard(e).unwrap_or(Vec3::ZERO);
            (e, [p.x, p.y])
        };
        Silhouette {
            outline_mm,
            electrodes_mm: [site(Electrode::RA), site(Electrode::LA), site(Electrode::LL)],
        }
    })
}

/// Draws the schematic in a box of `size`. The heart (the frame origin) is the small disc; the
/// lead pills sit around it at their frontal-plane angles.
pub fn torso_schematic(ui: &mut Ui, size: Vec2) -> Response {
    let p = Palette::of(ui);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    let s = silhouette();
    let c = TORSO_CENTRE_MM;
    let h = TORSO_HALF_EXTENTS_MM;
    let scale = (rect.height() * 0.9 / (2.0 * h.y)).min(rect.width() * 0.55 / (2.0 * h.x));
    let centre = rect.center();
    let to_screen =
        |x_mm: f32, y_mm: f32| Pos2::new(centre.x + (x_mm - c.x) * scale, centre.y - (y_mm - c.y) * scale);

    // the torso
    let outline: Vec<Pos2> = s.outline_mm.iter().map(|[x, y]| to_screen(*x, *y)).collect();
    painter.add(Shape::convex_polygon(
        outline,
        p.accent_soft,
        Stroke::new(1.0, p.card_stroke),
    ));

    // the Einthoven triangle and its electrodes
    let sites: Vec<Pos2> = s
        .electrodes_mm
        .iter()
        .map(|(_, [x, y])| to_screen(*x, *y))
        .collect();
    painter.add(Shape::closed_line(sites.clone(), Stroke::new(1.0, p.grid)));
    let site_font = FontId::proportional((rect.height() * 0.045).clamp(8.0, 10.0));
    for ((e, _), pos) in s.electrodes_mm.iter().zip(&sites) {
        painter.circle(*pos, 3.5, p.card, Stroke::new(1.5, p.ring));
        let label_at = *pos + Vec2::new(if *e == Electrode::RA { -12.0 } else { 12.0 }, 0.0);
        painter.text(
            label_at,
            egui::Align2::CENTER_CENTER,
            e.label(),
            site_font.clone(),
            p.muted,
        );
    }

    // the heart and the six leads at their angles around it
    let heart = to_screen(0.0, 0.0);
    let reach = (rect.width() * 0.5 - 22.0).min(rect.height() * 0.36).max(30.0);
    let pill_font = FontId::proportional((rect.height() * 0.05).clamp(9.0, 11.0));
    for lead in Lead::ALL.iter().filter(|l| l.is_limb()) {
        let deg = lead.frontal_angle_deg().unwrap_or(0.0);
        let dir = Vec2::new(deg.to_radians().cos() as f32, deg.to_radians().sin() as f32);
        let pill_at = heart + dir * reach;
        painter.line_segment([heart, pill_at], Stroke::new(1.0, p.grid));
        let lines = [
            (lead.label().to_owned(), pill_font.clone(), p.heading),
            (degrees_text(deg.round() as i32, true), pill_font.clone(), p.muted),
        ];
        paint_pill(&painter, pill_at, &lines, p.pill, Some(p.pill_stroke));
    }
    let bad = p.bad();
    painter.circle(heart, 6.0, bad.strong, Stroke::new(1.5, p.card));
    painter.circle_filled(heart + Vec2::new(-1.5, -1.5), 1.6, Color32::from_white_alpha(140));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_silhouette_is_a_closed_symmetric_outline_with_the_electrodes_inside() {
        let s = silhouette();
        assert!(s.outline_mm.len() > 40, "{}", s.outline_mm.len());
        let n = s.outline_mm.len() / 2;
        for k in 0..n {
            let [xl, yl] = s.outline_mm[k];
            let [xr, yr] = s.outline_mm[s.outline_mm.len() - 1 - k];
            assert!((yl - yr).abs() < 1e-3);
            assert!(((xl - TORSO_CENTRE_MM.x) + (xr - TORSO_CENTRE_MM.x)).abs() < 1e-2);
            assert!(xl >= TORSO_CENTRE_MM.x, "left side is on the patient's left (+x)");
        }
        let widest = s
            .outline_mm
            .iter()
            .map(|[x, _]| x - TORSO_CENTRE_MM.x)
            .fold(0.0, f32::max);
        assert!((widest - TORSO_HALF_EXTENTS_MM.x).abs() < 1.0, "{widest}");
        for (e, [x, y]) in s.electrodes_mm {
            assert!(x.abs() < 400.0 && y.abs() < 400.0, "{e:?} at {x}, {y}");
        }
        // RA is on the patient's right (−x), LA on the left, LL below both
        let [ra, la, ll] = s.electrodes_mm.map(|(_, p)| p);
        assert!(ra[0] < la[0]);
        assert!(ll[1] < ra[1] && ll[1] < la[1]);
    }
}
