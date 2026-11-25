//! M-mode (brief B34, echo simulator phase 5): one scan line of the same cross-section, sampled
//! over time into a column buffer.
//!
//! **One line, not one image.** A column is the cursor's own scan line of the label slice (what
//! the sector holds at that angle) and of the grey image formed from it, at a fixed time step
//! ([`MMODE_STEP_MS`]) that does not depend on the frame rate: column `k` is the heart at
//! `k · MMODE_STEP_MS`, whoever asks and however often. The line is cut through a thin sector of
//! [`MMODE_LINES`] lines about the cursor, with the angular spacing of the image the cursor sits
//! in, so that the lateral point-spread function and the interface normals see the neighbours
//! they would see in the sector: the middle line of that thin sector is, sample for sample, the
//! sector's line when the cursor is on one (`tests/mmode.rs`). No whole image is recomputed per
//! column.
//!
//! **What a column costs.** The heart has to be evaluated at the column's time (one
//! [`MotionModel::evaluate`] and one [`SliceState::update`]; the sector's own frame cannot serve,
//! it is at another time), then one thin slice and the grey image of 17 lines. A column is
//! therefore several times the cost of a line, not of an image; [`MModeScan::advance`] takes a
//! budget of columns per call and, when the display falls too far behind the clock, skips ahead
//! and leaves a gap rather than stall. Both are documented limits of the page, not of the model.
//!
//! **What it shows.** Exactly what the model's motion does at the cursor: no template, no
//! recorded trace. The speckle rides with the tissue (it is hashed from rest positions), so the
//! wall moves with its own texture.
//!
//! Conventions and values for what is read from it are in [`crate::measure`].

use crate::acoustic::AcousticTable;
use crate::anatomy::EchoAnatomy;
use crate::bmode::{EchoImager, GreyImage, ImageSettings};
use crate::history::MotionHistory;
use crate::model::{MotionFrame, MotionInput, MotionModel};
use crate::probe::{ProbeFrame, ProbePose};
use crate::slice::{LabelImage, SliceState};
use std::collections::VecDeque;
use std::sync::Arc;

/// Time between two columns, ms. Designed: 25 ms resolves the phases of a beat (isovolumic
/// periods are 50 ms and more in the model), keeps the cost of a second of trace at 40 evaluations
/// of the heart, and is drawn smoothed. A scanner's M-mode is far finer; a measurement of a time
/// on this trace is good to about this step.
pub const MMODE_STEP_MS: f64 = 25.0;

/// Lines of the thin sector one column is cut through: the lateral point-spread function reaches
/// 8 lines each side ([`crate::bmode`]), so the middle line sees all of its kernel.
pub const MMODE_LINES: usize = 17;

/// Columns kept (12 s at [`MMODE_STEP_MS`]).
pub const MMODE_CAPACITY: usize = 480;

/// Columns the display may fall behind the clock before [`MModeScan::advance`] computes only every
/// other one (and twice that before it skips ahead).
pub const MMODE_MAX_LAG_STEPS: i64 = 16;

/// The sweep speed of an M-mode or Doppler trace, mm of screen per second. 100 mm/s is the
/// default the guideline recommends for measurements of time ("rapid sweep speeds of 100 to
/// 200 mm/sec", Mitchell et al. 2019, p. 18); the slower ones show several beats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SweepSpeed {
    Mm25,
    Mm50,
    #[default]
    Mm100,
}

impl SweepSpeed {
    /// Every speed, slowest first.
    pub const ALL: [Self; 3] = [Self::Mm25, Self::Mm50, Self::Mm100];

    /// Millimetres of screen per second.
    #[must_use]
    pub fn mm_per_s(self) -> f32 {
        match self {
            Self::Mm25 => 25.0,
            Self::Mm50 => 50.0,
            Self::Mm100 => 100.0,
        }
    }

    /// The time `screen_mm` of trace represents, ms.
    #[must_use]
    pub fn time_ms(self, screen_mm: f32) -> f64 {
        f64::from(screen_mm) / f64::from(self.mm_per_s()) * 1000.0
    }

    /// The screen length of `time_ms` of trace, mm.
    #[must_use]
    pub fn screen_mm(self, time_ms: f64) -> f32 {
        (time_ms / 1000.0 * f64::from(self.mm_per_s())) as f32
    }
}

/// A buffer of items at a fixed time step: item `k` is the state at `k · step_ms`. Holds the
/// newest `capacity` steps; a step that was skipped is a gap (`None`).
#[derive(Debug, Clone)]
pub struct StepBuffer<T> {
    step_ms: f64,
    capacity: usize,
    /// Step index of `items[0]`.
    first: i64,
    items: VecDeque<Option<T>>,
}

impl<T> StepBuffer<T> {
    /// An empty buffer of `step_ms` and `capacity` steps.
    #[must_use]
    pub fn new(step_ms: f64, capacity: usize) -> Self {
        Self {
            step_ms,
            capacity: capacity.max(2),
            first: 0,
            items: VecDeque::new(),
        }
    }

    /// Empty the buffer.
    pub fn clear(&mut self) {
        self.items.clear();
        self.first = 0;
    }

    /// Whether nothing has been pushed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Time of a step, ms.
    #[must_use]
    pub fn time_of(&self, step: i64) -> f64 {
        step as f64 * self.step_ms
    }

    /// The step that holds `t_ms` (the latest step at or before it).
    #[must_use]
    pub fn step_at(&self, t_ms: f64) -> i64 {
        (t_ms / self.step_ms).floor() as i64
    }

    /// The step length, ms.
    #[must_use]
    pub fn step_ms(&self) -> f64 {
        self.step_ms
    }

    fn len_i64(&self) -> i64 {
        i64::try_from(self.items.len()).unwrap_or(i64::MAX)
    }

    /// The oldest and newest step held (`None` when empty).
    #[must_use]
    pub fn span(&self) -> Option<(i64, i64)> {
        (!self.items.is_empty()).then(|| (self.first, self.first + self.len_i64() - 1))
    }

    /// The newest step held.
    #[must_use]
    pub fn last_step(&self) -> Option<i64> {
        self.span().map(|s| s.1)
    }

    /// The item at `step`, if it is held and was computed.
    #[must_use]
    pub fn get(&self, step: i64) -> Option<&T> {
        let k = usize::try_from(step - self.first).ok()?;
        self.items.get(k)?.as_ref()
    }

    /// The item of `step`, or of the nearest held step within `reach` steps of it (the earlier of
    /// two equally near): what a trace shows where a column was not taken.
    #[must_use]
    pub fn nearest(&self, step: i64, reach: i64) -> Option<&T> {
        (0..=reach.max(0))
            .flat_map(|d| [step - d, step + d])
            .find_map(|k| self.get(k))
    }

    /// Append `item` as `step`, which must be after the newest step; skipped steps become gaps.
    /// A step at or before the newest is ignored.
    pub fn push(&mut self, step: i64, item: T) {
        if self.items.is_empty() {
            self.first = step;
        } else if step < self.first + self.len_i64() {
            return;
        }
        while self.first + self.len_i64() < step {
            self.items.push_back(None);
        }
        self.items.push_back(Some(item));
        while self.items.len() > self.capacity {
            self.items.pop_front();
            self.first += 1;
        }
    }

    /// The steps to compute to bring the buffer up to `until_ms`: at most `max` of them, oldest
    /// first. A buffer that is empty starts at `until_ms`; one whose newest step is after
    /// `until_ms` (the clock went back) is emptied and starts again. A buffer more than `max_lag`
    /// steps behind computes only the even steps (the display fills the odd ones from their
    /// neighbours), and one more than twice that behind skips ahead, leaving a gap. Deterministic
    /// in the buffer and `until_ms`; a buffer that keeps up computes every step.
    pub fn plan(&mut self, until_ms: f64, max: usize, max_lag: i64) -> Vec<i64> {
        self.plan_every(until_ms, max, max_lag, 1)
    }

    /// [`Self::plan`] taking only the steps that are multiples of `every` (1: every step) while
    /// the buffer keeps up; a buffer that falls behind thins out to every other step as `plan`
    /// does, never further than `every`. The steps it does compute are the ones `plan` would
    /// compute for them: `every` changes which instants are recorded, not what is recorded.
    pub fn plan_every(&mut self, until_ms: f64, max: usize, max_lag: i64, every: i64) -> Vec<i64> {
        let every = every.max(1);
        let target = self.step_at(until_ms);
        let (next, stride) = match self.last_step() {
            None => (target, every),
            Some(last) if last > target => {
                self.clear();
                (target, 1)
            }
            Some(last) => {
                let lag = target - last;
                if lag <= max_lag {
                    (last + 1, every)
                } else {
                    ((last + 1).max(target - 2 * max_lag + 1), every.max(2))
                }
            }
        };
        if next > target {
            return Vec::new();
        }
        (next..=target)
            .filter(|k| k.rem_euclid(stride) == 0)
            .take(max)
            .collect()
    }
}

/// How often an M-mode trace takes a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TraceRate {
    /// Every step, [`MMODE_STEP_MS`] apart.
    #[default]
    Full,
    /// Every other step: half the cost, and a time read from the trace is good to two steps.
    /// The columns it takes are bit for bit the ones `Full` takes at those steps.
    Half,
}

impl TraceRate {
    /// Steps from one recorded column to the next.
    #[must_use]
    pub fn every(self) -> i64 {
        match self {
            Self::Full => 1,
            Self::Half => 2,
        }
    }

    /// Time from one recorded column to the next, ms.
    #[must_use]
    pub fn column_ms(self) -> f64 {
        self.every() as f64 * MMODE_STEP_MS
    }
}

/// One column of an M-mode trace: the cursor's line at one instant.
#[derive(Debug, Clone, PartialEq)]
pub struct MModeColumn {
    /// The step it is the column of, and its time (`step · MMODE_STEP_MS`), ms.
    pub step: i64,
    pub t_ms: f64,
    /// Whether the probe had contact (otherwise the column is black and every label is outside).
    pub contact: bool,
    /// Depth of the line's last sample, mm.
    pub depth_mm: f32,
    /// The grey level of every sample along the line, skin first.
    pub grey: Vec<u8>,
    /// The tissue label of every sample ([`heartsim_geometry::Label`] as a byte).
    pub labels: Vec<u8>,
    /// Index of the first sample behind bone or lung (`labels.len()` when none).
    pub hidden_from: u16,
}

impl MModeColumn {
    /// Depth of sample `s` (its centre), mm.
    #[must_use]
    pub fn depth_of(&self, s: usize) -> f32 {
        (s as f32 + 0.5) / self.labels.len().max(1) as f32 * self.depth_mm
    }
}

/// The probe frame of the cursor's own thin sector: the beam along the cursor (`cursor_deg` from
/// the sector's central beam, positive toward the index marker), the marker axis turned with it,
/// the image plane unchanged.
#[must_use]
pub fn cursor_frame(frame: &ProbeFrame, cursor_deg: f32) -> ProbeFrame {
    let (s, c) = cursor_deg.to_radians().sin_cos();
    ProbeFrame {
        beam: frame.beam * c + frame.marker * s,
        marker: frame.marker * c - frame.beam * s,
        ..*frame
    }
}

/// The cursor angle of image line `line` of `lines` in a sector of `sector_deg` (the line's own
/// centre), degrees: a cursor at exactly this angle reads the sector's line.
#[must_use]
pub fn line_angle_deg(line: usize, lines: usize, sector_deg: f32) -> f32 {
    ((line as f32 + 0.5) / lines.max(1) as f32 - 0.5) * sector_deg
}

/// Cuts and images the cursor's line. Holds the scratch of one thin sector.
#[derive(Debug, Clone)]
pub struct MModeSampler {
    imager: EchoImager,
    labels: LabelImage,
    grey: GreyImage,
}

impl MModeSampler {
    /// A sampler on `table`.
    #[must_use]
    pub fn new(table: Arc<AcousticTable>) -> Self {
        Self {
            imager: EchoImager::new(table),
            labels: LabelImage::new(MMODE_LINES, 1),
            grey: GreyImage::new(MMODE_LINES, 1),
        }
    }

    /// The column at `step` for the cursor `cursor_deg` of the sector whose probe frame is
    /// `frame`. `settings` are the image's (their depth, sector width, line count and samples
    /// are the sector's: the thin sector takes the same angular and radial spacing); `state` is
    /// the slice state of the column's instant ([`SliceState::update`]).
    pub fn column(
        &mut self,
        anatomy: &EchoAnatomy,
        state: &SliceState,
        frame: &ProbeFrame,
        cursor_deg: f32,
        settings: &ImageSettings,
        step: i64,
    ) -> MModeColumn {
        let s = settings.clamped();
        let spacing = s.sector_deg / s.lines as f32;
        let thin = cursor_frame(frame, cursor_deg);
        let sector = spacing * MMODE_LINES as f32;
        if self.labels.samples != s.samples {
            self.labels = LabelImage::new(MMODE_LINES, s.samples);
        }
        state.slice_frame(anatomy, &thin, s.depth_mm, sector, &mut self.labels);
        self.imager.image_with(
            &self.labels,
            &thin,
            &s,
            |rest, heart| state.rest_label(anatomy, rest, heart) as u8,
            &mut self.grey,
        );
        let mid = MMODE_LINES / 2;
        let n = self.labels.samples;
        MModeColumn {
            step,
            t_ms: step as f64 * MMODE_STEP_MS,
            contact: self.labels.contact,
            depth_mm: s.depth_mm,
            grey: self.grey.pixels[mid * n..(mid + 1) * n].to_vec(),
            labels: self.labels.labels[mid * n..(mid + 1) * n].to_vec(),
            hidden_from: self.labels.hidden_from[mid],
        }
    }
}

/// What a scan needs to make a column, other than the model (which it evaluates).
#[derive(Debug, Clone, Copy)]
pub struct ScanContext<'a> {
    pub anatomy: &'a EchoAnatomy,
    pub history: &'a MotionHistory,
    /// The probe as the sector shows it (depth and sector width as imaged).
    pub pose: ProbePose,
    /// The cursor, degrees from the sector's central beam, positive toward the marker.
    pub cursor_deg: f32,
    /// The image settings (their depth, sector, lines and samples are the sector's).
    pub settings: &'a ImageSettings,
    /// Wall motion on (`false`: the rest anatomy, the leaflets still moving) and the page's
    /// exaggeration, so that the trace and the sector agree.
    pub enabled: bool,
    pub exaggeration: f32,
}

/// An M-mode trace being recorded: the column buffer and the scratch to fill it.
#[derive(Debug, Clone)]
pub struct MModeScan {
    /// The columns.
    pub buffer: StepBuffer<MModeColumn>,
    sampler: MModeSampler,
    frame: MotionFrame,
    state: SliceState,
    /// Columns computed since creation (tests, the cost report).
    pub computed: u64,
    /// How often a column is taken (every step by default).
    pub rate: TraceRate,
}

impl MModeScan {
    /// A scan for `anatomy`; `model` gives the frame buffers.
    #[must_use]
    pub fn new(anatomy: &EchoAnatomy, model: &MotionModel, table: Arc<AcousticTable>) -> Self {
        Self {
            buffer: StepBuffer::new(MMODE_STEP_MS, MMODE_CAPACITY),
            sampler: MModeSampler::new(table),
            frame: model.new_frame(),
            state: SliceState::new(anatomy),
            computed: 0,
            rate: TraceRate::Full,
        }
    }

    /// Bring the trace up to `until_ms` (see [`StepBuffer::plan`]), computing at most `max`
    /// columns; returns how many it computed. Column `k` is a pure function of the record, the
    /// pose, the cursor and `k`: two scans over the same record with different `until_ms`
    /// sequences hold the same columns wherever both computed them.
    pub fn advance(
        &mut self,
        model: &mut MotionModel,
        ctx: &ScanContext<'_>,
        until_ms: f64,
        max: usize,
    ) -> usize {
        let steps = self
            .buffer
            .plan_every(until_ms, max, MMODE_MAX_LAG_STEPS, self.rate.every());
        let pose = ctx.pose.clamped();
        let frame = pose.frame(ctx.anatomy.designed.contact_limit_deg);
        for &k in &steps {
            let t_ms = k as f64 * MMODE_STEP_MS;
            let input = MotionInput {
                history: ctx.history,
                t_ms,
                resp_phase: ctx.history.respiratory_phase(t_ms),
                enabled: ctx.enabled,
                exaggeration: ctx.exaggeration,
            };
            model.evaluate_positions(&input, &mut self.frame);
            self.state.update(ctx.anatomy, model, &self.frame, &input);
            let col = self
                .sampler
                .column(ctx.anatomy, &self.state, &frame, ctx.cursor_deg, ctx.settings, k);
            self.buffer.push(k, col);
            self.computed += 1;
        }
        steps.len()
    }

    /// Forget the trace (a new scenario, a new cursor the learner wants to start afresh).
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}
