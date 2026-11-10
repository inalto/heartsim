//! Cards, pills, badges and headings in the palette's style: the building blocks the panels share
//! so that every page looks like the axis panel. Pure layout helpers: no state, no behaviour.

use crate::theme::{CARD_RADIUS, ClassColours, Palette};
use egui::epaint::{PathShape, Shadow};
use egui::{
    Color32, CornerRadius, FontId, Frame, InnerResponse, Label, Margin, Painter, Pos2, Rect, Response,
    RichText, Sense, Stroke, StrokeKind, Ui, Vec2,
};

/// The frame of a card: rounded, filled, a hairline outline and a soft shadow.
pub fn card_frame(p: &Palette) -> Frame {
    Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.card_stroke))
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .inner_margin(Margin::same(10))
        .outer_margin(Margin {
            left: 1,
            right: 1,
            top: 2,
            bottom: 6,
        })
        .shadow(Shadow {
            offset: [0, 2],
            blur: 6,
            spread: 0,
            color: p.card_shadow,
        })
}

/// A card filling the available width.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    let p = Palette::of(ui);
    card_frame(&p).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        add(ui)
    })
}

/// The frame of a group of related controls: a faint tint of the accent, rounded, no outline.
pub fn group_frame(p: &Palette) -> Frame {
    Frame::new()
        .fill(p.accent_soft)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(8, 4))
}

/// A group of related controls (see [`group_frame`]).
pub fn group<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    let p = Palette::of(ui);
    group_frame(&p).show(ui, add)
}

/// A [`group`] in a wrapping row (`horizontal_wrapped`) that starts a new row when it would not fit
/// the rest of the current one. A frame's width is known only after its contents are laid out, so
/// the wrapping layout cannot break before it and the frame would overflow the window instead; the
/// width this group had on the previous frame decides (the first frame may overflow once, and
/// repaints). `id_salt` tells the groups of one row apart.
pub fn wrapping_group<R>(
    ui: &mut Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    add: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    let id = ui.scope_id().with(id_salt);
    let remembered = ui.data(|d| d.get_temp::<f32>(id));
    if let Some(w) = remembered
        && ui.available_size_before_wrap().x < w
        && ui.cursor().min.x > ui.max_rect().min.x + 1.0
    {
        ui.end_row();
    }
    let r = group(ui, add);
    let w = r.response.rect.width();
    if remembered.is_none_or(|old| (old - w).abs() > 0.5) {
        ui.data_mut(|d| d.insert_temp(id, w));
        ui.ctx().request_repaint();
    }
    r
}

/// A title band: white text on the strong accent, the full width.
pub fn title_band(ui: &mut Ui, text: impl Into<String>) -> Response {
    let p = Palette::of(ui);
    Frame::new()
        .fill(p.accent_strong)
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .inner_margin(Margin::symmetric(12, 6))
        .outer_margin(Margin {
            left: 1,
            right: 1,
            top: 2,
            bottom: 6,
        })
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                RichText::new(text.into())
                    .size(16.0)
                    .strong()
                    .color(Color32::WHITE),
            )
        })
        .inner
}

/// A card heading.
pub fn heading(ui: &mut Ui, text: impl Into<String>) -> Response {
    let p = Palette::of(ui);
    ui.label(RichText::new(text.into()).size(15.0).strong().color(p.heading))
}

/// A smaller heading inside a card.
pub fn subheading(ui: &mut Ui, text: impl Into<String>) -> Response {
    let p = Palette::of(ui);
    ui.label(RichText::new(text.into()).size(12.5).strong().color(p.muted))
}

/// A pill-shaped label in the neutral pill colours.
pub fn pill(ui: &mut Ui, text: impl Into<String>) -> Response {
    let p = Palette::of(ui);
    pill_coloured(ui, text, p.pill, p.text, Some(p.pill_stroke))
}

/// A pill-shaped label in the given colours.
pub fn pill_coloured(
    ui: &mut Ui,
    text: impl Into<String>,
    fill: Color32,
    text_colour: Color32,
    stroke: Option<Color32>,
) -> Response {
    Frame::new()
        .fill(fill)
        .stroke(stroke.map_or(Stroke::NONE, |c| Stroke::new(1.0, c)))
        .corner_radius(CornerRadius::same(99))
        .inner_margin(Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text.into()).size(12.0).color(text_colour))
        })
        .inner
}

/// A big value on a filled, rounded badge the full width (the measured axis).
pub fn badge(ui: &mut Ui, text: impl Into<String>, colours: ClassColours) -> Response {
    badge_in(ui, text, colours.strong, colours.on_strong)
}

/// A big value on a badge in the neutral pill colours (no value yet).
pub fn badge_muted(ui: &mut Ui, text: impl Into<String>) -> Response {
    let p = Palette::of(ui);
    badge_in(ui, text, p.pill, p.muted)
}

fn badge_in(ui: &mut Ui, text: impl Into<String>, fill: Color32, text_colour: Color32) -> Response {
    Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(text.into()).size(30.0).strong().color(text_colour))
            })
            .inner
        })
        .inner
}

/// A result box: a soft green box with a check mark for a correct answer, a soft rose box with an
/// exclamation mark for a wrong one. The icon and the wording carry the meaning; the colour is
/// a confirmation. Returns the text's response.
pub fn result_box(ui: &mut Ui, ok: bool, text: impl Into<String>) -> Response {
    let p = Palette::of(ui);
    let c = if ok { p.good() } else { p.bad() };
    Frame::new()
        .fill(c.soft)
        .stroke(Stroke::new(1.0, c.strong))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(8))
        .outer_margin(Margin::symmetric(0, 2))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_top(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                status_icon(ui.painter(), rect.center(), 8.0, ok, c);
                ui.add(Label::new(RichText::new(text.into()).color(c.on_soft)).wrap())
            })
            .inner
        })
        .inner
}

/// A check mark (`ok`) or an exclamation mark on a filled disc, drawn as shapes.
pub fn status_icon(painter: &Painter, centre: Pos2, radius: f32, ok: bool, c: ClassColours) {
    painter.circle_filled(centre, radius, c.strong);
    let r = radius;
    if ok {
        let pts = vec![
            centre + Vec2::new(-0.45 * r, 0.02 * r),
            centre + Vec2::new(-0.12 * r, 0.36 * r),
            centre + Vec2::new(0.5 * r, -0.36 * r),
        ];
        painter.add(PathShape::line(pts, Stroke::new(r * 0.24, c.on_strong)));
    } else {
        painter.line_segment(
            [
                centre + Vec2::new(0.0, -0.52 * r),
                centre + Vec2::new(0.0, 0.14 * r),
            ],
            Stroke::new(r * 0.26, c.on_strong),
        );
        painter.circle_filled(centre + Vec2::new(0.0, 0.46 * r), r * 0.14, c.on_strong);
    }
}

/// A small rounded colour swatch.
pub fn swatch(painter: &Painter, rect: Rect, colour: Color32) {
    painter.rect_filled(rect, 4.0, colour);
}

/// Paints a pill with centred lines of text at `centre` (the circle's tick labels and the lead
/// names, which are drawn, not laid out). Returns its rect.
pub fn paint_pill(
    painter: &Painter,
    centre: Pos2,
    lines: &[(String, FontId, Color32)],
    fill: Color32,
    stroke: Option<Color32>,
) -> Rect {
    let galleys: Vec<_> = lines
        .iter()
        .map(|(text, font, colour)| {
            (
                painter.layout_no_wrap(text.clone(), font.clone(), *colour),
                *colour,
            )
        })
        .collect();
    let width = galleys.iter().map(|(g, _)| g.size().x).fold(0.0, f32::max);
    let height: f32 = galleys.iter().map(|(g, _)| g.size().y).sum();
    let font_px = lines.first().map_or(10.0, |(_, f, _)| f.size);
    let pad = Vec2::new(font_px * 0.55, font_px * 0.2);
    let rect = Rect::from_center_size(centre, Vec2::new(width, height) + pad * 2.0);
    painter.rect(
        rect,
        rect.height() / 2.0,
        fill,
        stroke.map_or(Stroke::NONE, |c| Stroke::new(1.0, c)),
        StrokeKind::Inside,
    );
    let mut y = rect.min.y + pad.y;
    for (galley, colour) in galleys {
        let size = galley.size();
        painter.galley(Pos2::new(centre.x - size.x / 2.0, y), galley, colour);
        y += size.y;
    }
    rect
}
