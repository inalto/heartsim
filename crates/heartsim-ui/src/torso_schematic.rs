//! A small frontal schematic of the torso: Einthoven's figure. The limb electrodes are beads in
//! their cable colours on the shoulder tips and the groins; leads I, II and III are the sides of the
//! triangle between RA, LA and LL and the augmented leads run from its centroid to the electrodes,
//! each lead named by a pill at the middle of its line with its frontal angle, the one the circle
//! uses ([`Lead::frontal_angle_deg`]). The heart's electrical centre is the centroid of the
//! triangle. Behind it all sits a frontal picture of a torso, registered to the heart frame, at an
//! opacity the user chooses; at zero opacity the outline sampled from the procedural torso's
//! implicit surface is drawn instead.

use crate::axis_dial::degrees_text;
use crate::cards::paint_pill;
use crate::theme::Palette;
use egui::{
    Align2, Color32, ColorImage, Context, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke,
    TextureHandle, TextureOptions, Ui, Vec2,
};
use glam::Vec3;
use heartsim_core::{Electrode, Lead};
use heartsim_geometry::procedural::torso::{TORSO_CENTRE_MM, TORSO_HALF_EXTENTS_MM, torso_implicit};
use std::sync::OnceLock;

/// Where the figure draws the limb electrodes, heart-frame millimetres `[x, y]`: the classic
/// Einthoven placement of the teaching diagram, RA and LA on the shoulder tips (the acromia), LL on
/// the left groin and RL, the neutral, on the right groin. These are not the simulation's electrode
/// sites (on the procedural torso's chest and hips, shown in the electrode editor). Measured on the
/// registered picture: the deltoids end 192 mm either side of the midline (`TORSO_CENTRE_MM.x`) at
/// +195 mm, the inguinal creases are at −335 mm.
const ELECTRODES_MM: [(Electrode, [f32; 2]); 4] = [
    (Electrode::RA, [-240.0, 195.0]),
    (Electrode::LA, [110.0, 195.0]),
    (Electrode::LL, [-5.0, -335.0]),
    (Electrode::RL, [-125.0, -335.0]),
];

/// Einthoven's triangle, `[RA, LA, LL]`, and its centroid: the point the augmented leads radiate
/// from, where the figure puts the heart's electrical centre.
fn triangle_mm() -> ([[f32; 2]; 3], [f32; 2]) {
    let tri = [ELECTRODES_MM[0].1, ELECTRODES_MM[1].1, ELECTRODES_MM[2].1];
    let centroid = [
        (tri[0][0] + tri[1][0] + tri[2][0]) / 3.0,
        (tri[0][1] + tri[1][1] + tri[2][1]) / 3.0,
    ];
    (tri, centroid)
}

/// Cable colour of a limb electrode: the AHA code, the one the electrode editor's 3-D markers use
/// (RA white, LA black, LL red, RL green). The IEC code differs (red, yellow, green, black).
fn electrode_colour(e: Electrode) -> Color32 {
    match e {
        Electrode::RA => Color32::from_rgb(242, 242, 242),
        Electrode::LA => Color32::from_rgb(30, 30, 30),
        Electrode::LL => Color32::from_rgb(217, 31, 26),
        _ => Color32::from_rgb(26, 153, 46),
    }
}

/// A small shaded sphere in `colour`: a dark body, lighter discs shifted towards the light (upper
/// left) and a highlight, so the electrode reads as a bead on the skin rather than a flat dot.
fn paint_sphere(painter: &Painter, centre: Pos2, radius: f32, colour: Color32) {
    let [r, g, b, _] = colour.to_array();
    // `colour` moved a fraction `k` towards grey level `to`
    let mix = |k: f32, to: u8| -> Color32 {
        let m = |c: u8| (f32::from(c) + (f32::from(to) - f32::from(c)) * k).round() as u8;
        Color32::from_rgb(m(r), m(g), m(b))
    };
    let light = Vec2::new(-0.3, -0.3) * radius;
    painter.circle(
        centre,
        radius,
        mix(0.45, 0),
        Stroke::new(1.0, Color32::from_black_alpha(110)),
    );
    painter.circle_filled(centre + light * 0.4, radius * 0.8, mix(0.1, 0));
    painter.circle_filled(centre + light * 0.8, radius * 0.5, mix(0.3, 255));
    painter.circle_filled(centre + light * 1.2, radius * 0.22, mix(0.8, 255));
}

/// The frontal silhouette of the procedural torso, heart-frame millimetres `[x, y]`.
struct Silhouette {
    /// Closed outline, down the patient's left side then up the right.
    outline_mm: Vec<[f32; 2]>,
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
        Silhouette { outline_mm }
    })
}

/// The frontal torso picture drawn behind the leads: `assets/images/torso-front-axis.png` (401 × 463
/// px, RGBA, transparent background, legible on light and dark panels; licence in
/// `assets/images/LICENSE`), decoded once per egui context.
const TORSO_FRONT_PNG: &[u8] = include_bytes!("../../../assets/images/torso-front-axis.png");

/// Registration of the picture to the heart frame, measured on the alpha channel of
/// `torso-front-axis.png` (401 × 463 px). Scale: the arms separate from the trunk at row 193 and the
/// trunk is 174 px wide just below, against the procedural torso's 290 mm chest, i.e. 0.60 px per
/// millimetre. Anchor: the trunk midline (`TORSO_CENTRE_MM.x = −65`) is column 198.5 and the frame
/// origin (the ventricular centroid) sits behind the lower third of the sternum, row 196, 65 mm to
/// the patient's left of the midline: pixel (237.5, 196). With that the shoulder electrodes land on
/// the deltoids and the leg electrodes on the hips. The anchor is stored as fractions of the image,
/// so a re-export of the same framing at another resolution needs no change here.
const IMAGE_HEART_U: f32 = (198.5 + 65.0 * 0.60) / 401.0;
const IMAGE_HEART_V: f32 = 196.0 / 463.0;
const IMAGE_HEIGHT_MM: f32 = 463.0 / 0.60;

/// Normalised image coordinates `[u, v]` (0..1 inside the picture, `v` downwards) of a heart-frame
/// point, for a picture with width/height ratio `aspect`: the inverse of [`image_mm`], used by the
/// registration test.
#[cfg(test)]
fn image_uv(x_mm: f32, y_mm: f32, aspect: f32) -> [f32; 2] {
    let width_mm = IMAGE_HEIGHT_MM * aspect;
    [
        IMAGE_HEART_U + x_mm / width_mm,
        IMAGE_HEART_V - y_mm / IMAGE_HEIGHT_MM,
    ]
}

/// Heart-frame millimetres `[x, y]` of a normalised image point: the inverse of [`image_uv`].
fn image_mm(u: f32, v: f32, aspect: f32) -> [f32; 2] {
    let width_mm = IMAGE_HEIGHT_MM * aspect;
    [
        (u - IMAGE_HEART_U) * width_mm,
        (IMAGE_HEART_V - v) * IMAGE_HEIGHT_MM,
    ]
}

/// The embedded PNG as `(size, RGBA bytes)`, `None` if it does not decode.
fn decode_torso_png() -> Option<([usize; 2], Vec<u8>)> {
    let img = image::load_from_memory(TORSO_FRONT_PNG).ok()?.into_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some((size, img.into_raw()))
}

/// The picture as a texture, decoded on first use and kept in the context's memory so every later
/// frame is a lookup. `None` when the embedded PNG does not decode; the schematic then falls back
/// to the outline.
fn torso_texture(ctx: &Context) -> Option<TextureHandle> {
    let id = egui::Id::unique("heartsim_torso_front_texture");
    if let Some(cached) = ctx.data(|d| d.get_temp::<Option<TextureHandle>>(id)) {
        return cached;
    }
    // outside the `data` lock: `load_texture` takes its own
    let decoded = decode_torso_png().map(|(size, rgba)| {
        ctx.load_texture(
            "torso-front",
            ColorImage::from_rgba_unmultiplied(size, &rgba),
            TextureOptions::LINEAR,
        )
    });
    ctx.data_mut(|d| d.insert_temp(id, decoded.clone()));
    decoded
}

/// Draws the figure in a box of `size`. The small disc is the heart's electrical centre, the
/// centroid of Einthoven's triangle. `image_opacity` (`0.0..=1.0`) is that of the torso picture
/// behind the figure; at `0.0` the sampled outline of the procedural torso is drawn in its place.
pub fn torso_schematic(ui: &mut Ui, size: Vec2, image_opacity: f32) -> Response {
    let p = Palette::of(ui);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    let s = silhouette();
    let c = TORSO_CENTRE_MM;
    let h = TORSO_HALF_EXTENTS_MM;
    let opacity = if image_opacity.is_finite() {
        image_opacity.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let texture = (opacity > 0.0).then(|| torso_texture(ui.ctx())).flatten();
    let aspect = texture.as_ref().map(|t| {
        let [w, hh] = t.size();
        w as f32 / hh as f32
    });
    // the millimetre point placed at the centre of the box and the points per millimetre: the
    // whole picture fits the box; without it the torso fills nine tenths of the box's height
    let (origin_mm, scale) = match aspect {
        Some(aspect) => (
            image_mm(0.5, 0.5, aspect),
            (rect.height() / IMAGE_HEIGHT_MM).min(rect.width() / (IMAGE_HEIGHT_MM * aspect)) * 0.98,
        ),
        None => (
            [c.x, c.y],
            (rect.height() * 0.9 / (2.0 * h.y)).min(rect.width() * 0.55 / (2.0 * h.x)),
        ),
    };
    let centre = rect.center();
    let to_screen = |x_mm: f32, y_mm: f32| {
        Pos2::new(
            centre.x + (x_mm - origin_mm[0]) * scale,
            centre.y - (y_mm - origin_mm[1]) * scale,
        )
    };

    // the torso
    if let (Some(tex), Some(aspect)) = (&texture, aspect) {
        let [x0, y0] = image_mm(0.0, 0.0, aspect);
        let [x1, y1] = image_mm(1.0, 1.0, aspect);
        painter.image(
            tex.id(),
            Rect::from_two_pos(to_screen(x0, y0), to_screen(x1, y1)),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::from_white_alpha((opacity * 255.0).round() as u8),
        );
    } else {
        let outline: Vec<Pos2> = s.outline_mm.iter().map(|[x, y]| to_screen(*x, *y)).collect();
        painter.add(Shape::convex_polygon(
            outline,
            p.accent_soft,
            Stroke::new(1.0, p.card_stroke),
        ));
    }

    // Einthoven's triangle: leads I, II and III are its sides, the augmented leads run from its
    // centroid to the electrodes
    let (tri, centroid_mm) = triangle_mm();
    let [ra, la, ll] = tri.map(|[x, y]| to_screen(x, y));
    let centre_pt = to_screen(centroid_mm[0], centroid_mm[1]);
    let line = Stroke::new(1.0, p.grid);
    painter.add(Shape::closed_line(vec![ra, la, ll], line));
    for vertex in [ra, la, ll] {
        painter.line_segment([centre_pt, vertex], line);
    }

    // every lead pill at the middle of its line, with the lead's frontal angle
    let pill_font = FontId::proportional((rect.height() * 0.05).clamp(9.0, 11.0));
    for lead in Lead::ALL.iter().filter(|l| l.is_limb()) {
        let pill_at = match lead {
            Lead::I => ra.lerp(la, 0.5),
            Lead::II => ra.lerp(ll, 0.5),
            Lead::III => la.lerp(ll, 0.5),
            Lead::AVR => centre_pt.lerp(ra, 0.5),
            Lead::AVL => centre_pt.lerp(la, 0.5),
            _ => centre_pt.lerp(ll, 0.5),
        };
        let deg = lead.frontal_angle_deg().unwrap_or(0.0);
        let lines = [
            (lead.label().to_owned(), pill_font.clone(), p.heading),
            (degrees_text(deg.round() as i32, true), pill_font.clone(), p.muted),
        ];
        paint_pill(&painter, pill_at, &lines, p.pill, Some(p.pill_stroke));
    }

    // the electrodes as beads in their cable colours, labelled on the outer side
    let site_font = FontId::proportional((rect.height() * 0.045).clamp(8.0, 10.0));
    let radius = (rect.height() * 0.024).clamp(4.0, 6.0);
    for (e, [x, y]) in ELECTRODES_MM {
        let pos = to_screen(x, y);
        paint_sphere(&painter, pos, radius, electrode_colour(e));
        let outward = if x < TORSO_CENTRE_MM.x { -1.0 } else { 1.0 };
        painter.text(
            pos + Vec2::new(outward * (radius + 10.0), 0.0),
            Align2::CENTER_CENTER,
            e.label(),
            site_font.clone(),
            p.muted,
        );
    }

    // the heart's electrical centre
    let bad = p.bad();
    painter.circle(centre_pt, 6.0, bad.strong, Stroke::new(1.5, p.card));
    painter.circle_filled(
        centre_pt + Vec2::new(-1.5, -1.5),
        1.6,
        Color32::from_white_alpha(140),
    );
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
    }

    #[test]
    fn the_electrodes_sit_on_the_shoulders_and_groins_with_the_heart_near_the_centroid() {
        let [(ra_e, ra), (la_e, la), (ll_e, ll), (rl_e, rl)] = ELECTRODES_MM;
        assert_eq!(
            [ra_e, la_e, ll_e, rl_e],
            [Electrode::RA, Electrode::LA, Electrode::LL, Electrode::RL]
        );
        let mid = TORSO_CENTRE_MM.x;
        // RA on the patient's right (−x) and LA on the left, symmetric about the midline, at
        // shoulder height; LL and RL likewise at the groins
        assert!(ra[0] < mid && la[0] > mid);
        assert!(
            ((ra[0] - mid) + (la[0] - mid)).abs() < 1e-3,
            "shoulders symmetric"
        );
        assert!((ra[1] - la[1]).abs() < 1e-3 && ra[1] > 150.0);
        assert!(rl[0] < mid && ll[0] > mid);
        assert!((ll[1] - rl[1]).abs() < 1e-3 && ll[1] < -300.0);
        // the frame origin (the ventricular centroid) lies inside the triangle and within 5 cm of
        // its centroid, Einthoven's electrical centre
        let (tri, c) = triangle_mm();
        let side = |a: [f32; 2], b: [f32; 2]| (b[0] - a[0]) * (0.0 - a[1]) - (b[1] - a[1]) * (0.0 - a[0]);
        let signs = [side(tri[0], tri[1]), side(tri[1], tri[2]), side(tri[2], tri[0])];
        assert!(
            signs.iter().all(|s| *s > 0.0) || signs.iter().all(|s| *s < 0.0),
            "origin outside the triangle: {signs:?}"
        );
        let d = c[0].hypot(c[1]);
        assert!(d < 50.0, "the origin is {d} mm from the centroid {c:?}");
    }

    #[test]
    fn the_picture_is_registered_to_the_torso_frame() {
        let (size, rgba) = decode_torso_png().expect("the embedded PNG decodes");
        assert!(size[0] > 100 && size[1] > size[0], "a portrait picture: {size:?}");
        let aspect = size[0] as f32 / size[1] as f32;
        let alpha_at = |x_mm: f32, y_mm: f32| -> u8 {
            let [u, v] = image_uv(x_mm, y_mm, aspect);
            assert!(
                (0.0..1.0).contains(&u) && (0.0..1.0).contains(&v),
                "({x_mm}, {y_mm}) mm is off the picture at ({u}, {v})"
            );
            let px = ((u * size[0] as f32) as usize).min(size[0] - 1);
            let py = ((v * size[1] as f32) as usize).min(size[1] - 1);
            rgba[4 * (py * size[0] + px) + 3]
        };
        // the heart, the torso centre, the four limb electrodes and the triangle's centroid land
        // on the body (whose own alpha is about 150: the picture is translucent by design)
        assert!(alpha_at(0.0, 0.0) > 100, "heart");
        assert!(
            alpha_at(TORSO_CENTRE_MM.x, TORSO_CENTRE_MM.y) > 100,
            "torso centre"
        );
        for (e, [x, y]) in ELECTRODES_MM {
            assert!(alpha_at(x, y) > 100, "{e:?} at ({x}, {y}) mm is off the body");
        }
        let (_, c) = triangle_mm();
        assert!(alpha_at(c[0], c[1]) > 100, "centroid");
        // the gap between the right arm and the flank, and the air beside the neck, are clear
        assert!(alpha_at(-218.0, -75.0) < 16, "right flank gap");
        assert!(alpha_at(-300.0, 250.0) < 16, "beside the neck");
        // the picture is a frontal view: the patient's left (+x) is on the right of the image
        let [u_left, _] = image_uv(100.0, 0.0, aspect);
        let [u_right, _] = image_uv(-200.0, 0.0, aspect);
        assert!(u_left > u_right);
        // mm → uv → mm round trip
        let [x, y] = image_mm(0.3, 0.7, aspect);
        let [u, v] = image_uv(x, y, aspect);
        assert!((u - 0.3).abs() < 1e-4 && (v - 0.7).abs() < 1e-4, "{u}, {v}");
    }
}
