//! The Echo calculators page (brief B29): a searchable catalogue of `heartsim-calc` on the left,
//! the selected calculator on the right with its inputs, the live result, the worked formula, the
//! findings with their criterion, the references and the review status.
//!
//! What is typed lives only in this struct for the session: [`EchoCalcPage::storage_string`] holds
//! the favourites and the last calculator opened, never a value. The result is recomputed from the
//! fields every frame, so it can never be stale: any invalid field removes it.

use std::collections::HashMap;
use std::fmt::Write as _;

use eframe::egui::{self, RichText};
use heartsim_calc::{
    ALL, Calculator, Category, ErrorKind, InputError, InputKind, Inputs, Outcome, Review, Severity, SimLink,
    Value, by_id, fmt, in_category, search,
};
use heartsim_ui::theme::Palette;
use heartsim_ui::{cards, i18n, t};

/// eframe storage key of the favourites and the last calculator opened.
pub const STORAGE_KEY: &str = "echo_calculators";

/// The page state.
#[derive(Debug)]
pub struct EchoCalcPage {
    selected: &'static Calculator,
    /// The search box.
    pub query: String,
    favourites: Vec<&'static str>,
    /// Typed text per calculator id and input id (session only).
    fields: HashMap<&'static str, HashMap<&'static str, String>>,
    /// Input to give the keyboard focus to on the next frame (Enter moves on).
    focus: Option<&'static str>,
    /// Phone width: the detail is shown instead of the list.
    narrow_detail: bool,
    /// The fields the Echo page sent a measurement to (calculator, input) and the text it put
    /// there: the mark "from the echo simulator" shows while the field still holds that text.
    /// Session only, like every typed value.
    from_echo: HashMap<(&'static str, &'static str), String>,
}

impl Default for EchoCalcPage {
    fn default() -> Self {
        Self {
            selected: ALL[0],
            query: String::new(),
            favourites: Vec::new(),
            fields: HashMap::new(),
            focus: None,
            narrow_detail: false,
            from_echo: HashMap::new(),
        }
    }
}

/// `s` with the decimal point of the active language (a comma except in English), only between
/// digits so that a sentence keeps its full stops.
fn localise(s: &str) -> String {
    if i18n::language() == "en" {
        return s.to_owned();
    }
    let chars: Vec<char> = s.chars().collect();
    chars
        .iter()
        .enumerate()
        .map(|(k, c)| {
            let digit = |j: Option<&char>| j.is_some_and(char::is_ascii_digit);
            if *c == '.' && k > 0 && digit(chars.get(k - 1)) && digit(chars.get(k + 1)) {
                ','
            } else {
                *c
            }
        })
        .collect()
}

fn severity_label(s: Severity) -> String {
    match s {
        Severity::Normal => t!("calc-sev-normal"),
        Severity::Mild => t!("calc-sev-mild"),
        Severity::Moderate => t!("calc-sev-moderate"),
        Severity::Severe => t!("calc-sev-severe"),
        Severity::Info => t!("calc-sev-info"),
        Severity::Abnormal => t!("calc-sev-abnormal"),
        Severity::Indeterminate => t!("calc-sev-indeterminate"),
    }
}

impl EchoCalcPage {
    /// The calculator shown.
    #[must_use]
    pub fn selected(&self) -> &'static Calculator {
        self.selected
    }

    /// Opens the calculator `id` (unknown ids are ignored).
    pub fn select(&mut self, id: &str) {
        if let Some(c) = by_id(id) {
            self.selected = c;
            self.narrow_detail = true;
        }
    }

    /// The favourite calculators, in the order they were marked.
    #[must_use]
    pub fn favourites(&self) -> &[&'static str] {
        &self.favourites
    }

    /// Marks or unmarks `id` as a favourite.
    pub fn toggle_favourite(&mut self, id: &str) {
        if let Some(k) = self.favourites.iter().position(|f| *f == id) {
            self.favourites.remove(k);
        } else if let Some(c) = by_id(id) {
            self.favourites.push(c.id);
        }
    }

    /// What is saved between sessions: the favourites and the last calculator, never a typed value.
    #[must_use]
    pub fn storage_string(&self) -> String {
        format!("v1;last={};fav={}", self.selected.id, self.favourites.join(","))
    }

    /// Restores [`Self::storage_string`]; unknown ids (from a later version) are dropped.
    pub fn restore(&mut self, s: &str) {
        for part in s.split(';') {
            if let Some(id) = part.strip_prefix("last=")
                && let Some(c) = by_id(id)
            {
                self.selected = c;
            } else if let Some(list) = part.strip_prefix("fav=") {
                self.favourites = list.split(',').filter_map(by_id).map(|c| c.id).collect();
            }
        }
    }

    fn fields_mut(&mut self) -> &mut HashMap<&'static str, String> {
        self.fields.entry(self.selected.id).or_default()
    }

    /// The text of field `input` of the selected calculator.
    #[must_use]
    pub fn field(&self, input: &str) -> &str {
        self.fields
            .get(self.selected.id)
            .and_then(|f| f.get(input))
            .map_or("", String::as_str)
    }

    /// Puts a measurement of the Echo page (brief B34) into field `input` of calculator
    /// `calculator`, marked "from the echo simulator" until the learner edits it. Unknown ids are
    /// ignored; nothing is stored beyond the session.
    pub fn receive_from_echo(&mut self, calculator: &str, input: &str, text: &str) {
        let Some(c) = by_id(calculator) else { return };
        let Some(spec) = c.input(input) else { return };
        self.fields
            .entry(c.id)
            .or_default()
            .insert(spec.id, text.to_owned());
        self.from_echo.insert((c.id, spec.id), text.to_owned());
    }

    /// Whether field `input` of the selected calculator holds what the Echo page sent.
    #[must_use]
    pub fn is_from_echo(&self, input: &str) -> bool {
        let text = self.field(input);
        self.from_echo
            .get(&(self.selected.id, input))
            .is_some_and(|sent| !text.is_empty() && sent == text)
    }

    /// Types `text` into field `input` of the selected calculator.
    pub fn set_field(&mut self, input: &str, text: &str) {
        if let Some(spec) = self.selected.input(input) {
            self.fields_mut().insert(spec.id, text.to_owned());
        }
    }

    /// Fills the fields with the calculator's worked example.
    pub fn load_example(&mut self) {
        let c = self.selected;
        let mut filled = HashMap::new();
        for (id, v) in c.example {
            let Some(spec) = c.input(id) else { continue };
            let text = match v {
                heartsim_calc::ExampleValue::Number(x) => localise(&fmt::trim(*x)),
                heartsim_calc::ExampleValue::Choice(ch) => (*ch).to_owned(),
            };
            filled.insert(spec.id, text);
        }
        self.fields.insert(c.id, filled);
    }

    /// Empties every field of the selected calculator.
    pub fn clear(&mut self) {
        self.fields.remove(self.selected.id);
    }

    /// The fields as typed inputs, or the fields that are not numbers.
    pub fn inputs(&self) -> Result<Inputs, Vec<InputError>> {
        let mut inputs = Inputs::new();
        let mut errors = Vec::new();
        for spec in self.selected.inputs {
            let text = self.field(spec.id).trim();
            if text.is_empty() {
                continue;
            }
            match spec.kind {
                InputKind::Choice(_) => inputs.set(spec.id, Value::Choice(text.to_owned())),
                InputKind::Number { .. } => match text.replace(',', ".").parse::<f64>() {
                    Ok(v) => inputs.set(spec.id, Value::Number(v)),
                    Err(_) => {
                        // a field that is not shown cannot be blamed
                        if Calculator::visible(spec, &inputs) {
                            errors.push(InputError {
                                input: Some(spec.id),
                                kind: ErrorKind::NotANumber,
                            });
                        }
                    }
                },
            }
        }
        if errors.is_empty() {
            Ok(inputs)
        } else {
            Err(errors)
        }
    }

    /// The outcome of the fields as they are now.
    pub fn evaluate(&self) -> Result<Outcome, Vec<InputError>> {
        let inputs = self.inputs()?;
        self.selected.evaluate(&inputs)
    }

    /// The page. `heart_rate_bpm` is the ventricular rate of the running rhythm, if it has beats.
    pub fn ui(&mut self, ui: &mut egui::Ui, heart_rate_bpm: Option<f64>) {
        let narrow = crate::responsive::is_narrow(ui.ctx());
        if narrow {
            egui::CentralPanel::default().show(ui, |ui| {
                if self.narrow_detail {
                    if ui.button(t!("calc-back")).clicked() {
                        self.narrow_detail = false;
                    }
                    egui::ScrollArea::vertical().show(ui, |ui| self.detail_ui(ui, heart_rate_bpm));
                } else {
                    egui::ScrollArea::vertical().show(ui, |ui| self.list_ui(ui));
                }
            });
            return;
        }
        egui::Panel::left("echo_calc_list")
            .max_size(crate::responsive::max_side(ui, 100_000.0))
            .default_size(300.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.list_ui(ui));
            });
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.detail_ui(ui, heart_rate_bpm));
        });
    }

    fn list_entry(&mut self, ui: &mut egui::Ui, c: &'static Calculator, lang: &str) {
        let mut text = RichText::new(c.name.get(lang));
        if c.review == Review::FormulaOnly {
            text = text.italics();
        }
        if ui
            .selectable_label(std::ptr::eq(self.selected, c), text)
            .clicked()
        {
            self.selected = c;
            self.narrow_detail = true;
        }
    }

    fn list_ui(&mut self, ui: &mut egui::Ui) {
        let lang = i18n::language();
        cards::title_band(ui, t!("tab-echo-calculators"));
        // a filled field, so it reads as a place to type in both themes
        let fill = Palette::of(ui).pill;
        let search_box = egui::TextEdit::singleline(&mut self.query)
            .hint_text(t!("calc-search-hint"))
            .background_color(fill)
            .desired_width(f32::INFINITY);
        let label = ui.label(t!("calc-search"));
        ui.add(search_box).labelled_by(label.id);
        ui.add_space(4.0);
        // what the italic names of the list mean (see `Review::FormulaOnly`)
        heartsim_ui::helper_text(ui, t!("calc-italic-legend"));
        ui.add_space(4.0);
        if !self.query.trim().is_empty() {
            let hits = search(&self.query, lang);
            if hits.is_empty() {
                heartsim_ui::helper_text(ui, t!("calc-no-match"));
            }
            for c in hits {
                self.list_entry(ui, c, lang);
            }
            return;
        }
        if !self.favourites.is_empty() {
            cards::subheading(ui, t!("calc-favourites"));
            for id in self.favourites.clone() {
                if let Some(c) = by_id(id) {
                    self.list_entry(ui, c, lang);
                }
            }
            ui.add_space(6.0);
        }
        for cat in Category::ALL {
            cards::subheading(ui, cat.name().get(lang));
            for c in in_category(cat) {
                self.list_entry(ui, c, lang);
            }
            ui.add_space(4.0);
        }
    }

    fn detail_ui(&mut self, ui: &mut egui::Ui, heart_rate_bpm: Option<f64>) {
        let lang = i18n::language();
        let p = Palette::of(ui);
        let c = self.selected;
        ui.horizontal_wrapped(|ui| {
            cards::heading(ui, c.name.get(lang));
            let colours = if c.review == Review::Verified {
                p.good()
            } else {
                p.warn()
            };
            cards::pill_coloured(
                ui,
                c.review.label().get(lang),
                colours.soft,
                colours.on_soft,
                None,
            );
        });
        ui.weak(c.category.name().get(lang));
        ui.label(c.summary.get(lang));
        heartsim_ui::helper_text(ui, t!("calc-disclaimer"));
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button(t!("calc-example")).clicked() {
                self.load_example();
            }
            if ui.button(t!("calc-clear")).clicked() {
                self.clear();
            }
            let mut fav = self.favourites.contains(&c.id);
            if ui.toggle_value(&mut fav, t!("calc-favourite")).clicked() {
                self.toggle_favourite(c.id);
            }
            if let Some(SimLink::HeartRate(input)) = c.sim_link {
                match heart_rate_bpm {
                    Some(hr) => {
                        if ui
                            .button(t!("calc-from-sim", bpm = format!("{hr:.0}")))
                            .on_hover_text(t!("calc-from-sim-hint"))
                            .clicked()
                        {
                            let text = format!("{hr:.0}");
                            self.set_field(input, &text);
                        }
                    }
                    None => {
                        ui.add_enabled(false, egui::Button::new(t!("calc-from-sim-none")));
                    }
                }
            }
        });
        ui.add_space(6.0);

        let result = self.evaluate();
        let errors: Vec<InputError> = result.as_ref().err().cloned().unwrap_or_default();
        self.inputs_ui(ui, &errors, lang);
        ui.add_space(8.0);
        match &result {
            Ok(out) => Self::outcome_ui(ui, c, out, lang, &p),
            Err(errors) => {
                cards::subheading(ui, t!("calc-result"));
                let only_missing = errors.iter().all(|e| e.kind == ErrorKind::Missing);
                if only_missing {
                    heartsim_ui::helper_text(ui, t!("calc-empty"));
                } else {
                    ui.colored_label(p.bad().strong, t!("calc-no-result"));
                    for e in errors.iter().filter(|e| e.input.is_none()) {
                        ui.colored_label(p.bad().strong, e.message(lang));
                    }
                }
            }
        }
        ui.add_space(8.0);
        cards::subheading(ui, t!("calc-references"));
        for r in c.references {
            let rf = r.reference;
            ui.label(format!("{} {}. {}.", rf.authors, rf.title, rf.journal));
            ui.horizontal_wrapped(|ui| {
                heartsim_ui::helper_text(ui, t!("calc-locator", place = r.locator));
                if let Some(doi) = rf.doi {
                    ui.hyperlink_to(format!("doi:{doi}"), format!("https://doi.org/{doi}"));
                }
                if let Some(url) = rf.url {
                    ui.hyperlink_to(t!("calc-open-copy"), url);
                }
            });
        }
        ui.add_space(6.0);
        heartsim_ui::helper_text(
            ui,
            match c.review {
                Review::Verified => t!("calc-status-verified"),
                Review::FormulaOnly => t!("calc-status-formula"),
            },
        );
    }

    fn inputs_ui(&mut self, ui: &mut egui::Ui, errors: &[InputError], lang: &str) {
        let c = self.selected;
        cards::subheading(ui, t!("calc-inputs"));
        let parsed = self.inputs().unwrap_or_default();
        let visible: Vec<_> = c
            .inputs
            .iter()
            .filter(|s| Calculator::visible(s, &parsed))
            .collect();
        let mut next_focus = None;
        let focus = self.focus.take();
        egui::Grid::new(("echo_calc_inputs", c.id))
            .num_columns(3)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                for (k, spec) in visible.iter().enumerate() {
                    let label = ui.label(spec.label.get(lang));
                    let mut text = self.field(spec.id).to_owned();
                    let before = text.clone();
                    match spec.kind {
                        InputKind::Number {
                            unit,
                            min,
                            max,
                            decimals,
                        } => {
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut text)
                                    .background_color(Palette::of(ui).pill)
                                    .desired_width(90.0)
                                    .id(egui::Id::unique(("calc", c.id, spec.id))),
                            );
                            let resp = resp.labelled_by(label.id);
                            if focus == Some(spec.id) {
                                resp.request_focus();
                            }
                            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                next_focus = visible[k + 1..]
                                    .iter()
                                    .find(|s| matches!(s.kind, InputKind::Number { .. }))
                                    .map(|s| s.id);
                            }
                            let range = t!(
                                "calc-range",
                                min = localise(&fmt::num(min, decimals)),
                                max = localise(&fmt::num(max, decimals))
                            );
                            let mut hint = format!("{}  {range}", unit.symbol());
                            if spec.optional {
                                let _ = write!(hint, " · {}", t!("calc-optional"));
                            }
                            let from_echo = self.is_from_echo(spec.id);
                            ui.vertical(|ui| {
                                ui.horizontal_wrapped(|ui| {
                                    heartsim_ui::helper_text(ui, hint.trim_start());
                                    Self::field_error(ui, errors, spec.id, lang);
                                });
                                // the origin of a value the Echo page sent (brief B34): a pill where
                                // there is room, plain text where the column is narrow
                                if from_echo {
                                    if ui.available_width() >= 200.0 {
                                        cards::pill(ui, t!("calc-from-echo"));
                                    } else {
                                        heartsim_ui::helper_text(ui, t!("calc-from-echo"));
                                    }
                                }
                            });
                        }
                        InputKind::Choice(choices) => {
                            let shown = choices
                                .iter()
                                .find(|ch| ch.id == text)
                                .map_or_else(|| t!("calc-choose"), |ch| ch.label.get(lang).to_owned());
                            // named by its label for screen readers, like the number fields
                            let _ = egui::ComboBox::from_id_salt(("calc-choice", c.id, spec.id))
                                .selected_text(shown)
                                .show_ui(ui, |ui| {
                                    if spec.optional {
                                        ui.selectable_value(&mut text, String::new(), t!("calc-none"));
                                    }
                                    for ch in choices {
                                        ui.selectable_value(&mut text, ch.id.to_owned(), ch.label.get(lang));
                                    }
                                })
                                .response
                                .labelled_by(label.id);
                            ui.horizontal(|ui| {
                                if spec.optional {
                                    heartsim_ui::helper_text(ui, t!("calc-optional"));
                                }
                                Self::field_error(ui, errors, spec.id, lang);
                            });
                        }
                    }
                    if text != before {
                        self.fields_mut().insert(spec.id, text);
                    }
                    ui.end_row();
                }
            });
        self.focus = next_focus;
    }

    fn field_error(ui: &mut egui::Ui, errors: &[InputError], id: &str, lang: &str) {
        // an empty required field is not an error to shout about while the form is being filled
        if let Some(e) = errors
            .iter()
            .find(|e| e.input == Some(id) && e.kind != ErrorKind::Missing)
        {
            let p = Palette::of(ui);
            ui.colored_label(p.bad().strong, localise(&e.message(lang)));
        }
    }

    fn outcome_ui(ui: &mut egui::Ui, c: &Calculator, out: &Outcome, lang: &str, p: &Palette) {
        cards::subheading(ui, t!("calc-result"));
        egui::Grid::new(("echo_calc_values", c.id))
            .num_columns(2)
            .spacing([12.0, 2.0])
            .show(ui, |ui| {
                for q in &out.values {
                    ui.label(q.label.get(lang));
                    ui.label(RichText::new(localise(&q.display())).strong());
                    ui.end_row();
                }
            });
        ui.add_space(6.0);
        cards::subheading(ui, t!("calc-formula"));
        for s in &out.steps {
            ui.label(
                RichText::new(localise(&format!(
                    "{}  {}  {}",
                    s.formula, s.substituted, s.result
                )))
                .monospace(),
            );
        }
        ui.add_space(6.0);
        // a guideline algorithm lists the questions it asked and the result it reached
        let tree = heartsim_calc::tree::for_calculator(c.id).is_some();
        cards::subheading(
            ui,
            if tree {
                t!("calc-path")
            } else {
                t!("calc-findings")
            },
        );
        if tree {
            heartsim_ui::helper_text(ui, t!("calc-path-hint"));
        }
        if out.findings.is_empty() {
            heartsim_ui::helper_text(ui, t!("calc-no-findings"));
        }
        for f in &out.findings {
            ui.horizontal_wrapped(|ui| {
                let colours = match f.severity {
                    Severity::Normal => Some(p.good()),
                    Severity::Mild | Severity::Moderate | Severity::Abnormal => Some(p.warn()),
                    Severity::Severe => Some(p.bad()),
                    Severity::Info | Severity::Indeterminate => None,
                };
                match colours {
                    Some(c) => cards::pill_coloured(ui, severity_label(f.severity), c.soft, c.on_soft, None),
                    None => cards::pill(ui, severity_label(f.severity)),
                };
                ui.label(localise(f.text.get(lang)));
            });
            heartsim_ui::helper_text(
                ui,
                localise(&format!(
                    "{} — {}, {}",
                    f.criterion, f.source.reference.journal, f.source.locator
                )),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_commas_only_between_digits() {
        i18n::set_language("it");
        assert_eq!(localise("AVA = 0.79 cm². Fine."), "AVA = 0,79 cm². Fine.");
        i18n::set_language("en");
        assert_eq!(localise("0.79"), "0.79");
    }
}
