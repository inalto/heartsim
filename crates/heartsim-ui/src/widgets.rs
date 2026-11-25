//! The shared design primitives of the pages: the one primary action of a panel, labelled slider
//! rows, helper text and its disclosure, read-only status marks and the segmented control. Each
//! function's doc comment states its rule of use; colours come from [`Palette`], so the two themes,
//! the high-contrast setting and the colour-blind setting stay consistent.
//!
//! Every control here has an accessible name (AccessKit) and is reachable with Tab.

use crate::t;
use crate::theme::Palette;
pub use crate::theme::PrimaryColours;
use egui::{
    Align, Color32, CornerRadius, Label, Layout, Response, RichText, Role, Sense, Slider, Stroke, StrokeKind,
    Ui, Vec2, WidgetInfo,
};

/// Size of [`helper_text`], points: one size for every explanatory paragraph, never below 12.
pub const HELPER_TEXT_PT: f32 = 12.5;

/// Narrowest slider of a [`slider_row`], points. Below this (plus the label column and the value
/// box) the row stacks: the label above the slider.
const SLIDER_MIN_PX: f32 = 64.0;
/// Widest slider of a [`slider_row`], points (a long rail on a wide bar is no easier to set).
const SLIDER_MAX_PX: f32 = 200.0;
/// Room kept for the value box right of the slider, points.
const VALUE_BOX_PX: f32 = 66.0;
/// The label column of a [`slider_row`] never takes more than this share of the row.
const LABEL_SHARE_MAX: f32 = 0.42;

// ---- primary action ----------------------------------------------------------------------------

/// The main action of a panel (Run, Check answer, Next question, Start, Show me): an accent-filled
/// button whose text reads at 4.5:1 or more on its fill in both themes and in every state (rest,
/// hover, pressed). With the high-contrast setting it is drawn in the text colour with the
/// background colour as its text (21:1). In a disabled `Ui` (`ui.add_enabled_ui(false, …)`) it
/// loses the accent and is drawn as a faded grey button, so a disabled main action never looks
/// available.
///
/// **Rule of use:** at most one per panel (or card), for the action the panel exists for; every
/// other button stays a plain `ui.button` (secondary). Do not use it for toggles or selections
/// (those are `Button::selectable` / [`segmented`]).
pub fn primary_button(ui: &mut Ui, text: impl Into<String>) -> Response {
    let text = text.into();
    let p = Palette::of(ui);
    let high_contrast = ui.visuals().override_text_color;
    if !ui.is_enabled() {
        // a plain grey button, faded by the disabled Ui: no accent on an unavailable action
        let response = ui.add(egui::Button::new(
            RichText::new(text).color(ui.visuals().weak_text_color()),
        ));
        mark_primary(ui, &response);
        return response;
    }
    let c = match high_contrast {
        Some(fg) => PrimaryColours {
            fill: fg,
            hover: fg,
            pressed: fg,
            text: ui.visuals().panel_fill,
        },
        None => p.primary(),
    };
    let response = ui
        .scope(|ui| {
            let w = &mut ui.visuals_mut().widgets;
            for (state, fill) in [
                (&mut w.inactive, c.fill),
                (&mut w.hovered, c.hover),
                (&mut w.active, c.pressed),
            ] {
                state.weak_bg_fill = fill;
                state.bg_fill = fill;
                state.fg_stroke.color = c.text;
                state.bg_stroke = Stroke::NONE;
            }
            ui.add(egui::Button::new(RichText::new(text).strong().color(c.text)))
        })
        .inner;
    mark_primary(ui, &response);
    if response.has_focus() {
        // keyboard focus: a ring outside the fill, in the text colour of the panel
        ui.painter().rect_stroke(
            response.rect.expand(2.0),
            CornerRadius::same(8),
            Stroke::new(2.0, ui.visuals().text_color()),
            StrokeKind::Outside,
        );
    }
    response
}

/// The AccessKit class name of a [`primary_button`] (also when it is disabled): lets a test count
/// the primary actions of a panel, which the rule of use limits to one.
pub const PRIMARY_CLASS: &str = "primary-button";

fn mark_primary(ui: &Ui, response: &Response) {
    ui.ctx()
        .accesskit_node_builder(response.id, |b| b.set_class_name(PRIMARY_CLASS));
}

// ---- slider rows -------------------------------------------------------------------------------

/// The label width of the rows drawn in one parent `Ui`: the widest of the previous frame (used
/// now, so consecutive rows share one column) and the widest so far in this frame.
#[derive(Debug, Clone, Copy, Default)]
struct LabelColumn {
    pass: u64,
    last: f32,
    this: f32,
}

/// One slider as a row of three columns: the **label left** in a column as wide as the widest
/// label of the consecutive rows in the same parent `Ui` (so the rails line up), the **slider**,
/// then its **value box** (the unit as the slider's suffix: `Slider::new(&mut v, range).suffix(" mm")`).
/// The slider is named by the label for screen readers. When the row is too narrow for the three
/// (label column + 64 + 66 points; under about 230 points), the label goes above the slider. Returns the slider's response, for
/// `.changed()` and `.on_hover_text(…)`.
///
/// **Rule of use:** every labelled slider in a panel is a `slider_row`; do not pass `.text(…)` to
/// the slider (that draws the name right of the value). Put consecutive rows directly in the same
/// `Ui` (a card, a vertical group) so they align. In a horizontal bar (a toolbar) the row keeps
/// egui's slider width and never stacks, so the bar does not grow.
pub fn slider_row(ui: &mut Ui, label: &str, slider: Slider<'_>) -> Response {
    let in_bar = ui.layout().is_horizontal();
    let column_id = ui.scope_id().with("heartsim_slider_row_column");
    let pass = ui.ctx().cumulative_pass_nr();
    let font = egui::TextStyle::Body.resolve(ui.style());
    let label_w = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, Color32::WHITE)
        .size()
        .x
        .ceil();
    let mut col = ui
        .data(|d| d.get_temp::<LabelColumn>(column_id))
        .unwrap_or_default();
    if col.pass != pass {
        // a new frame: the widest label of the last one sets the column now
        col = LabelColumn {
            pass,
            last: col.this,
            this: 0.0,
        };
    }
    col.this = col.this.max(label_w);
    let wanted = col.last.max(col.this);
    if (wanted - col.last).abs() > 0.5 {
        ui.ctx().request_repaint();
    }
    ui.data_mut(|d| d.insert_temp(column_id, col));

    let gap = ui.spacing().item_spacing.x;
    let available = ui.available_width();
    let column = wanted.min(available * LABEL_SHARE_MAX);
    let stacked = !in_bar && available < column + SLIDER_MIN_PX + VALUE_BOX_PX + 2.0 * gap;
    if stacked {
        return ui
            .vertical(|ui| {
                let name = ui.add(Label::new(label).wrap());
                ui.spacing_mut().slider_width =
                    (ui.available_width() - VALUE_BOX_PX - gap).clamp(40.0, SLIDER_MAX_PX);
                ui.add(slider).labelled_by(name.id)
            })
            .inner;
    }
    ui.horizontal(|ui| {
        let height = ui.spacing().interact_size.y;
        let name = ui
            .allocate_ui_with_layout(
                Vec2::new(column, height),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.set_min_width(column);
                    ui.add(Label::new(label).wrap())
                },
            )
            .inner;
        if !in_bar {
            ui.spacing_mut().slider_width =
                (ui.available_width() - VALUE_BOX_PX - gap).clamp(SLIDER_MIN_PX, SLIDER_MAX_PX);
        }
        ui.add(slider).labelled_by(name.id)
    })
    .inner
}

// ---- helper text -------------------------------------------------------------------------------

/// An explanatory paragraph: one size ([`HELPER_TEXT_PT`]), wrapped, in [`Palette::helper_text`],
/// which reads at 4.5:1 or more on panels, cards and group fills in both themes (the high-contrast
/// setting keeps its own text colour).
///
/// **Rule of use:** for notes, disclaimers and how-to lines under a control, never for a value or a
/// result. Longer than about two lines in a side panel: use [`info_disclosure`] instead. Do not
/// shrink disclaimers below this size; do not use `ui.weak` or `.small()` for sentences.
pub fn helper_text(ui: &mut Ui, text: impl Into<String>) -> Response {
    ui.add(Label::new(helper_rich(ui, text.into())).wrap())
}

fn helper_rich(ui: &Ui, text: String) -> RichText {
    let rich = RichText::new(text).size(HELPER_TEXT_PT);
    if ui.visuals().override_text_color.is_some() {
        rich
    } else {
        rich.color(Palette::of(ui).helper_text())
    }
}

/// A one-line `summary` with a "More" toggle at its right that reveals the full `body` as
/// [`helper_text`] below it ("Less" closes it). The summary never wraps: a summary longer than
/// the row is cut with "…" and shown whole on hover, so the line has the same height whatever its
/// text (what follows it does not move when the summary changes). Closed by default; whether it is
/// open is kept in egui's memory under `id_salt` (scoped to the parent `Ui`) for the session. The
/// toggle reports its expanded state to screen readers. Returns the toggle's response.
///
/// **Rule of use:** for explanations a learner reads once (what a mode means, the detail of a
/// disclaimer, how to drag): the summary is the one short sentence that must stay visible; the body
/// holds the rest. One `id_salt` per disclosure on a page.
pub fn info_disclosure(
    ui: &mut Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    summary: impl Into<String>,
    body: impl Into<String>,
) -> Response {
    let id = ui.scope_id().with(id_salt);
    let mut open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let word = if open { t!("ui-less") } else { t!("ui-more") };
    let font = egui::FontId::proportional(HELPER_TEXT_PT);
    let word_w = ui
        .painter()
        .layout_no_wrap(word.clone(), font, Color32::WHITE)
        .size()
        .x;
    let summary = helper_rich(ui, summary.into());
    let toggle = ui
        .horizontal(|ui| {
            let gap = ui.spacing().item_spacing.x;
            let room = (ui.available_width() - word_w - gap - 2.0).max(20.0);
            ui.allocate_ui_with_layout(
                Vec2::new(room, ui.spacing().interact_size.y),
                Layout::left_to_right(Align::Center),
                |ui| ui.add(Label::new(summary).truncate()),
            );
            let toggle = ui.link(RichText::new(word).size(HELPER_TEXT_PT));
            let expanded = open;
            ui.ctx()
                .accesskit_node_builder(toggle.id, |b| b.set_expanded(expanded));
            toggle
        })
        .inner;
    if toggle.clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_temp(id, open));
    }
    if open {
        helper_text(ui, body);
    }
    toggle
}

// ---- collapsible section -----------------------------------------------------------------------

/// A section of a long side panel: a header row (arrow and title) that opens and closes its body.
/// Whether it is open lives in the caller's `open` (so the caller can persist it); a click on the
/// header, or Space / Enter on it, flips it. Returns the header's response.
///
/// **Rule of use:** for the sections of a panel that does not fit the window; put them in a
/// vertical scroll area with a visible bar so the learner sees there is more below. The title is
/// the section's name, one to three words.
pub fn collapsible_section(
    ui: &mut Ui,
    open: &mut bool,
    title: &str,
    body: impl FnOnce(&mut Ui),
) -> Response {
    let header = egui::CollapsingHeader::new(RichText::new(title).strong())
        .id_salt(title)
        .open(Some(*open))
        .show(ui, body)
        .header_response;
    if header.clicked() {
        *open = !*open;
    }
    header
}

// ---- status ------------------------------------------------------------------------------------

/// A read-only status line: a filled tick (`done`) or an empty ring, then `text`. Not interactive
/// and not focusable; screen readers read "Met: text" or "Not met: text" (the glyph and the word
/// carry the state, the colour only confirms it).
///
/// **Rule of use:** for a checklist the program evaluates (what the image shows, a requirement met).
/// Never use a checkbox for state the learner cannot change.
pub fn status_row(ui: &mut Ui, done: bool, text: &str) -> Response {
    const GLYPH: f32 = 16.0;
    let p = Palette::of(ui);
    let gap = ui.spacing().item_spacing.x;
    let width = ui.available_width();
    let colour = ui.visuals().text_color();
    let galley = ui.painter().layout(
        text.to_owned(),
        egui::TextStyle::Body.resolve(ui.style()),
        colour,
        (width - GLYPH - gap).max(40.0),
    );
    let height = galley.size().y.max(GLYPH + 2.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let state = if done {
        t!("ui-status-met")
    } else {
        t!("ui-status-not-met")
    };
    let name = format!("{state}: {text}");
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &name));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let centre = egui::pos2(
            rect.left() + GLYPH / 2.0,
            rect.top() + height.min(GLYPH + 2.0) / 2.0,
        );
        if done {
            let c = if ui.visuals().override_text_color.is_some() {
                crate::theme::ClassColours {
                    strong: colour,
                    on_strong: ui.visuals().panel_fill,
                    ..p.good()
                }
            } else {
                p.good()
            };
            crate::cards::status_icon(painter, centre, GLYPH / 2.0 - 1.0, true, c);
        } else {
            let ring = if ui.visuals().override_text_color.is_some() {
                colour
            } else {
                p.helper_text()
            };
            painter.circle_stroke(centre, GLYPH / 2.0 - 2.0, Stroke::new(1.5, ring));
        }
        painter.galley(
            egui::pos2(rect.left() + GLYPH + gap, rect.center().y - galley.size().y / 2.0),
            galley,
            colour,
        );
    }
    response
}

// ---- segmented control -------------------------------------------------------------------------

/// A segmented control: one choice of a few (two to four), as equal-width buttons joined in a
/// tinted band across the available width. The chosen one is drawn selected (fill and readable
/// selected text); each segment is a selectable button with its own name, reachable with Tab and
/// pressed with Space or Enter. Returns whether the choice changed this frame.
///
/// **Rule of use:** to switch what a panel shows (sections, display modes, practise / test). Not for
/// an action (that is a button) and not for more than four options (use a combo box).
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, current: &mut T, options: &[(T, String)]) -> bool {
    let mut changed = false;
    let p = Palette::of(ui);
    let n = options.len().max(1) as f32;
    crate::cards::group_frame(&p)
        .inner_margin(egui::Margin::same(2))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let width = ((ui.available_width() - 2.0 * (n - 1.0)) / n).floor().max(24.0);
                // as tall as a padded button, so a selected segment's fill is centred on the row
                let height = ui.spacing().interact_size.y.max(
                    ui.text_style_height(&egui::TextStyle::Button) + 2.0 * ui.spacing().button_padding.y,
                );
                for (value, name) in options {
                    let selected = *current == *value;
                    let button = egui::Button::selectable(selected, name.as_str())
                        .min_size(Vec2::new(width, height))
                        .truncate();
                    let r = ui.add_sized(Vec2::new(width, height), button);
                    if r.clicked() && !selected {
                        *current = *value;
                        changed = true;
                    }
                }
            });
        });
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::contrast_ratio;

    /// The primary button's text reads at AA on its fill at rest, hovered and pressed, in both themes.
    #[test]
    fn primary_text_reads_on_every_state_of_its_fill() {
        for p in [Palette::light(), Palette::dark()] {
            let c = p.primary();
            for fill in [c.fill, c.hover, c.pressed] {
                let r = contrast_ratio(c.text, fill);
                assert!(r >= 4.5, "dark={} {fill:?}: {r:.2}", p.dark);
            }
            // and the button stands out from the panel it sits on
            assert!(contrast_ratio(c.fill, p.panel) >= 2.0, "dark={}", p.dark);
        }
    }

    #[test]
    fn helper_text_is_not_below_twelve_points() {
        const { assert!(HELPER_TEXT_PT >= 12.0) };
    }
}
