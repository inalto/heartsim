//! Hexaxial (Cabrera) reference circle: the six frontal-plane leads, the four clinical axis
//! classes as soft-coloured sectors (the one under the pointer brightens and shows its name and
//! range), degree ticks in pills, and the measured or guessed QRS axis as a thick arrow with a
//! drop shadow. Screen frame: 0° = lead I (patient's left, drawn to the right as in a frontal
//! view), +90° = aVF (down), −90° = up.
//!
//! [`axis_panel`] adds the cards next to the circle (the measured axis on a badge in the class
//! colour, the interpretation with the net polarity of each limb lead and a torso schematic) and
//! the legend row under it. The class boundaries and the sector of each class come from
//! [`AxisClass`]; nothing here restates them.

use crate::cards::{self, paint_pill};
use crate::t;
use crate::theme::Palette;
use crate::torso_schematic::torso_schematic;
use egui::epaint::PathShape;
use egui::text::LayoutJob;
use egui::{
    Align, Color32, CursorIcon, FontId, Frame, Label, Margin, Pos2, Response, RichText, Sense, Shape, Stroke,
    Ui, Vec2,
};
use heartsim_core::{AxisClass, FrontalAxis, Lead};

/// Net QRS polarity of a limb lead for a frontal axis: `cos(axis − θ_lead)`. A lead nearly
/// perpendicular to the axis (|cos| < 0.2, i.e. within ~12°) is equiphasic: the classic trick for
/// finding the axis is "the equiphasic lead is perpendicular to it".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Polarity {
    Positive,
    Negative,
    Equiphasic,
}

impl Polarity {
    /// The polarity of `lead` for an axis, `None` for a precordial lead.
    #[must_use]
    pub fn of(axis_deg: f64, lead: Lead) -> Option<Polarity> {
        let c = (FrontalAxis { degrees: axis_deg }).projection_onto(lead)?;
        Some(if c > 0.2 {
            Polarity::Positive
        } else if c < -0.2 {
            Polarity::Negative
        } else {
            Polarity::Equiphasic
        })
    }

    /// The sign as a glyph that reads without colour: `+`, `−`, `±`.
    #[must_use]
    pub fn glyph(self) -> &'static str {
        match self {
            Polarity::Positive => "+",
            Polarity::Negative => "−",
            Polarity::Equiphasic => "±",
        }
    }

    /// The translated word.
    #[must_use]
    pub fn word(self) -> String {
        match self {
            Polarity::Positive => t!("polarity-positive"),
            Polarity::Negative => t!("polarity-negative"),
            Polarity::Equiphasic => t!("polarity-equiphasic"),
        }
    }
}

/// The translated polarity word of `lead` for an axis (empty for a precordial lead).
#[must_use]
pub fn lead_polarity(axis_deg: f64, lead: Lead) -> String {
    Polarity::of(axis_deg, lead).map_or_else(String::new, Polarity::word)
}

/// Degrees for the screen, with the typographic minus: "−30°", "0°", and "+60°" when `signed`
/// (else "60°").
#[must_use]
pub fn degrees_text(deg: i32, signed: bool) -> String {
    match deg {
        0 => "0°".to_owned(),
        d if d < 0 => format!("−{}°", -d),
        d if signed => format!("+{d}°"),
        d => format!("{d}°"),
    }
}

/// The translated name of a class, as written in its sector and on the legend.
#[must_use]
pub fn class_name(class: AxisClass) -> String {
    match class {
        AxisClass::Normal => t!("axis-sector-normal"),
        AxisClass::LeftDeviation => t!("axis-sector-left"),
        AxisClass::RightDeviation => t!("axis-sector-right"),
        AxisClass::ExtremeDeviation => t!("axis-sector-extreme"),
    }
}

/// The stated range of a class, "−30° to +90°"; the extreme class ends at "±180°" (its −180).
#[must_use]
pub fn class_range_text(class: AxisClass) -> String {
    let (from, to) = class.range_deg();
    let end = |d: i32| {
        if d == -180 {
            "±180°".to_owned()
        } else {
            degrees_text(d, true)
        }
    };
    t!("axis-range", from = end(from), to = end(to))
}

/// The one-sentence interpretation of a class with its range.
#[must_use]
pub fn class_interpretation(class: AxisClass) -> String {
    let (from, to) = class.range_deg();
    let end = |d: i32| {
        if d == -180 {
            "±180°".to_owned()
        } else {
            degrees_text(d, true)
        }
    };
    let (from, to) = (end(from), end(to));
    match class {
        AxisClass::Normal => t!("axis-interp-normal", from = from, to = to),
        AxisClass::LeftDeviation => t!("axis-interp-left", from = from, to = to),
        AxisClass::RightDeviation => t!("axis-interp-right", from = from, to = to),
        AxisClass::ExtremeDeviation => t!("axis-interp-extreme", from = from, to = to),
    }
}

fn polar(centre: Pos2, radius: f32, deg: f64) -> Pos2 {
    let r = deg.to_radians();
    centre + Vec2::new(r.cos() as f32, r.sin() as f32) * radius
}

/// What the circle reported back.
#[derive(Debug)]
pub struct AxisDialResponse {
    pub response: Response,
    /// The angle (hexaxial degrees, −180..180) the user clicked or dragged to this frame.
    pub dragged_to: Option<f64>,
    /// The pointer is over the draggable circle (or dragging it): the arrow is highlighted.
    pub hovered: bool,
    /// The sector under the pointer, whose name and range are shown.
    pub hovered_sector: Option<AxisClass>,
    pub centre: Pos2,
    pub radius: f32,
}

/// The circle in a square of side `side`. `axis` is the measured axis, `guess` the learner's
/// answer. The sector under the pointer brightens and shows its name and range in either mode.
/// With `interactive` the whole disc can also be clicked or dragged to set the axis: the pointer
/// becomes a hand, the arrow brightens and a tooltip shows the angle.
pub fn axis_circle(
    ui: &mut Ui,
    side: f32,
    axis: Option<f64>,
    guess: Option<f64>,
    interactive: bool,
    p: &Palette,
) -> AxisDialResponse {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::splat(side),
        if interactive {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        },
    );
    let centre = rect.center();
    // the tick pills sit at 1.22 r and must stay inside the square
    let tick_px = (side * 0.034).clamp(8.0, 12.0);
    let radius = ((side * 0.5 - tick_px * 1.7 - 4.0) / 1.22).max(20.0);
    let painter = ui.painter_at(rect);

    let pointer_inside = response
        .hover_pos()
        .is_some_and(|pos| (pos - centre).length() <= radius * 1.06);
    // the sector under the pointer (a hover only: clicking it does nothing new)
    let hovered_sector = response
        .hover_pos()
        .filter(|pos| response.hovered() && (*pos - centre).length() <= radius)
        .map(|pos| {
            let d = pos - centre;
            FrontalAxis {
                degrees: f64::from(d.y).atan2(f64::from(d.x)).to_degrees(),
            }
            .classify()
        });
    let dragging = interactive && response.dragged();
    let hovered = interactive && (dragging || (pointer_inside && response.hovered()));
    if dragging {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    } else if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::Grab);
    }

    // the four classes as soft sectors; the hovered one a little brighter
    for class in AxisClass::ALL {
        let (from, to) = class.sector_deg();
        let steps = 36;
        let mut pts = Vec::with_capacity(steps + 2);
        pts.push(centre);
        for k in 0..=steps {
            pts.push(polar(
                centre,
                radius,
                from + (to - from) * k as f64 / steps as f64,
            ));
        }
        let c = p.class(class);
        let fill = if hovered_sector == Some(class) {
            c.soft
                .lerp_to_gamma(if p.dark { c.on_soft } else { Color32::WHITE }, 0.22)
        } else {
            c.soft
        };
        painter.add(Shape::convex_polygon(pts, fill, Stroke::NONE));
    }
    // faint dotted inner rings and the six spokes
    for f in [1.0 / 3.0, 2.0 / 3.0] {
        let pts: Vec<Pos2> = (0..=90)
            .map(|k| polar(centre, radius * f, f64::from(k) * 4.0))
            .collect();
        painter.extend(Shape::dotted_line(&pts, p.grid, 5.0, 0.8));
    }
    for k in 0..6 {
        let deg = f64::from(k) * 30.0;
        painter.line_segment(
            [polar(centre, radius, deg), polar(centre, radius, deg + 180.0)],
            Stroke::new(0.8, p.grid),
        );
    }
    // the class boundaries, stronger, and the ring
    for class in AxisClass::ALL {
        let (from, _) = class.sector_deg();
        painter.line_segment([centre, polar(centre, radius, from)], Stroke::new(1.4, p.ring));
    }
    painter.circle_stroke(centre, radius, Stroke::new(2.0, p.ring));

    // degree ticks every 30° with the value in a pill
    let tick_font = FontId::proportional(tick_px);
    for k in -5..=6 {
        let deg = f64::from(k) * 30.0;
        painter.line_segment(
            [polar(centre, radius, deg), polar(centre, radius * 1.07, deg)],
            Stroke::new(1.2, p.ring),
        );
        paint_pill(
            &painter,
            polar(centre, radius * 1.22, deg),
            &[(degrees_text(k * 30, false), tick_font.clone(), p.text)],
            p.pill,
            Some(p.pill_stroke),
        );
    }
    // the six limb leads at their angles, on the ring
    let lead_font = FontId::proportional((radius * 0.1).clamp(9.0, 13.0));
    for lead in Lead::ALL.iter().filter(|l| l.is_limb()) {
        let deg = lead.frontal_angle_deg().unwrap_or(0.0);
        paint_pill(
            &painter,
            polar(centre, radius, deg),
            &[(lead.label().to_owned(), lead_font.clone(), p.heading)],
            p.card.gamma_multiply(0.92),
            Some(p.ring),
        );
    }

    // the arrows: a rounded shaft and head with a soft drop shadow
    let width = (radius * 0.075).clamp(4.0, 9.0) + if hovered { 1.5 } else { 0.0 };
    let arrow = |deg: f64, colour: Color32| {
        let length = radius * 0.9;
        let head_len = (radius * 0.17).max(10.0);
        let head_half = width * 1.9;
        let dir = (polar(centre, 1.0, deg) - centre).normalized();
        let perp = dir.rot90();
        let tip = polar(centre, length, deg);
        let base = polar(centre, length - head_len, deg);
        let shadow = Vec2::new(2.0, 3.0);
        for (extra, alpha) in [(5.0, 0.10), (2.5, 0.18), (0.0, 0.28)] {
            let col = p.shadow.gamma_multiply(alpha);
            painter.line_segment(
                [centre + shadow, base + dir * 2.0 + shadow],
                Stroke::new(width + extra, col),
            );
            painter.circle_filled(centre + shadow, f32::midpoint(width, extra), col);
            painter.add(PathShape::convex_polygon(
                vec![
                    tip + shadow,
                    base + perp * head_half + shadow,
                    base - perp * head_half + shadow,
                ],
                col,
                Stroke::new(extra.max(0.5), col),
            ));
        }
        painter.line_segment([centre, base + dir * 2.0], Stroke::new(width, colour));
        painter.circle_filled(centre, width / 2.0, colour);
        painter.add(PathShape::convex_polygon(
            vec![tip, base + perp * head_half, base - perp * head_half],
            colour,
            Stroke::new(width * 0.5, colour),
        ));
        painter.circle_filled(tip, width * 0.3, colour);
    };
    if let Some(g) = guess {
        arrow(
            g,
            if hovered {
                p.guess.gamma_multiply(1.15)
            } else {
                p.guess
            },
        );
    }
    if let Some(a) = axis {
        arrow(a, if hovered { p.arrow_hover } else { p.arrow });
    }
    painter.circle(centre, width * 0.8, p.card, Stroke::new(1.5, p.ring));

    // the hovered sector's name and range, on a small card inside the sector
    if let Some(class) = hovered_sector {
        let (from, to) = class.sector_deg();
        let (mid, half) = (f64::midpoint(from, to), (to - from) / 2.0);
        let c = p.class(class);
        let name_px = (radius * 0.105).clamp(9.0, 14.0);
        let dist = radius * 0.52;
        let anchor = polar(centre, dist, mid);
        let chord = 2.0 * dist * half.to_radians().sin() as f32 * 0.95;
        let wrap = chord.max(name_px * 5.0);
        let mut job = LayoutJob::simple(class_name(class), FontId::proportional(name_px), c.on_soft, wrap);
        job.halign = Align::Center;
        let name = painter.layout_job(job);
        let mut job = LayoutJob::simple(
            class_range_text(class),
            FontId::proportional(name_px * 0.86),
            c.on_soft,
            wrap,
        );
        job.halign = Align::Center;
        let range = painter.layout_job(job);
        let size = Vec2::new(name.size().x.max(range.size().x), name.size().y + range.size().y);
        let pad = Vec2::new(name_px * 0.6, name_px * 0.35);
        let card = egui::Rect::from_center_size(anchor, size + pad * 2.0);
        painter.rect(
            card,
            6.0,
            p.card.gamma_multiply(0.94),
            Stroke::new(1.0, c.strong),
            egui::StrokeKind::Inside,
        );
        let mut y = card.min.y + pad.y;
        painter.galley(Pos2::new(anchor.x, y), name.clone(), c.on_soft);
        y += name.size().y;
        painter.galley(Pos2::new(anchor.x, y), range, c.on_soft);
    }

    let mut dragged_to = None;
    if interactive
        && (response.clicked() || response.dragged())
        && let Some(pos) = response.interact_pointer_pos()
    {
        let d = pos - centre;
        if d.length() > 6.0 {
            dragged_to = Some(FrontalAxis::normalise(
                f64::from(d.y).atan2(f64::from(d.x)).to_degrees(),
            ));
        }
    }
    if hovered {
        let shown = if guess.is_some() { guess } else { axis }.or_else(|| {
            response.hover_pos().map(|pos| {
                let d = pos - centre;
                FrontalAxis::normalise(f64::from(d.y).atan2(f64::from(d.x)).to_degrees())
            })
        });
        if let Some(deg) = shown {
            let text = degrees_text(FrontalAxis::normalise(deg).round() as i32, true);
            let key = if guess.is_some() {
                t!("axis-guess-tooltip", deg = text)
            } else {
                t!("axis-arrow-tooltip", deg = text)
            };
            response.clone().on_hover_text(key);
        }
    }
    AxisDialResponse {
        response,
        dragged_to,
        hovered,
        hovered_sector,
        centre,
        radius,
    }
}

/// The circle alone, in the theme's palette. Returns the angle (hexaxial degrees, −180..180)
/// the user clicked or dragged to, if any.
pub fn axis_dial(
    ui: &mut Ui,
    side: f32,
    axis: Option<f64>,
    guess: Option<f64>,
    interactive: bool,
) -> Option<f64> {
    let p = Palette::of(ui);
    axis_circle(ui, side, axis, guess, interactive, &p).dragged_to
}

/// What [`axis_panel`] draws.
#[derive(Debug, Clone, Copy)]
pub struct AxisPanel<'a> {
    /// The measured (or revealed) axis.
    pub axis: Option<f64>,
    /// The learner's answer in the quiz.
    pub guess: Option<f64>,
    /// The circle can be clicked or dragged.
    pub interactive: bool,
    /// Largest side of the circle, points; it also shrinks to the available width.
    pub max_side: f32,
    /// The measured-axis and interpretation cards (to the right of the circle where the width
    /// allows, else under it).
    pub show_cards: bool,
    /// The torso schematic at the bottom of the interpretation card.
    pub show_torso: bool,
    /// An extra sentence in the interpretation card (the trainer's reading rule).
    pub note: Option<&'a str>,
}

/// What the panel reported back.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisPanelResponse {
    pub dragged_to: Option<f64>,
    pub hovered: bool,
}

/// Width of the card column beside the circle, points.
const CARD_WIDTH: f32 = 230.0;
/// Smallest circle beside which the cards are still placed to the right.
const MIN_SIDE_BESIDE: f32 = 250.0;

/// The circle, its cards and the legend row.
pub fn axis_panel(ui: &mut Ui, cfg: &AxisPanel<'_>) -> AxisPanelResponse {
    let p = Palette::of(ui);
    let avail = ui.available_width();
    let gap = 12.0;
    let beside = cfg.show_cards && avail >= MIN_SIDE_BESIDE + gap + CARD_WIDTH;
    let mut out = AxisPanelResponse {
        dragged_to: None,
        hovered: false,
    };
    if beside {
        let side = (avail - CARD_WIDTH - gap).min(cfg.max_side).max(MIN_SIDE_BESIDE);
        ui.horizontal_top(|ui| {
            let r = axis_circle(ui, side, cfg.axis, cfg.guess, cfg.interactive, &p);
            out.dragged_to = r.dragged_to;
            out.hovered = r.hovered;
            ui.add_space(gap);
            ui.vertical(|ui| {
                ui.set_width((avail - side - gap).max(CARD_WIDTH));
                axis_cards(ui, cfg, &p);
            });
        });
    } else {
        let side = avail.min(cfg.max_side).max(120.0);
        ui.vertical_centered(|ui| {
            let r = axis_circle(ui, side, cfg.axis, cfg.guess, cfg.interactive, &p);
            out.dragged_to = r.dragged_to;
            out.hovered = r.hovered;
        });
        if cfg.show_cards {
            axis_cards(ui, cfg, &p);
        }
    }
    axis_legend(ui, &p);
    out
}

fn axis_cards(ui: &mut Ui, cfg: &AxisPanel<'_>, p: &Palette) {
    let class = cfg.axis.map(|a| FrontalAxis { degrees: a }.classify());
    cards::card(ui, |ui| {
        cards::heading(ui, t!("axis-measured-heading"));
        if let (Some(a), Some(c)) = (cfg.axis, class) {
            cards::badge(
                ui,
                degrees_text(FrontalAxis::normalise(a).round() as i32, true),
                p.class(c),
            );
        } else {
            cards::badge_muted(ui, "—");
            ui.label(RichText::new(t!("axis-no-axis")).color(p.muted).size(12.0));
        }
    });
    cards::card(ui, |ui| {
        cards::heading(ui, t!("axis-interpretation-heading"));
        if let (Some(a), Some(c)) = (cfg.axis, class) {
            let cc = p.class(c);
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
                cards::status_icon(ui.painter(), rect.center(), 9.0, c == AxisClass::Normal, cc);
                cards::pill_coloured(ui, class_name(c), cc.soft, cc.on_soft, Some(cc.strong));
            });
            ui.add(Label::new(RichText::new(class_interpretation(c)).color(p.text)).wrap());
            if let Some(note) = cfg.note {
                ui.add(Label::new(RichText::new(note).color(p.muted).size(12.0)).wrap());
            }
            ui.add_space(4.0);
            cards::subheading(ui, t!("axis-polarity-heading"));
            polarity_table(ui, a, p);
        } else {
            ui.label(RichText::new(t!("axis-no-axis")).color(p.muted));
        }
        if cfg.show_torso {
            ui.add_space(6.0);
            cards::subheading(ui, t!("axis-torso-heading"));
            let w = ui.available_width();
            torso_schematic(ui, Vec2::new(w, (w * 1.0).clamp(150.0, 220.0)));
        }
    });
}

/// The net QRS polarity of the six limb leads: a lead pill, a signed glyph on the class colour
/// and the word, so the reading survives without colour.
pub fn polarity_table(ui: &mut Ui, axis_deg: f64, p: &Palette) {
    egui::Grid::new(ui.scope_id().with("axis_polarity"))
        .spacing([6.0, 4.0])
        .show(ui, |ui| {
            for lead in Lead::ALL.iter().filter(|l| l.is_limb()) {
                let Some(pol) = Polarity::of(axis_deg, *lead) else {
                    continue;
                };
                let cc = match pol {
                    Polarity::Positive => p.good(),
                    Polarity::Negative => p.bad(),
                    Polarity::Equiphasic => p.warn(),
                };
                cards::pill(ui, lead.label());
                cards::pill_coloured(ui, pol.glyph(), cc.soft, cc.on_soft, Some(cc.strong));
                ui.label(RichText::new(pol.word()).color(p.text));
                ui.end_row();
            }
        });
}

/// The legend row: every class with its swatch, name and range. It is also the colour key.
pub fn axis_legend(ui: &mut Ui, p: &Palette) {
    cards::subheading(ui, t!("axis-legend-heading"));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
        for class in AxisClass::ALL {
            let c = p.class(class);
            let name = RichText::new(class_name(class))
                .size(12.0)
                .strong()
                .color(c.on_soft);
            let range = RichText::new(class_range_text(class)).size(11.0).color(c.on_soft);
            // an entry is never squeezed: it moves to the next row whole when the rest of this
            // one is too narrow for it (a frame's width is only known once it has been shown)
            let text_w = [(class_name(class), 12.0), (class_range_text(class), 11.0)]
                .into_iter()
                .map(|(text, size)| {
                    ui.painter()
                        .layout_no_wrap(text, FontId::proportional(size), c.on_soft)
                        .size()
                        .x
                })
                .fold(0.0_f32, f32::max);
            let needed = 14.0 + ui.spacing().item_spacing.x + text_w + 2.0 * 8.0 + 2.0;
            if ui.available_width() < needed && ui.cursor().min.x > ui.max_rect().min.x + 1.0 {
                ui.end_row();
            }
            Frame::new()
                .fill(c.soft)
                .stroke(Stroke::new(1.0, c.strong))
                .corner_radius(8)
                .inner_margin(Margin::symmetric(8, 5))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                        cards::swatch(ui.painter(), rect, c.strong);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            ui.add(Label::new(name).wrap_mode(egui::TextWrapMode::Extend));
                            ui.add(Label::new(range).wrap_mode(egui::TextWrapMode::Extend));
                        });
                    });
                });
        }
    });
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
        assert_eq!(Polarity::of(60.0, Lead::V1), None);
        assert_eq!(Polarity::Negative.glyph(), "−");
    }

    #[test]
    fn degrees_use_the_typographic_minus_and_an_optional_plus() {
        assert_eq!(degrees_text(-30, true), "−30°");
        assert_eq!(degrees_text(-30, false), "−30°");
        assert_eq!(degrees_text(0, true), "0°");
        assert_eq!(degrees_text(60, true), "+60°");
        assert_eq!(degrees_text(60, false), "60°");
    }

    #[test]
    fn the_ranges_read_from_the_normal_boundary_outward() {
        crate::i18n::set_language("en");
        assert_eq!(class_range_text(AxisClass::Normal), "−30° to +90°");
        assert_eq!(class_range_text(AxisClass::LeftDeviation), "−30° to −90°");
        assert_eq!(class_range_text(AxisClass::RightDeviation), "+90° to +180°");
        assert_eq!(class_range_text(AxisClass::ExtremeDeviation), "−90° to ±180°");
        assert!(class_interpretation(AxisClass::LeftDeviation).contains("left axis deviation"));
    }
}
