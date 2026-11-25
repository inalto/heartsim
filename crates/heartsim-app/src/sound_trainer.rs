//! The Sound Trainer (brief B28, Practise group): single findings as loops to listen to again and
//! again, with a listen counter, and contrast pairs to switch between at the same point of the cycle.
//!
//! The evidence the brief rests on: repetition is what works (500 listens per murmur took students
//! from about 20 % to 85 %, unsupervised as well as supervised; Barrett et al. 2004), and a trace
//! with a moving cursor adds a modest gain (Watsjold et al. 2021). So each finding is one clip
//! ([`crate::sound_clips`]) that loops through the heart-sound sink, its trace shown or hidden, and
//! every loop heard to its end is counted, per finding and in total, and kept between sessions.
//!
//! Without a sound output the page still teaches from the trace: it cannot be hidden then, and the
//! page says why.

use crate::narrow::{Narrow, Pane, PaneSpec, Side};
use std::collections::BTreeMap;
use std::rc::Rc;

use eframe::egui;
use heartsim_core::BreathPhase;
use heartsim_ecg::{ChestPiece, ClipOptions, ListeningChoice, ListeningSite};
use heartsim_ui::{ClipTrace, StripStyle, cards, clip_view, t};

use crate::heart_sound_player::HeartSoundPlayer;
use crate::reference_clips::{ReferenceClip, ReferenceLibrary};
use crate::scenarios::ScenarioLibrary;
use crate::sound_clips::{ClipCache, ClipEntry, sound_available};

/// eframe storage key of the listen counts.
pub const LISTENS_KEY: &str = "sound_trainer_listens";

/// One finding: the scenario that shows it and where it is best heard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Finding {
    /// Stable id: the counter's key and the message keys `st-<id>` and `st-<id>-listen`.
    pub id: &'static str,
    pub scenario: &'static str,
    pub site: ListeningSite,
    pub chest: ChestPiece,
}

const fn finding(
    id: &'static str,
    scenario: &'static str,
    site: ListeningSite,
    chest: ChestPiece,
) -> Finding {
    Finding {
        id,
        scenario,
        site,
        chest,
    }
}

use ChestPiece::{Bell, Diaphragm};
use ListeningSite::{Aortic, Erb, Mitral, Overview, Pulmonic, Tricuspid};

/// The findings, in teaching order: the two heart sounds, the splits, what the rhythm does to S1,
/// the gallops, clicks and the snap, each murmur of brief B27, the rub. The sites follow the
/// sources' "best heard at" (research note, "Recommended design"); the chest piece is the bell for
/// the low sounds.
pub const FINDINGS: [Finding; 22] = [
    finding("s1_s2", "sinus_normal", Mitral, Diaphragm),
    finding("split_physiological", "sinus_normal", Pulmonic, Diaphragm),
    finding("split_wide", "rbbb", Pulmonic, Diaphragm),
    finding("split_fixed", "atrial_septal_defect", Pulmonic, Diaphragm),
    finding("split_paradoxical", "lbbb", Pulmonic, Diaphragm),
    finding("s1_soft", "avb_1st", Mitral, Diaphragm),
    finding("s1_variable", "avb_3rd_junctional", Mitral, Diaphragm),
    finding("s3", "heart_failure_s3", Mitral, Bell),
    finding("s4", "lvh_s4", Mitral, Bell),
    finding("summation_gallop", "summation_gallop", Mitral, Bell),
    finding("ejection_click", "aortic_stenosis_mild", Aortic, Diaphragm),
    finding("midsystolic_click", "mitral_valve_prolapse", Mitral, Diaphragm),
    finding("opening_snap", "mitral_stenosis_sinus", Mitral, Diaphragm),
    finding("murmur_as", "aortic_stenosis_severe", Aortic, Diaphragm),
    finding("murmur_ar", "aortic_regurgitation", Erb, Diaphragm),
    finding("murmur_mr", "mitral_regurgitation", Mitral, Diaphragm),
    finding("murmur_ms", "mitral_stenosis_sinus", Mitral, Bell),
    finding("murmur_tr", "tricuspid_regurgitation", Tricuspid, Diaphragm),
    finding("murmur_ps", "pulmonary_stenosis", Pulmonic, Diaphragm),
    finding("murmur_vsd", "ventricular_septal_defect", Tricuspid, Diaphragm),
    finding("murmur_innocent", "innocent_murmur", Tricuspid, Diaphragm),
    finding("rub", "pericarditis", Erb, Diaphragm),
];

/// Two or three clips to switch between with one key, heard at the same site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContrastPair {
    /// Stable id: the counter's key and the message key `st-<id>`.
    pub id: &'static str,
    /// The scenarios, in switching order, each with the message key of its short name.
    pub members: &'static [(&'static str, &'static str)],
    pub site: ListeningSite,
    pub chest: ChestPiece,
}

/// The contrast pairs of the brief: the confusions the research names (innocent murmurs over-called,
/// ejection against regurgitant murmurs) and the sounds that crowd S2 and S1.
pub const PAIRS: [ContrastPair; 4] = [
    ContrastPair {
        id: "pair_innocent_as",
        members: &[
            ("innocent_murmur", "st-member-innocent"),
            ("aortic_stenosis_mild", "st-member-as"),
        ],
        site: Overview,
        chest: Diaphragm,
    },
    ContrastPair {
        id: "pair_ejection_regurgitant",
        members: &[
            ("aortic_stenosis_severe", "st-member-ejection"),
            ("mitral_regurgitation", "st-member-regurgitant"),
        ],
        site: Overview,
        chest: Diaphragm,
    },
    ContrastPair {
        id: "pair_split_snap_s3",
        members: &[
            ("rbbb", "st-member-split-s2"),
            ("mitral_stenosis_sinus", "st-member-snap"),
            ("heart_failure_s3", "st-member-s3"),
        ],
        site: Overview,
        chest: Diaphragm,
    },
    ContrastPair {
        id: "pair_s4_split_s1",
        members: &[("lvh_s4", "st-member-s4"), ("rbbb", "st-member-split-s1")],
        site: Overview,
        chest: Diaphragm,
    },
];

/// What the trainer has selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    Finding(usize),
    Pair(usize),
}

/// Something the page asks the app to do.
#[derive(Debug, Clone)]
pub enum TrainerRequest {
    /// Save the clip (as played) as a WAV file.
    SaveWav(Rc<ClipEntry>),
}

/// Trainer state. See the module documentation.
#[derive(Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent switches of the page: half speed, trace, small speakers, playing"
)]
pub struct SoundTrainer {
    pub selection: Selection,
    /// Which member of a contrast pair is selected.
    pub member: usize,
    /// Play the half-speed loop (the slow-motion synthesiser offline: altered, labelled so).
    pub half_speed: bool,
    /// Show the clip's trace while listening (it is always shown without sound output).
    pub show_trace: bool,
    /// Where and how the clip is heard (starts at the finding's own site).
    pub listening: ListeningChoice,
    pub breath_hold: Option<BreathPhase>,
    /// Small-speaker mode (the Settings value, set by the app).
    pub small_speaker: bool,
    /// The real recordings on offer (set by the app; empty when the assets hold none).
    pub references: ReferenceLibrary,
    /// The real recording selected, as an index into [`ReferenceLibrary::for_finding`] of the
    /// selected finding; `None` plays the synthesised finding. Recordings are not counted as
    /// listens: the counter is the evidence-based repetition of the model's finding, and a recording
    /// is a different stimulus (decided conservatively, brief B28 Part C).
    pub real: Option<usize>,
    /// A clip is set to play.
    pub playing: bool,
    counts: BTreeMap<String, u64>,
    /// Loops of the playing clip already counted.
    loops_counted: u64,
    /// The id the playing clip counts for.
    counting: Option<&'static str>,
    cache: ClipCache,
    pub error: Option<String>,
}

impl Default for SoundTrainer {
    fn default() -> Self {
        let mut t = Self {
            selection: Selection::Finding(0),
            member: 0,
            half_speed: false,
            show_trace: true,
            listening: ListeningChoice::default(),
            breath_hold: None,
            small_speaker: false,
            references: ReferenceLibrary::default(),
            real: None,
            playing: false,
            counts: BTreeMap::new(),
            loops_counted: 0,
            counting: None,
            cache: ClipCache::default(),
            error: None,
        };
        t.listening = t.default_listening();
        t
    }
}

impl SoundTrainer {
    /// The id the counter of the selection is kept under.
    #[must_use]
    pub fn selection_id(&self) -> &'static str {
        match self.selection {
            Selection::Finding(i) => FINDINGS[i].id,
            Selection::Pair(i) => PAIRS[i].id,
        }
    }

    /// The scenario that plays now.
    #[must_use]
    pub fn scenario_id(&self) -> &'static str {
        match self.selection {
            Selection::Finding(i) => FINDINGS[i].scenario,
            Selection::Pair(i) => PAIRS[i].members[self.member % PAIRS[i].members.len()].0,
        }
    }

    fn default_listening(&self) -> ListeningChoice {
        let (site, chest) = match self.selection {
            Selection::Finding(i) => (FINDINGS[i].site, FINDINGS[i].chest),
            Selection::Pair(i) => (PAIRS[i].site, PAIRS[i].chest),
        };
        ListeningChoice { site, chest }
    }

    /// The options of the clip that plays now.
    #[must_use]
    pub fn options(&self) -> ClipOptions {
        ClipOptions {
            listening: self.listening,
            breath_hold: self.breath_hold,
            small_speaker: self.small_speaker,
        }
    }

    /// Listens counted for `id` (a finding or a pair).
    #[must_use]
    pub fn count(&self, id: &str) -> u64 {
        self.counts.get(id).copied().unwrap_or(0)
    }

    /// Every listen counted.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// The counts as stored between sessions: `id=n` joined by commas, ids in order.
    #[must_use]
    pub fn counts_string(&self) -> String {
        self.counts
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Restore counts saved with [`Self::counts_string`]; malformed entries are skipped.
    pub fn restore_counts(&mut self, s: &str) {
        for part in s.split(',') {
            if let Some((k, v)) = part.split_once('=')
                && let Ok(n) = v.trim().parse::<u64>()
                && !k.trim().is_empty()
            {
                self.counts.insert(k.trim().to_owned(), n);
            }
        }
    }

    /// Select a finding or a pair: playback stops, the site and chest piece become its own.
    pub fn select(&mut self, selection: Selection, player: &mut HeartSoundPlayer) {
        self.stop(player);
        self.selection = selection;
        self.member = 0;
        self.real = None;
        self.listening = self.default_listening();
        self.error = None;
    }

    /// The real recordings that belong beside the selected finding (none for a contrast pair).
    #[must_use]
    pub fn recordings(&self) -> Vec<&ReferenceClip> {
        match self.selection {
            Selection::Finding(i) => self.references.for_finding(FINDINGS[i].id),
            Selection::Pair(_) => Vec::new(),
        }
    }

    /// The real recording that plays now, if one is selected.
    #[must_use]
    pub fn selected_recording(&self) -> Option<&ReferenceClip> {
        let k = self.real?;
        self.recordings().get(k).copied()
    }

    /// Select the real recording `k` of the finding (or go back to the model with `None`); a
    /// playing clip is replaced by the new one.
    pub fn select_recording(&mut self, k: Option<usize>, player: &mut HeartSoundPlayer) {
        if self.real == k {
            return;
        }
        let was_playing = self.playing;
        self.stop(player);
        self.real = k.filter(|k| *k < self.recordings().len());
        if was_playing || self.real.is_some() {
            self.play_recording(player);
        }
    }

    /// Loop the selected recording from its start. Not counted (see [`Self::real`]).
    fn play_recording(&mut self, player: &mut HeartSoundPlayer) {
        if let Some(c) = self.selected_recording() {
            let samples = Rc::clone(&c.samples);
            player.play_clip(samples, 0, true);
            self.playing = true;
            self.loops_counted = 0;
            self.counting = None;
            self.error = None;
        }
    }

    /// The clip of the current selection and options.
    ///
    /// # Errors
    /// The scenario is missing or cannot be run.
    pub fn entry(&mut self, library: &ScenarioLibrary) -> Result<Rc<ClipEntry>, String> {
        let options = self.options();
        self.cache.get(library, self.scenario_id(), options)
    }

    /// Start looping the current clip from its beginning.
    pub fn play(&mut self, library: &ScenarioLibrary, player: &mut HeartSoundPlayer) {
        if self.selected_recording().is_some() {
            self.play_recording(player);
            return;
        }
        self.play_from(library, player, None);
    }

    /// Start (or restart) the current clip at the trace point `(cycle, fraction)` of the cardiac
    /// cycle, or from the beginning.
    fn play_from(
        &mut self,
        library: &ScenarioLibrary,
        player: &mut HeartSoundPlayer,
        phase: Option<(usize, f64)>,
    ) {
        match self.entry(library) {
            Ok(e) => {
                let start = phase.map_or(0, |(c, f)| {
                    e.playback_index(e.clip.index_at_phase(c, f), self.half_speed)
                });
                player.play_clip(e.samples(self.half_speed), start, true);
                self.playing = true;
                self.loops_counted = 0;
                self.counting = Some(self.selection_id());
                self.error = None;
            }
            Err(err) => {
                self.error = Some(err);
                self.stop(player);
            }
        }
    }

    /// Stop playback. A loop that was not heard to its end is not counted.
    pub fn stop(&mut self, player: &mut HeartSoundPlayer) {
        if self.playing {
            player.stop_clip();
        }
        self.playing = false;
        self.counting = None;
        self.loops_counted = 0;
    }

    /// The point of the cycle being heard now, if a clip plays.
    fn current_phase(
        &mut self,
        library: &ScenarioLibrary,
        player: &HeartSoundPlayer,
    ) -> Option<(usize, f64)> {
        let at = player.clip_playhead()?;
        let e = self.entry(library).ok()?;
        e.clip.cycle_phase(e.trace_index(at, self.half_speed))
    }

    /// The next member of a contrast pair, at the same point of the cycle when playing.
    pub fn switch_member(&mut self, library: &ScenarioLibrary, player: &mut HeartSoundPlayer) {
        let Selection::Pair(i) = self.selection else {
            return;
        };
        let phase = self
            .playing
            .then(|| self.current_phase(library, player))
            .flatten();
        self.member = (self.member + 1) % PAIRS[i].members.len();
        if self.playing {
            self.play_from(library, player, phase);
        }
    }

    /// Re-record after a change of options (site, chest piece, breathing, speed, small speakers)
    /// and, while playing, continue at the same point of the cycle.
    pub fn options_changed(
        &mut self,
        library: &ScenarioLibrary,
        player: &mut HeartSoundPlayer,
        old_options: ClipOptions,
        old_half: bool,
    ) {
        if !self.playing || self.real.is_some() {
            return;
        }
        let id = self.scenario_id();
        let phase = player.clip_playhead().and_then(|at| {
            let e = self.cache.get(library, id, old_options).ok()?;
            e.clip.cycle_phase(e.trace_index(at, old_half))
        });
        self.play_from(library, player, phase);
    }

    /// Once per frame after the player ran: count the loops heard to their end.
    pub fn update(&mut self, player: &HeartSoundPlayer) {
        if !self.playing {
            return;
        }
        if !player.clip_active() {
            // another page or the sound check took the sink
            self.playing = false;
            self.counting = None;
            return;
        }
        let loops = player.clip_loops();
        if loops > self.loops_counted
            && let Some(id) = self.counting
        {
            *self.counts.entry(id.to_owned()).or_default() += loops - self.loops_counted;
            self.loops_counted = loops;
        }
    }

    /// The title of the selection.
    #[must_use]
    pub fn title(&self) -> String {
        heartsim_ui::i18n::tr(&format!("st-{}", self.selection_id()))
    }

    /// The page. Returns a request for the app (saving a WAV file).
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        library: &ScenarioLibrary,
        player: &mut HeartSoundPlayer,
        style: &StripStyle,
    ) -> Option<TrainerRequest> {
        let mut request = None;
        let sound = sound_available(player);
        if !sound {
            self.show_trace = true;
        }
        let (list_name, listen_name) = (t!("st-findings"), t!("pane-listen"));
        let spec = PaneSpec {
            id: "sound_trainer",
            left: Some(Side {
                id: "sound_trainer_list",
                default_px: 300.0,
                cap_px: 100_000.0,
                name: &list_name,
            }),
            right: None,
            centre: &listen_name,
            // on a phone the list and the listening page are two segments; a pick opens the page
            narrow: Narrow::Segmented { start: Pane::Centre },
        };
        crate::narrow::panes(ui, &spec, |ui, pane| match pane {
            Pane::Left => {
                egui::ScrollArea::vertical().show(ui, |ui| self.list_ui(ui, player));
            }
            Pane::Centre => request = self.main_ui(ui, library, player, style, sound),
            Pane::Right => {}
        });
        // one key switches the pair (when no text box has the keyboard)
        if matches!(self.selection, Selection::Pair(_))
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.ctx().input(|i| i.key_pressed(egui::Key::S))
        {
            self.switch_member(library, player);
        }
        request
    }

    fn list_ui(&mut self, ui: &mut egui::Ui, player: &mut HeartSoundPlayer) {
        cards::title_band(ui, t!("st-findings"));
        heartsim_ui::helper_text(ui, t!("st-count-legend"));
        let mut pick = None;
        for (i, f) in FINDINGS.iter().enumerate() {
            let sel = Selection::Finding(i);
            let label = format!(
                "{}  ({})",
                heartsim_ui::i18n::tr(&format!("st-{}", f.id)),
                self.count(f.id)
            );
            let count_hint = t!("st-count-hint", count = self.count(f.id).to_string());
            if ui
                .selectable_label(self.selection == sel, label)
                .on_hover_text(count_hint)
                .clicked()
            {
                pick = Some(sel);
            }
        }
        ui.add_space(6.0);
        cards::title_band(ui, t!("st-pairs"));
        for (i, p) in PAIRS.iter().enumerate() {
            let sel = Selection::Pair(i);
            let label = format!(
                "{}  ({})",
                heartsim_ui::i18n::tr(&format!("st-{}", p.id)),
                self.count(p.id)
            );
            let count_hint = t!("st-count-hint", count = self.count(p.id).to_string());
            if ui
                .selectable_label(self.selection == sel, label)
                .on_hover_text(count_hint)
                .clicked()
            {
                pick = Some(sel);
            }
        }
        if let Some(sel) = pick
            && sel != self.selection
        {
            self.select(sel, player);
            crate::narrow::show_pane(ui.ctx(), "sound_trainer", crate::narrow::Pane::Centre);
        }
    }

    #[allow(clippy::too_many_lines, reason = "one page laid out top to bottom")]
    fn main_ui(
        &mut self,
        ui: &mut egui::Ui,
        library: &ScenarioLibrary,
        player: &mut HeartSoundPlayer,
        style: &StripStyle,
        sound: bool,
    ) -> Option<TrainerRequest> {
        #[cfg_attr(
            not(feature = "export"),
            allow(unused_mut, reason = "set only by the WAV button")
        )]
        let mut request = None;
        ui.heading(self.title());
        let listen_key = match self.selection {
            Selection::Finding(i) => format!("st-{}-listen", FINDINGS[i].id),
            Selection::Pair(i) => format!("st-{}-listen", PAIRS[i].id),
        };
        ui.add(egui::Label::new(heartsim_ui::i18n::tr(&listen_key)).wrap());
        ui.add_space(4.0);
        let old_half = self.half_speed;
        let old_options = self.options();
        ui.horizontal_wrapped(|ui| {
            let label = if self.playing {
                t!("st-stop")
            } else {
                t!("st-play")
            };
            // Play is the one primary action of the page; Stop is a plain button
            let playing = self.playing;
            let play = ui
                .add_enabled_ui(sound, |ui| {
                    if playing {
                        ui.button(label)
                    } else {
                        heartsim_ui::primary_button(ui, label)
                    }
                })
                .inner
                .on_disabled_hover_text(t!("st-no-sound"));
            if play.clicked() {
                if self.playing {
                    self.stop(player);
                } else {
                    self.play(library, player);
                }
            }
            ui.label(t!(
                "st-listens",
                count = self.count(self.selection_id()).to_string(),
                total = self.total().to_string()
            ));
            ui.separator();
            let model = self.real.is_none();
            // widgets, not `add_enabled_ui` scopes: a scope is a child Ui as wide as the rest of
            // the row, which squeezed the second checkbox into a column on a narrow window
            ui.add_enabled(
                model,
                egui::Checkbox::new(&mut self.half_speed, t!("st-half-speed")),
            )
            .on_hover_text(t!("st-half-speed-hint"));
            ui.add_enabled(
                sound && model,
                egui::Checkbox::new(&mut self.show_trace, t!("st-show-trace")),
            )
            .on_disabled_hover_text(t!("st-trace-no-sound"));
        });
        if !sound {
            heartsim_ui::helper_text(ui, t!("st-trace-no-sound"));
        }
        self.recording_row_ui(ui, player, sound);
        if let Selection::Pair(i) = self.selection {
            let mut switch = false;
            ui.horizontal_wrapped(|ui| {
                for (k, (_, key)) in PAIRS[i].members.iter().enumerate() {
                    let r = ui.selectable_label(self.member == k, heartsim_ui::i18n::tr(key));
                    if r.clicked() && self.member != k {
                        // the switch below steps to it, keeping the cycle position
                        self.member = (k + PAIRS[i].members.len() - 1) % PAIRS[i].members.len();
                        switch = true;
                    }
                }
                if ui
                    .button(t!("st-switch"))
                    .on_hover_text(t!("st-switch-hint"))
                    .clicked()
                {
                    switch = true;
                }
            });
            if switch {
                self.switch_member(library, player);
            }
        }
        ui.add_enabled_ui(self.real.is_none(), |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(t!("ecg-listening-site"));
                let combo = egui::ComboBox::from_id_salt("st_site")
                    .selected_text(heartsim_ui::auscultation_chest::site_name(self.listening.site))
                    .show_ui(ui, |ui| {
                        for site in ListeningSite::ALL {
                            ui.selectable_value(
                                &mut self.listening.site,
                                site,
                                heartsim_ui::auscultation_chest::site_name(site),
                            );
                        }
                    });
                let name = t!("ecg-listening-site");
                combo
                    .response
                    .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, &name));
                ui.add_enabled_ui(!self.listening.is_overview(), |ui| {
                    for chest in ChestPiece::ALL {
                        ui.radio_value(
                            &mut self.listening.chest,
                            chest,
                            heartsim_ui::pcg_lane::chest_name(chest),
                        );
                    }
                });
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(t!("ecg-breathing"))
                    .on_hover_text(t!("ecg-breathing-hint"));
                for (value, label) in [
                    (None, t!("ecg-breathing-free")),
                    (
                        Some(BreathPhase::EndExpiration),
                        t!("ecg-breathing-hold-expiration"),
                    ),
                    (
                        Some(BreathPhase::PeakInspiration),
                        t!("ecg-breathing-hold-inspiration"),
                    ),
                ] {
                    ui.radio_value(&mut self.breath_hold, value, label);
                }
            });
        });
        if self.options() != old_options || self.half_speed != old_half {
            self.options_changed(library, player, old_options, old_half);
        }
        if let Some(e) = &self.error {
            cards::result_box(ui, false, e.clone());
        }
        #[cfg(feature = "export")]
        if self.real.is_none()
            && ui
                .button(t!("st-save-wav"))
                .on_hover_text(t!("st-save-wav-hint"))
                .clicked()
            && let Ok(e) = self.entry(library)
        {
            request = Some(TrainerRequest::SaveWav(e));
        }
        heartsim_ui::helper_text(
            ui,
            if self.real.is_some() {
                t!("st-real-note")
            } else {
                t!("st-model-note")
            },
        );
        ui.separator();
        let rect = egui::Rect::from_min_size(ui.cursor().min, ui.available_size());
        if let Some(c) = self.selected_recording() {
            Self::recording_card_ui(ui, c);
        } else if self.show_trace || !sound {
            match self.entry(library) {
                Ok(e) => {
                    let playhead = self
                        .playing
                        .then(|| player.clip_playhead())
                        .flatten()
                        .map(|p| e.trace_index(p, self.half_speed) as u64);
                    let title = heartsim_ui::pcg_lane::lane_title(self.listening);
                    let tag = self.altered_tag();
                    clip_view(
                        ui,
                        rect,
                        &ClipTrace {
                            ecg: &e.ecg_ii,
                            lead_label: "II",
                            pcg: &e.pcg_ring,
                            marks: &e.clip.marks,
                            playhead,
                            title: &title,
                            tag: tag.as_deref(),
                        },
                        style,
                    );
                }
                Err(err) => {
                    cards::result_box(ui, false, err);
                }
            }
        } else {
            ui.label(t!("st-trace-hidden"));
        }
        request
    }

    /// The buttons of the real recordings beside the finding, and why they are off without sound.
    fn recording_row_ui(&mut self, ui: &mut egui::Ui, player: &mut HeartSoundPlayer, sound: bool) {
        let n = self.recordings().len();
        if n == 0 {
            return;
        }
        let mut pick = None;
        ui.horizontal_wrapped(|ui| {
            for k in 0..n {
                let on = self.real == Some(k);
                let label = if n == 1 {
                    t!("st-real")
                } else {
                    format!("{} {}", t!("st-real"), k + 1)
                };
                if ui
                    .add_enabled(sound, egui::Button::new(label).selected(on))
                    .on_hover_text(t!("st-real-hint"))
                    .on_disabled_hover_text(t!("st-real-no-sound"))
                    .clicked()
                {
                    pick = Some(if on { None } else { Some(k) });
                }
            }
            if !sound {
                heartsim_ui::helper_text(ui, t!("st-real-no-sound"));
            }
        });
        if let Some(k) = pick {
            self.select_recording(k, player);
        }
    }

    /// What replaces the trace while a real recording plays: that it is a recording and not the
    /// model, what it contains, and the source, licence and credit the licence requires.
    fn recording_card_ui(ui: &mut egui::Ui, c: &ReferenceClip) {
        let m = &c.meta;
        ui.heading(t!("st-real-heading", title = m.title.clone()));
        ui.add(egui::Label::new(t!("st-real-no-trace")).wrap());
        ui.add_space(4.0);
        ui.add(egui::Label::new(t!("st-real-contains", condition = m.condition.clone())).wrap());
        ui.add_space(4.0);
        ui.label(egui::RichText::new(t!("st-real-credit")).strong());
        ui.add(egui::Label::new(m.attribution.clone()).wrap().selectable(true));
        ui.add(
            egui::Label::new(t!(
                "st-real-licence",
                licence = m.licence.clone(),
                source = m.source.clone()
            ))
            .wrap()
            .selectable(true),
        );
    }

    /// The tag the lane shows when the sound played is altered (half speed, small speakers).
    #[must_use]
    pub fn altered_tag(&self) -> Option<String> {
        match (self.half_speed, self.small_speaker) {
            (false, false) => None,
            (true, false) => Some(t!("ecg-sound-altered-slow")),
            (false, true) => Some(t!("ecg-sound-altered-small")),
            (true, true) => Some(format!(
                "{} · {}",
                t!("ecg-sound-altered-slow"),
                t!("ecg-sound-altered-small")
            )),
        }
    }
}
