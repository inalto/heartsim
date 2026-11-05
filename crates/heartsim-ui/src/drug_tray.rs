//! Drug tray (brief B14): one row per entry of the drug table with dose buttons, a stop button
//! and a timer showing the current effect.
//!
//! The tray never changes the simulation itself: it returns `Action::Drug`s, which the app
//! hands to the timing engine (as a timeline or tutorial would), and it reads back what the
//! engine's pharmacology reports ([`ActiveDrug`]), so the timer always shows the engine's state.

use crate::t;
use egui::{Color32, ProgressBar, RichText, Ui};
use heartsim_core::Action;
use heartsim_sim::pharma::{ActiveDrug, DrugTable};

/// `m:ss` of a duration in ms.
#[must_use]
pub fn format_elapsed(ms: f64) -> String {
    let s = (ms.max(0.0) / 1000.0).round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The drug tray. `active` is `Pharmacology::active(now)`; returns the actions to apply now.
pub fn drug_tray(ui: &mut Ui, table: &DrugTable, active: &[ActiveDrug]) -> Vec<Action> {
    let mut actions = Vec::new();
    let give = |name: &str, dose: f64| Action::Drug {
        name: name.to_string(),
        dose,
    };
    egui::Grid::new("drug_tray")
        .num_columns(3)
        .striped(true)
        .show(ui, |ui| {
            for d in &table.drugs {
                let now = active.iter().find(|a| a.name == d.name);
                let title = if now.is_some() {
                    RichText::new(&d.title).strong()
                } else {
                    RichText::new(&d.title)
                };
                ui.label(title).on_hover_ui(|ui| {
                    ui.strong(&d.title);
                    ui.label(&d.class);
                    ui.label(t!("drug-dose-one", dose = d.standard_dose.as_str()));
                    if !d.notes.is_empty() {
                        ui.separator();
                        ui.label(&d.notes);
                    }
                });
                ui.horizontal(|ui| {
                    for &dose in &d.doses {
                        if ui
                            .small_button(format!("×{dose}"))
                            .on_hover_text(format!("{dose} × {}", d.standard_dose))
                            .clicked()
                        {
                            actions.push(give(&d.name, dose));
                        }
                    }
                    if now.is_some() && ui.small_button(t!("drug-stop")).clicked() {
                        actions.push(give(&d.name, 0.0));
                    }
                });
                match now {
                    Some(a) => {
                        let bar = ProgressBar::new(a.fraction.clamp(0.0, 1.0) as f32)
                            .desired_width(120.0)
                            .fill(Color32::from_rgb(70, 130, 200))
                            .text(format!(
                                "{} · {:.0} %",
                                format_elapsed(a.since_ms),
                                100.0 * a.fraction
                            ));
                        ui.add(bar).on_hover_text(t!(
                            "drug-dose-given",
                            dose = crate::i18n::number(a.dose, 2),
                            ago = format_elapsed(a.since_ms),
                            effect = format!("{:.0}", 100.0 * a.fraction)
                        ));
                    }
                    None => {
                        ui.weak("—");
                    }
                }
                ui.end_row();
            }
        });
    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_is_minutes_and_seconds() {
        assert_eq!(format_elapsed(0.0), "0:00");
        assert_eq!(format_elapsed(65_400.0), "1:05");
        assert_eq!(format_elapsed(-5.0), "0:00");
    }
}
