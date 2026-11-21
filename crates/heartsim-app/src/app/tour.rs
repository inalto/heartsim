//! Screenshot tour for the documentation: with `HEARTSIM_SCREENSHOT_DIR=<dir>` the app walks
//! through a fixed list of tabs and scenarios, runs each for a few simulated seconds, pauses with
//! the strip cursor shortly after a QRS onset (so the scrubbed heart shows the activation wave),
//! saves a PNG of the window and quits. Runs are 12 s or more: the strip can only be scrubbed
//! once it holds more than one screen (10 s). Used headless with xvfb + lavapipe (see `docs/images/`).
//! `HEARTSIM_SCREENSHOT_THEME=light|dark` takes the shots in that theme instead of the stored setting.
//! `HEARTSIM_SCREENSHOT_OPEN=adjust,display,changed` (any of them, comma-separated) opens the rhythm bar's
//! Adjust row, opens the strip's Display popup, and moves the sinus rate and QRS width off the rhythm's own
//! values (so the Adjust toggle shows its count); without it every shot shows the default, collapsed state.
//! `sounds` in the same list switches on the heart-sound lane under the strip (B24).
//! `HEARTSIM_SCREENSHOT_ONLY=simulator-sinus.png,…` takes only the named shots.

use super::{HeartSimApp, Tab, TwelveLeadLayout};
use eframe::egui;
use heartsim_ecg::{ChestPiece, ListeningChoice, ListeningSite};
use heartsim_sim::Engine as _;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// What else to set up before a shot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Extra {
    None,
    /// Ask a rhythm-quiz question and answer it wrongly, so the explanation shows.
    RhythmQuiz,
    /// Draw an axis-quiz target.
    AxisQuiz,
    /// Ask an interval-drill question.
    IntervalDrill,
}

/// One screenshot.
#[derive(Debug, Clone, Copy)]
struct Step {
    file: &'static str,
    tab: Tab,
    scenario: &'static str,
    /// Simulated seconds to run before pausing.
    run_s: f64,
    /// Where the strip cursor goes: ms after the last ventricular activation.
    after_qrs_ms: f64,
    vector: bool,
    layout: TwelveLeadLayout,
    /// Dial this frontal QRS axis (the axis trainer's shot).
    axis_deg: Option<f64>,
    extra: Extra,
    /// Listen at this site with this chest piece (B27 part B); `None` is the overview.
    listening: Option<ListeningChoice>,
}

const DEFAULT: Step = Step {
    file: "",
    tab: Tab::Simulator,
    scenario: "sinus_normal",
    run_s: 12.0,
    after_qrs_ms: 45.0,
    vector: false,
    layout: TwelveLeadLayout::Stacked,
    axis_deg: None,
    extra: Extra::None,
    listening: None,
};

const STEPS: [Step; 21] = [
    Step {
        file: "simulator-sinus.png",
        ..DEFAULT
    },
    Step {
        file: "simulator-lbbb-vector.png",
        scenario: "lbbb",
        after_qrs_ms: 70.0,
        vector: true,
        ..DEFAULT
    },
    Step {
        file: "heart-vector-lbbb.png",
        tab: Tab::HeartVector,
        scenario: "lbbb",
        after_qrs_ms: 70.0,
        vector: true,
        ..DEFAULT
    },
    Step {
        file: "simulator-wpw.png",
        scenario: "wpw_left_lateral",
        after_qrs_ms: 30.0,
        ..DEFAULT
    },
    Step {
        file: "simulator-flutter.png",
        scenario: "flutter_2to1",
        after_qrs_ms: 40.0,
        ..DEFAULT
    },
    Step {
        file: "simulator-vf.png",
        scenario: "vf_coarse",
        after_qrs_ms: 0.0,
        ..DEFAULT
    },
    Step {
        file: "twelve-lead-mobitz1.png",
        tab: Tab::TwelveLead,
        scenario: "avb_2nd_mobitz1",
        run_s: 13.0,
        after_qrs_ms: 300.0,
        ..DEFAULT
    },
    Step {
        file: "twelve-lead-clinical.png",
        tab: Tab::TwelveLead,
        run_s: 13.0,
        after_qrs_ms: 300.0,
        layout: TwelveLeadLayout::Clinical,
        ..DEFAULT
    },
    Step {
        file: "twelve-lead-stemi.png",
        tab: Tab::TwelveLead,
        scenario: "stemi_anterior",
        run_s: 13.0,
        after_qrs_ms: 300.0,
        layout: TwelveLeadLayout::TwoColumn,
        ..DEFAULT
    },
    Step {
        file: "twelve-lead-torsades.png",
        tab: Tab::TwelveLead,
        scenario: "torsades",
        run_s: 13.0,
        after_qrs_ms: 0.0,
        ..DEFAULT
    },
    Step {
        file: "axis-trainer.png",
        tab: Tab::AxisTrainer,
        after_qrs_ms: 300.0,
        axis_deg: Some(-50.0),
        ..DEFAULT
    },
    Step {
        file: "axis-quiz.png",
        tab: Tab::Quiz,
        after_qrs_ms: 300.0,
        extra: Extra::AxisQuiz,
        ..DEFAULT
    },
    Step {
        file: "electrodes.png",
        tab: Tab::Electrodes,
        after_qrs_ms: 300.0,
        ..DEFAULT
    },
    Step {
        file: "rhythm-quiz.png",
        tab: Tab::RhythmQuiz,
        run_s: 1.0,
        after_qrs_ms: 0.0,
        extra: Extra::RhythmQuiz,
        ..DEFAULT
    },
    Step {
        file: "interval-drill.png",
        tab: Tab::IntervalDrill,
        run_s: 3.0,
        after_qrs_ms: 0.0,
        extra: Extra::IntervalDrill,
        ..DEFAULT
    },
    Step {
        file: "tutorials.png",
        tab: Tab::Tutorials,
        run_s: 1.0,
        after_qrs_ms: 0.0,
        ..DEFAULT
    },
    Step {
        file: "ep-lab-avrt.png",
        tab: Tab::EpLab,
        scenario: EP_TOUR_PROCEDURE,
        run_s: 7.0,
        after_qrs_ms: 0.0,
        ..DEFAULT
    },
    // brief B27: a systolic and a diastolic murmur in the heart-sound lane (`sounds` in
    // HEARTSIM_SCREENSHOT_OPEN shows it)
    Step {
        file: "simulator-aortic-stenosis.png",
        scenario: "aortic_stenosis_severe",
        after_qrs_ms: 60.0,
        ..DEFAULT
    },
    Step {
        file: "simulator-mitral-stenosis.png",
        scenario: "mitral_stenosis_sinus",
        after_qrs_ms: 60.0,
        ..DEFAULT
    },
    // brief B27 part B: a site and a chest piece named in the lane's title (with `display` in
    // HEARTSIM_SCREENSHOT_OPEN, the site control and the chest markers), the mitral ring flashing
    // with M1 and the stenotic aortic valve glowing through its murmur
    Step {
        file: "simulator-mitral-stenosis-apex-bell.png",
        scenario: "mitral_stenosis_sinus",
        after_qrs_ms: 60.0,
        listening: Some(ListeningChoice {
            site: ListeningSite::Mitral,
            chest: ChestPiece::Bell,
        }),
        ..DEFAULT
    },
    Step {
        file: "simulator-aortic-stenosis-aortic-area.png",
        scenario: "aortic_stenosis_severe",
        after_qrs_ms: 220.0,
        listening: Some(ListeningChoice {
            site: ListeningSite::Aortic,
            chest: ChestPiece::Diaphragm,
        }),
        ..DEFAULT
    },
];

/// The tour's EP-lab shot: this procedure, with the tachycardia induced by a premature stimulus.
const EP_TOUR_PROCEDURE: &str = "ep_ap_left_lateral";

#[derive(Debug)]
enum Phase {
    Setup,
    Running,
    /// Frames left to render before the capture.
    Settle(u32),
    Capturing(Arc<Mutex<Option<anyhow::Result<()>>>>),
}

/// Tour progress.
#[derive(Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent switches of HEARTSIM_SCREENSHOT_OPEN"
)]
pub(super) struct Tour {
    dir: PathBuf,
    step: usize,
    phase: Phase,
    /// The theme the shots are taken in (`HEARTSIM_SCREENSHOT_THEME`), or the stored setting.
    theme: Option<egui::Theme>,
    /// `HEARTSIM_SCREENSHOT_OPEN=adjust`: the Adjust row is open.
    open_adjust: bool,
    /// `HEARTSIM_SCREENSHOT_OPEN=display`: the strip's Display popup is open.
    open_display: bool,
    /// `HEARTSIM_SCREENSHOT_OPEN=changed`: two Adjust values are away from the rhythm's own.
    changed: bool,
    /// `HEARTSIM_SCREENSHOT_OPEN=sounds`: the heart-sound lane is on.
    sounds: bool,
    /// `HEARTSIM_SCREENSHOT_ONLY`: the file names to take; empty takes every shot.
    only: Vec<String>,
}

impl Tour {
    /// A tour if `HEARTSIM_SCREENSHOT_DIR` is set.
    pub(super) fn from_env() -> Option<Self> {
        let dir = PathBuf::from(std::env::var_os("HEARTSIM_SCREENSHOT_DIR")?);
        let theme = match std::env::var("HEARTSIM_SCREENSHOT_THEME").ok().as_deref() {
            Some("light") => Some(egui::Theme::Light),
            Some("dark") => Some(egui::Theme::Dark),
            Some(other) => {
                tracing::warn!(
                    "HEARTSIM_SCREENSHOT_THEME={other}: expected light or dark; using the stored theme"
                );
                None
            }
            None => None,
        };
        let open = std::env::var("HEARTSIM_SCREENSHOT_OPEN").unwrap_or_default();
        let wants = |name: &str| open.split(',').any(|w| w.trim() == name);
        Some(Self {
            dir,
            step: 0,
            phase: Phase::Setup,
            theme,
            open_adjust: wants("adjust"),
            open_display: wants("display"),
            changed: wants("changed"),
            sounds: wants("sounds"),
            only: std::env::var("HEARTSIM_SCREENSHOT_ONLY")
                .unwrap_or_default()
                .split(',')
                .map(|f| f.trim().to_owned())
                .filter(|f| !f.is_empty())
                .collect(),
        })
    }
}

fn save_png(img: &egui::ColorImage, path: &std::path::Path) -> anyhow::Result<()> {
    let [w, h] = img.size;
    let bytes: Vec<u8> = img
        .pixels
        .iter()
        .flat_map(egui::Color32::to_srgba_unmultiplied)
        .collect();
    let rgba = image::RgbaImage::from_raw(w as u32, h as u32, bytes)
        .ok_or_else(|| anyhow::anyhow!("screenshot buffer size"))?;
    rgba.save(path)?;
    Ok(())
}

impl HeartSimApp {
    /// Advance the screenshot tour by one frame (no-op without one).
    pub(super) fn drive_tour(&mut self, ctx: &egui::Context) {
        let Some(mut tour) = self.tour.take() else {
            return;
        };
        while STEPS
            .get(tour.step)
            .is_some_and(|s| !tour.only.is_empty() && !tour.only.iter().any(|f| f == s.file))
        {
            tour.step += 1;
        }
        let Some(step) = STEPS.get(tour.step).copied() else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        };
        match &tour.phase {
            Phase::Setup => {
                if let Some(theme) = tour.theme {
                    ctx.set_theme(theme);
                }
                // the X pointer rests mid-window under xvfb: keep its hover read-out out of the shots
                ctx.all_styles_mut(|s| s.interaction.tooltip_delay = f32::INFINITY);
                if step.tab == Tab::EpLab {
                    self.start_ep_tour(step.scenario);
                } else if let Some(i) = self.library.index_of(step.scenario) {
                    self.load_scenario(i);
                }
                self.tab = step.tab;
                self.settings.adjust_open = tour.open_adjust;
                if tour.changed {
                    self.engine.params.sinus.rate_bpm = 55.0;
                    self.synth.qrs_width_scale = 1.4;
                }
                if tour.open_display {
                    self.ecg.request_display_popup();
                }
                if tour.sounds {
                    self.ecg.show_heart_sounds = true;
                }
                self.ecg.listening = step.listening.unwrap_or_default();
                self.vector.arrow = step.vector;
                self.vector.show_loop = step.vector;
                // the EP lab makes the heart see-through itself, for its catheters
                if step.tab != Tab::EpLab
                    && let Some(h) = &mut self.heart
                {
                    let mut alphas: Vec<&mut f32> = h.panel.layers.iter_mut().map(|l| &mut l.alpha).collect();
                    // the app opens with the see-through heart, so every shot shows that default
                    self.vector.set_see_through(true, &mut alphas);
                }
                self.layout = step.layout;
                if let Some(deg) = step.axis_deg {
                    self.set_qrs_axis(deg);
                }
                match step.extra {
                    Extra::None => {}
                    Extra::RhythmQuiz => {
                        self.rhythm_quiz.next_question(&self.library);
                        if let Some(q) = &self.rhythm_quiz.question {
                            let wrong = q
                                .options
                                .iter()
                                .copied()
                                .find(|&i| i != q.answer)
                                .unwrap_or(q.answer);
                            self.rhythm_quiz.answer(wrong);
                        }
                    }
                    Extra::AxisQuiz => {
                        let target = self.quiz.next_question();
                        self.set_qrs_axis(target);
                    }
                    Extra::IntervalDrill => self.interval_drill.next_question(&self.library),
                }
                tour.phase = Phase::Running;
            }
            Phase::Running => {
                if self.engine.now().0 >= step.run_s * 1000.0 {
                    self.ecg.toggle_pause();
                    let back = self.engine.now().0 - (self.last_v_activation.0 + step.after_qrs_ms);
                    if step.after_qrs_ms > 0.0 && back > 0.0 {
                        self.ecg.playback.set_scrub_ms(back, back);
                    }
                    tour.phase = Phase::Settle(8);
                }
            }
            Phase::Settle(n) => {
                if tour.open_display {
                    self.ecg.request_display_popup();
                }
                tour.phase = if *n == 0 {
                    let slot = Arc::new(Mutex::new(None));
                    let out = Arc::clone(&slot);
                    let path = tour.dir.join(step.file);
                    ctx.request_screenshot(move |img| {
                        let r = save_png(&img, &path);
                        if let Ok(mut s) = out.lock() {
                            *s = Some(r);
                        }
                    });
                    Phase::Capturing(slot)
                } else {
                    Phase::Settle(n - 1)
                };
            }
            Phase::Capturing(slot) => {
                let done = slot.lock().ok().and_then(|mut s| s.take());
                if let Some(r) = done {
                    match r {
                        Ok(()) => tracing::info!("saved {}", tour.dir.join(step.file).display()),
                        Err(e) => tracing::error!("{}: {e:#}", step.file),
                    }
                    tour.step += 1;
                    tour.phase = Phase::Setup;
                }
            }
        }
        self.tour = Some(tour);
    }
}
