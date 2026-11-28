//! The rhythm library (`assets/scenarios/*.ron`, brief B11) in the app: loading, the picker, and
//! the timeline of the running scenario.
//!
//! **One source of truth.** Loading a scenario replaces the timing engine and the synthesiser and
//! re-times the 3-D engine from the same `TimingParams`; the 3-D engine keeps following the timing
//! engine's events, so the strip and the heart show the scenario's beats and nothing else. Timeline
//! actions go to the timing engine only, for the same reason.

use crate::assets::AssetBundle;
use eframe::egui;
use heartsim_core::{Millis, TimedAction};
use heartsim_ecg::Tier0Scenario;
use heartsim_ui::t;
#[cfg(feature = "native")]
use std::path::{Path, PathBuf};

/// Where the scenario library is looked for: the first assets root (see
/// [`crate::asset_dirs::asset_roots`]) with a `scenarios` directory.
#[cfg(feature = "native")]
#[must_use]
pub fn default_scenario_dir() -> Option<PathBuf> {
    crate::asset_dirs::find_asset_subdir("scenarios")
}

/// All loadable rhythm presets, sorted by title. Files that fail to parse are skipped (and
/// logged) so one bad file does not hide the rest.
#[derive(Debug, Clone, Default)]
pub struct ScenarioLibrary {
    pub entries: Vec<Tier0Scenario>,
    /// Ids of the entries that came from an opened case file (B41) for this session: listed under
    /// their own heading in the picker and never drawn by a quiz or drill.
    from_cases: std::collections::BTreeSet<String>,
}

impl ScenarioLibrary {
    /// Load every `*.ron` in `dir`.
    ///
    /// # Errors
    /// When the directory cannot be read.
    #[cfg(feature = "native")]
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
        Ok(Self {
            entries,
            from_cases: std::collections::BTreeSet::new(),
        })
    }

    /// Every `scenarios/*.ron` of a downloaded bundle; same rules as [`Self::load_dir`].
    #[must_use]
    pub fn from_bundle(bundle: &AssetBundle) -> Self {
        let mut entries: Vec<Tier0Scenario> = bundle
            .files_in("scenarios", ".ron")
            .filter_map(|p| {
                Tier0Scenario::from_ron(&bundle.text(p)?)
                    .inspect_err(|e| tracing::warn!("{p}: {e}"))
                    .ok()
            })
            .collect();
        entries.sort_by(|a, b| a.scenario.title.cmp(&b.scenario.title));
        Self {
            entries,
            from_cases: std::collections::BTreeSet::new(),
        }
    }

    /// The default library, or an empty one when none is installed (always empty without the
    /// `native` feature: the web build uses [`Self::from_bundle`]).
    #[must_use]
    pub fn load_default() -> Self {
        #[cfg(feature = "native")]
        {
            default_scenario_dir()
                .and_then(|d| {
                    Self::load_dir(&d)
                        .inspect_err(|e| tracing::warn!("{}: {e}", d.display()))
                        .ok()
                })
                .unwrap_or_default()
        }
        #[cfg(not(feature = "native"))]
        {
            Self::default()
        }
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

    /// Whether entry `i` came from an opened case file ([`Self::add_from_case`]).
    #[must_use]
    pub fn is_from_case(&self, i: usize) -> bool {
        self.entries
            .get(i)
            .is_some_and(|e| self.from_cases.contains(&e.scenario.id))
    }

    /// Whether `id` is a rhythm of the build's own library (not one a case file brought): a case
    /// file may load it but never replace it.
    #[must_use]
    pub fn is_builtin(&self, id: &str) -> bool {
        !self.from_cases.contains(id) && self.index_of(id).is_some()
    }

    /// Add a rhythm bundled in a case file for the session (B41), or replace the one an earlier
    /// case file brought under the same id; indices of the other entries do not move. Returns its
    /// index.
    ///
    /// # Errors
    /// When the id is a built-in rhythm's (the case-file reader refuses such files first).
    pub fn add_from_case(&mut self, s: Tier0Scenario) -> Result<usize, String> {
        let id = s.scenario.id.clone();
        if self.is_builtin(&id) {
            return Err(format!("`{id}` is a built-in rhythm"));
        }
        if let Some(i) = self.index_of(&id) {
            self.entries[i] = s;
            Ok(i)
        } else {
            self.entries.push(s);
            self.from_cases.insert(id);
            Ok(self.entries.len() - 1)
        }
    }

    /// Whether entry `i` may be drawn by the ECG quizzes and drills (the Rhythm Quiz and the
    /// Interval Drill): every scenario except the auscultation cases of brief B27
    /// ([`heartsim_core::Scenario::is_auscultation_case`]), whose ordinary sinus strip would be a
    /// wrong question, and the rhythms of opened case files (B41), which nobody at heartsim
    /// reviewed. The one rule every pool uses.
    #[must_use]
    pub fn in_ecg_quiz_pool(&self, i: usize) -> bool {
        self.entries
            .get(i)
            .is_some_and(|e| !e.scenario.is_auscultation_case())
            && !self.is_from_case(i)
    }

    /// The indices the ECG quizzes and drills draw from, in library order.
    #[must_use]
    pub fn ecg_quiz_pool(&self) -> Vec<usize> {
        (0..self.entries.len())
            .filter(|&i| self.in_ecg_quiz_pool(i))
            .collect()
    }

    /// The auscultation cases ([`heartsim_core::Scenario::is_auscultation_case`]), the only
    /// scenarios the auscultation quiz asks and offers (B28): the complement of
    /// [`Self::ecg_quiz_pool`].
    #[must_use]
    pub fn auscultation_pool(&self) -> Vec<usize> {
        (0..self.entries.len())
            .filter(|&i| self.entries[i].scenario.is_auscultation_case() && !self.is_from_case(i))
            .collect()
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

/// The picker's groups in display order: the rhythms (no heading), then the auscultation cases
/// under their own heading when there are any, then the rhythms opened case files brought ("From
/// cases", B41). Each group keeps the library's order.
#[must_use]
pub fn picker_groups(library: &ScenarioLibrary) -> Vec<(Option<String>, Vec<usize>)> {
    let (from_cases, own): (Vec<usize>, Vec<usize>) =
        (0..library.len()).partition(|&i| library.is_from_case(i));
    let (rhythms, sounds): (Vec<usize>, Vec<usize>) =
        own.into_iter().partition(|&i| library.in_ecg_quiz_pool(i));
    let mut out = vec![(None, rhythms)];
    if !sounds.is_empty() {
        out.push((Some(t!("scenario-group-sounds")), sounds));
    }
    if !from_cases.is_empty() {
        out.push((Some(t!("scenario-group-cases")), from_cases));
    }
    out
}

/// Width of the rhythm combo box, points: fits the longest preset name; a longer one is cut.
pub const PICKER_WIDTH_PX: f32 = 230.0;

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
    // a fixed width whatever the rhythm's name: a long name is cut with an ellipsis (and shown whole
    // on hover), so the bar around the picker does not reflow when the rhythm changes
    let combo = ui
        .allocate_ui_with_layout(
            egui::vec2(PICKER_WIDTH_PX, ui.spacing().interact_size.y),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_max_width(PICKER_WIDTH_PX);
                egui::ComboBox::from_id_salt("scenario_picker")
                    .selected_text(title.as_str())
                    .width(PICKER_WIDTH_PX)
                    .truncate()
                    .show_ui(ui, |ui| {
                        for (heading, group) in picker_groups(library) {
                            if let Some(heading) = heading {
                                ui.separator();
                                ui.weak(heading);
                            }
                            for i in group {
                                let s = &library.entries[i];
                                if ui
                                    .selectable_label(current == Some(i), &s.scenario.title)
                                    .on_hover_text(&s.scenario.description)
                                    .clicked()
                                {
                                    picked = Some(i);
                                }
                            }
                        }
                    })
            },
        )
        .inner;
    combo
        .response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, t!("scenario-rhythm")));
    heartsim_ui::highlight::mark(&combo.response, "rhythm_picker");
    combo.response.on_hover_text(title);
    // always drawn (disabled for the custom settings, which have no rhythm to restart or explain),
    // so the bar keeps its size
    let s = current.and_then(|i| library.entries.get(i));
    let r = ui
        .add_enabled(s.is_some(), egui::Button::new(t!("scenario-restart")))
        .on_hover_text(t!("scenario-restart-hint"));
    if r.clicked() {
        picked = current;
    }
    let about = ui.add_enabled(s.is_some(), egui::Button::new(t!("scenario-about")));
    if let Some(s) = s {
        // a real button: hover gives the one-line description, a click opens the explanation
        let about = about.on_hover_ui(|ui| {
            ui.set_max_width(420.0);
            ui.strong(&s.scenario.title);
            ui.label(&s.scenario.description);
        });
        egui::Popup::from_toggle_button_response(&about)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_max_width(460.0);
                ui.strong(&s.scenario.title);
                ui.add(egui::Label::new(&s.scenario.description).wrap());
                if let Some(text) = crate::rhythm_quiz::teaching_text(s.scenario.notes.as_deref()) {
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                        ui.add(egui::Label::new(text).wrap());
                    });
                }
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
    fn auscultation_cases_are_kept_out_of_the_ecg_pool_and_grouped_in_the_picker() {
        let lib = library();
        let pool = lib.ecg_quiz_pool();
        let cases: Vec<&str> = lib
            .entries
            .iter()
            .filter(|e| e.scenario.is_auscultation_case())
            .map(|e| e.scenario.id.as_str())
            .collect();
        assert!(cases.len() >= 15, "{cases:?}");
        for id in [
            "aortic_stenosis_severe",
            "mitral_stenosis_af",
            "heart_failure_s3",
            "innocent_murmur",
        ] {
            let i = lib.index_of(id).unwrap();
            assert!(!lib.in_ecg_quiz_pool(i) && !pool.contains(&i), "{id}");
        }
        // the ECG is the lesson: sounds ride along, the scenario stays in the pool
        for id in ["pericarditis", "stemi_anterior", "sinus_normal"] {
            assert!(pool.contains(&lib.index_of(id).unwrap()), "{id}");
        }
        assert_eq!(pool.len() + cases.len(), lib.len());
        assert!(!lib.in_ecg_quiz_pool(lib.len()), "out of range");
        let groups = picker_groups(&lib);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].1, pool);
        assert!(groups[0].0.is_none() && groups[1].0.is_some());
        assert_eq!(groups[1].1.len(), cases.len());
    }

    #[test]
    fn rhythms_from_case_files_join_the_picker_but_never_a_quiz_pool() {
        let mut lib = library();
        let ecg = lib.ecg_quiz_pool();
        let sounds = lib.auscultation_pool();
        let mut s = lib.entries[lib.index_of("sinus_normal").unwrap()].clone();
        assert!(lib.add_from_case(s.clone()).is_err(), "a built-in id is refused");
        s.scenario.id = "from_a_file".into();
        let i = lib.add_from_case(s.clone()).unwrap();
        assert_eq!(i, lib.len() - 1);
        assert!(lib.is_from_case(i) && !lib.is_builtin("from_a_file"));
        // the same id again replaces it in place
        assert_eq!(lib.add_from_case(s).unwrap(), i);
        assert_eq!(lib.ecg_quiz_pool(), ecg);
        assert_eq!(lib.auscultation_pool(), sounds);
        let groups = picker_groups(&lib);
        assert_eq!(groups.last().unwrap().1, [i]);
        assert!(groups.last().unwrap().0.is_some());
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
