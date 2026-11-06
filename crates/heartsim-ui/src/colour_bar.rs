//! Colour-bar legend for the 3-D colour map with a draggable scalar window.
//!
//! The bar spans the whole adjustable `range`; inside the window `[min, max]` it shows the LUT
//! ramp, outside it the clamped end colours — exactly what the fragment shader does — so the
//! legend is a faithful key to the surface colours. Two handles below the bar drag `min` and `max`.

use crate::t;
use egui::{Align2, Color32, FontId, Mesh, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2, pos2};
use std::ops::RangeInclusive;

/// Smallest window the handles allow, as a fraction of `range`, so the map never divides by zero.
const MIN_SPAN_FRACTION: f32 = 0.02;

/// Colour of the LUT at normalised position `t` (0–1, clamped). `table` holds sRGB RGBA8.
#[must_use]
pub fn lut_colour(table: &[[u8; 4]; 256], t: f32) -> Color32 {
    let i = (t.clamp(0.0, 1.0) * 255.0).round() as usize;
    let [r, g, b, _] = table[i];
    Color32::from_rgb(r, g, b)
}

/// Draw the legend `width` points wide and let the user drag the window. `min`/`max` are in the
/// units of `unit` (e.g. `"mV"`), kept inside `range` with `min < max`. Returns whether the
/// window changed.
pub fn colour_bar(
    ui: &mut Ui,
    width: f32,
    table: &[[u8; 4]; 256],
    min: &mut f32,
    max: &mut f32,
    range: RangeInclusive<f32>,
    unit: &str,
) -> bool {
    let (lo, hi) = (*range.start(), *range.end());
    let span = (hi - lo).max(f32::EPSILON);
    let bar_h = 16.0;
    let handle_h = 10.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, bar_h + handle_h + 14.0), Sense::hover());
    let bar = Rect::from_min_size(rect.min, Vec2::new(width, bar_h));
    let x_of = |v: f32| bar.left() + (v - lo) / span * bar.width();
    let v_of = |x: f32| lo + (x - bar.left()) / bar.width() * span;

    // Gradient: one quad per pixel column band, coloured as the shader would at that value.
    let window = (*max - *min).max(f32::EPSILON);
    let mut mesh = Mesh::default();
    let steps = 64;
    for k in 0..=steps {
        let x = bar.left() + bar.width() * k as f32 / steps as f32;
        let c = lut_colour(table, (v_of(x) - *min) / window);
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(pos2(x, bar.top()), c);
        mesh.colored_vertex(pos2(x, bar.bottom()), c);
        if k > 0 {
            mesh.add_triangle(base - 2, base - 1, base);
            mesh.add_triangle(base - 1, base + 1, base);
        }
    }
    let painter = ui.painter_at(rect.expand(2.0));
    painter.add(Shape::mesh(mesh));
    let text = ui.visuals().text_color();
    painter.rect_stroke(bar, 0.0, Stroke::new(1.0, text), egui::StrokeKind::Inside);

    let mut changed = false;
    let min_span = span * MIN_SPAN_FRACTION;
    for (which, value) in [(0, *min), (1, *max)] {
        let x = x_of(value);
        let tip = Pos2::new(x, bar.bottom());
        let handle = Rect::from_center_size(
            Pos2::new(x, bar.bottom() + handle_h * 0.5),
            Vec2::new(handle_h * 1.6, handle_h),
        );
        let id = ui.scope_id().with(("colour_bar_handle", which));
        let resp = ui.interact(handle, id, Sense::drag());
        if resp.dragged()
            && let Some(p) = resp.interact_pointer_pos()
        {
            let v = v_of(p.x);
            if which == 0 {
                *min = v.clamp(lo, *max - min_span);
            } else {
                *max = v.clamp(*min + min_span, hi);
            }
            changed = true;
        }
        let fill = if resp.hovered() || resp.dragged() {
            ui.visuals().selection.bg_fill
        } else {
            text
        };
        painter.add(Shape::convex_polygon(
            vec![
                tip,
                Pos2::new(x - handle_h * 0.6, tip.y + handle_h),
                Pos2::new(x + handle_h * 0.6, tip.y + handle_h),
            ],
            fill,
            Stroke::NONE,
        ));
    }
    let font = FontId::proportional(11.0);
    let y = bar.bottom() + handle_h + 1.0;
    painter.text(
        Pos2::new(x_of(*min), y),
        Align2::CENTER_TOP,
        format!("{min:.0}"),
        font.clone(),
        text,
    );
    painter.text(
        Pos2::new(x_of(*max), y),
        Align2::CENTER_TOP,
        format!("{max:.0} {unit}"),
        font,
        text,
    );

    ui.horizontal(|ui| {
        let step = f64::from(span) / 200.0;
        changed |= ui
            .add(
                egui::DragValue::new(min)
                    .speed(step)
                    .range(lo..=*max - min_span)
                    .prefix(format!("{} ", t!("colour-min"))),
            )
            .changed();
        changed |= ui
            .add(
                egui::DragValue::new(max)
                    .speed(step)
                    .range(*min + min_span..=hi)
                    .prefix(format!("{} ", t!("colour-max"))),
            )
            .changed();
        ui.label(unit);
    });
    changed
}
