//! The Measure section of the Echo page (brief B34, echo simulator phase 5): an M-mode cursor on the
//! sector and the trace it records, annular tissue Doppler at the mitral and tricuspid annuli,
//! calipers, the model's own value to compare a measurement with, and *Send to calculator*.
//!
//! **Everything is the model, and says so.** The M-mode trace is one scan line of the same
//! cross-section as the sector, recorded at a fixed time step (`heartsim_echo::mmode`); the
//! Doppler trace is the velocity of one annulus point of the motion model along the beam
//! (`heartsim_echo::doppler`), tissue only — there is no blood Doppler of any kind and no control
//! that suggests one. The trace and every measurement are labelled *model*; the thin band around
//! the Doppler curve is *designed*. Nothing here is graded and no normal range is shown: the
//! interpretation is the calculators', with their sources.
//!
//! **What is kept.** The trace, the calipers and the list of measurements live in this struct for
//! the session. Nothing measured or typed is stored: not in the settings, not on disk.
//!
//! **The cost.** A column of the M-mode trace is a full evaluation of the heart at the column's
//! time plus one thin slice and image (`heartsim_echo::mmode`), several times a line of the sector;
//! the page asks for at most [`MMODE_COLUMNS_PER_FRAME`] columns a frame and, when the trace falls
//! behind the clock, records every other column and draws the gaps from their neighbours. The
//! Doppler trace needs only the ventricles' own motion at one point. Both are computed only while
//! their mode is chosen, the heart is not frozen and the wall motion is shown.

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2 as EVec2};
use heartsim_echo::doppler::{DOPPLER_STEP_MS, DopplerContext, DopplerScan, Marker, WaveKind, snap_gate};
use heartsim_echo::measure::{ExcursionContext, Plane2, TraceScale};
use heartsim_echo::measure::{Structure, Unit, column_samples, cursor_toward, structure_span};
use heartsim_echo::mmode::{MMODE_STEP_MS, MModeScan, ScanContext, SweepSpeed, TraceRate, line_angle_deg};
use heartsim_echo::phases::Side;
use heartsim_echo::{
    AcousticTable, AnnulusSite, BeatTimes, Feedback, Named, ViewId, feedback, polygon_area_cm2,
    reference_excursion, reference_on_mmode, reference_on_plane, reference_velocity, slope_cm_s,
};
use heartsim_render::SectionLayout;
use heartsim_ui::{cards, t};

use super::{EchoPage, EchoSection, right_aligned_row};
use crate::heart_view::HeartView;

/// Most measurements kept in the list.
pub const MAX_MEASUREMENTS: usize = 4;
/// Columns of the M-mode trace computed per drawn frame at most.
pub const MMODE_COLUMNS_PER_FRAME: usize = 2;
/// Doppler samples computed per drawn frame at most.
pub const DOPPLER_SAMPLES_PER_FRAME: usize = 8;
/// Screen pixels per millimetre the sweep speed is nominal at (a 96 dpi screen). The trace's time
/// scale is `speed / PX_PER_MM` px per second; the page says "nominal".
pub const PX_PER_MM: f32 = 3.78;
/// Height of the trace and of the ECG under it, logical px.
pub const TRACE_PX: f32 = 138.0;
pub const ECG_PX: f32 = 34.0;
/// The same on a phone-narrow window, where the image and the controls share a short page.
pub const TRACE_COMPACT_PX: f32 = 80.0;
pub const ECG_COMPACT_PX: f32 = 28.0;
/// Height the trace panel takes under the sector: the trace, the ECG and the gaps.
pub const TRACE_PANEL_PX: f32 = TRACE_PX + ECG_PX + 12.0;
pub const TRACE_PANEL_COMPACT_PX: f32 = TRACE_COMPACT_PX + ECG_COMPACT_PX + 12.0;
/// Height kept for the read-out and the legend while a trace is shown (the same in every display
/// mode), logical px.
pub const LEGEND_WITH_TRACE_PX: f32 = 44.0;
/// Largest velocity the Doppler trace shows, cm/s (the guideline sets the scale at or under 25
/// cm/s, Mitchell 2019, p. 44).
pub const VELOCITY_RANGE_CM_S: f32 = 25.0;
/// Half-width of the designed spectral band, cm/s: the floor and the share of the velocity.
const BAND_FLOOR_CM_S: f32 = 0.8;
const BAND_SHARE: f32 = 0.12;
/// Distance at which a click grabs a caliper point, logical px.
const GRAB_PX: f32 = 9.0;

/// What the Measure section does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum MeasureMode {
    /// Calipers on the frozen sector.
    #[default]
    Plane,
    /// The M-mode cursor and its trace.
    MMode,
    /// Annular tissue Doppler.
    Doppler,
}

impl MeasureMode {
    /// Every mode, in the order of the switch.
    pub const ALL: [Self; 3] = [Self::Plane, Self::MMode, Self::Doppler];

    /// Whether a trace is shown under the sector.
    #[must_use]
    pub fn is_trace(self) -> bool {
        self != Self::Plane
    }

    /// The name on the switch.
    #[must_use]
    pub fn name(self) -> String {
        match self {
            Self::Plane => t!("echo-measure-mode-plane"),
            Self::MMode => t!("echo-measure-mode-mmode"),
            Self::Doppler => t!("echo-measure-mode-doppler"),
        }
    }

    fn tools(self) -> &'static [Tool] {
        match self {
            Self::Plane => &[Tool::Distance, Tool::Area],
            Self::MMode => &[Tool::Depth, Tool::Interval, Tool::Slope],
            Self::Doppler => &[Tool::Velocity, Tool::Interval],
        }
    }
}

/// A caliper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    /// Distance between two points of the sector, mm.
    Distance,
    /// Area of a traced outline of the sector, cm².
    Area,
    /// Depth between two points of the M-mode trace, mm.
    Depth,
    /// Time between two points of a trace, ms.
    Interval,
    /// Slope between two points of the M-mode trace, cm/s.
    Slope,
    /// Velocity at a point of the Doppler trace, cm/s.
    Velocity,
}

impl Tool {
    fn name(self) -> String {
        match self {
            Self::Distance => t!("echo-tool-distance"),
            Self::Area => t!("echo-tool-area"),
            Self::Depth => t!("echo-tool-depth"),
            Self::Interval => t!("echo-tool-time"),
            Self::Slope => t!("echo-tool-slope"),
            Self::Velocity => t!("echo-tool-velocity"),
        }
    }

    /// Points a caliper has; `None` for the traced area, which takes as many as the learner puts.
    fn points(self) -> Option<usize> {
        match self {
            Self::Area => None,
            Self::Velocity => Some(1),
            _ => Some(2),
        }
    }

    /// The measurements the reading can be named as.
    fn names(self) -> &'static [Named] {
        match self {
            Self::Distance => &[
                Named::Lvidd,
                Named::Lvids,
                Named::Ivsd,
                Named::Lvpwd,
                Named::LaDiameter,
                Named::AorticRoot,
            ],
            Self::Depth => &[
                Named::Lvidd,
                Named::Lvids,
                Named::Ivsd,
                Named::Lvpwd,
                Named::LaDiameter,
                Named::AorticRoot,
                Named::Tapse,
                Named::Mapse,
            ],
            Self::Velocity => &[Named::SPrime, Named::EPrime, Named::APrime],
            Self::Area | Self::Interval | Self::Slope => &[],
        }
    }
}

/// What a finished caliper reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reading {
    pub value: f32,
    pub unit: &'static str,
}

/// A point of a caliper: `(x, y)` is `(mm, mm)` of the sector plane (`x` lateral, `y` depth) or
/// `(ms, mm)` of an M-mode trace or `(ms, cm/s)` of a Doppler trace.
pub type Pt = (f64, f32);

/// The reading of `points` with `tool`, `None` while the caliper is not complete.
#[must_use]
pub fn reading_of(tool: Tool, points: &[Pt]) -> Option<Reading> {
    let two = |f: &dyn Fn(Pt, Pt) -> f32, unit| match points {
        [a, b] => Some(Reading {
            value: f(*a, *b),
            unit,
        }),
        _ => None,
    };
    match tool {
        Tool::Distance => two(
            &|a, b| Plane2::new(a.0 as f32, a.1).distance(Plane2::new(b.0 as f32, b.1)),
            "mm",
        ),
        Tool::Area => (points.len() >= 3).then(|| Reading {
            value: polygon_area_cm2(
                &points
                    .iter()
                    .map(|p| Plane2::new(p.0 as f32, p.1))
                    .collect::<Vec<_>>(),
            ),
            unit: "cm²",
        }),
        Tool::Depth => two(&|a, b| (b.1 - a.1).abs(), "mm"),
        Tool::Interval => two(&|a, b| (b.0 - a.0).abs() as f32, "ms"),
        Tool::Slope => two(
            &|a, b| {
                let (first, second) = if a.0 <= b.0 { (a, b) } else { (b, a) };
                slope_cm_s(second.0 - first.0, second.1 - first.1).unwrap_or(0.0)
            },
            "cm/s",
        ),
        Tool::Velocity => match points {
            [a] => Some(Reading {
                value: a.1,
                unit: "cm/s",
            }),
            _ => None,
        },
    }
}

/// A measurement in the list.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub name: Named,
    pub mode: MeasureMode,
    pub tool: Tool,
    pub points: Vec<Pt>,
    /// What the learner read, in the measurement's unit (always the magnitude).
    pub value: f32,
    /// The annulus the velocity was read at, for the calculators' choice of input.
    pub site: Option<AnnulusSite>,
    /// The model's value and the difference.
    pub feedback: Feedback,
    /// For TAPSE and MAPSE: the model's read-out of the beat along its own long axis, mm, and the
    /// angle between the cursor and the annulus's motion, degrees.
    pub axis_note: Option<(f32, f32)>,
}

/// What to send: the entry and the app's answer (the calculators page opens with the value filled).
#[derive(Debug, Clone, PartialEq)]
pub struct SendRequest {
    pub name: Named,
    /// The value in the measurement's own unit (mm or cm/s).
    pub value: f32,
    pub site: Option<AnnulusSite>,
}

/// The ECG lead under the trace: samples every `dt_ms`, the first at `first_ms` (the simulation
/// clock the trace is on).
#[derive(Debug, Clone, Default)]
pub struct EcgTrace {
    pub samples: Vec<f32>,
    pub first_ms: f64,
    pub dt_ms: f64,
    pub lead: String,
}

/// A scripted measurement, for the documentation tour and the tutorials.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Demo {
    /// M-mode at the parasternal long axis through the mitral leaflet tips, LVIDd and LVIDs measured.
    Lvid,
    /// M-mode at the apical four-chamber through the lateral tricuspid annulus, TAPSE measured.
    Tapse,
    /// Tissue Doppler at the lateral mitral annulus, e′ measured.
    EPrime,
}

/// What the page keeps of the Measure section.
#[derive(Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent switches and frame requests"
)]
pub(super) struct MeasureState {
    pub mode: MeasureMode,
    pub speed: SweepSpeed,
    pub frozen: bool,
    /// The M-mode cursor, degrees from the sector's central beam, positive toward the marker.
    pub cursor_deg: f32,
    pub tool: Tool,
    /// The caliper being placed.
    pub points: Vec<Pt>,
    pub name: Option<Named>,
    /// The phone layout is in use (the trace is lower).
    pub compact: bool,
    /// Move a placed point to the nearest boundary between tissues on its beam (off by default).
    pub snap: bool,
    /// Take an M-mode column every other step (Settings, "Faster M-mode trace").
    pub half_rate: bool,
    pub list: Vec<Entry>,
    mmode: Option<MModeScan>,
    doppler: DopplerScan,
    /// The simulation time at the right edge of the trace while frozen, ms.
    pub scrub_ms: Option<f64>,
    pub beats: Vec<BeatTimes>,
    pub markers: Vec<Marker>,
    pub ecg: Option<EcgTrace>,
    texture: Option<Tex>,
    texture_key: Option<(i64, usize, usize, u32, u64, u32)>,
    send: Option<SendRequest>,
    pending_gate: Option<Plane2>,
    pending_site: Option<AnnulusSite>,
    pending_aim: Option<Aim>,
    pending_demo: Option<Demo>,
    /// The scripted measurement that was started (what [`EchoPage::finish_demo`] finishes).
    demo: Option<Demo>,
    demo_finish: bool,
    /// After the scripted measurement, send its first entry to the calculators (the tour).
    demo_send: bool,
    /// Columns and samples computed in the last frame, and in all (tests, the cost report).
    pub computed_last: usize,
    pub computed_total: u64,
    /// The trace as last drawn, in screen points.
    pub trace_rect: Option<Rect>,
    /// What the last frame said about the Doppler gate.
    pub gate_note: Option<String>,
}

/// The M-mode texture (a handle has no `Debug`).
struct Tex(egui::TextureHandle);

impl std::fmt::Debug for Tex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Tex")
    }
}

/// Where the M-mode cursor is aimed by a button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Aim {
    /// Just beyond the mitral leaflet tips, perpendicular to the LV long axis.
    MitralTips,
    /// Through the lateral tricuspid annulus.
    TricuspidAnnulus,
    /// Through the lateral mitral annulus.
    MitralAnnulus,
}

impl Default for MeasureState {
    fn default() -> Self {
        Self {
            mode: MeasureMode::Plane,
            speed: SweepSpeed::default(),
            frozen: false,
            cursor_deg: 0.0,
            tool: Tool::Distance,
            points: Vec::new(),
            name: None,
            snap: false,
            half_rate: false,
            compact: false,
            list: Vec::new(),
            mmode: None,
            doppler: DopplerScan::new(),
            scrub_ms: None,
            beats: Vec::new(),
            markers: Vec::new(),
            ecg: None,
            texture: None,
            texture_key: None,
            send: None,
            pending_gate: None,
            pending_site: None,
            pending_aim: None,
            pending_demo: None,
            demo: None,
            demo_finish: false,
            demo_send: false,
            computed_last: 0,
            computed_total: 0,
            trace_rect: None,
            gate_note: None,
        }
    }
}

/// The time at the right edge and the span of the trace window, ms, for a trace `width_px` wide.
fn window_ms(speed: SweepSpeed, width_px: f32) -> f64 {
    speed.time_ms(width_px / PX_PER_MM)
}

/// The name of a measurement on the list and in the calculators' "from the echo simulator" mark.
#[must_use]
pub fn name_of(n: Named) -> String {
    match n {
        Named::Lvidd => t!("echo-name-lvidd"),
        Named::Lvids => t!("echo-name-lvids"),
        Named::Ivsd => t!("echo-name-ivsd"),
        Named::Lvpwd => t!("echo-name-lvpwd"),
        Named::LaDiameter => t!("echo-name-la"),
        Named::AorticRoot => t!("echo-name-ao-root"),
        Named::Tapse => t!("echo-name-tapse"),
        Named::Mapse => t!("echo-name-mapse"),
        Named::EPrime => t!("echo-name-e-prime"),
        Named::APrime => t!("echo-name-a-prime"),
        Named::SPrime => t!("echo-name-s-prime"),
    }
}

/// The name of an annulus site.
#[must_use]
pub fn site_name(s: AnnulusSite) -> String {
    match s {
        AnnulusSite::MitralSeptal => t!("echo-site-mitral-septal"),
        AnnulusSite::MitralLateral => t!("echo-site-mitral-lateral"),
        AnnulusSite::TricuspidLateral => t!("echo-site-tricuspid-lateral"),
    }
}

fn unit_text(u: Unit) -> &'static str {
    u.symbol()
}

impl EchoPage {
    // ---- state the tests, the tour and the app read ------------------------------------------

    /// What the Measure section does.
    #[must_use]
    pub fn measure_mode(&self) -> MeasureMode {
        self.measure.mode
    }

    /// Choose what the Measure section does (the switch, the tour). A trace mode shows the
    /// section and starts the trace afresh.
    pub fn set_measure_mode(&mut self, mode: MeasureMode) {
        if self.measure.mode != mode {
            self.measure.mode = mode;
            self.measure.points.clear();
            self.measure.tool = mode.tools()[0];
            self.measure.name = None;
            self.measure.scrub_ms = None;
            self.measure.frozen = false;
            if let Some(m) = &mut self.measure.mmode {
                m.clear();
            }
            self.measure.doppler.clear();
            self.measure.beats.clear();
            self.measure.markers.clear();
        }
        self.section = EchoSection::Measure;
    }

    /// The sweep speed.
    #[must_use]
    pub fn sweep_speed(&self) -> SweepSpeed {
        self.measure.speed
    }

    /// Set the sweep speed.
    pub fn set_sweep_speed(&mut self, speed: SweepSpeed) {
        self.measure.speed = speed;
    }

    /// Whether the sector and the trace are held still.
    #[must_use]
    pub fn frozen(&self) -> bool {
        self.measure.frozen
    }

    /// Freeze or release the sector and the traces.
    pub fn set_frozen(&mut self, frozen: bool) {
        self.measure.frozen = frozen;
        if !frozen {
            self.measure.scrub_ms = None;
        }
    }

    /// The M-mode cursor, degrees from the sector's central beam.
    #[must_use]
    pub fn cursor_deg(&self) -> f32 {
        self.measure.cursor_deg
    }

    /// Put the M-mode cursor at `deg` (the trace restarts only from the next column on).
    pub fn set_cursor_deg(&mut self, deg: f32) {
        let half = self.pose.map_or(45.0, |p| p.sector_deg * 0.5);
        self.measure.cursor_deg = if deg.is_finite() {
            deg.clamp(-half, half)
        } else {
            0.0
        };
    }

    /// Whether placed points snap to the nearest tissue boundary on their beam (off by default).
    #[must_use]
    pub fn snap(&self) -> bool {
        self.measure.snap
    }

    /// Switch snapping of the calipers to tissue boundaries on or off.
    pub fn set_snap(&mut self, on: bool) {
        self.measure.snap = on;
    }

    /// The caliper tool in use.
    #[must_use]
    pub fn tool(&self) -> Tool {
        self.measure.tool
    }

    /// Choose the caliper tool (it must belong to the mode; otherwise the first of the mode).
    pub fn set_tool(&mut self, tool: Tool) {
        self.measure.tool = if self.measure.mode.tools().contains(&tool) {
            tool
        } else {
            self.measure.mode.tools()[0]
        };
        self.measure.points.clear();
        self.measure.name = None;
    }

    /// Put one point of the caliper in use, in the units of [`Pt`]; freezes the sector or trace
    /// first, as a click on it would.
    pub fn place_caliper_point(&mut self, p: Pt) {
        self.measure.frozen = true;
        match self.measure.tool.points() {
            Some(n) if self.measure.points.len() >= n => self.measure.points.clear(),
            _ => {}
        }
        self.measure.points.push(p);
    }

    /// The reading of the caliper in use.
    #[must_use]
    pub fn reading(&self) -> Option<Reading> {
        match self.measure.tool.points() {
            Some(n) if self.measure.points.len() != n => None,
            _ => reading_of(self.measure.tool, &self.measure.points),
        }
    }

    /// Choose the name the reading is to be added under.
    pub fn set_measurement_name(&mut self, name: Option<Named>) {
        self.measure.name = name.filter(|n| self.measure.tool.names().contains(n));
    }

    /// The measurements in the list.
    #[must_use]
    pub fn measurements(&self) -> &[Entry] {
        &self.measure.list
    }

    /// Clear the list and the caliper in use.
    pub fn clear_measurements(&mut self) {
        self.measure.list.clear();
        self.measure.points.clear();
        self.measure.name = None;
    }

    /// Ask for the entry `index` of the list to be sent to the calculators that take it.
    pub fn request_send(&mut self, index: usize) {
        if let Some(e) = self.measure.list.get(index) {
            self.measure.send = Some(SendRequest {
                name: e.name,
                value: e.value,
                site: e.site,
            });
        }
    }

    /// The send the learner asked for, once (the app carries it to the calculators page).
    pub fn take_send(&mut self) -> Option<SendRequest> {
        self.measure.send.take()
    }

    /// Record the M-mode trace at half its rate (Settings, "Faster M-mode trace"): a column
    /// every 50 ms. Takes effect from the next column; what is already recorded stays.
    pub fn set_fast_mmode(&mut self, on: bool) {
        self.measure.half_rate = on;
    }

    /// Whether the M-mode trace is recorded at half its rate.
    #[must_use]
    pub fn fast_mmode(&self) -> bool {
        self.measure.half_rate
    }

    /// The ECG lead to draw under the trace.
    pub fn set_ecg(&mut self, ecg: Option<EcgTrace>) {
        self.measure.ecg = ecg;
    }

    /// The simulation time the trace's window ends at, ms, and its length for a trace of the width
    /// last drawn; `None` before any column.
    #[must_use]
    pub fn trace_window(&self) -> Option<(f64, f64)> {
        let w = self.measure.trace_rect.map_or(380.0, |r| r.width());
        Some((
            self.trace_right_ms()?,
            window_ms(self.measure.speed, w - TRACE_LEFT_PX),
        ))
    }

    /// The M-mode columns recorded so far (tests).
    #[must_use]
    pub fn mmode_columns(&self) -> usize {
        self.measure.mmode.as_ref().map_or(0, |m| {
            m.buffer
                .span()
                .map_or(0, |(a, b)| (a..=b).filter(|&k| m.buffer.get(k).is_some()).count())
        })
    }

    /// Where the trace was last drawn, in screen points (`None` when no trace is shown).
    #[must_use]
    pub fn trace_rect(&self) -> Option<Rect> {
        self.measure.trace_rect
    }

    /// The Doppler samples recorded so far (tests).
    #[must_use]
    pub fn doppler_samples(&self) -> usize {
        self.measure.doppler.buffer.span().map_or(0, |(a, b)| {
            (a..=b)
                .filter(|&k| self.measure.doppler.buffer.get(k).is_some())
                .count()
        })
    }

    /// The tissue-Doppler gate.
    #[must_use]
    pub fn gate_site(&self) -> Option<AnnulusSite> {
        self.measure.doppler.gate().map(|g| g.site)
    }

    /// The angle between the beam and the annulus's motion at the latest S' peak in view, degrees:
    /// the velocity is not corrected for it (brief B34), so it is shown beside the trace.
    #[must_use]
    pub fn doppler_angle_deg(&self) -> Option<f32> {
        let m = self
            .measure
            .markers
            .iter()
            .filter(|m| m.kind == WaveKind::S)
            .max_by(|a, b| a.t_ms.total_cmp(&b.t_ms))?;
        self.measure.doppler.angle_deg(
            self.measure
                .doppler
                .buffer
                .step_at(m.t_ms + DOPPLER_STEP_MS * 0.5),
        )
    }

    /// The waves marked on the visible Doppler trace.
    #[must_use]
    pub fn doppler_markers(&self) -> &[Marker] {
        &self.measure.markers
    }

    /// Put the Doppler gate on an annulus at the next frame (snapped, as a click on it would).
    pub fn request_gate(&mut self, site: AnnulusSite) {
        self.measure.pending_site = Some(site);
    }

    /// Aim the M-mode cursor at the next frame.
    pub(super) fn aim_cursor(&mut self, aim: Aim) {
        self.measure.pending_aim = Some(aim);
    }

    /// Start a scripted measurement as soon as the anatomy is built: the probe goes to its view,
    /// the mode and the cursor or gate are set, the trace starts recording.
    pub fn request_demo(&mut self, demo: Demo) {
        self.measure.pending_demo = Some(demo);
        self.measure.demo_finish = false;
    }

    /// Finish the scripted measurement: freeze, place the calipers on what the model holds (with
    /// the small offsets of a hand), name them and add them to the list. Done at the next frame
    /// that has the trace.
    pub fn finish_demo(&mut self) {
        self.measure.demo_finish = true;
    }

    /// [`Self::finish_demo`], then send the first measurement to the calculators.
    pub fn finish_demo_and_send(&mut self) {
        self.measure.demo_finish = true;
        self.measure.demo_send = true;
    }

    fn trace_right_ms(&self) -> Option<f64> {
        let newest = match self.measure.mode {
            MeasureMode::MMode => self
                .measure
                .mmode
                .as_ref()?
                .buffer
                .last_step()
                .map(|k| k as f64 * MMODE_STEP_MS)?,
            MeasureMode::Doppler => self
                .measure
                .doppler
                .buffer
                .last_step()
                .map(|k| k as f64 * DOPPLER_STEP_MS)?,
            MeasureMode::Plane => return None,
        };
        Some(self.measure.scrub_ms.map_or(newest, |s| s.min(newest)))
    }

    /// Height kept under the sector for the trace, logical px; zero when no trace is shown.
    pub(super) fn trace_reserve_px(&self) -> f32 {
        if self.measure.mode.is_trace() {
            if self.measure.compact {
                TRACE_PANEL_COMPACT_PX
            } else {
                TRACE_PANEL_PX
            }
        } else {
            0.0
        }
    }

    /// Height kept under the sector for the read-out and the legend.
    pub(super) fn legend_px(&self) -> f32 {
        if self.measure.mode.is_trace() {
            LEGEND_WITH_TRACE_PX
        } else {
            super::LEGEND_PX
        }
    }

    // ---- per-frame work ----------------------------------------------------------------------

    /// Process what was asked for, then advance the trace that is shown by the budget of the
    /// frame.
    pub(super) fn update_measure(&mut self, ui: &egui::Ui, heart: &mut HeartView) {
        self.measure.computed_last = 0;
        self.process_requests(heart);
        if !self.measure.mode.is_trace() {
            return;
        }
        let Some(pose) = self.pose else { return };
        let mut settings = self.settings.clone();
        settings.depth_mm = pose.depth_mm;
        settings.sector_deg = pose.sector_deg;
        let frozen = self.measure.frozen;
        let cursor_deg = self.measure.cursor_deg;
        let mode = self.measure.mode;
        let Some((anatomy, parts)) = heart.echo_scan_parts() else {
            return;
        };
        let pose = settings.apply_to(&pose);
        let until = parts.t_ms;
        let mut computed = 0;
        if !frozen && !parts.reduced_motion {
            match mode {
                MeasureMode::MMode => {
                    let scan = self.measure.mmode.get_or_insert_with(|| {
                        MModeScan::new(anatomy, parts.model, AcousticTable::builtin())
                    });
                    let ctx = ScanContext {
                        anatomy,
                        history: parts.history,
                        pose,
                        cursor_deg,
                        settings: &settings,
                        enabled: parts.enabled,
                        exaggeration: parts.exaggeration,
                    };
                    scan.rate = if self.measure.half_rate {
                        TraceRate::Half
                    } else {
                        TraceRate::Full
                    };
                    computed = scan.advance(parts.model, &ctx, until, MMODE_COLUMNS_PER_FRAME);
                }
                MeasureMode::Doppler => {
                    let ctx = DopplerContext {
                        history: parts.history,
                        enabled: parts.enabled,
                        exaggeration: parts.exaggeration,
                    };
                    computed =
                        self.measure
                            .doppler
                            .advance(parts.model, &ctx, until, DOPPLER_SAMPLES_PER_FRAME);
                }
                MeasureMode::Plane => {}
            }
        }
        self.measure.computed_last = computed;
        self.measure.computed_total += computed as u64;
        // the beats under the window and the waves marked on them
        if mode == MeasureMode::Doppler
            && let Some(g) = self.measure.doppler.gate()
            && let Some(right) = self.trace_right_ms_for(mode)
        {
            let span = window_ms(
                self.measure.speed,
                self.measure.trace_rect.map_or(380.0, |r| r.width()) - TRACE_LEFT_PX,
            );
            let side = g.site.side();
            self.measure.beats = beats_in(parts.model, parts.history, right - span, right, side);
            let mut markers = Vec::new();
            for b in &self.measure.beats {
                markers.extend(self.measure.doppler.markers(b));
            }
            self.measure.markers = markers;
        }
        if computed > 0 {
            ui.ctx().request_repaint();
        }
    }

    fn trace_right_ms_for(&self, _mode: MeasureMode) -> Option<f64> {
        self.trace_right_ms()
    }

    fn process_requests(&mut self, heart: &mut HeartView) {
        if let Some(demo) = self.measure.pending_demo.take() {
            self.begin_demo(heart, demo);
        }
        if let Some(aim) = self.measure.pending_aim.take() {
            self.aim_now(heart, aim);
        }
        if let Some(site) = self.measure.pending_site.take() {
            self.gate_on_site(heart, site);
        }
        if let Some(p) = self.measure.pending_gate.take() {
            self.gate_at(heart, p);
        }
        if std::mem::take(&mut self.measure.demo_finish) {
            self.run_demo_finish(heart);
        }
    }

    fn begin_demo(&mut self, heart: &mut HeartView, demo: Demo) {
        let Some(views) = heart.echo_views() else {
            self.measure.pending_demo = Some(demo);
            return;
        };
        let Some(anatomy) = heart.echo_anatomy() else {
            self.measure.pending_demo = Some(demo);
            return;
        };
        let view = if demo == Demo::Lvid {
            ViewId::ParasternalLongAxis
        } else {
            ViewId::ApicalFourChamber
        };
        self.pose = Some(views.reference(anatomy, view).pose);
        self.window = None;
        self.measure.demo = Some(demo);
        // a scripted measurement starts from a clean page: no trace, no calipers, no list
        self.measure.list.clear();
        self.measure.points.clear();
        self.measure.name = None;
        self.measure.frozen = false;
        self.measure.scrub_ms = None;
        if let Some(m) = &mut self.measure.mmode {
            m.clear();
        }
        self.measure.doppler.set_gate(None);
        self.measure.doppler.clear();
        self.measure.beats.clear();
        self.measure.markers.clear();
        self.set_measure_mode(if demo == Demo::EPrime {
            MeasureMode::Doppler
        } else {
            MeasureMode::MMode
        });
        self.measure.speed = SweepSpeed::Mm50;
        match demo {
            Demo::Lvid => self.measure.pending_aim = Some(Aim::MitralTips),
            Demo::Tapse => self.measure.pending_aim = Some(Aim::TricuspidAnnulus),
            Demo::EPrime => self.measure.pending_site = Some(AnnulusSite::MitralLateral),
        }
    }

    fn aim_now(&mut self, heart: &mut HeartView, aim: Aim) {
        let Some(pose) = self.pose else {
            self.measure.pending_aim = Some(aim);
            return;
        };
        let t_ms = heart
            .motion()
            .map_or(0.0, crate::heart_motion::HeartMotion::time_ms);
        let (Some(views), Some(anatomy)) = (heart.echo_views(), heart.echo_anatomy()) else {
            self.measure.pending_aim = Some(aim);
            return;
        };
        let frame = pose.frame(anatomy.designed.contact_limit_deg);
        let k = &views.landmarks;
        let target = match aim {
            Aim::MitralTips => {
                // just beyond the leaflet tips (about 18 mm below the annulus plane in diastole,
                // Mitchell 2019, p. 31), on the LV long axis
                k.mitral + (k.apex - k.mitral).normalize_or_zero() * MITRAL_TIP_BELOW_MM
            }
            Aim::TricuspidAnnulus | Aim::MitralAnnulus => {
                let site = if aim == Aim::TricuspidAnnulus {
                    AnnulusSite::TricuspidLateral
                } else {
                    AnnulusSite::MitralLateral
                };
                let Some(parts) = heart.echo_scan_parts().map(|x| x.1) else {
                    return;
                };
                let input = heartsim_echo::MotionInput {
                    history: parts.history,
                    t_ms,
                    resp_phase: parts.history.respiratory_phase(t_ms),
                    enabled: parts.enabled,
                    exaggeration: parts.exaggeration,
                };
                match parts.model.annulus_position(&input, site) {
                    Some(p) => p,
                    None => return,
                }
            }
        };
        self.set_cursor_deg(cursor_toward(&frame, target));
    }

    fn gate_on_site(&mut self, heart: &mut HeartView, site: AnnulusSite) {
        let Some(pose) = self.pose else {
            self.measure.pending_site = Some(site);
            return;
        };
        let Some((anatomy, parts)) = heart.echo_scan_parts() else {
            self.measure.pending_site = Some(site);
            return;
        };
        let frame = pose.frame(anatomy.designed.contact_limit_deg);
        let t_ms = parts.t_ms;
        let input = heartsim_echo::MotionInput {
            history: parts.history,
            t_ms,
            resp_phase: parts.history.respiratory_phase(t_ms),
            enabled: parts.enabled,
            exaggeration: parts.exaggeration,
        };
        let Some(p) = parts.model.annulus_position(&input, site) else {
            return;
        };
        let (q, _) = Plane2::of_world(&frame, p);
        let gate = snap_gate(parts.model, &input, &frame, q);
        self.measure.gate_note = gate.is_none().then(|| t!("echo-gate-none"));
        self.measure.doppler.set_gate(gate);
    }

    fn gate_at(&mut self, heart: &mut HeartView, plane: Plane2) {
        let Some(pose) = self.pose else { return };
        let Some((anatomy, parts)) = heart.echo_scan_parts() else {
            return;
        };
        let frame = pose.frame(anatomy.designed.contact_limit_deg);
        let t_ms = parts.t_ms;
        let input = heartsim_echo::MotionInput {
            history: parts.history,
            t_ms,
            resp_phase: parts.history.respiratory_phase(t_ms),
            enabled: parts.enabled,
            exaggeration: parts.exaggeration,
        };
        let gate = snap_gate(parts.model, &input, &frame, plane);
        self.measure.gate_note = gate.is_none().then(|| t!("echo-gate-none"));
        if gate.is_some() {
            self.measure.doppler.set_gate(gate);
        }
    }

    // ---- committing a measurement -------------------------------------------------------------

    /// Add the caliper in use to the list under `name`, with the model's own value beside it.
    /// Returns whether it was added (a complete caliper, a name that fits it, room in the list).
    pub fn add_measurement(&mut self, heart: &mut HeartView, name: Named) -> bool {
        if self.measure.list.len() >= MAX_MEASUREMENTS || !self.measure.tool.names().contains(&name) {
            return false;
        }
        let Some(r) = self.reading() else { return false };
        let tool = self.measure.tool;
        let mode = self.measure.mode;
        let points = self.measure.points.clone();
        let value = r.value.abs();
        let (reference, axis_note, site) = self.reference_for(heart, mode, tool, &points, name);
        self.measure.list.push(Entry {
            name,
            mode,
            tool,
            points,
            value,
            site,
            feedback: feedback(value, reference),
            axis_note,
        });
        self.measure.points.clear();
        self.measure.name = None;
        true
    }

    #[allow(clippy::type_complexity)]
    fn reference_for(
        &mut self,
        heart: &mut HeartView,
        mode: MeasureMode,
        tool: Tool,
        pts: &[Pt],
        name: Named,
    ) -> (Option<f32>, Option<(f32, f32)>, Option<AnnulusSite>) {
        match (mode, tool) {
            (MeasureMode::Plane, Tool::Distance) => {
                let (a, b) = (pts[0], pts[1]);
                (
                    reference_on_plane(
                        &self.image,
                        name,
                        Plane2::new(a.0 as f32, a.1),
                        Plane2::new(b.0 as f32, b.1),
                    ),
                    None,
                    None,
                )
            }
            (MeasureMode::MMode, Tool::Depth) => {
                let (a, b) = (pts[0], pts[1]);
                let Some(scan) = &self.measure.mmode else {
                    return (None, None, None);
                };
                if name.structure().is_some() {
                    let t = a.0.min(b.0);
                    let r = reference_on_mmode(&scan.buffer, name, t, f32::midpoint(a.1, b.1));
                    return (r, None, None);
                }
                // an excursion: the annulus the cursor passes, between the two times
                let Some(pose) = self.pose else {
                    return (None, None, None);
                };
                let cursor = self.measure.cursor_deg;
                let Some((anatomy, parts)) = heart.echo_scan_parts() else {
                    return (None, None, None);
                };
                let frame = pose.frame(anatomy.designed.contact_limit_deg);
                let ctx = ExcursionContext {
                    history: parts.history,
                    frame,
                    cursor_deg: cursor,
                    enabled: parts.enabled,
                    exaggeration: parts.exaggeration,
                };
                let (t1, t2) = (a.0.min(b.0), a.0.max(b.0));
                let ex = reference_excursion(parts.model, &ctx, name, t1, t2);
                let along = ex.map(|e| e.along_beam_mm);
                let axis = ex.and_then(|e| {
                    let input = heartsim_echo::MotionInput {
                        history: parts.history,
                        t_ms: t2 + 60.0,
                        resp_phase: parts.history.respiratory_phase(t2 + 60.0),
                        enabled: parts.enabled,
                        exaggeration: 1.0,
                    };
                    let ro = parts.model.readout(&input);
                    let v = if name == Named::Tapse {
                        ro.tapse_mm
                    } else {
                        ro.mapse_mm
                    };
                    v.map(|v| (v as f32, e.angle_deg))
                });
                (along, axis, ex.map(|e| e.site))
            }
            (MeasureMode::Doppler, Tool::Velocity) => {
                let t = pts[0].0;
                let site = self.measure.doppler.gate().map(|g| g.site);
                let Some(bt) = self
                    .measure
                    .beats
                    .iter()
                    .find(|b| b.qrs_ms <= t && t < b.next_qrs_ms.unwrap_or(b.qrs_ms + 2_000.0))
                else {
                    return (None, None, site);
                };
                let marks = self.measure.doppler.markers(bt);
                (reference_velocity(name, &marks), None, site)
            }
            _ => (None, None, None),
        }
    }

    // ---- the scripted measurements ------------------------------------------------------------

    fn run_demo_finish(&mut self, heart: &mut HeartView) {
        self.measure.frozen = true;
        match self.measure.demo {
            Some(Demo::Lvid) => self.demo_lvid(),
            Some(Demo::Tapse) => self.demo_tapse(heart),
            Some(Demo::EPrime) => self.demo_e_prime(heart),
            None => {}
        }
        if std::mem::take(&mut self.measure.demo_send) {
            self.request_send(0);
        }
    }

    /// LVIDd and LVIDs on the recorded M-mode, put where a careful hand would put them (the
    /// model's boundaries, 1.2 mm and 0.8 mm off).
    fn demo_lvid(&mut self) {
        let Some(scan) = &self.measure.mmode else { return };
        let (Some((a, b)), true) = (
            scan.buffer.span(),
            self.measure.list.len() + 2 <= MAX_MEASUREMENTS,
        ) else {
            return;
        };
        // the column with the largest and the one with the smallest cavity on the line
        let mut best: Vec<(f32, i64, f32, f32)> = Vec::new();
        // the columns in view (the window of the trace as drawn, a little less)
        let span_ms = window_ms(
            self.measure.speed,
            self.measure.trace_rect.map_or(380.0, |r| r.width()) - TRACE_LEFT_PX,
        ) * 0.9;
        let from = b - (span_ms / MMODE_STEP_MS) as i64;
        for k in a.max(from)..=b {
            let Some(col) = scan.buffer.get(k) else { continue };
            let line = column_samples(col);
            let n = col.labels.len();
            let step = col.depth_mm / n as f32;
            // the middle of the longest run of blood on the line
            let blood: Vec<usize> = (0..n)
                .filter(|&i| col.labels[i] == heartsim_geometry::Label::LvBlood as u8)
                .collect();
            let Some(&mid) = blood.get(blood.len() / 2) else {
                continue;
            };
            if let Some((s0, s1)) = structure_span(&line, mid, Structure::LvCavity) {
                best.push(((s1 - s0) as f32 * step, k, s0 as f32 * step, s1 as f32 * step));
            }
        }
        let (Some(big), Some(small)) = (
            best.iter().copied().max_by(|x, y| x.0.total_cmp(&y.0)),
            best.iter().copied().min_by(|x, y| x.0.total_cmp(&y.0)),
        ) else {
            return;
        };
        self.measure.tool = Tool::Depth;
        self.measure.mode = MeasureMode::MMode;
        // hand placement: both ends a little off the true boundary
        for (named, c, shift) in [
            (Named::Lvidd, big, (1.2_f32, -0.5_f32)),
            (Named::Lvids, small, (0.8, -1.1)),
        ] {
            let t = c.1 as f64 * MMODE_STEP_MS;
            self.measure.points = vec![(t, c.2 + shift.0), (t, c.3 + shift.1)];
            self.measure.name = Some(named);
        }
        // the two readings are added with the reference of their column
        for (named, c, shift) in [
            (Named::Lvidd, big, (1.2_f32, -0.5_f32)),
            (Named::Lvids, small, (0.8, -1.1)),
        ] {
            let t = c.1 as f64 * MMODE_STEP_MS;
            self.measure.points = vec![(t, c.2 + shift.0), (t, c.3 + shift.1)];
            let r = reading_of(Tool::Depth, &self.measure.points);
            let Some(r) = r else { continue };
            let reference = self
                .measure
                .mmode
                .as_ref()
                .and_then(|s| reference_on_mmode(&s.buffer, named, t, f32::midpoint(c.2, c.3)));
            self.measure.list.push(Entry {
                name: named,
                mode: MeasureMode::MMode,
                tool: Tool::Depth,
                points: self.measure.points.clone(),
                value: r.value.abs(),
                site: None,
                feedback: feedback(r.value.abs(), reference),
                axis_note: None,
            });
        }
        self.measure.points.clear();
        self.measure.name = None;
        self.measure.scrub_ms = None;
    }

    fn demo_tapse(&mut self, heart: &mut HeartView) {
        let Some(pose) = self.pose else { return };
        let cursor = self.measure.cursor_deg;
        let Some((anatomy, parts)) = heart.echo_scan_parts() else {
            return;
        };
        let frame = pose.frame(anatomy.designed.contact_limit_deg);
        let thin = heartsim_echo::cursor_frame(&frame, cursor);
        let Some(scan) = &self.measure.mmode else { return };
        let Some((_, last)) = scan.buffer.span() else {
            return;
        };
        let t_last = last as f64 * MMODE_STEP_MS;
        let Some(bt) = parts.model.beat_times(parts.history, t_last, Side::Right) else {
            return;
        };
        // the beat the window holds: its end-diastole and end-systole, or the one before
        let bt = if bt.ejection_ms.is_some_and(|(_, c)| c < t_last) {
            bt
        } else {
            match parts
                .model
                .beat_times(parts.history, bt.qrs_ms - 5.0, Side::Right)
            {
                Some(b) => b,
                None => return,
            }
        };
        let Some((_, es)) = bt.ejection_ms else { return };
        let depth_at = |model: &mut heartsim_echo::MotionModel, t: f64| -> Option<f32> {
            let input = heartsim_echo::MotionInput {
                history: parts.history,
                t_ms: t,
                resp_phase: parts.history.respiratory_phase(t),
                enabled: parts.enabled,
                exaggeration: parts.exaggeration,
            };
            model
                .annulus_position(&input, AnnulusSite::TricuspidLateral)
                .map(|p| (p - thin.apex_mm).dot(thin.beam))
        };
        let (Some(d1), Some(d2)) = (
            depth_at(parts.model, bt.end_diastole_ms),
            depth_at(parts.model, es),
        ) else {
            return;
        };
        // the learner reads the leading edge a little off
        self.measure.tool = Tool::Depth;
        self.measure.points = vec![(bt.end_diastole_ms, d1 + 0.9), (es, d2 - 0.6)];
        let name = Named::Tapse;
        let value = (self.measure.points[0].1 - self.measure.points[1].1).abs();
        let ctx = ExcursionContext {
            history: parts.history,
            frame,
            cursor_deg: cursor,
            enabled: parts.enabled,
            exaggeration: parts.exaggeration,
        };
        let ex = reference_excursion(parts.model, &ctx, name, bt.end_diastole_ms, es);
        let input = heartsim_echo::MotionInput {
            history: parts.history,
            t_ms: es + 60.0,
            resp_phase: parts.history.respiratory_phase(es + 60.0),
            enabled: parts.enabled,
            exaggeration: 1.0,
        };
        let axis = ex.and_then(|e| {
            parts
                .model
                .readout(&input)
                .tapse_mm
                .map(|v| (v as f32, e.angle_deg))
        });
        self.measure.list.push(Entry {
            name,
            mode: MeasureMode::MMode,
            tool: Tool::Depth,
            points: self.measure.points.clone(),
            value,
            site: ex.map(|e| e.site),
            feedback: feedback(value, ex.map(|e| e.along_beam_mm)),
            axis_note: axis,
        });
        self.measure.points.clear();
    }

    fn demo_e_prime(&mut self, heart: &mut HeartView) {
        let Some((_, parts)) = heart.echo_scan_parts() else {
            return;
        };
        let _ = parts;
        let Some(g) = self.measure.doppler.gate() else {
            return;
        };
        let Some(m) = self
            .measure
            .markers
            .iter()
            .rev()
            .find(|m| m.kind == WaveKind::E)
            .copied()
        else {
            return;
        };
        self.measure.tool = Tool::Velocity;
        // a hand a little off the peak, in time and in height
        let v = m.cm_s + 0.7;
        self.measure.points = vec![(m.t_ms + 10.0, v)];
        let Some(bt) = self
            .measure
            .beats
            .iter()
            .find(|b| b.qrs_ms <= m.t_ms && m.t_ms < b.next_qrs_ms.unwrap_or(b.qrs_ms + 2_000.0))
            .copied()
        else {
            return;
        };
        let marks = self.measure.doppler.markers(&bt);
        self.measure.list.push(Entry {
            name: Named::EPrime,
            mode: MeasureMode::Doppler,
            tool: Tool::Velocity,
            points: self.measure.points.clone(),
            value: v.abs(),
            site: Some(g.site),
            feedback: feedback(v.abs(), reference_velocity(Named::EPrime, &marks)),
            axis_note: None,
        });
        self.measure.points.clear();
    }
}

/// How far below the mitral annulus plane the cursor of the *mitral tips* aim goes, mm: the open
/// anterior leaflet is 25.1 mm long (`heartsim_echo::DesignedParams`) and its tip lies about a
/// centimetre and a half to two below the plane.
const MITRAL_TIP_BELOW_MM: f32 = 18.0;

/// Left margin of the trace for the depth or velocity scale, logical px.
pub const TRACE_LEFT_PX: f32 = 34.0;

/// The beats of the ventricle of `side` whose cycles touch `t0..=t1`, newest first.
fn beats_in(
    model: &heartsim_echo::MotionModel,
    history: &heartsim_echo::MotionHistory,
    t0: f64,
    t1: f64,
    side: Side,
) -> Vec<BeatTimes> {
    let mut out: Vec<BeatTimes> = Vec::new();
    let mut t = t1;
    for _ in 0..8 {
        let Some(b) = model.beat_times(history, t, side) else {
            break;
        };
        if out.last().is_some_and(|l| l.qrs_ms <= b.qrs_ms) {
            break;
        }
        t = b.qrs_ms - 5.0;
        let done = b.qrs_ms < t0 - 1_000.0;
        out.push(b);
        if done || t < 0.0 {
            break;
        }
    }
    out
}

// ---- the left column ---------------------------------------------------------------------------

impl EchoPage {
    /// The Measure section: the mode, the sweep, freeze and scrub, the calipers, the list.
    pub(super) fn measure_controls(&mut self, ui: &mut egui::Ui, heart: &mut HeartView) {
        let mut mode = self.measure.mode;
        // the mode switch has the whole column and a little less padding in its buttons, so that
        // "Tissue Doppler" is not cut
        ui.scope(|ui| {
            ui.spacing_mut().button_padding.x = 3.0;
            let options = MeasureMode::ALL.map(|m| (m, m.name()));
            if heartsim_ui::segmented(ui, &mut mode, &options) {
                self.set_measure_mode(mode);
            }
        });
        ui.add_space(4.0);
        cards::card(ui, |ui| {
            heartsim_ui::info_disclosure(
                ui,
                "echo_measure_info",
                t!("echo-measure-intro-line"),
                format!(
                    "{}\n\n{}\n\n{}",
                    t!("echo-measure-intro"),
                    t!("echo-measure-no-blood"),
                    t!("echo-measure-conventions")
                ),
            );
            ui.add_space(4.0);
            ui.add_space(4.0);
            match self.measure.mode {
                MeasureMode::Plane => {
                    heartsim_ui::helper_text(ui, t!("echo-measure-plane-help"));
                }
                MeasureMode::MMode => self.mmode_controls(ui, heart),
                MeasureMode::Doppler => self.doppler_controls(ui, heart),
            }
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                // a plain button that names what it does next (Freeze while live, Live while
                // frozen), with the state beside it: a toggle is never the primary button
                let frozen = self.measure.frozen;
                if ui
                    .button(if frozen {
                        t!("echo-measure-live")
                    } else {
                        t!("echo-measure-freeze")
                    })
                    .on_hover_text(t!("echo-measure-freeze-hint"))
                    .clicked()
                {
                    self.set_frozen(!frozen);
                }
                heartsim_ui::status_row(
                    ui,
                    frozen,
                    &if frozen {
                        t!("echo-measure-state-frozen")
                    } else {
                        t!("echo-measure-state-live")
                    },
                );
            });
            if self.measure.frozen && self.measure.mode.is_trace() {
                self.scrub_row(ui);
            }
        });
        self.caliper_card(ui, heart);
        self.list_card(ui, heart);
    }

    fn mmode_controls(&mut self, ui: &mut egui::Ui, heart: &mut HeartView) {
        heartsim_ui::helper_text(ui, t!("echo-measure-mmode-help"));
        if self.measure.half_rate {
            heartsim_ui::helper_text(ui, t!("echo-measure-half-rate"));
        }
        self.speed_row(ui);
        ui.label(t!(
            "echo-measure-cursor",
            deg = format!("{:+.1}", self.measure.cursor_deg)
        ));
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(t!("echo-aim-mitral-tips"))
                .on_hover_text(t!("echo-aim-mitral-tips-hint"))
                .clicked()
            {
                self.aim_cursor(Aim::MitralTips);
            }
            if ui.button(t!("echo-aim-tricuspid")).clicked() {
                self.aim_cursor(Aim::TricuspidAnnulus);
            }
            if ui.button(t!("echo-aim-mitral-annulus")).clicked() {
                self.aim_cursor(Aim::MitralAnnulus);
            }
        });
        let _ = heart;
    }

    fn doppler_controls(&mut self, ui: &mut egui::Ui, heart: &mut HeartView) {
        heartsim_ui::helper_text(ui, t!("echo-measure-doppler-help"));
        self.speed_row(ui);
        ui.horizontal_wrapped(|ui| {
            for site in AnnulusSite::ALL {
                let on = self.measure.doppler.gate().is_some_and(|g| g.site == site);
                if ui.add(egui::Button::selectable(on, site_name(site))).clicked() {
                    self.request_gate(site);
                }
            }
        });
        match self.measure.doppler.gate() {
            Some(g) => {
                heartsim_ui::status_row(ui, true, &t!("echo-gate-on", site = site_name(g.site)));
                match self.doppler_angle_deg() {
                    Some(deg) => {
                        heartsim_ui::helper_text(ui, t!("echo-gate-angle", deg = format!("{deg:.0}")))
                    }
                    None => heartsim_ui::helper_text(ui, t!("echo-gate-angle-note")),
                };
            }
            None => {
                heartsim_ui::status_row(ui, false, &t!("echo-gate-place"));
            }
        }
        if let Some(n) = &self.measure.gate_note {
            heartsim_ui::helper_text(ui, n.clone());
        }
        let _ = heart;
    }

    fn speed_row(&mut self, ui: &mut egui::Ui) {
        let options = SweepSpeed::ALL.map(|s| (s, format!("{:.0}", s.mm_per_s())));
        ui.label(t!("echo-measure-sweep"));
        heartsim_ui::segmented(ui, &mut self.measure.speed, &options);
        heartsim_ui::helper_text(ui, t!("echo-measure-sweep-note"));
    }

    fn scrub_row(&mut self, ui: &mut egui::Ui) {
        let span = match self.measure.mode {
            MeasureMode::MMode => self
                .measure
                .mmode
                .as_ref()
                .and_then(|m| m.buffer.span())
                .map(|(a, b)| (a as f64 * MMODE_STEP_MS, b as f64 * MMODE_STEP_MS)),
            MeasureMode::Doppler => self
                .measure
                .doppler
                .buffer
                .span()
                .map(|(a, b)| (a as f64 * DOPPLER_STEP_MS, b as f64 * DOPPLER_STEP_MS)),
            MeasureMode::Plane => None,
        };
        let Some((lo, hi)) = span.filter(|(a, b)| b > a) else {
            return;
        };
        let mut back_s = ((hi - self.measure.scrub_ms.unwrap_or(hi).min(hi)) / 1000.0) as f32;
        let range = ((hi - lo) / 1000.0) as f32;
        if heartsim_ui::slider_row(
            ui,
            &t!("echo-measure-scrub"),
            egui::Slider::new(&mut back_s, 0.0..=range)
                .suffix(format!(" {}", t!("echo-unit-s-back")))
                .custom_formatter(|v, _| format!("{v:.1}")),
        )
        .changed()
        {
            self.measure.scrub_ms = Some(hi - f64::from(back_s) * 1000.0);
        }
    }

    fn caliper_card(&mut self, ui: &mut egui::Ui, heart: &mut HeartView) {
        cards::card(ui, |ui| {
            cards::heading(ui, t!("echo-measure-calipers"));
            let tools = self.measure.mode.tools();
            let options: Vec<(Tool, String)> = tools.iter().map(|&t| (t, t.name())).collect();
            let mut tool = self.measure.tool;
            if heartsim_ui::segmented(ui, &mut tool, &options) {
                self.set_tool(tool);
            }
            heartsim_ui::helper_text(
                ui,
                match self.measure.tool {
                    Tool::Distance => t!("echo-tool-distance-help"),
                    Tool::Area => t!("echo-tool-area-help"),
                    Tool::Depth => t!("echo-tool-depth-help"),
                    Tool::Interval => t!("echo-tool-time-help"),
                    Tool::Slope => t!("echo-tool-slope-help"),
                    Tool::Velocity => t!("echo-tool-velocity-help"),
                },
            );
            if matches!(self.measure.tool, Tool::Distance | Tool::Area | Tool::Depth) {
                ui.checkbox(&mut self.measure.snap, t!("echo-measure-snap"))
                    .on_hover_text(t!("echo-measure-snap-hint"));
            }
            if self.measure.tool == Tool::Area {
                ui.horizontal_wrapped(|ui| {
                    if ui.button(t!("echo-area-undo")).clicked() {
                        self.measure.points.pop();
                    }
                });
            }
            let reading = self.reading_now();
            match reading {
                Some(r) => {
                    ui.label(
                        egui::RichText::new(t!(
                            "echo-measure-reading",
                            value = format!("{:.1}", r.value),
                            unit = r.unit
                        ))
                        .strong(),
                    );
                }
                None => {
                    heartsim_ui::helper_text(ui, t!("echo-measure-no-reading"));
                }
            }
            let names = self.measure.tool.names();
            if !names.is_empty() {
                let current = self
                    .measure
                    .name
                    .map_or_else(|| t!("echo-measure-name-choose"), name_of);
                egui::ComboBox::from_label(t!("echo-measure-name"))
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        for &n in names {
                            if ui
                                .selectable_label(self.measure.name == Some(n), name_of(n))
                                .clicked()
                            {
                                self.measure.name = Some(n);
                            }
                        }
                    });
            }
            let can_add = reading.is_some()
                && self.measure.name.is_some()
                && self.measure.list.len() < MAX_MEASUREMENTS;
            // the one primary button of the section, only while a reading is ready to add
            if can_add {
                if heartsim_ui::primary_button(ui, t!("echo-measure-add")).clicked()
                    && let Some(n) = self.measure.name
                {
                    self.add_measurement(heart, n);
                }
            } else {
                ui.add_enabled(false, egui::Button::new(t!("echo-measure-add")));
            }
            if self.measure.list.len() >= MAX_MEASUREMENTS {
                heartsim_ui::helper_text(ui, t!("echo-measure-list-full"));
            }
            right_aligned_row(ui, |ui| {
                if ui.button(t!("echo-measure-clear-calipers")).clicked() {
                    self.measure.points.clear();
                }
            });
        });
    }

    fn reading_now(&self) -> Option<Reading> {
        self.reading()
    }

    fn list_card(&mut self, ui: &mut egui::Ui, heart: &mut HeartView) {
        let _ = heart;
        cards::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                cards::heading(ui, t!("echo-measure-list"));
                cards::pill(ui, t!("echo-model-tag"));
            });
            if self.measure.list.is_empty() {
                heartsim_ui::helper_text(ui, t!("echo-measure-list-empty"));
            }
            let mut remove = None;
            let mut send = None;
            for (i, e) in self.measure.list.iter().enumerate() {
                ui.separator();
                ui.label(
                    egui::RichText::new(t!(
                        "echo-measure-row",
                        name = name_of(e.name),
                        value = format!("{:.1}", e.value),
                        unit = unit_text(e.name.unit())
                    ))
                    .strong(),
                );
                match (e.feedback.reference, e.feedback.difference) {
                    (Some(r), Some(d)) => {
                        heartsim_ui::helper_text(
                            ui,
                            t!(
                                "echo-measure-model-value",
                                value = format!("{r:.1}"),
                                unit = unit_text(e.name.unit()),
                                diff = format!("{d:+.1}")
                            ),
                        );
                    }
                    _ => {
                        heartsim_ui::helper_text(ui, t!("echo-measure-no-reference"));
                    }
                }
                if let Some((along_axis, angle)) = e.axis_note {
                    heartsim_ui::helper_text(
                        ui,
                        t!(
                            "echo-measure-axis-note",
                            axis = format!("{along_axis:.1}"),
                            angle = format!("{angle:.0}")
                        ),
                    );
                }
                ui.horizontal_wrapped(|ui| {
                    let targets = crate::echo_send::targets(e.name, e.site);
                    ui.add_enabled_ui(!targets.is_empty(), |ui| {
                        if ui
                            .button(t!("echo-measure-send"))
                            .on_hover_text(if targets.is_empty() {
                                t!("echo-measure-send-none")
                            } else {
                                t!("echo-measure-send-hint")
                            })
                            .clicked()
                        {
                            send = Some(i);
                        }
                    });
                    if ui.button(t!("echo-measure-remove")).clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = send {
                self.request_send(i);
            }
            if let Some(i) = remove {
                self.measure.list.remove(i);
            }
            if !self.measure.list.is_empty() {
                heartsim_ui::helper_text(ui, t!("echo-measure-not-stored"));
            }
        });
    }
}

// ---- the sector: the cursor, the gate, the 2-D calipers ----------------------------------------

impl EchoPage {
    /// The pointer on the sector: set the M-mode cursor, place the Doppler gate or a 2-D caliper
    /// point. `rect` is the sector's widget, `layout` its mapping.
    pub(super) fn measure_sector_input(
        &mut self,
        response: &egui::Response,
        rect: Rect,
        layout: &SectionLayout,
    ) {
        let Some(p) = response.interact_pointer_pos().or_else(|| response.hover_pos()) else {
            return;
        };
        let local = glam::Vec2::new(p.x - rect.left(), p.y - rect.top());
        let lines = self.image.lines;
        match self.measure.mode {
            MeasureMode::MMode if response.dragged() || response.clicked() => {
                if let Some((line, _, _)) = layout.locate(local, lines, self.image.samples) {
                    let sector = self.image.sector_deg.max(1.0);
                    self.set_cursor_deg(line_angle_deg(line, lines, sector));
                }
            }
            MeasureMode::Doppler if response.clicked() => {
                let d = local - layout.apex_px;
                self.measure.pending_gate = Some(Plane2::new(d.x / layout.px_per_mm, d.y / layout.px_per_mm));
            }
            MeasureMode::Plane if response.clicked() && self.image.contact => {
                let d = local - layout.apex_px;
                if d.y > 0.0 && layout.locate(local, lines, self.image.samples).is_some() {
                    let mut q = Plane2::new(d.x / layout.px_per_mm, d.y / layout.px_per_mm);
                    if self.measure.snap {
                        q = snap_plane(&self.image, q);
                    }
                    self.place_caliper_point((f64::from(q.x_mm), q.z_mm));
                }
            }
            _ => {}
        }
    }

    /// The cursor, the gate and the 2-D calipers over the sector.
    pub(super) fn paint_measure_overlay(&self, painter: &egui::Painter, rect: Rect, layout: &SectionLayout) {
        let at = |x_mm: f32, z_mm: f32| {
            rect.min
                + EVec2::new(
                    layout.apex_px.x + x_mm * layout.px_per_mm,
                    layout.apex_px.y + z_mm * layout.px_per_mm,
                )
        };
        let accent = Color32::from_rgb(255, 214, 10);
        let stroke = Stroke::new(1.5, accent);
        match self.measure.mode {
            MeasureMode::MMode => {
                let (s, c) = self.measure.cursor_deg.to_radians().sin_cos();
                let depth = layout.depth_mm;
                painter.line_segment([at(0.0, 0.0), at(s * depth, c * depth)], stroke);
                painter.text(
                    at(s * depth * 0.92, c * depth * 0.92) + EVec2::new(4.0, 0.0),
                    Align2::LEFT_CENTER,
                    "M",
                    FontId::proportional(12.0),
                    accent,
                );
            }
            MeasureMode::Doppler => {
                if let Some(g) = self.measure.doppler.gate() {
                    painter.circle_stroke(at(g.at.x_mm, g.at.z_mm), 7.0, stroke);
                    painter.line_segment(
                        [at(0.0, 0.0), at(g.at.x_mm, g.at.z_mm)],
                        Stroke::new(1.0, accent.gamma_multiply(0.7)),
                    );
                }
            }
            MeasureMode::Plane => {
                let pts: Vec<Pos2> = self.measure.points.iter().map(|p| at(p.0 as f32, p.1)).collect();
                Self::paint_points(painter, &pts, self.measure.tool == Tool::Area, accent);
                if let Some(r) = self.reading()
                    && let Some(&last) = pts.last()
                {
                    painter.text(
                        last + EVec2::new(8.0, -8.0),
                        Align2::LEFT_BOTTOM,
                        format!("{:.1} {}", r.value, r.unit),
                        FontId::proportional(12.0),
                        accent,
                    );
                }
                // committed distances stay on the frozen image
                for e in self.measure.list.iter().filter(|e| e.mode == MeasureMode::Plane) {
                    let pts: Vec<Pos2> = e.points.iter().map(|p| at(p.0 as f32, p.1)).collect();
                    Self::paint_points(painter, &pts, false, accent.gamma_multiply(0.6));
                }
            }
        }
    }

    fn paint_points(painter: &egui::Painter, pts: &[Pos2], close: bool, colour: Color32) {
        let stroke = Stroke::new(1.5, colour);
        for w in pts.windows(2) {
            painter.line_segment([w[0], w[1]], stroke);
        }
        if close && pts.len() >= 3 {
            painter.line_segment(
                [pts[pts.len() - 1], pts[0]],
                Stroke::new(1.0, colour.gamma_multiply(0.6)),
            );
        }
        for p in pts {
            painter.circle_filled(*p, 3.0, colour);
        }
    }
}

// ---- the trace under the sector ----------------------------------------------------------------

/// Where the trace's pixel column `x` (0 at the left edge of the plotted area) is in time, ms.
fn time_at(right_ms: f64, speed: SweepSpeed, width: f32, x: f32) -> f64 {
    right_ms - speed.time_ms((width - x) / PX_PER_MM)
}

fn x_at(right_ms: f64, speed: SweepSpeed, width: f32, t_ms: f64) -> f32 {
    width - speed.screen_mm(right_ms - t_ms) * PX_PER_MM
}

impl EchoPage {
    /// The trace panel under the sector: the M-mode or Doppler trace, the ECG lead in time with
    /// it, the calipers and the caption. `width` is the panel's width.
    pub(super) fn trace_pane(&mut self, ui: &mut egui::Ui) {
        if !self.measure.mode.is_trace() {
            self.measure.trace_rect = None;
            return;
        }
        let width = ui.available_width().max(120.0);
        let (trace_h, ecg_h) = if self.measure.compact {
            (TRACE_COMPACT_PX, ECG_COMPACT_PX)
        } else {
            (TRACE_PX, ECG_PX)
        };
        let (outer, response) =
            ui.allocate_exact_size(EVec2::new(width, trace_h + ecg_h), Sense::click_and_drag());
        self.measure.trace_rect = Some(outer);
        let painter = ui.painter_at(outer);
        let trace = Rect::from_min_size(
            outer.min + EVec2::new(TRACE_LEFT_PX, 0.0),
            EVec2::new(width - TRACE_LEFT_PX, trace_h),
        );
        let ecg = Rect::from_min_size(
            outer.min + EVec2::new(TRACE_LEFT_PX, trace_h),
            EVec2::new(width - TRACE_LEFT_PX, ecg_h),
        );
        painter.rect_filled(trace, 2.0, Color32::BLACK);
        let ink = Color32::from_gray(220);
        let right = self.trace_right_ms();
        let speed = self.measure.speed;
        let w = trace.width();
        match (self.measure.mode, right) {
            (MeasureMode::MMode, Some(r)) => self.paint_mmode(ui, &painter, trace, r),
            (MeasureMode::Doppler, Some(r)) => self.paint_doppler(&painter, trace, r),
            _ => {
                let msg =
                    if self.measure.mode == MeasureMode::Doppler && self.measure.doppler.gate().is_none() {
                        t!("echo-gate-place")
                    } else {
                        t!("echo-trace-waiting")
                    };
                painter.text(
                    trace.center(),
                    Align2::CENTER_CENTER,
                    msg,
                    FontId::proportional(12.0),
                    ink,
                );
            }
        }
        // the ECG lead aligned in time, and the time marks
        painter.rect_filled(ecg, 2.0, Color32::from_gray(18));
        if let Some(r) = right {
            self.paint_ecg(&painter, ecg, r, ink);
            let t0 = time_at(r, speed, w, 0.0);
            let mut tick = (t0 / 200.0).ceil() * 200.0;
            while tick <= r {
                let x = trace.left() + x_at(r, speed, w, tick);
                let major =
                    (tick / 1000.0).fract().abs() < 1e-6 || ((tick / 1000.0).fract() - 1.0).abs() < 1e-6;
                painter.line_segment(
                    [
                        Pos2::new(x, ecg.bottom() - if major { 8.0 } else { 4.0 }),
                        Pos2::new(x, ecg.bottom()),
                    ],
                    Stroke::new(1.0, ink.gamma_multiply(0.8)),
                );
                tick += 200.0;
            }
        }
        // the scale on the left
        match self.measure.mode {
            MeasureMode::MMode => {
                let depth = self.settings.depth_mm;
                for cm in (0..=((depth / 10.0) as i32)).step_by(2) {
                    let y = trace.top() + cm as f32 * 10.0 / depth * trace.height();
                    if y > trace.bottom() - 22.0 {
                        continue;
                    }
                    painter.text(
                        Pos2::new(trace.left() - 4.0, y.max(trace.top() + 5.0)),
                        Align2::RIGHT_CENTER,
                        format!("{cm}"),
                        FontId::proportional(10.0),
                        ink,
                    );
                }
                painter.text(
                    Pos2::new(outer.left() + 2.0, trace.bottom() - 8.0),
                    Align2::LEFT_CENTER,
                    t!("echo-unit-cm"),
                    FontId::proportional(10.0),
                    ink,
                );
            }
            MeasureMode::Doppler => {
                for v in [-20.0, -10.0, 0.0, 10.0, 20.0] {
                    let y = trace.center().y - v / VELOCITY_RANGE_CM_S * (trace.height() * 0.5);
                    painter.text(
                        Pos2::new(trace.left() - 4.0, y),
                        Align2::RIGHT_CENTER,
                        format!("{v:.0}"),
                        FontId::proportional(10.0),
                        ink,
                    );
                }
                painter.text(
                    Pos2::new(outer.left() + 2.0, trace.top() + 6.0),
                    Align2::LEFT_CENTER,
                    t!("echo-unit-cm-s"),
                    FontId::proportional(10.0),
                    ink,
                );
            }
            MeasureMode::Plane => {}
        }
        painter.text(
            Pos2::new(outer.left() + 2.0, ecg.center().y),
            Align2::LEFT_CENTER,
            self.measure
                .ecg
                .as_ref()
                .map_or(String::new(), |e| e.lead.clone()),
            FontId::proportional(10.0),
            ink,
        );
        // the label: it is the model
        if self.measure.mode == MeasureMode::Doppler
            && let Some(deg) = self.doppler_angle_deg()
        {
            painter.text(
                trace.right_bottom() + EVec2::new(-4.0, -3.0),
                Align2::RIGHT_BOTTOM,
                t!("echo-trace-angle", deg = format!("{deg:.0}")),
                FontId::proportional(10.0),
                Color32::from_rgb(150, 200, 240),
            );
        }
        painter.text(
            trace.right_top() + EVec2::new(-4.0, 3.0),
            Align2::RIGHT_TOP,
            t!("echo-model-tag"),
            FontId::proportional(11.0),
            Color32::from_rgb(120, 220, 230),
        );
        self.paint_trace_calipers(&painter, trace, right);
        self.trace_pointer(&response, trace, right);
    }

    fn paint_mmode(&mut self, ui: &egui::Ui, painter: &egui::Painter, trace: Rect, right: f64) {
        let Some(scan) = &self.measure.mmode else { return };
        let w = trace.width().max(1.0);
        let speed = self.measure.speed;
        let depth = self.settings.depth_mm.max(1.0);
        // one texel per recorded column over the steps in view (and one more each side); the
        // texture filter blends them, so a slow sweep is smooth instead of blocky
        let t_left = time_at(right, speed, w, 0.0);
        let s0 = scan.buffer.step_at(t_left) - 1;
        let s1 = scan.buffer.step_at(right) + 1;
        let cols = usize::try_from(s1 - s0 + 1).unwrap_or(1).max(2);
        let rows = 384usize;
        let key = (
            s0,
            cols,
            rows,
            depth.to_bits(),
            self.measure.computed_total + scan.buffer.last_step().unwrap_or(0) as u64,
            u32::from(self.measure.mmode.is_some()),
        );
        if self.measure.texture_key != Some(key) || self.measure.texture.is_none() {
            let mut grey = vec![0u8; cols * rows];
            for x in 0..cols {
                let step = s0 + i64::try_from(x).unwrap_or(0);
                // a gap (a column skipped when the trace fell behind) takes its neighbour's
                let Some(col) = scan.buffer.nearest(step, 2).filter(|c| c.contact) else {
                    continue;
                };
                let n = col.grey.len();
                for y in 0..rows {
                    let d = (y as f32 + 0.5) / rows as f32 * depth;
                    if d >= col.depth_mm {
                        continue;
                    }
                    let smp = ((d / col.depth_mm * n as f32) as usize).min(n - 1);
                    grey[y * cols + x] = col.grey[smp];
                }
            }
            let img = egui::ColorImage::from_gray([cols, rows], &grey);
            match &mut self.measure.texture {
                Some(tex) => tex.0.set(img, egui::TextureOptions::LINEAR),
                None => {
                    self.measure.texture = Some(Tex(ui.ctx().load_texture(
                        "echo_mmode",
                        img,
                        egui::TextureOptions::LINEAR,
                    )));
                }
            }
            self.measure.texture_key = Some(key);
        }
        if let Some(tex) = &self.measure.texture {
            // texel `i` is the column of step `s0 + i`, centred at `(i + 0.5) / cols`
            let u = |t_ms: f64| (((t_ms / MMODE_STEP_MS) - s0 as f64 + 0.5) / cols as f64) as f32;
            painter.image(
                tex.0.id(),
                trace,
                Rect::from_min_max(Pos2::new(u(t_left), 0.0), Pos2::new(u(right), 1.0)),
                Color32::WHITE,
            );
        }
    }

    fn paint_doppler(&self, painter: &egui::Painter, trace: Rect, right: f64) {
        let speed = self.measure.speed;
        let w = trace.width();
        let mid = trace.center().y;
        let scale = trace.height() * 0.5 / VELOCITY_RANGE_CM_S;
        painter.line_segment(
            [Pos2::new(trace.left(), mid), Pos2::new(trace.right(), mid)],
            Stroke::new(1.0, Color32::from_gray(90)),
        );
        let t0 = time_at(right, speed, w, 0.0);
        let series = self.measure.doppler.series(t0, right);
        // the designed band: a thin spectral spread about the curve, for the look
        let band_col = Color32::from_rgba_unmultiplied(120, 200, 255, 70);
        for pair in series.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let (xa, xb) = (
                trace.left() + x_at(right, speed, w, a.0),
                trace.left() + x_at(right, speed, w, b.0),
            );
            let half = |v: f32| BAND_FLOOR_CM_S + BAND_SHARE * v.abs();
            let top = |t: (f64, f32)| mid - (t.1 + half(t.1)) * scale;
            let bot = |t: (f64, f32)| mid - (t.1 - half(t.1)) * scale;
            let quad = vec![
                Pos2::new(xa, top(a)),
                Pos2::new(xb, top(b)),
                Pos2::new(xb, bot(b)),
                Pos2::new(xa, bot(a)),
            ];
            painter.add(egui::Shape::convex_polygon(quad, band_col, Stroke::NONE));
        }
        let pts: Vec<Pos2> = series
            .iter()
            .map(|&(t, v)| Pos2::new(trace.left() + x_at(right, speed, w, t), mid - v * scale))
            .collect();
        painter.add(egui::Shape::line(
            pts,
            Stroke::new(1.6, Color32::from_rgb(240, 240, 120)),
        ));
        // S', e' and a' on the beats in view
        for m in &self.measure.markers {
            let x = trace.left() + x_at(right, speed, w, m.t_ms);
            if x < trace.left() || x > trace.right() {
                continue;
            }
            let y = mid - m.cm_s * scale;
            painter.circle_filled(Pos2::new(x, y), 3.0, Color32::from_rgb(255, 120, 80));
            let up = m.cm_s >= 0.0;
            painter.text(
                Pos2::new(x, y + if up { -6.0 } else { 6.0 }),
                if up {
                    Align2::CENTER_BOTTOM
                } else {
                    Align2::CENTER_TOP
                },
                format!("{} {:.1}", m.kind.symbol(), m.cm_s),
                FontId::proportional(10.5),
                Color32::from_rgb(255, 170, 130),
            );
        }
        painter.text(
            trace.left_bottom() + EVec2::new(4.0, -3.0),
            Align2::LEFT_BOTTOM,
            t!("echo-band-designed"),
            FontId::proportional(10.0),
            Color32::from_rgb(150, 200, 240),
        );
    }

    fn paint_ecg(&self, painter: &egui::Painter, rect: Rect, right: f64, ink: Color32) {
        let Some(ecg) = &self.measure.ecg else { return };
        if ecg.samples.is_empty() || ecg.dt_ms <= 0.0 {
            return;
        }
        let speed = self.measure.speed;
        let w = rect.width();
        let t0 = time_at(right, speed, w, 0.0);
        let first = (((t0 - ecg.first_ms) / ecg.dt_ms).floor().max(0.0)) as usize;
        let last = ((((right - ecg.first_ms) / ecg.dt_ms).ceil()).max(0.0) as usize)
            .min(ecg.samples.len().saturating_sub(1));
        if first >= last {
            return;
        }
        let slice = &ecg.samples[first..=last];
        let peak = slice.iter().fold(0.2_f32, |m, v| m.max(v.abs()));
        let scale = (rect.height() * 0.42) / peak;
        let pts: Vec<Pos2> = slice
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let t = ecg.first_ms + (first + i) as f64 * ecg.dt_ms;
                Pos2::new(
                    rect.left() + x_at(right, speed, w, t),
                    rect.center().y - v * scale,
                )
            })
            .filter(|p| p.x >= rect.left() && p.x <= rect.right())
            .collect();
        painter.add(egui::Shape::line(pts, Stroke::new(1.2, ink)));
    }

    fn paint_trace_calipers(&self, painter: &egui::Painter, trace: Rect, right: Option<f64>) {
        let Some(right) = right else { return };
        let speed = self.measure.speed;
        let w = trace.width();
        let accent = Color32::from_rgb(255, 214, 10);
        let to_screen = |p: &Pt| -> Pos2 {
            let x = trace.left() + x_at(right, speed, w, p.0);
            let y = match self.measure.mode {
                MeasureMode::Doppler => trace.center().y - p.1 / VELOCITY_RANGE_CM_S * trace.height() * 0.5,
                _ => trace.top() + p.1 / self.settings.depth_mm.max(1.0) * trace.height(),
            };
            Pos2::new(x, y)
        };
        let draw = |pts: &[Pt], colour: Color32| {
            let s: Vec<Pos2> = pts.iter().map(&to_screen).collect();
            for p in &s {
                if trace.expand(4.0).contains(*p) {
                    painter.circle_filled(*p, 3.0, colour);
                }
            }
            if let [a, b] = s[..] {
                painter.line_segment([a, b], Stroke::new(1.5, colour));
            }
            if let [a] = s[..] {
                painter.line_segment(
                    [Pos2::new(trace.left(), a.y), Pos2::new(trace.right(), a.y)],
                    Stroke::new(1.0, colour.gamma_multiply(0.6)),
                );
            }
        };
        draw(&self.measure.points, accent);
        for e in self.measure.list.iter().filter(|e| e.mode == self.measure.mode) {
            draw(&e.points, accent.gamma_multiply(0.55));
        }
        if let Some(r) = self.reading() {
            let s: Vec<Pos2> = self.measure.points.iter().map(&to_screen).collect();
            if let Some(&p) = s.last() {
                painter.text(
                    p + EVec2::new(8.0, -8.0),
                    Align2::LEFT_BOTTOM,
                    format!("{:.1} {}", r.value, r.unit),
                    FontId::proportional(11.0),
                    accent,
                );
            }
        }
    }

    fn trace_pointer(&mut self, response: &egui::Response, trace: Rect, right: Option<f64>) {
        let Some(right) = right else { return };
        if !response.clicked() {
            return;
        }
        let Some(p) = response.interact_pointer_pos() else {
            return;
        };
        if !trace.contains(p) {
            return;
        }
        let speed = self.measure.speed;
        let t = time_at(right, speed, trace.width(), p.x - trace.left());
        let v = match self.measure.mode {
            MeasureMode::Doppler => (trace.center().y - p.y) / (trace.height() * 0.5) * VELOCITY_RANGE_CM_S,
            _ => (p.y - trace.top()) / trace.height() * self.settings.depth_mm,
        };
        let v = if self.measure.snap && self.measure.mode == MeasureMode::MMode {
            self.measure
                .mmode
                .as_ref()
                .and_then(|m| m.buffer.nearest(m.buffer.step_at(t + MMODE_STEP_MS * 0.5), 2))
                .map_or(v, |col| snap_depth(col, v))
        } else {
            v
        };
        // a point near one already placed moves it, otherwise it adds
        let to_screen = |q: &Pt| {
            Pos2::new(
                trace.left() + x_at(right, speed, trace.width(), q.0),
                match self.measure.mode {
                    MeasureMode::Doppler => {
                        trace.center().y - q.1 / VELOCITY_RANGE_CM_S * trace.height() * 0.5
                    }
                    _ => trace.top() + q.1 / self.settings.depth_mm.max(1.0) * trace.height(),
                },
            )
        };
        let grab = self
            .measure
            .points
            .iter()
            .position(|q| to_screen(q).distance(p) <= GRAB_PX);
        if self.measure.tool == Tool::Area || self.measure.tool == Tool::Distance {
            return;
        }
        self.measure.frozen = true;
        if self.measure.scrub_ms.is_none() {
            self.measure.scrub_ms = Some(right);
        }
        match grab {
            Some(i) => self.measure.points[i] = (t, v),
            None => self.place_caliper_point((t, v)),
        }
    }
}

/// How far a point may move to snap to a tissue boundary, mm.
pub const SNAP_MM: f32 = 3.0;

/// The depth of the tissue boundary nearest `depth_mm` in an M-mode column within [`SNAP_MM`]
/// (a change of label along the line), or `depth_mm` itself where there is none.
#[must_use]
pub fn snap_depth(col: &heartsim_echo::MModeColumn, depth_mm: f32) -> f32 {
    let n = col.labels.len();
    if n < 2 || col.depth_mm <= 0.0 {
        return depth_mm;
    }
    let step = col.depth_mm / n as f32;
    let mut best: Option<f32> = None;
    for i in 1..n {
        if col.labels[i] != col.labels[i - 1] {
            let d = i as f32 * step;
            if (d - depth_mm).abs() <= SNAP_MM
                && best.is_none_or(|b| (d - depth_mm).abs() < (b - depth_mm).abs())
            {
                best = Some(d);
            }
        }
    }
    best.unwrap_or(depth_mm)
}

/// The point of the sector nearest `p` on the same beam (the line from the apex through it) where
/// the tissue label changes, within [`SNAP_MM`]; `p` itself where there is none.
#[must_use]
pub fn snap_plane(image: &heartsim_echo::LabelImage, p: Plane2) -> Plane2 {
    let r = p.x_mm.hypot(p.z_mm);
    if r < 1.0 {
        return p;
    }
    let u = (p.x_mm / r, p.z_mm / r);
    let at = |d: f32| Plane2::new(u.0 * d, u.1 * d);
    let label = |d: f32| heartsim_echo::measure::label_at_plane(image, at(d)).map(|l| l.0);
    let mut best: Option<f32> = None;
    let mut d = r - SNAP_MM;
    while d < r + SNAP_MM {
        let next = d + 0.5;
        if label(d) != label(next) && label(d).is_some() && label(next).is_some() {
            let edge = d + 0.25;
            if best.is_none_or(|b| (edge - r).abs() < (b - r).abs()) {
                best = Some(edge);
            }
        }
        d = next;
    }
    best.map_or(p, at)
}

/// The scale of a trace for the arithmetic of the calipers (the page's own conversions, for tests).
#[must_use]
pub fn trace_scale(speed: SweepSpeed, depth_mm: f32) -> TraceScale {
    TraceScale {
        speed,
        depth_mm,
        height_mm: TRACE_PX / PX_PER_MM,
    }
}
