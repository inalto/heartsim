//! The rhythm library (`assets/scenarios/*.ron`, brief B11) in the app: loading, the picker, and
//! the timeline of the running scenario.
//!
//! **One source of truth.** Loading a scenario replaces the timing engine and the synthesiser and
//! re-times the 3-D engine from the same `TimingParams`; the 3-D engine keeps following the timing
//! engine's events, so the strip and the heart show the scenario's beats and nothing else. Timeline
//! actions go to the timing engine only, for the same reason.

use eframe::egui;
use heartsim_core::{Millis, TimedAction};
use heartsim_ecg::Tier0Scenario;
use heartsim_ui::t;
use std::path::{Path, PathBuf};

/// Where the scenario library is looked for: next to the anatomy packs (`$HEARTSIM_ASSETS`, the
/// installed layout, macOS bundle resources, the working directory, the source tree).
#[must_use]
pub fn default_scenario_dir() -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(dir) = std::env::var_os("HEARTSIM_ASSETS") {
        roots.push(dir.into());
    }
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        roots.push(exe_dir.join("assets"));
        roots.push(exe_dir.join("../Resources/assets"));
    }
    roots.push(PathBuf::from("assets"));
    roots.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets"));
    roots
        .into_iter()
        .map(|r| r.join("scenarios"))
        .find(|d| d.is_dir())
}

/// All loadable rhythm presets, sorted by title. Files that fail to parse are skipped (and
/// logged) so one bad file does not hide the rest.
#[derive(Debug, Clone, Default)]
pub struct ScenarioLibrary {
    pub entries: Vec<Tier0Scenario>,
}

impl ScenarioLibrary {
    /// Load every `*.ron` in `dir`.
    ///
    /// # Errors
    /// When the directory cannot be read.
    pub fn load_dir(dir: &Path) -> std::io::Result<Self> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "ron"))
            .collect();
        files.sort();
        let mut entries: Vec<Tier0Scenario> = files
            .iter()
            .filter_map(|p| {
                let text = std::fs::read_to_string(p)
                    .inspect_err(|e| tracing::warn!("{}: {e}", p.display()))
                    .ok()?;
                Tier0Scenario::from_ron(&text)
                    .inspect_err(|e| tracing::warn!("{}: {e}", p.display()))
                    .ok()
            })
            .collect();
        entries.sort_by(|a, b| a.scenario.title.cmp(&b.scenario.title));
        Ok(Self { entries })
    }

    /// The default library, or an empty one when none is installed.
    #[must_use]
    pub fn load_default() -> Self {
        default_scenario_dir()
            .and_then(|d| {
                Self::load_dir(&d)
                    .inspect_err(|e| tracing::warn!("{}: {e}", d.display()))
                    .ok()
            })
            .unwrap_or_default()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Index of the scenario with this id.
    #[must_use]
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.entries.iter().position(|e| e.scenario.id == id)
    }
}

/// The timeline of the running scenario: actions are handed out once, in time order, when the
/// simulation clock reaches them.
#[derive(Debug, Clone, Default)]
pub struct Timeline {
    actions: Vec<TimedAction>,
    next: usize,
}

impl Timeline {
    #[must_use]
    pub fn new(mut actions: Vec<TimedAction>) -> Self {
        actions.sort_by(|a, b| a.at.0.total_cmp(&b.at.0));
        Self { actions, next: 0 }
    }

    /// Actions due at or before `now` that were not handed out yet (same rule as
    /// `Tier0Scenario::record`, so the app reproduces the golden recordings).
    pub fn due(&mut self, now: Millis) -> impl Iterator<Item = &TimedAction> {
        let start = self.next;
        while self.actions.get(self.next).is_some_and(|a| a.at.0 <= now.0) {
            self.next += 1;
        }
        self.actions[start..self.next].iter()
    }

    /// Whether any action is still pending.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.next >= self.actions.len()
    }
}

/// Scenario combo box. Returns the index the user picked (also when re-picking the current
/// one, which restarts it).
pub fn scenario_picker(
    ui: &mut egui::Ui,
    library: &ScenarioLibrary,
    current: Option<usize>,
) -> Option<usize> {
    let mut picked = None;
    let title = current
        .and_then(|i| library.entries.get(i))
        .map_or_else(|| t!("scenario-custom"), |s| s.scenario.title.clone());
    ui.label(t!("scenario-rhythm"));
    let combo = egui::ComboBox::from_id_salt("scenario_picker")
        .selected_text(title)
        .width(320.0)
        .show_ui(ui, |ui| {
            for (i, s) in library.entries.iter().enumerate() {
                if ui
                    .selectable_label(current == Some(i), &s.scenario.title)
                    .on_hover_text(&s.scenario.description)
                    .clicked()
                {
                    picked = Some(i);
                }
            }
        });
    combo
        .response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, t!("scenario-rhythm")));
    heartsim_ui::highlight::mark(&combo.response, "rhythm_picker");
    if let Some(s) = current.and_then(|i| library.entries.get(i)) {
        let r = ui
            .button(t!("scenario-restart"))
            .on_hover_text(t!("scenario-restart-hint"));
        if r.clicked() {
            picked = current;
        }
        ui.label(t!("scenario-about")).on_hover_ui(|ui| {
            ui.set_max_width(420.0);
            ui.strong(&s.scenario.title);
            ui.label(&s.scenario.description);
        });
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;
    use heartsim_core::{Action, StimSite};

    fn library() -> ScenarioLibrary {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios");
        ScenarioLibrary::load_dir(&dir).expect("scenario dir")
    }

    #[test]
    fn whole_library_loads_sorted_by_title() {
        let lib = library();
        assert!(lib.len() >= 27, "only {} scenarios", lib.len());
        assert!(
            lib.entries
                .windows(2)
                .all(|w| w[0].scenario.title <= w[1].scenario.title)
        );
        assert!(lib.index_of("sinus_normal").is_some());
        assert!(lib.index_of("nope").is_none());
    }

    #[test]
    fn timeline_hands_out_each_action_once_in_order() {
        let stim = |at: f64| TimedAction {
            at: Millis(at),
            action: Action::Stimulate {
                site: StimSite::Landmark("rv_apex".into()),
                strength: 1.0,
                duration_ms: 2.0,
            },
        };
        let mut t = Timeline::new(vec![stim(300.0), stim(100.0), stim(300.0)]);
        assert_eq!(t.due(Millis(50.0)).count(), 0);
        let first: Vec<f64> = t.due(Millis(100.0)).map(|a| a.at.0).collect();
        assert_eq!(first, [100.0]);
        assert_eq!(t.due(Millis(200.0)).count(), 0);
        assert_eq!(t.due(Millis(1000.0)).count(), 2);
        assert!(t.is_finished());
        assert_eq!(t.due(Millis(5000.0)).count(), 0);
    }
}
