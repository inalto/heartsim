//! B15 acceptance: tutorial navigation through AccessKit nodes (self-study gating, tutor mode,
//! progress), and the scenario timeline editor's save / load equality. No GPU.

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_app::scenarios::ScenarioLibrary;
use heartsim_app::tutorial::{Mode, TutorialHost, TutorialLibrary, UiAction};
use heartsim_app::tutorial_ui::Tutorials;
use heartsim_core::{Action, Millis, StimSite, TimedAction};
use heartsim_ecg::Tier0Scenario;
use heartsim_ui::TimelineEditor;
use std::path::{Path, PathBuf};

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

/// Records what the pages ask for; simulated time stands still, so waits never end.
#[derive(Debug, Default)]
struct Host {
    actions: Vec<Action>,
    ui: Vec<UiAction>,
}

impl TutorialHost for Host {
    fn apply_action(&mut self, action: &Action) {
        self.actions.push(action.clone());
    }
    fn apply_ui(&mut self, action: &UiAction) {
        self.ui.push(action.clone());
    }
    fn sim_now_ms(&self) -> f64 {
        0.0
    }
    fn last_qrs_ms(&self) -> f64 {
        -1.0
    }
}

struct Model {
    tutorials: Tutorials,
    host: Host,
}

fn harness() -> Harness<'static, Model> {
    let library = TutorialLibrary::load_dir(&assets().join("tutorials")).unwrap();
    Harness::builder()
        .with_size(egui::vec2(900.0, 900.0))
        .build_ui_state(
            |ui, m: &mut Model| {
                if m.tutorials.is_open() {
                    m.tutorials.reader_ui(ui);
                } else {
                    m.tutorials.library_ui(ui);
                }
                m.tutorials.poll(&mut m.host);
            },
            Model {
                tutorials: Tutorials::new(library, None),
                host: Host::default(),
            },
        )
}

/// The reader shows a spinner while a page waits for the simulation, so the UI never settles:
/// step a fixed number of frames instead of `run()`.
fn frames(h: &mut Harness<'_, Model>) {
    h.run_steps(4);
}

fn click(h: &mut Harness<'_, Model>, label: &str) {
    h.get_by_label(label).click();
    frames(h);
}

/// Click a button (the page text may use the same word, e.g. "press **Next**").
fn button(h: &mut Harness<'_, Model>, label: &str) {
    h.get_by_role_and_label(Role::Button, label).click();
    frames(h);
}

fn page(h: &Harness<'_, Model>) -> Option<usize> {
    h.state().tutorials.position().map(|(_, p)| p)
}

#[test]
fn navigation_self_study_gating_and_tutor_mode() {
    let mut h = harness();
    frames(&mut h);
    let basics = h.state().tutorials.library.tutorials[0].clone();
    assert_eq!(basics.id, "01_basics");

    click(&mut h, &format!("Start {}", basics.title));
    assert_eq!(page(&h), Some(0));
    assert_eq!(
        h.host_ui().first(),
        Some(&UiAction::LoadScenario("sinus_normal".into())),
        "page actions run on entry"
    );
    let _ = h.get_by_label(&basics.pages[0].title);

    button(&mut h, "Next");
    button(&mut h, "Next");
    assert_eq!(page(&h), Some(2));
    let q = basics.pages[2].check.clone().expect("page 3 has a check");

    // self-study: Next is blocked until the check is answered correctly
    button(&mut h, "Next");
    assert_eq!(page(&h), Some(2));
    let wrong = (0..q.choices.len()).find(|&i| i != q.answer).unwrap();
    click(&mut h, &q.choices[wrong]);
    let _ = h.get_by_label("Not quite. Try again.");
    button(&mut h, "Next");
    assert_eq!(page(&h), Some(2));
    click(&mut h, &q.choices[q.answer]);
    let _ = h.get_by_label("Correct.");
    button(&mut h, "Next");
    assert_eq!(page(&h), Some(3));
    let progress = h.state().tutorials.progress.get("01_basics");
    assert!(progress.passed.contains(&2));
    assert_eq!(progress.furthest, 3);
    assert!(!progress.completed);

    button(&mut h, "Previous");
    assert_eq!(page(&h), Some(2));
    button(&mut h, "Next");

    // page 4 has an unanswered check: tutor mode moves on anyway
    h.state_mut().tutorials.mode = Mode::Tutor;
    button(&mut h, "Next");
    assert_eq!(page(&h), Some(4));
    assert!(
        h.host_ui().contains(&UiAction::LoadScenario("avb_1st".into())),
        "page 5 loads its rhythm"
    );

    button(&mut h, "Close tutorial");
    assert!(!h.state().tutorials.is_open());
    // resume where we left off
    click(&mut h, "Resume at page 5");
    assert_eq!(page(&h), Some(4));
}

/// Every page of every shipped tutorial lays out (all its glyphs exist in the bundled fonts) and
/// tutor mode reaches the end of each.
#[test]
fn every_page_renders_in_tutor_mode() {
    let mut h = harness();
    h.state_mut().tutorials.mode = Mode::Tutor;
    let tutorials = h.state().tutorials.library.tutorials.clone();
    for (i, t) in tutorials.iter().enumerate() {
        h.state_mut().tutorials.open(i, 0);
        frames(&mut h);
        for p in 1..t.pages.len() {
            button(&mut h, "Next");
            assert_eq!(page(&h), Some(p), "{}", t.id);
            let _ = h.get_by_label(&t.pages[p].title);
            if let Some(q) = &t.pages[p].check {
                click(&mut h, &q.choices[q.answer]);
                let _ = h.get_by_label("Correct.");
            }
        }
        button(&mut h, "Finish");
        assert!(!h.state().tutorials.is_open());
    }
}

trait HostUi {
    fn host_ui(&self) -> Vec<UiAction>;
}

impl HostUi for Harness<'_, Model> {
    fn host_ui(&self) -> Vec<UiAction> {
        self.state().host.ui.clone()
    }
}

#[test]
fn every_library_scenario_saves_and_loads_unchanged() {
    let lib = ScenarioLibrary::load_dir(&assets().join("scenarios")).unwrap();
    for s in &lib.entries {
        let text = heartsim_app::scenario_file::to_ron(s).unwrap();
        let back =
            Tier0Scenario::from_ron(&text).unwrap_or_else(|e| panic!("{}: {e}\n{text}", s.scenario.id));
        assert_eq!(back.scenario, s.scenario, "{}", s.scenario.id);
        assert_eq!(back.timing, s.timing, "{}", s.scenario.id);
        assert_eq!(back.morphology, s.morphology, "{}", s.scenario.id);
    }
}

#[test]
fn edited_timeline_save_load_equality() {
    let lib = ScenarioLibrary::load_dir(&assets().join("scenarios")).unwrap();
    let orig = lib.entries[lib.index_of("lafb").unwrap()].clone();
    let mut ed = TimelineEditor::new(&orig.scenario.timeline);
    let base = ed.len();
    ed.add(
        1500.0,
        &Action::Shock {
            joules: 150.0,
            synchronised: true,
        },
    );
    ed.add(500.0, &Action::SetGlobalSinusRate { bpm: 90.0 });
    ed.add(
        800.0,
        &Action::Stimulate {
            site: StimSite::Landmark("rv_apex".into()),
            strength: 2.0,
            duration_ms: 2.0,
        },
    );
    assert_eq!(ed.len(), base + 3);
    // rows are kept in time order: 500, 800, 1500 (plus whatever the scenario had)
    let at_500 = ed
        .timeline()
        .unwrap()
        .iter()
        .position(|a| a.at == Millis(500.0))
        .unwrap();
    assert!(!ed.set_text(at_500, "SetGlobalSinusRate(bpm: "));
    assert!(ed.timeline().is_none(), "a broken row blocks the timeline");
    assert!(ed.set_text(at_500, "Drug(name: \"adenosine\", dose: 2.0)"));
    ed.set_time(at_500, 2500.0);
    let stim = ed
        .timeline()
        .unwrap()
        .iter()
        .position(|a| a.at == Millis(800.0))
        .unwrap();
    ed.remove(stim);
    let edited = ed.timeline().unwrap();
    assert_eq!(edited.len(), base + 2);
    assert!(edited.windows(2).all(|w| w[0].at.0 <= w[1].at.0));
    assert!(edited.contains(&TimedAction {
        at: Millis(2500.0),
        action: Action::Drug {
            name: "adenosine".into(),
            dose: 2.0,
        },
    }));

    let mut saved = orig.clone();
    saved.scenario.timeline.clone_from(&edited);
    let dir = std::env::temp_dir().join(format!("heartsim-b15-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("lafb_edited.ron");
    std::fs::write(&path, heartsim_app::scenario_file::to_ron(&saved).unwrap()).unwrap();
    let back = Tier0Scenario::from_ron(&std::fs::read_to_string(&path).unwrap()).unwrap();
    std::fs::remove_dir_all(&dir).ok();

    assert_eq!(back.scenario.timeline, edited);
    assert_eq!(back.scenario, saved.scenario);
    assert_eq!(back.timing, orig.timing);
    assert_eq!(back.morphology, orig.morphology);
    assert_eq!(
        TimelineEditor::new(&back.scenario.timeline).timeline().unwrap(),
        edited
    );
}
