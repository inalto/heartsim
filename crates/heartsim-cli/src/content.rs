//! Teaching-content checks (brief B18): `run-tutorial` and `validate-cases`.
//!
//! Both run the page / stage actions of the shipped files headlessly on the same pipeline the
//! app drives for the Tier-0 library: timing engine plus synthesiser, 1 ms fixed step, no
//! randomness. A [`Headless`] host implements [`TutorialHost`], so `PageRun` executes the actions
//! exactly as it does in the application; every scenario action that the engine refuses, every
//! unknown rhythm, highlight id or re-entry preset, and every wait that never ends is an error.
//!
//! Re-entry presets (`StartReentry`) are parsed and their timing section is run on the timing
//! engine; the reaction–diffusion tissue itself is not stepped here (it is not real time and is
//! covered by `heartsim-ecg/tests/reentry.rs`).

use anyhow::{Context, Result, bail};
use heartsim_core::case::Case;
use heartsim_core::tutorial::{PageRun, Step, Tutorial, TutorialHost, UiAction, WaitEvent};
use heartsim_core::{Action, Millis, TimedAction};
use heartsim_ecg::synth::SynthModulation;
use heartsim_ecg::{GaussianSynth, Tier0Scenario};
use heartsim_sim::tier2::Preset;
use heartsim_sim::{Engine, EventKind, TimingEngine, TimingParams};
use std::path::{Path, PathBuf};

/// Simulated time a page's actions may wait for an event before the run is declared stuck, ms.
const MAX_WAIT_MS: f64 = 120_000.0;
/// Simulated time every page and stage is watched after its actions finished, ms.
const DWELL_MS: f64 = 5_000.0;
/// Simulation speed range accepted by `SetSpeed` (the application clamps to it).
const SPEED_RANGE: (f64, f64) = (0.05, 2.0);

/// The repository's `assets/` directory: `--assets`, `$HEARTSIM_ASSETS`, `./assets`, the source
/// tree.
pub(crate) fn assets_root(arg: Option<&Path>) -> Result<PathBuf> {
    let mut roots: Vec<PathBuf> = arg.map(Path::to_path_buf).into_iter().collect();
    roots.extend(std::env::var_os("HEARTSIM_ASSETS").map(PathBuf::from));
    roots.push(PathBuf::from("assets"));
    roots.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets"));
    roots
        .into_iter()
        .find(|r| r.join("scenarios").is_dir())
        .ok_or_else(|| anyhow::anyhow!("no assets directory with a scenarios/ folder found (use --assets)"))
}

/// The model the actions drive: the timing engine and synthesiser of the loaded rhythm.
struct Headless {
    assets: PathBuf,
    engine: TimingEngine,
    synth: GaussianSynth,
    timeline: Vec<TimedAction>,
    next_timeline: usize,
    last_qrs_ms: f64,
    /// Ventricular activations since the counter was last reset (the dwell check).
    activations: usize,
    /// `true` after `StartReentry`: the rhythm is a tissue model this run does not step.
    reentry: bool,
    errors: Vec<String>,
}

impl Headless {
    fn new(assets: &Path) -> Self {
        Self {
            assets: assets.to_path_buf(),
            engine: TimingEngine::new(TimingParams::default()),
            synth: GaussianSynth::textbook(),
            timeline: Vec::new(),
            next_timeline: 0,
            last_qrs_ms: -1000.0,
            activations: 0,
            reentry: false,
            errors: Vec::new(),
        }
    }

    fn load(&mut self, scenario: &Tier0Scenario) {
        match scenario.synth() {
            Ok(s) => self.synth = s,
            Err(e) => self.errors.push(format!("{}: {e}", scenario.scenario.id)),
        }
        self.engine = TimingEngine::new(scenario.timing.clone());
        self.engine.set_shock_seed(scenario.scenario.seed);
        self.timeline.clone_from(&scenario.scenario.timeline);
        self.timeline.sort_by(|a, b| a.at.0.total_cmp(&b.at.0));
        self.next_timeline = 0;
        self.last_qrs_ms = -1000.0;
        self.activations = 0;
        self.reentry = false;
    }

    /// One 1 ms step: due timeline actions, the engine, the synthesiser (as `Tier0Scenario::record`).
    fn step(&mut self) {
        while let Some(a) = self.timeline.get(self.next_timeline)
            && a.at.0 <= self.engine.now().0
        {
            if let Err(e) = self.engine.apply(&a.action, a.at) {
                self.errors
                    .push(format!("timeline action at {:.0} ms: {e}", a.at.0));
            }
            self.next_timeline += 1;
        }
        self.engine.step(Millis(1.0));
        let events = self.engine.drain_events();
        for e in &events {
            if e.region.is_ventricular() && e.kind != EventKind::Blocked {
                self.last_qrs_ms = e.at.0;
                self.activations += 1;
            }
        }
        self.synth.on_events(&events);
        self.synth.on_pacer_markers(&self.engine.drain_pacer_markers());
        self.synth.modulation =
            SynthModulation::from_state(&self.engine.drug_effects(), self.engine.ischaemia());
        let _ = self.synth.sample(self.engine.now());
    }

    /// Run `steps` until they are done (waits advance the simulation), then watch the rhythm for
    /// [`DWELL_MS`]. Returns the problems of this run.
    fn run_steps(&mut self, at: &str, steps: Vec<Step>) -> Vec<String> {
        let mut run = PageRun::from_steps(steps);
        let start = self.engine.now().0;
        while !run.poll(self) {
            if self.engine.now().0 - start > MAX_WAIT_MS {
                self.errors.push("a wait never ended".into());
                break;
            }
            self.step();
        }
        self.activations = 0;
        for _ in 0..DWELL_MS as usize {
            self.step();
        }
        if self.activations == 0 && !self.reentry {
            self.errors.push(format!(
                "no ventricular activation in the {DWELL_MS:.0} ms after the actions"
            ));
        }
        std::mem::take(&mut self.errors)
            .into_iter()
            .map(|e| format!("{at}: {e}"))
            .collect()
    }

    fn scenario(&self, id: &str) -> Result<Tier0Scenario> {
        let path = self.assets.join("scenarios").join(format!("{id}.ron"));
        let text = std::fs::read_to_string(&path).with_context(|| format!("no scenario `{id}`"))?;
        Tier0Scenario::from_ron(&text).map_err(|e| anyhow::anyhow!("{id}: {e}"))
    }
}

impl TutorialHost for Headless {
    fn apply_action(&mut self, action: &Action) {
        let now = self.engine.now();
        if let Err(e) = self.engine.apply(action, now) {
            self.errors.push(format!("{action:?}: {e}"));
        }
    }

    fn apply_ui(&mut self, action: &UiAction) {
        match action {
            UiAction::LoadScenario(id) => match self.scenario(id) {
                Ok(s) => self.load(&s),
                Err(e) => self.errors.push(format!("{e:#}")),
            },
            UiAction::StartReentry(id) => {
                let path = self.assets.join("scenarios/reentry").join(format!("{id}.ron"));
                match std::fs::read_to_string(&path)
                    .map_err(|e| format!("no re-entry preset `{id}`: {e}"))
                    .and_then(|t| Preset::from_ron(&t).map_err(|e| format!("{id}: {e}")))
                {
                    Ok(Some(p)) => {
                        self.engine = TimingEngine::new(p.timing.clone());
                        self.engine.set_shock_seed(p.scenario.seed);
                        // the initiation protocol paces the tissue model, which is not stepped here
                        self.timeline.clone_from(&p.scenario.timeline);
                        self.timeline.sort_by(|a, b| a.at.0.total_cmp(&b.at.0));
                        self.next_timeline = 0;
                        self.last_qrs_ms = -1000.0;
                        self.reentry = true;
                    }
                    Ok(None) => self.errors.push(format!("`{id}` has no reentry section")),
                    Err(e) => self.errors.push(e),
                }
            }
            UiAction::Highlight(tool) => {
                if !heartsim_ui::highlight::TOOLS.contains(&tool.as_str()) {
                    self.errors.push(format!("unknown highlight `{tool}`"));
                }
            }
            UiAction::SetSpeed(s) => {
                if !(SPEED_RANGE.0..=SPEED_RANGE.1).contains(s) {
                    self.errors
                        .push(format!("speed {s} outside {}..{}", SPEED_RANGE.0, SPEED_RANGE.1));
                }
            }
            UiAction::SetQrsAxis(deg) => {
                if (-180.0..=180.0).contains(deg) {
                    self.synth.axis_shift_deg = *deg;
                } else {
                    self.errors.push(format!("axis {deg}° outside -180..180"));
                }
            }
            UiAction::WaitForEvent(WaitEvent::ElapsedMs(ms)) if *ms <= 0.0 => {
                self.errors.push("wait of zero or negative length".into());
            }
            // panel, camera, pause, resume, lead: nothing for the headless model to do
            _ => {}
        }
    }

    fn sim_now_ms(&self) -> f64 {
        self.engine.now().0
    }

    fn last_qrs_ms(&self) -> f64 {
        self.last_qrs_ms
    }
}

/// The `tutorial.ron` files named by `target`: a file, a directory of tutorial folders, or the id
/// of an installed tutorial.
fn tutorial_files(target: &str, assets: &Path) -> Result<Vec<PathBuf>> {
    let p = PathBuf::from(target);
    if p.is_file() {
        return Ok(vec![p]);
    }
    let is_dir = p.is_dir();
    let dir = if is_dir { p } else { assets.join("tutorials") };
    if !is_dir {
        let file = dir.join(target).join("tutorial.ron");
        if file.is_file() {
            return Ok(vec![file]);
        }
        if target != "all" {
            bail!("no tutorial file, directory or id `{target}`");
        }
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path().join("tutorial.ron")))
        .filter(|f| f.is_file())
        .collect();
    files.sort();
    Ok(files)
}

/// `heartsim-cli run-tutorial`: load every named tutorial, check its content rules and run all
/// page actions. Fails if anything does.
pub(crate) fn run_tutorials(target: &str, assets_arg: Option<&Path>) -> Result<()> {
    let assets = assets_root(assets_arg)?;
    let files = tutorial_files(target, &assets)?;
    anyhow::ensure!(!files.is_empty(), "no tutorials found for `{target}`");
    let (mut failed, mut pages) = (0usize, 0usize);
    for file in &files {
        let stem = file
            .parent()
            .and_then(Path::file_name)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut problems = Vec::new();
        let text = std::fs::read_to_string(file).with_context(|| file.display().to_string())?;
        match Tutorial::from_ron(&text) {
            Err(e) => problems.push(e.to_string()),
            Ok(t) => {
                if t.id != stem {
                    problems.push(format!("id `{}` differs from the directory name", t.id));
                }
                problems.extend(t.content_problems());
                let mut host = Headless::new(&assets);
                for (i, p) in t.pages.iter().enumerate() {
                    problems.extend(host.run_steps(&format!("{} page {}", t.id, i + 1), p.actions.clone()));
                }
                pages += t.pages.len();
                println!(
                    "{:24} {:2} pages, {} checks",
                    t.id,
                    t.pages.len(),
                    t.pages.iter().filter(|p| p.check.is_some()).count()
                );
            }
        }
        if !problems.is_empty() {
            failed += 1;
            for p in &problems {
                println!("  FAIL {stem}: {p}");
            }
        }
    }
    println!("{} tutorials, {pages} pages, {failed} failed", files.len());
    if failed > 0 {
        bail!("{failed} of {} tutorials failed", files.len());
    }
    Ok(())
}

/// `heartsim-cli validate-cases`: load every `*.ron` in `dir` (a sibling `scenarios/` folder holds
/// the rhythms), check the editorial rules and run every stage's actions. Fails if any case does.
pub(crate) fn validate_cases(dir: &Path) -> Result<()> {
    let assets = assets_root(dir.parent())?;
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
        .collect();
    files.sort();
    let mut ids = std::collections::BTreeSet::new();
    let (mut failed, mut questions) = (0usize, 0usize);
    for file in &files {
        let stem = file
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let mut problems = Vec::new();
        let text = std::fs::read_to_string(file)?;
        match Case::from_ron(&text) {
            Err(e) => problems.push(e.to_string()),
            Ok(c) => {
                if c.id != stem {
                    problems.push(format!("id `{}` differs from the file name", c.id));
                }
                if !ids.insert(c.id.clone()) {
                    problems.push(format!("duplicate id `{}`", c.id));
                }
                problems.extend(c.content_problems());
                let mut host = Headless::new(&assets);
                for (i, s) in c.stages.iter().enumerate() {
                    problems.extend(host.run_steps(&format!("{} stage {}", c.id, i + 1), s.actions.clone()));
                }
                questions += c.question_count();
                println!(
                    "{:28} {} stages, {} questions",
                    c.id,
                    c.stages.len(),
                    c.question_count()
                );
            }
        }
        if !problems.is_empty() {
            failed += 1;
            for p in &problems {
                println!("  FAIL {stem}: {p}");
            }
        }
    }
    println!("{} cases, {questions} questions, {failed} failed", files.len());
    anyhow::ensure!(!files.is_empty(), "no cases in {}", dir.display());
    if failed > 0 {
        bail!("{failed} of {} cases failed validation", files.len());
    }
    Ok(())
}
