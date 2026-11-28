//! The case editor's page (brief B42): the form for [`Draft`], with the problems panel always in
//! view above it. Independent of the GPU and of [`crate::HeartSimApp`]; saving and trying the draft
//! are requests ([`EditorAction`]) that [`crate::case_ui::Cases`] carries out, so this file has no
//! `cfg` and no I/O.
//!
//! Layout: the header (title, "not saved", Back, Try it, Save as…) and the problems panel sit in a
//! top panel that does not scroll; the form below scrolls. On a phone-width window
//! ([`crate::responsive::is_narrow`]) the stage list stacks above the selected stage and the
//! vignette's preview goes under its text instead of beside it.

use crate::case_edit::{
    ActionExample, Draft, MAX_CHOICES, MIN_CHOICES, PANELS, Problem, Row, action_examples, panel_name,
};
use crate::scenarios::{ScenarioLibrary, picker_groups};
use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use heartsim_casefile::MAX_ID_LEN;
use heartsim_core::Lead;
use heartsim_core::tutorial::{Step, UiAction, WaitEvent};
use heartsim_ui::{cards, t};

/// What the page asks [`crate::case_ui::Cases`] to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorAction {
    /// Leave the editor for the list; the draft is kept.
    Back,
    /// Play the draft in the reader from the selected stage.
    TryIt,
    /// Write the draft as a `.hs` file.
    Save,
}

/// The draft being written and the page's own state.
#[derive(Debug, Clone)]
pub struct Editor {
    pub draft: Draft,
    /// The draft as it was last saved, or as it began (a new case, a copy): the page says "not
    /// saved" while the draft differs from it.
    baseline: Draft,
    /// The stage shown on the right (0-based).
    pub selected: usize,
    /// Seconds the "Wait" helper inserts.
    wait_s: f64,
    /// The entry of the "Other action" list.
    other: usize,
    /// The last thing the page refused to do (bundling a built-in rhythm), shown under the details.
    note: Option<String>,
    examples: Vec<ActionExample>,
}

impl Editor {
    /// An editor that starts with `draft`, counted as saved until it changes.
    #[must_use]
    pub fn new(draft: Draft) -> Self {
        Self {
            baseline: draft.clone(),
            draft,
            selected: 0,
            wait_s: 3.0,
            other: 0,
            note: None,
            examples: action_examples(),
        }
    }

    /// `true` when the draft differs from what was last saved.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.draft != self.baseline
    }

    /// The draft as saved is `saved` (a snapshot taken when Save as… was clicked): the page stops
    /// saying "not saved" unless the draft changed since.
    pub fn mark_saved(&mut self, saved: Draft) {
        self.baseline = saved;
    }

    /// The title for lists: the draft's, or "(no title)".
    #[must_use]
    pub fn display_title(&self) -> String {
        if self.draft.title.trim().is_empty() {
            t!("ced-untitled-case")
        } else {
            self.draft.title.clone()
        }
    }
}

/// A change the page makes to the draft after drawing it (the widgets borrow the draft while they
/// are drawn).
#[derive(Debug, Clone)]
enum Op {
    Select(usize),
    AddStage,
    DuplicateStage,
    DeleteStage,
    MoveStage(bool),
    AddStep(Step),
    DeleteRow(usize),
    MoveRow(usize, bool),
    AddQuestion,
    DeleteQuestion(usize),
    MoveQuestion(usize, bool),
    SetAnswer(usize, usize),
    AddChoice(usize),
    RemoveChoice(usize, usize),
    Bundle(String),
    Unbundle(String),
    ResetId,
}

fn apply(ed: &mut Editor, op: Op, scenarios: &ScenarioLibrary) {
    let si = ed.selected;
    let d = &mut ed.draft;
    match op {
        Op::Select(i) => ed.selected = i.min(d.stages.len() - 1),
        Op::AddStage => ed.selected = d.add_stage(),
        Op::DuplicateStage => ed.selected = d.duplicate_stage(si),
        Op::DeleteStage => ed.selected = d.delete_stage(si),
        Op::MoveStage(up) => ed.selected = d.move_stage(si, up),
        Op::AddStep(step) => d.add_step(si, &step),
        Op::DeleteRow(ri) => d.delete_row(si, ri),
        Op::MoveRow(ri, up) => d.move_row(si, ri, up),
        Op::AddQuestion => {
            d.add_question(si);
        }
        Op::DeleteQuestion(qi) => d.delete_question(si, qi),
        Op::MoveQuestion(qi, up) => d.move_question(si, qi, up),
        Op::SetAnswer(qi, ci) => d.set_answer(si, qi, ci),
        Op::AddChoice(qi) => d.add_choice(si, qi),
        Op::RemoveChoice(qi, ci) => d.remove_choice(si, qi, ci),
        Op::Bundle(id) => ed.note = d.bundle(&id, scenarios).err(),
        Op::Unbundle(id) => d.unbundle(&id),
        Op::ResetId => d.reset_id(),
    }
}

/// A label, then its field, tied together for screen readers.
fn labelled(ui: &mut egui::Ui, label: String, edit: egui::TextEdit<'_>) -> egui::Response {
    let l = ui.label(label);
    ui.add(edit).labelled_by(l.id)
}

fn line(text: &mut String) -> egui::TextEdit<'_> {
    egui::TextEdit::singleline(text).desired_width(f32::INFINITY)
}

fn block(text: &mut String, rows: usize) -> egui::TextEdit<'_> {
    egui::TextEdit::multiline(text)
        .desired_rows(rows)
        .desired_width(f32::INFINITY)
}

/// The editor page: the top panel (header and problems) and the scrolling form. Returns what the
/// user asked for this frame.
pub fn editor_ui(
    ui: &mut egui::Ui,
    ed: &mut Editor,
    scenarios: &ScenarioLibrary,
    markdown: &mut CommonMarkCache,
    busy: bool,
    status: Option<(bool, &str)>,
) -> Option<EditorAction> {
    ed.selected = ed.selected.min(ed.draft.stages.len().saturating_sub(1));
    let problems = ed.draft.problems(scenarios);
    let fix = problems.iter().filter(|p| p.blocking).count();
    let mut action = None;
    let mut ops: Vec<Op> = Vec::new();

    egui::Panel::top("case_editor_top").show(ui, |ui| {
        header(ui, ed, fix == 0 && !busy, fix, &mut action);
        if let Some((ok, text)) = status {
            if ok {
                cards::result_box(ui, true, text);
            } else {
                ui.colored_label(ui.visuals().error_fg_color, text);
            }
        }
        problems_panel(ui, &problems, &mut ops);
    });
    egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical()
            .id_salt("case_editor_form")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                details(ui, ed, scenarios, &mut ops);
                ui.add_space(8.0);
                stages(ui, ed, scenarios, markdown, &mut ops);
            });
    });
    for op in ops {
        apply(ed, op, scenarios);
    }
    action
}

fn header(ui: &mut egui::Ui, ed: &Editor, can_save: bool, fix: usize, action: &mut Option<EditorAction>) {
    ui.horizontal_wrapped(|ui| {
        ui.heading(t!("ced-heading"));
        ui.strong(ed.display_title());
        if ed.is_dirty() {
            cards::pill(ui, t!("ced-not-saved"));
        }
    });
    ui.horizontal_wrapped(|ui| {
        if ui
            .button(t!("ced-back"))
            .on_hover_text(t!("ced-back-hint"))
            .clicked()
        {
            *action = Some(EditorAction::Back);
        }
        if ui
            .button(t!("ced-try"))
            .on_hover_text(t!("ced-try-hint"))
            .clicked()
        {
            *action = Some(EditorAction::TryIt);
        }
        let hint = if fix == 0 {
            t!("ced-save-hint")
        } else {
            t!("ced-save-blocked")
        };
        let r = ui
            .add_enabled_ui(can_save, |ui| heartsim_ui::primary_button(ui, t!("ced-save")))
            .inner;
        if r.on_hover_text(hint.as_str())
            .on_disabled_hover_text(hint)
            .clicked()
        {
            *action = Some(EditorAction::Save);
        }
    });
}

fn problems_panel(ui: &mut egui::Ui, problems: &[Problem], ops: &mut Vec<Op>) {
    let fix = problems.iter().filter(|p| p.blocking).count();
    let advice = problems.len() - fix;
    ui.add_space(2.0);
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());
        if problems.is_empty() {
            ui.label(t!("ced-no-problems"));
            return;
        }
        ui.strong(t!(
            "ced-problems",
            fix = fix.to_string(),
            advice = advice.to_string()
        ));
        egui::ScrollArea::vertical()
            .id_salt("case_editor_problems")
            .max_height(96.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for p in problems {
                    ui.horizontal_wrapped(|ui| {
                        let (tag, colour) = if p.blocking {
                            (t!("ced-must-fix"), ui.visuals().error_fg_color)
                        } else {
                            (t!("ced-advice"), ui.visuals().warn_fg_color)
                        };
                        ui.colored_label(colour, tag);
                        if let Some(s) = p.stage {
                            if ui.link(p.text.as_str()).clicked() {
                                ops.push(Op::Select(s));
                            }
                        } else {
                            ui.label(p.text.as_str());
                        }
                    });
                }
            });
    });
}

fn details(ui: &mut egui::Ui, ed: &mut Editor, scenarios: &ScenarioLibrary, ops: &mut Vec<Op>) {
    egui::CollapsingHeader::new(t!("ced-details"))
        .default_open(true)
        .show(ui, |ui| {
            heartsim_ui::helper_text(ui, t!("ced-synthetic"));
            let d = &mut ed.draft;
            let mut title = d.title.clone();
            if labelled(ui, t!("ced-title"), line(&mut title)).changed() {
                d.set_title(&title);
            }
            let mut id = d.id.clone();
            if labelled(ui, t!("ced-id"), line(&mut id).font(egui::TextStyle::Monospace)).changed() {
                d.set_id(&id);
            }
            ui.horizontal_wrapped(|ui| {
                heartsim_ui::helper_text(ui, t!("ced-id-hint", max = MAX_ID_LEN.to_string()));
                if d.id_edited && ui.button(t!("ced-id-from-title")).clicked() {
                    ops.push(Op::ResetId);
                }
            });
            labelled(ui, t!("ced-summary"), block(&mut d.summary, 2));
            labelled(ui, t!("ced-patient"), line(&mut d.patient));

            let l = ui.label(t!("ced-language"));
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut d.language).desired_width(80.0))
                    .labelled_by(l.id);
                egui::ComboBox::from_id_salt("ced_language")
                    .selected_text(t!("ced-pick"))
                    .show_ui(ui, |ui| {
                        for lang in heartsim_ui::i18n::LANGUAGES {
                            if ui.selectable_label(d.language == lang.code, lang.name).clicked() {
                                lang.code.clone_into(&mut d.language);
                            }
                        }
                    });
            });

            labelled(ui, t!("ced-authors"), block(&mut d.authors, 2));

            let l = ui.label(t!("ced-licence"));
            ui.add(line(&mut d.licence)).labelled_by(l.id);
            ui.horizontal_wrapped(|ui| {
                egui::ComboBox::from_id_salt("ced_licence")
                    .selected_text(t!("ced-licences"))
                    .show_ui(ui, |ui| {
                        for lic in crate::case_edit::LICENCES {
                            if ui.selectable_label(d.licence == lic, lic).clicked() {
                                lic.clone_into(&mut d.licence);
                            }
                        }
                    });
                heartsim_ui::helper_text(ui, t!("ced-licence-hint"));
            });

            labelled(ui, t!("ced-references"), block(&mut d.references, 3));
            heartsim_ui::helper_text(ui, t!("ced-references-hint"));

            ui.add_space(4.0);
            ui.strong(t!("ced-bundled"));
            if d.bundled.is_empty() {
                heartsim_ui::helper_text(ui, t!("ced-bundled-none"));
            }
            for (id, _) in &d.bundled {
                ui.horizontal_wrapped(|ui| {
                    let title = scenarios
                        .index_of(id)
                        .and_then(|i| scenarios.entries.get(i))
                        .map_or_else(String::new, |s| s.scenario.title.clone());
                    ui.label(format!("{id} {title}").trim().to_owned());
                    if ui
                        .button(t!("ced-unbundle"))
                        .on_hover_text(t!("ced-unbundle-hint", id = id.as_str()))
                        .clicked()
                    {
                        ops.push(Op::Unbundle(id.clone()));
                    }
                });
            }
            for id in d.to_bundle(scenarios) {
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        t!("ced-bundle-offer", id = id.as_str()),
                    );
                    if ui.button(t!("ced-bundle")).clicked() {
                        ops.push(Op::Bundle(id.clone()));
                    }
                });
            }
            if let Some(n) = &ed.note {
                ui.colored_label(ui.visuals().error_fg_color, n.as_str());
            }
        });
}

fn stages(
    ui: &mut egui::Ui,
    ed: &mut Editor,
    scenarios: &ScenarioLibrary,
    markdown: &mut CommonMarkCache,
    ops: &mut Vec<Op>,
) {
    let narrow = crate::responsive::is_narrow(ui.ctx());
    ui.strong(t!("ced-stages"));
    if narrow {
        stage_list(ui, ed, true, ops);
        ui.separator();
        stage_ui(ui, ed, scenarios, markdown, true, ops);
    } else {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(190.0);
                stage_list(ui, ed, false, ops);
            });
            ui.separator();
            ui.vertical(|ui| {
                stage_ui(ui, ed, scenarios, markdown, false, ops);
            });
        });
    }
}

/// The stage list with its buttons: add, duplicate, delete, move.
fn stage_list(ui: &mut egui::Ui, ed: &Editor, narrow: bool, ops: &mut Vec<Op>) {
    let n = ed.draft.stages.len();
    let entries = |ui: &mut egui::Ui| {
        for (i, s) in ed.draft.stages.iter().enumerate() {
            let title = if s.title.trim().is_empty() {
                t!("ced-untitled")
            } else {
                s.title.clone()
            };
            let label = t!("ced-stage-n", n = (i + 1).to_string(), title = title);
            if ui.selectable_label(ed.selected == i, label).clicked() {
                ops.push(Op::Select(i));
            }
        }
    };
    if narrow {
        ui.horizontal_wrapped(entries);
    } else {
        ui.vertical(entries);
    }
    ui.horizontal_wrapped(|ui| {
        if ui.button(t!("ced-stage-add")).clicked() {
            ops.push(Op::AddStage);
        }
        if ui.button(t!("ced-stage-dup")).clicked() {
            ops.push(Op::DuplicateStage);
        }
        if ui
            .add_enabled(n > 1, egui::Button::new(t!("ced-stage-del")))
            .clicked()
        {
            ops.push(Op::DeleteStage);
        }
        if ui
            .add_enabled(ed.selected > 0, egui::Button::new(t!("ced-stage-up")))
            .clicked()
        {
            ops.push(Op::MoveStage(true));
        }
        if ui
            .add_enabled(ed.selected + 1 < n, egui::Button::new(t!("ced-stage-down")))
            .clicked()
        {
            ops.push(Op::MoveStage(false));
        }
    });
}

fn stage_ui(
    ui: &mut egui::Ui,
    ed: &mut Editor,
    scenarios: &ScenarioLibrary,
    markdown: &mut CommonMarkCache,
    narrow: bool,
    ops: &mut Vec<Op>,
) {
    let si = ed.selected;
    let Editor {
        draft,
        wait_s,
        other,
        examples,
        ..
    } = ed;
    let Draft { stages, bundled, .. } = draft;
    let Some(stage) = stages.get_mut(si) else {
        return;
    };
    // a rhythm the row loads is known when it is bundled or in the session's library
    let known = |id: &str| bundled.iter().any(|(b, _)| b == id) || scenarios.index_of(id).is_some();
    ui.push_id(("case_editor_stage", si), |ui| {
        labelled(ui, t!("ced-stage-title"), line(&mut stage.title));

        // the vignette, with its preview beside it (under it on a narrow window)
        let preview = |ui: &mut egui::Ui, md: &mut CommonMarkCache, text: &str| {
            ui.strong(t!("ced-preview"));
            egui::ScrollArea::vertical()
                .id_salt("case_editor_preview")
                .max_height(240.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    CommonMarkViewer::new().show(ui, md, text);
                });
        };
        let label = ui.label(t!("ced-vignette"));
        if narrow {
            ui.add(block(&mut stage.markdown, 8)).labelled_by(label.id);
            preview(ui, markdown, &stage.markdown);
        } else {
            ui.columns(2, |cols| {
                cols[0].add(block(&mut stage.markdown, 10)).labelled_by(label.id);
                preview(&mut cols[1], markdown, &stage.markdown);
            });
        }

        ui.add_space(6.0);
        ui.strong(t!("ced-actions"));
        heartsim_ui::helper_text(ui, t!("ced-actions-hint"));
        helpers(ui, scenarios, wait_s, other, examples, ops);
        if stage.rows.is_empty() {
            heartsim_ui::helper_text(ui, t!("ced-no-rows"));
        }
        let n_rows = stage.rows.len();
        for (ri, r) in stage.rows.iter_mut().enumerate() {
            action_row(ui, ri, n_rows, r, &known, ops);
        }

        ui.add_space(6.0);
        ui.strong(t!("ced-questions"));
        if stage.questions.is_empty() {
            heartsim_ui::helper_text(ui, t!("ced-no-questions"));
        }
        let n_q = stage.questions.len();
        for (qi, q) in stage.questions.iter_mut().enumerate() {
            question_ui(ui, qi, n_q, q, ops);
        }
        if ui.button(t!("ced-question-add")).clicked() {
            ops.push(Op::AddQuestion);
        }
    });
}

/// The helpers above the rows: each inserts a correct row.
fn helpers(
    ui: &mut egui::Ui,
    scenarios: &ScenarioLibrary,
    wait_s: &mut f64,
    other: &mut usize,
    examples: &[ActionExample],
    ops: &mut Vec<Op>,
) {
    let step = |a: UiAction| Op::AddStep(Step::Ui(a));
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("ced_load_rhythm")
            .selected_text(t!("ced-load-rhythm"))
            .show_ui(ui, |ui| {
                for (heading, group) in picker_groups(scenarios) {
                    if let Some(h) = heading {
                        ui.separator();
                        ui.weak(h);
                    }
                    for i in group {
                        let s = &scenarios.entries[i].scenario;
                        if ui
                            .selectable_label(false, s.title.as_str())
                            .on_hover_text(s.description.as_str())
                            .clicked()
                        {
                            ops.push(step(UiAction::LoadScenario(s.id.clone())));
                        }
                    }
                }
            });
        egui::ComboBox::from_id_salt("ced_show_panel")
            .selected_text(t!("ced-show-panel"))
            .show_ui(ui, |ui| {
                for p in PANELS {
                    if ui.selectable_label(false, panel_name(p)).clicked() {
                        ops.push(step(UiAction::FocusPanel(p)));
                    }
                }
            });
        egui::ComboBox::from_id_salt("ced_select_lead")
            .selected_text(t!("ced-select-lead"))
            .show_ui(ui, |ui| {
                for l in Lead::ALL {
                    if ui.selectable_label(false, l.label()).clicked() {
                        ops.push(step(UiAction::SelectLead(l)));
                    }
                }
            });
    });
    ui.horizontal_wrapped(|ui| {
        let l = ui.label(t!("ced-wait-seconds"));
        ui.add(
            egui::DragValue::new(wait_s)
                .range(0.1..=600.0)
                .speed(0.1)
                .max_decimals(1),
        )
        .labelled_by(l.id);
        if ui
            .button(t!("ced-wait"))
            .on_hover_text(t!("ced-wait-hint"))
            .clicked()
        {
            ops.push(step(UiAction::WaitForEvent(WaitEvent::ElapsedMs(
                (*wait_s * 1000.0).round(),
            ))));
        }
    });
    ui.horizontal_wrapped(|ui| {
        *other = (*other).min(examples.len().saturating_sub(1));
        let shown = examples.get(*other).map_or_else(String::new, |e| e.kind.clone());
        egui::ComboBox::from_id_salt("ced_other_action")
            .selected_text(format!("{} {shown}", t!("ced-other")))
            .show_ui(ui, |ui| {
                for (i, e) in examples.iter().enumerate() {
                    ui.selectable_value(other, i, e.kind.as_str())
                        .on_hover_text(e.text());
                }
            });
        if let Some(e) = examples.get(*other)
            && ui.button(t!("ced-other-add")).on_hover_text(e.text()).clicked()
        {
            ops.push(Op::AddStep(e.step.clone()));
        }
    });
}

/// One action row: its text, the buttons and what is wrong with it.
fn action_row(
    ui: &mut egui::Ui,
    ri: usize,
    n_rows: usize,
    r: &mut Row,
    known: &dyn Fn(&str) -> bool,
    ops: &mut Vec<Op>,
) {
    ui.push_id(("case_editor_row", ri), |ui| {
        let failed = r.parsed.is_err();
        let l = ui.horizontal_wrapped(|ui| {
            let l = ui.label(t!("ced-row-n", n = (ri + 1).to_string()));
            if ui.add_enabled(ri > 0, egui::Button::new(t!("ced-up"))).clicked() {
                ops.push(Op::MoveRow(ri, true));
            }
            if ui
                .add_enabled(ri + 1 < n_rows, egui::Button::new(t!("ced-down")))
                .clicked()
            {
                ops.push(Op::MoveRow(ri, false));
            }
            if ui.button(t!("ced-delete")).clicked() {
                ops.push(Op::DeleteRow(ri));
            }
            l
        });
        let mut edit = egui::TextEdit::singleline(&mut r.text)
            .desired_width(f32::INFINITY)
            .font(egui::TextStyle::Monospace);
        if failed {
            edit = edit.text_color(ui.visuals().error_fg_color);
        }
        let resp = ui.add(edit).labelled_by(l.inner.id);
        if resp.changed() {
            r.reparse();
        }
        match &r.parsed {
            Err(e) => {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    t!("ced-row-error", error = e.as_str()),
                );
            }
            Ok(Step::Ui(UiAction::LoadScenario(id))) if !known(id) => {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    t!("ced-row-unknown-rhythm", id = id.as_str()),
                );
            }
            Ok(_) => {}
        }
    });
}

/// One question: prompt, three to five choices with a radio for the right one, explanation.
fn question_ui(
    ui: &mut egui::Ui,
    qi: usize,
    n_q: usize,
    q: &mut heartsim_core::tutorial::Question,
    ops: &mut Vec<Op>,
) {
    ui.push_id(("case_editor_question", qi), |ui| {
        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.strong(t!("ced-question-n", n = (qi + 1).to_string()));
                if ui.add_enabled(qi > 0, egui::Button::new(t!("ced-up"))).clicked() {
                    ops.push(Op::MoveQuestion(qi, true));
                }
                if ui
                    .add_enabled(qi + 1 < n_q, egui::Button::new(t!("ced-down")))
                    .clicked()
                {
                    ops.push(Op::MoveQuestion(qi, false));
                }
                if ui.button(t!("ced-delete")).clicked() {
                    ops.push(Op::DeleteQuestion(qi));
                }
            });
            labelled(ui, t!("ced-prompt"), block(&mut q.prompt, 2));
            let n_choices = q.choices.len();
            for (ci, c) in q.choices.iter_mut().enumerate() {
                ui.push_id(ci, |ui| {
                    ui.horizontal(|ui| {
                        let right = q.answer == ci;
                        if ui
                            .radio(right, t!("ced-correct"))
                            .on_hover_text(t!("ced-correct-hint", n = (ci + 1).to_string()))
                            .clicked()
                        {
                            ops.push(Op::SetAnswer(qi, ci));
                        }
                        let l = ui.label(t!("ced-choice-n", n = (ci + 1).to_string()));
                        let w = (ui.available_width() - 90.0).max(60.0);
                        ui.add(egui::TextEdit::singleline(c).desired_width(w))
                            .labelled_by(l.id);
                        if ui
                            .add_enabled(n_choices > MIN_CHOICES, egui::Button::new(t!("ced-choice-del")))
                            .clicked()
                        {
                            ops.push(Op::RemoveChoice(qi, ci));
                        }
                    });
                });
            }
            if ui
                .add_enabled(n_choices < MAX_CHOICES, egui::Button::new(t!("ced-choice-add")))
                .clicked()
            {
                ops.push(Op::AddChoice(qi));
            }
            labelled(ui, t!("ced-explanation"), block(&mut q.explanation, 3));
        });
    });
}
