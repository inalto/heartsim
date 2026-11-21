//! The five auscultation sites as clickable markers on the chest (brief B27, part B): the torso
//! picture of the axis panel's Einthoven figure ([`crate::torso_schematic`]), cropped to the thorax,
//! with a marker at each site's intercostal landmark. A marker's position comes from the electrode
//! editor's landmark tables ([`ListeningSite::landmark_mm`]: height of the intercostal space, azimuth
//! of the vertical line), projected onto the frontal plane through the procedural torso's surface,
//! so the drawing and the electrode editor agree on where the 2nd space or the mid-clavicular line
//! is.

use crate::t;
use crate::theme::Palette;
use crate::torso_schematic::paint_torso;
use egui::{Align2, FontId, Pos2, Rect, Response, Role, Sense, Stroke, Ui, Vec2, WidgetInfo};
use glam::Vec3;
use heartsim_ecg::ListeningSite;
use heartsim_geometry::procedural::torso::{TORSO_CENTRE_MM, TORSO_HALF_EXTENTS_MM, torso_implicit};

/// The heart-frame region the drawing shows, `(centre, half extents)` in mm: the thorax from the
/// clavicles to below the costal margin, the full chest width. Designed for legibility.
const VIEW_HALF_MM: [f32; 2] = [175.0, 170.0];
const VIEW_CENTRE_Y_MM: f32 = 35.0;

/// Frontal position `[x, y]` (heart-frame mm) of a site: at the height of its intercostal space,
/// where the ray from the torso axis along its vertical line's azimuth meets the procedural torso's
/// surface. `None` for the overview.
#[must_use]
pub fn site_position_mm(site: ListeningSite) -> Option<[f32; 2]> {
    let (y, az_deg) = site.landmark_mm()?;
    let (sin, cos) = az_deg.to_radians().sin_cos();
    let c = TORSO_CENTRE_MM;
    let inside = |r: f32| torso_implicit(Vec3::new(c.x + r * sin, y, c.z + r * cos)) < 0.0;
    let (mut lo, mut hi) = (0.0f32, 2.0 * TORSO_HALF_EXTENTS_MM.x.max(TORSO_HALF_EXTENTS_MM.z));
    if !inside(lo) {
        return None;
    }
    for _ in 0..40 {
        let mid = f32::midpoint(lo, hi);
        if inside(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some([c.x + f32::midpoint(lo, hi) * sin, y])
}

/// The site's full name ("Mitral (apex)").
#[must_use]
pub fn site_name(site: ListeningSite) -> String {
    match site {
        ListeningSite::Overview => t!("ecg-site-overview"),
        ListeningSite::Aortic => t!("ecg-site-aortic"),
        ListeningSite::Pulmonic => t!("ecg-site-pulmonic"),
        ListeningSite::Erb => t!("ecg-site-erb"),
        ListeningSite::Tricuspid => t!("ecg-site-tricuspid"),
        ListeningSite::Mitral => t!("ecg-site-mitral"),
    }
}

/// The site's short name, for the lane title ("Mitral").
#[must_use]
pub fn site_short_name(site: ListeningSite) -> String {
    match site {
        ListeningSite::Overview => t!("ecg-site-short-overview"),
        ListeningSite::Aortic => t!("ecg-site-short-aortic"),
        ListeningSite::Pulmonic => t!("ecg-site-short-pulmonic"),
        ListeningSite::Erb => t!("ecg-site-short-erb"),
        ListeningSite::Tricuspid => t!("ecg-site-short-tricuspid"),
        ListeningSite::Mitral => t!("ecg-site-short-mitral"),
    }
}

/// The letter on the site's marker (A, P, E, T, M).
fn site_letter(site: ListeningSite) -> String {
    match site {
        ListeningSite::Overview => String::new(),
        ListeningSite::Aortic => t!("ecg-site-letter-aortic"),
        ListeningSite::Pulmonic => t!("ecg-site-letter-pulmonic"),
        ListeningSite::Erb => t!("ecg-site-letter-erb"),
        ListeningSite::Tricuspid => t!("ecg-site-letter-tricuspid"),
        ListeningSite::Mitral => t!("ecg-site-letter-mitral"),
    }
}

/// The chest with the five site markers in a box of `size`; a click on a marker selects that site
/// in `site` (the selected one is filled). `image_opacity` is the torso picture's (`0.0` draws the
/// procedural outline). Each marker is a radio button for screen readers, named
/// "<site> on the chest". Returns the response of the whole drawing, marked changed when a marker
/// was clicked.
pub fn auscultation_chest(ui: &mut Ui, size: Vec2, site: &mut ListeningSite, image_opacity: f32) -> Response {
    let p = Palette::of(ui);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    let view = (
        [TORSO_CENTRE_MM.x, VIEW_CENTRE_Y_MM],
        [VIEW_HALF_MM[0], VIEW_HALF_MM[1]],
    );
    let frame = paint_torso(ui, &painter, rect, image_opacity, Some(view));
    let radius = (rect.height() * 0.055).clamp(7.0, 11.0);
    let font = FontId::proportional(radius * 1.15);
    for s in ListeningSite::SITES {
        let Some([x, y]) = site_position_mm(s) else {
            continue;
        };
        let centre = frame.to_screen(x, y);
        let hit = Rect::from_center_size(centre, Vec2::splat(2.0 * radius + 4.0));
        let id = response.id.with(s.id());
        let r = ui.interact(hit, id, Sense::click());
        let selected = *site == s;
        let name = t!("ecg-site-marker", site = site_name(s));
        r.widget_info(|| WidgetInfo::selected(Role::RadioButton, true, selected, &name));
        let (fill, text, stroke) = if selected {
            (p.accent_strong, p.card, Stroke::new(2.0, p.card))
        } else if r.hovered() {
            (p.accent_soft, p.heading, Stroke::new(1.5, p.accent_strong))
        } else {
            (p.card, p.heading, Stroke::new(1.2, p.accent_strong))
        };
        painter.circle(centre, radius, fill, stroke);
        painter.text(centre, Align2::CENTER_CENTER, site_letter(s), font.clone(), text);
        if r.clicked() && !selected {
            *site = s;
            response.mark_changed();
        }
        r.on_hover_text(site_name(s));
    }
    response
}

/// Screen position of `site`'s marker in a drawing laid out in `rect` (tests).
#[must_use]
pub fn marker_position(rect: Rect, site: ListeningSite) -> Option<Pos2> {
    let [x, y] = site_position_mm(site)?;
    let half = VIEW_HALF_MM;
    let scale = (rect.height() / (2.0 * half[1])).min(rect.width() / (2.0 * half[0]));
    let c = rect.center();
    Some(Pos2::new(
        c.x + (x - TORSO_CENTRE_MM.x) * scale,
        c.y - (y - VIEW_CENTRE_Y_MM) * scale,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sites_sit_where_the_textbook_puts_them() {
        let pos = |s| site_position_mm(s).unwrap_or_else(|| panic!("{s:?}"));
        let mid = TORSO_CENTRE_MM.x;
        let [ao, pu, erb, tri, mi] = ListeningSite::SITES.map(pos);
        // aortic on the patient's right of the sternum, the others on the left
        assert!(ao[0] < mid && pu[0] > mid && erb[0] > mid && tri[0] > mid && mi[0] > mid);
        // aortic and pulmonic mirror each other in the 2nd space, Erb's point below the pulmonic
        // area, the tricuspid area below that, all on the left sternal border
        assert!(((ao[0] - mid) + (pu[0] - mid)).abs() < 1.0);
        assert!((ao[1] - pu[1]).abs() < 1e-3);
        assert!(pu[1] > erb[1] && erb[1] > tri[1] && tri[1] > mi[1]);
        assert!((pu[0] - erb[0]).abs() < 1.0 && (erb[0] - tri[0]).abs() < 1.0);
        // the apex is the lowest and furthest left, near the heart's own apex (left of and below the
        // frame origin)
        assert!(mi[0] > tri[0] + 30.0, "{mi:?}");
        assert!(mi[0] > 0.0 && mi[1] < 0.0, "{mi:?}");
        assert_eq!(site_position_mm(ListeningSite::Overview), None);
    }
}
