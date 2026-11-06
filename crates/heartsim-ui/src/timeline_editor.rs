//! Scenario timeline editor (brief B15): a table of [`TimedAction`]s with add / edit / delete, a
//! scrub bar with one marker per action, and markers on the ECG strip.
//!
//! Each action is edited as its RON text (the scenario-file syntax), so every [`Action`] variant
//! — including ones added after this widget was written — can be authored without a form per
//! variant. A row whose text does not parse is shown in red and keeps the timeline from being
//! taken out until it is fixed.

use crate::t;
use egui::{Color32, Painter, Pos2, Sense, Stroke, Ui, vec2};
use heartsim_core::{Action, Millis, Region, StimSite, TimedAction};

use crate::StripWindow;

const MARKER: Color32 = Color32::from_rgb(80, 160, 255);
const MARKER_SELECTED: Color32 = Color32::from_rgb(255, 196, 0);

/// One table row: time and the action's RON text, with the parse result.
#[derive(Debug, Clone)]
struct Row {
    at_ms: f64,
    text: String,
    parsed: Result<Action, String>,
}

impl Row {
    fn new(at_ms: f64, action: &Action) -> Self {
        Self {
            at_ms,
            text: ron::to_string(action).unwrap_or_default(),
            parsed: Ok(action.clone()),
        }
    }

    fn reparse(&mut self) {
        self.parsed = ron::from_str::<Action>(&self.text).map_err(|e| e.to_string());
    }
}

/// Starting point for a new row of each kind (the user edits the values).
#[must_use]
pub fn templates() -> Vec<Action> {
    let site = || StimSite::Landmark("rv_apex".into());
    vec![
        Action::SetGlobalSinusRate { bpm: 60.0 },
        Action::Block {
            region: Region::LeftBundleBranch,
            blocked: true,
        },
        Action::Stimulate {
            site: site(),
            strength: 2.0,
            duration_ms: 2.0,
        },
        Action::PacingTrain {
            site: site(),
            s1_cycle_ms: 600.0,
            s1_count: 8,
            s2_ms: Some(300.0),
            s3_ms: None,
        },
        Action::AccessoryPathway {
            enabled: true,
            antegrade: true,
            retrograde: true,
        },
        Action::Drug {
            name: "adenosine".into(),
            dose: 1.0,
        },
        Action::Shock {
            joules: 200.0,
            synchronised: true,
        },
        Action::Stenosis {
            branch: "lad".into(),
            severity: 1.0,
        },
    ]
}

/// The variant name of an action, as written in scenario files (`"Block"`, `"Shock"`, …).
#[must_use]
pub fn action_kind(action: &Action) -> String {
    let text = ron::to_string(action).unwrap_or_default();
    text.split(['(', ' ']).next().unwrap_or_default().to_owned()
}

/// Editing state of one scenario timeline.
#[derive(Debug, Clone, Default)]
pub struct TimelineEditor {
    rows: Vec<Row>,
    /// Row shown selected in the table and on the scrub bar.
    pub selected: Option<usize>,
    /// Template chosen for the next "Add".
    new_kind: usize,
    /// Time for the next "Add", ms.
    new_at_ms: f64,
}

impl TimelineEditor {
    #[must_use]
    pub fn new(actions: &[TimedAction]) -> Self {
        let mut e = Self {
            rows: actions.iter().map(|a| Row::new(a.at.0, &a.action)).collect(),
            ..Self::default()
        };
        e.sort();
        e
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Every row parses.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.rows.iter().all(|r| r.parsed.is_ok())
    }

    /// The edited timeline in time order, or `None` while a row does not parse.
    #[must_use]
    pub fn timeline(&self) -> Option<Vec<TimedAction>> {
        self.rows
            .iter()
            .map(|r| {
                r.parsed.as_ref().ok().map(|a| TimedAction {
                    at: Millis(r.at_ms),
                    action: a.clone(),
                })
            })
            .collect()
    }

    /// Add an action; the table stays sorted by time (stable for equal times).
    pub fn add(&mut self, at_ms: f64, action: &Action) {
        let at_ms = at_ms.max(0.0);
        self.rows.push(Row::new(at_ms, action));
        self.sort();
        self.selected = self.rows.iter().rposition(|r| r.at_ms <= at_ms);
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.rows.len() {
            self.rows.remove(index);
            self.selected = None;
        }
    }

    /// Replace row `index`'s action text; returns whether it parses.
    pub fn set_text(&mut self, index: usize, text: &str) -> bool {
        let Some(r) = self.rows.get_mut(index) else {
            return false;
        };
        text.clone_into(&mut r.text);
        r.reparse();
        r.parsed.is_ok()
    }

    /// Move row `index` to a new time.
    pub fn set_time(&mut self, index: usize, at_ms: f64) {
        if let Some(r) = self.rows.get_mut(index) {
            r.at_ms = at_ms.max(0.0);
        }
        self.sort();
    }

    fn sort(&mut self) {
        self.rows.sort_by(|a, b| a.at_ms.total_cmp(&b.at_ms));
    }

    /// The table, add row and scrub bar. `now_ms` puts a cursor on the scrub bar (the running
    /// scenario's clock). Returns `true` when the timeline changed.
    pub fn ui(&mut self, ui: &mut Ui, now_ms: Option<f64>) -> bool {
        let mut changed = false;
        let times: Vec<f64> = self.rows.iter().map(|r| r.at_ms).collect();
        if let Some(i) = scrub_bar(ui, &times, now_ms, self.selected) {
            self.selected = Some(i);
        }
        ui.add_space(4.0);
        let mut remove = None;
        let mut retime = None;
        egui::Grid::new("timeline_rows")
            .striped(true)
            .num_columns(4)
            .show(ui, |ui| {
                let time_header = ui.strong(t!("timeline-time")).id;
                ui.strong(t!("timeline-kind"));
                let action_header = ui.strong(t!("timeline-action")).id;
                ui.label("");
                ui.end_row();
                for (i, r) in self.rows.iter_mut().enumerate() {
                    let mut at = r.at_ms;
                    let resp = ui
                        .add(egui::DragValue::new(&mut at).speed(10.0).range(0.0..=3_600_000.0))
                        .labelled_by(time_header);
                    if resp.changed() {
                        retime = Some((i, at));
                    }
                    let kind = r.parsed.as_ref().map_or_else(|_| "?".to_owned(), action_kind);
                    if ui.selectable_label(self.selected == Some(i), kind).clicked() {
                        self.selected = Some(i);
                    }
                    let colour = if r.parsed.is_ok() {
                        ui.visuals().text_color()
                    } else {
                        Color32::LIGHT_RED
                    };
                    let edit = egui::TextEdit::singleline(&mut r.text)
                        .text_color(colour)
                        .desired_width(ui.available_width().max(200.0) - 40.0);
                    let resp = ui.add(edit).labelled_by(action_header);
                    if let Err(e) = &r.parsed {
                        resp.clone().on_hover_text(e);
                    }
                    if resp.changed() {
                        r.reparse();
                        changed = true;
                    }
                    if ui.button(t!("timeline-delete")).clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
        if let Some((i, at)) = retime {
            self.set_time(i, at);
            changed = true;
        }
        if let Some(i) = remove {
            self.remove(i);
            changed = true;
        }
        ui.horizontal(|ui| {
            let tpl = templates();
            self.new_kind = self.new_kind.min(tpl.len() - 1);
            let new_label = ui.label(t!("timeline-new-action")).id;
            egui::ComboBox::from_id_salt("timeline_new_kind")
                .selected_text(action_kind(&tpl[self.new_kind]))
                .show_ui(ui, |ui| {
                    for (i, a) in tpl.iter().enumerate() {
                        ui.selectable_value(&mut self.new_kind, i, action_kind(a));
                    }
                })
                .response
                .labelled_by(new_label);
            let at = ui.label(t!("timeline-at")).id;
            ui.add(
                egui::DragValue::new(&mut self.new_at_ms)
                    .speed(10.0)
                    .range(0.0..=3_600_000.0)
                    .suffix(format!(" {}", t!("timeline-ms"))),
            )
            .labelled_by(at);
            if ui.button(t!("timeline-add")).clicked() {
                self.add(self.new_at_ms, &tpl[self.new_kind]);
                changed = true;
            }
        });
        changed
    }
}

/// A bar from 0 to the end of the timeline (or `now_ms`, if later) with a tick per action and a
/// cursor at `now_ms`. Returns the index of a clicked tick.
pub fn scrub_bar(
    ui: &mut Ui,
    times_ms: &[f64],
    now_ms: Option<f64>,
    selected: Option<usize>,
) -> Option<usize> {
    let end_ms = times_ms
        .iter()
        .copied()
        .chain(now_ms)
        .fold(10_000.0_f64, f64::max)
        * 1.05;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click());
    let painter = ui.painter_at(rect);
    let x_of = |t: f64| rect.left() + (t / end_ms) as f32 * rect.width();
    painter.rect_filled(rect, 3.0, ui.visuals().extreme_bg_color);
    for s in 0..=(end_ms / 1000.0) as u32 {
        let x = x_of(f64::from(s) * 1000.0);
        let h = if s % 5 == 0 { 8.0 } else { 4.0 };
        painter.line_segment(
            [Pos2::new(x, rect.bottom() - h), Pos2::new(x, rect.bottom())],
            Stroke::new(1.0, ui.visuals().weak_text_color()),
        );
    }
    for (i, &t) in times_ms.iter().enumerate() {
        let colour = if selected == Some(i) {
            MARKER_SELECTED
        } else {
            MARKER
        };
        let x = x_of(t);
        painter.line_segment(
            [Pos2::new(x, rect.top() + 2.0), Pos2::new(x, rect.bottom() - 2.0)],
            Stroke::new(3.0, colour),
        );
    }
    if let Some(now) = now_ms {
        let x = x_of(now);
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(1.5, Color32::LIGHT_RED),
        );
    }
    let clicked = resp
        .clicked()
        .then(|| resp.interact_pointer_pos())
        .flatten()
        .and_then(|p| {
            times_ms
                .iter()
                .enumerate()
                .map(|(i, &t)| (i, (x_of(t) - p.x).abs()))
                .filter(|&(_, d)| d < 6.0)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i)
        });
    resp.on_hover_text(t!("timeline-hint"));
    clicked
}

/// Absolute strip-sample index of simulated time `t_ms`, given that the newest sample
/// (`newest_sample`) was taken at `now_ms`. `None` before the strip's first sample.
#[must_use]
pub fn sample_of_time(t_ms: f64, now_ms: f64, newest_sample: u64, sample_rate_hz: f32) -> Option<u64> {
    let back = ((now_ms - t_ms) * f64::from(sample_rate_hz) / 1000.0).round();
    (back >= 0.0 && back <= newest_sample as f64).then(|| newest_sample - back as u64)
}

/// Mark the timeline's actions on an ECG strip: a dashed-looking vertical line labelled with the
/// action kind at each action's time.
pub fn draw_strip_markers(
    painter: &Painter,
    window: &StripWindow,
    actions: &[TimedAction],
    now_ms: f64,
    newest_sample: u64,
) {
    let top = window.rect.top() + 2.0;
    for a in actions.iter().filter(|a| a.at.0 <= now_ms) {
        let Some(x) = sample_of_time(a.at.0, now_ms, newest_sample, window.sample_rate_hz)
            .and_then(|s| window.x_of_sample(s))
        else {
            continue;
        };
        let mut y = top + 12.0;
        while y < window.rect.bottom() {
            painter.line_segment(
                [Pos2::new(x, y), Pos2::new(x, (y + 4.0).min(window.rect.bottom()))],
                Stroke::new(1.0, MARKER),
            );
            y += 8.0;
        }
        painter.text(
            Pos2::new(x + 2.0, top),
            egui::Align2::LEFT_TOP,
            action_kind(&a.action),
            egui::FontId::proportional(10.0),
            MARKER,
        );
    }
}
