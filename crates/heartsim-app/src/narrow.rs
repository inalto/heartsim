//! Pages made of a centre figure and one or two side panels, laid out for any width: with room, the
//! panels sit beside the figure; below [`crate::responsive::NARROW_PX`] they cannot (they leave the
//! figure a sliver), so the page is either **segmented** (one pane at a time behind a segmented
//! control) or **stacked** (the figure at a fixed useful height on top, the panels below it in a
//! scroll area). The page supplies one closure that draws a pane by name; the figure is never
//! drawn smaller than the height the page asks for.

use crate::responsive;
use eframe::egui;

/// One of the three places of a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    /// The panel at the left.
    Left,
    /// The central figure (the trace, the dial, the 3-D view).
    Centre,
    /// The panel at the right.
    Right,
}

/// A side panel: its id (egui remembers its width), its default and widest width and, for the
/// segmented control of the narrow layout, its name.
#[derive(Debug, Clone, Copy)]
pub struct Side<'a> {
    pub id: &'a str,
    pub default_px: f32,
    pub cap_px: f32,
    pub name: &'a str,
}

/// How a page arranges its panes when the window is phone-narrow.
#[derive(Debug, Clone, Copy)]
pub enum Narrow {
    /// One pane at a time behind a segmented control, starting at `start`.
    Segmented { start: Pane },
    /// The centre at `centre_height` points on top, the side panels below it in a scroll area.
    Stacked { centre_height: f32 },
}

/// What [`panes`] lays out.
#[derive(Debug, Clone, Copy)]
pub struct PaneSpec<'a> {
    /// Key of the page (the chosen pane is kept in egui's memory under it).
    pub id: &'a str,
    pub left: Option<Side<'a>>,
    pub right: Option<Side<'a>>,
    /// Name of the centre pane in the segmented control.
    pub centre: &'a str,
    pub narrow: Narrow,
}

fn memory_id(key: &str) -> egui::Id {
    egui::Id::unique(("narrow_pane", key))
}

/// The pane a segmented page shows now (`start` before the learner chose one).
#[must_use]
pub fn current_pane(ctx: &egui::Context, key: &str, start: Pane) -> Pane {
    ctx.data(|d| d.get_temp::<PaneChoice>(memory_id(key)))
        .map_or(start, |c| c.0)
}

/// Show `pane` on the segmented page `key` from the next frame (a pick in a list opens its page).
pub fn show_pane(ctx: &egui::Context, key: &str, pane: Pane) {
    ctx.data_mut(|d| d.insert_temp(memory_id(key), PaneChoice(pane)));
}

#[derive(Debug, Clone, Copy)]
struct PaneChoice(Pane);

fn figure_id(key: &str) -> egui::Id {
    egui::Id::unique(("narrow_figure", key))
}

/// The rectangle the centre figure of page `key` was given in the last frame it was drawn (for
/// tests: the trace or the figure must keep a useful size at every width).
#[must_use]
pub fn figure_rect(ctx: &egui::Context, key: &str) -> Option<egui::Rect> {
    ctx.data(|d| d.get_temp::<egui::Rect>(figure_id(key)))
}

fn record_figure(ui: &egui::Ui, key: &str, rect: egui::Rect) {
    ui.ctx().data_mut(|d| d.insert_temp(figure_id(key), rect));
}

/// Draw the page: `body(ui, pane)` draws one pane into the space it is given.
pub fn panes(ui: &mut egui::Ui, spec: &PaneSpec<'_>, mut body: impl FnMut(&mut egui::Ui, Pane)) {
    if !responsive::is_narrow(ui.ctx()) {
        if let Some(s) = spec.left {
            egui::Panel::left(s.id)
                .max_size(responsive::max_side(ui, s.cap_px))
                .default_size(s.default_px)
                .show(ui, |ui| body(ui, Pane::Left));
        }
        if let Some(s) = spec.right {
            egui::Panel::right(s.id)
                .max_size(responsive::max_side(ui, s.cap_px))
                .default_size(s.default_px)
                .show(ui, |ui| body(ui, Pane::Right));
        }
        egui::CentralPanel::default().show(ui, |ui| {
            record_figure(ui, spec.id, ui.available_rect_before_wrap());
            body(ui, Pane::Centre);
        });
        return;
    }
    egui::CentralPanel::default().show(ui, |ui| match spec.narrow {
        Narrow::Segmented { start } => {
            let mut shown = current_pane(ui.ctx(), spec.id, start);
            let mut options = Vec::new();
            if let Some(s) = spec.left {
                options.push((Pane::Left, s.name.to_owned()));
            }
            options.push((Pane::Centre, spec.centre.to_owned()));
            if let Some(s) = spec.right {
                options.push((Pane::Right, s.name.to_owned()));
            }
            if !options.iter().any(|(p, _)| *p == shown) {
                shown = Pane::Centre;
            }
            heartsim_ui::segmented(ui, &mut shown, &options);
            show_pane(ui.ctx(), spec.id, shown);
            if shown == Pane::Centre {
                record_figure(ui, spec.id, ui.available_rect_before_wrap());
                body(ui, Pane::Centre);
            } else {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| body(ui, shown));
            }
        }
        Narrow::Stacked { centre_height } => {
            let width = ui.available_width();
            let height = centre_height.min((ui.available_height() * 0.6).max(120.0));
            ui.allocate_ui_with_layout(
                egui::vec2(width, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_size(egui::vec2(width, height));
                    record_figure(ui, spec.id, ui.max_rect());
                    body(ui, Pane::Centre);
                },
            );
            ui.separator();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if spec.left.is_some() {
                        body(ui, Pane::Left);
                    }
                    if spec.right.is_some() {
                        body(ui, Pane::Right);
                    }
                });
        }
    });
}
