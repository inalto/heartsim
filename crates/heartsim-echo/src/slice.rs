//! The anatomical cross-section (brief B31, item 7): a polar label image of the plane the probe
//! cuts through the beating heart.
//!
//! Each sample point `x` of the sector is taken back to the rest anatomy — through the
//! displacement field inside the heart box ([`crate::field`]), by the thorax's respiratory shift
//! outside it — and labelled: the heart box first (with the designed leaflets, papillary muscles
//! and moderator band over its blood, and the pericardium), else the thorax box, else outside.
//! Air between the probe face and the thorax's skin within [`DesignedParams::skin_gap_mm`] is drawn
//! as soft tissue (the figure's skin is not the KCL skin).
//!
//! **Occlusion is geometric, not acoustic**: on each line, every sample after the first bone or
//! lung sample is marked hidden. Phase 3 replaces it with an acoustic line model.
//!
//! The image is a pure function of (motion frame, motion record, time, respiration, pose): the
//! same scenario, seed, pose and time give the same bytes, whatever the sampling cadence.

use crate::anatomy::{EchoAnatomy, Posed};
use crate::designed::{DesignedParams, valve_openings};
use crate::field::DisplacementField;
use crate::model::{MotionFrame, MotionInput, MotionModel};
use crate::probe::{ProbeFrame, ProbePose};
use glam::Vec3;
use heartsim_geometry::Label;

/// Default number of image lines (brief: 128).
pub const DEFAULT_LINES: usize = 128;
/// Default samples per line (brief: 512).
pub const DEFAULT_SAMPLES: usize = 512;
/// The inverse map is solved at every this many samples of a line and interpolated between
/// (see [`SliceState::line_rests`]).
pub const KNOT_STRIDE: usize = 4;
/// Bit set in [`LabelImage::texel`] for a hidden sample.
pub const HIDDEN_BIT: u8 = 0x80;

/// A polar label image: `lines × samples`, line-major (`labels[line * samples + sample]`), each
/// byte a [`Label`] value. Line 0 is the side away from the index marker, the last line the marker
/// side; sample 0 is at the skin, the last at the full depth.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelImage {
    pub lines: usize,
    pub samples: usize,
    pub labels: Vec<u8>,
    /// Per line, the index of the first hidden sample (`samples` when nothing is hidden).
    pub hidden_from: Vec<u16>,
    /// Whether the probe had contact; without it every label is [`Label::Outside`].
    pub contact: bool,
    /// Depth and sector width of the pose the image was made with (clamped), mm and degrees.
    pub depth_mm: f32,
    pub sector_deg: f32,
    /// Per sample (same layout as `labels`), the rest position its label was read at, mm: the
    /// inverse-mapped point for a heart-box label, the point moved back by the thorax's
    /// respiratory shift otherwise. The grey image hashes it for speckle (B32), so the speckle
    /// rides with the tissue. Unspecified without contact.
    pub rest_mm: Vec<Vec3>,
    /// Per sample, whether its label came from the heart box (and `rest_mm` from the inverse
    /// map) rather than the thorax box.
    pub from_heart: Vec<bool>,
}

impl LabelImage {
    /// An empty image of `lines × samples` (each at least 1, `samples` at most 65 535).
    #[must_use]
    pub fn new(lines: usize, samples: usize) -> Self {
        let lines = lines.max(1);
        let samples = samples.clamp(1, usize::from(u16::MAX));
        Self {
            lines,
            samples,
            labels: vec![Label::Outside as u8; lines * samples],
            hidden_from: vec![samples as u16; lines],
            contact: false,
            depth_mm: 0.0,
            sector_deg: 0.0,
            rest_mm: vec![Vec3::ZERO; lines * samples],
            from_heart: vec![false; lines * samples],
        }
    }

    /// The label of a sample.
    #[must_use]
    pub fn label(&self, line: usize, sample: usize) -> Label {
        Label::from_u8(self.labels[line * self.samples + sample]).unwrap_or(Label::Outside)
    }

    /// Whether a sample lies behind the first bone or lung sample of its line.
    #[must_use]
    pub fn is_hidden(&self, line: usize, sample: usize) -> bool {
        sample >= usize::from(self.hidden_from[line])
    }

    /// The byte to upload for a sample: its label, with [`HIDDEN_BIT`] when hidden.
    #[must_use]
    pub fn texel(&self, line: usize, sample: usize) -> u8 {
        let l = self.labels[line * self.samples + sample];
        if self.is_hidden(line, sample) {
            l | HIDDEN_BIT
        } else {
            l
        }
    }

    /// Every texel, line-major (an `R8` texture of width `samples`, height `lines`).
    #[must_use]
    pub fn texels(&self) -> Vec<u8> {
        let mut out = self.labels.clone();
        for line in 0..self.lines {
            let from = usize::from(self.hidden_from[line]);
            for t in &mut out[line * self.samples + from..(line + 1) * self.samples] {
                *t |= HIDDEN_BIT;
            }
        }
        out
    }

    /// Depth of sample `s`, mm (its centre).
    #[must_use]
    pub fn depth_of(&self, sample: usize) -> f32 {
        (sample as f32 + 0.5) / self.samples as f32 * self.depth_mm
    }

    /// How many samples hold `label`.
    #[must_use]
    pub fn count(&self, label: Label) -> usize {
        self.labels
            .iter()
            .fold(0usize, |n, l| n + usize::from(*l == label as u8))
    }

    /// How many samples are hidden.
    #[must_use]
    pub fn hidden_count(&self) -> usize {
        self.hidden_from
            .iter()
            .map(|h| self.samples - usize::from(*h))
            .sum()
    }

    /// The labels present, in numeric order (for a legend).
    #[must_use]
    pub fn labels_present(&self) -> Vec<Label> {
        let mut seen = [false; 256];
        for l in &self.labels {
            seen[*l as usize] = true;
        }
        Label::ALL.into_iter().filter(|l| seen[*l as usize]).collect()
    }
}

/// The time-dependent part of the cross-section: the displacement grid and the valve openings of
/// one drawn instant. Keep one per view and refresh it with [`Self::update`] whenever the 3-D
/// heart is re-evaluated (or the time changes with the motion off).
#[derive(Debug, Clone)]
pub struct SliceState {
    field: DisplacementField,
    openings: [f32; 4],
    /// The leaflets posed at `openings` (the designed cones and the measured valve).
    posed: Posed,
    thorax_shift: Vec3,
    /// Width factor of the designed lung-edge retraction at this instant (1 in expiration).
    retraction: f32,
}

impl SliceState {
    /// A state at rest for `anatomy`.
    #[must_use]
    pub fn new(anatomy: &EchoAnatomy) -> Self {
        let mut field = anatomy.field_template().clone();
        field.set_rest(Vec3::ZERO);
        let a = anatomy.designed.av_diastasis_opening as f32;
        let openings = [a, a, 0.0, 0.0];
        Self {
            field,
            posed: anatomy.pose(&openings),
            openings,
            thorax_shift: Vec3::ZERO,
            retraction: 1.0,
        }
    }

    /// Take the instant `input` describes: `frame` must be what `model.evaluate(input, frame)`
    /// produced for it (the same frame the 3-D heart draws: one evaluation serves both). With
    /// `input.enabled == false` the field is at rest (the frame is then the rest mesh anyway) and
    /// the leaflets still open and close with the valve schedule.
    pub fn update(
        &mut self,
        anatomy: &EchoAnatomy,
        model: &MotionModel,
        frame: &MotionFrame,
        input: &MotionInput<'_>,
    ) {
        self.openings = valve_openings(
            input.history,
            model.params(),
            model.ef_scale(),
            input.t_ms,
            &anatomy.designed,
        );
        anatomy.repose(&self.openings, &mut self.posed);
        if input.enabled {
            let depth = heartsim_sim::timing::inspiration_depth(input.resp_phase) as f32;
            self.retraction =
                (1.0 - anatomy.designed.lung_retraction_inspiration_loss * depth).clamp(0.0, 1.0);
            let shift = -(anatomy.respiratory_shift_mm() * f64::from(depth)) as f32;
            self.thorax_shift = Vec3::new(0.0, shift * anatomy.designed.thorax_respiratory_fraction, 0.0);
            self.field.update(frame, self.thorax_shift);
        } else {
            self.thorax_shift = Vec3::ZERO;
            self.retraction = 1.0;
            self.field.set_rest(Vec3::ZERO);
        }
    }

    /// The valve openings of the current instant, [`heartsim_ecg::Valve::index`] order.
    #[must_use]
    pub fn openings(&self) -> [f32; 4] {
        self.openings
    }

    /// Pose the leaflets at `openings` instead of the schedule's (tests and tools; the next
    /// [`Self::update`] takes the schedule's again).
    pub fn set_openings(&mut self, anatomy: &EchoAnatomy, openings: [f32; 4]) {
        self.openings = openings;
        anatomy.repose(&self.openings, &mut self.posed);
    }

    /// The leaflets as posed for this instant.
    #[must_use]
    pub fn posed(&self) -> &Posed {
        &self.posed
    }

    /// The displacement field of the current instant.
    #[must_use]
    pub fn field(&self) -> &DisplacementField {
        &self.field
    }

    /// The label at rest point `rest_mm` — the heart box with the designed parts posed at this
    /// instant when `from_heart`, else the thorax box with this instant's lung retraction —
    /// as [`Self::slice`] reads it (B32's interface normals look across the image plane with it).
    #[must_use]
    pub fn rest_label(&self, anatomy: &EchoAnatomy, rest_mm: Vec3, from_heart: bool) -> Label {
        if from_heart && let Some(l) = anatomy.heart_label_posed(rest_mm, &self.posed) {
            return l;
        }
        anatomy.thorax_label_retracted(rest_mm, self.retraction)
    }

    /// The thorax box's displacement at this instant (the respiratory shift), mm: a thorax
    /// sample at `x` was read at rest point `x − thorax_shift`.
    #[must_use]
    pub fn thorax_shift_mm(&self) -> Vec3 {
        self.thorax_shift
    }

    /// The label at current-position point `x_mm` (no skin-gap rule, no occlusion).
    #[must_use]
    pub fn label_at(&self, anatomy: &EchoAnatomy, x_mm: Vec3) -> Label {
        if self.field.may_reach_heart(x_mm) {
            let rest = self.field.rest_of(x_mm);
            if let Some(l) = anatomy.heart_label_posed(rest, &self.posed) {
                return l;
            }
        }
        anatomy.thorax_label_retracted(x_mm - self.thorax_shift, self.retraction)
    }

    /// The rest point of every sample of one line (`None` where it cannot lie in the heart box's
    /// tissue). The inverse map is solved exactly at every [`KNOT_STRIDE`]-th sample and at the
    /// last one — warm-started from the previous knot's displacement, [`INVERSE_ITERATIONS`]
    /// fixed-point steps — and interpolated linearly in between: the rest map is smooth on the
    /// scale of a few samples (≈ 1.25 mm at the default depth; `tests/slice.rs` bounds the
    /// difference from the exact map by 0.3 mm). Where one knot is reachable and the next not,
    /// the samples between are solved exactly.
    ///
    /// [`INVERSE_ITERATIONS`]: crate::field::INVERSE_ITERATIONS
    #[doc(hidden)]
    pub fn line_rests(
        &self,
        dir: Vec3,
        apex: Vec3,
        depth_mm: f32,
        samples: usize,
        out: &mut Vec<Option<Vec3>>,
    ) {
        out.clear();
        out.resize(samples, None);
        let at = |s: usize| apex + dir * ((s as f32 + 0.5) / samples as f32 * depth_mm);
        let mut warm: Option<Vec3> = None;
        let solve = |x: Vec3, warm: &mut Option<Vec3>| -> Option<Vec3> {
            if !self.field.may_reach_heart(x) {
                *warm = None;
                return None;
            }
            let start = match *warm {
                Some(u) => x - u,
                None => x - self.field.at(x),
            };
            let r = self.field.solve_rest(x, start);
            *warm = Some(x - r);
            Some(r)
        };
        let last = samples - 1;
        let mut a = 0usize;
        out[0] = solve(at(0), &mut warm);
        while a < last {
            let b = (a + KNOT_STRIDE).min(last);
            out[b] = solve(at(b), &mut warm);
            match (out[a], out[b]) {
                (Some(ra), Some(rb)) => {
                    for (k, o) in out[a + 1..b].iter_mut().enumerate() {
                        *o = Some(ra.lerp(rb, (k + 1) as f32 / (b - a) as f32));
                    }
                }
                (None, None) => {}
                _ => {
                    let mut w = None;
                    for (k, o) in out[a + 1..b].iter_mut().enumerate() {
                        *o = solve(at(a + 1 + k), &mut w);
                    }
                }
            }
            a = b;
        }
    }

    /// Fill `out` with the cross-section of `pose` (its size decides the resolution).
    pub fn slice(&self, anatomy: &EchoAnatomy, pose: &ProbePose, out: &mut LabelImage) {
        let pose = pose.clamped();
        let frame: ProbeFrame = pose.frame(anatomy.designed.contact_limit_deg);
        self.slice_frame(anatomy, &frame, pose.depth_mm, pose.sector_deg, out);
    }

    /// [`Self::slice`] for a probe frame given directly: the sector of `sector_deg` about
    /// `frame.beam`, `depth_mm` deep. The M-mode cursor cuts a thin sector of its own about its
    /// line this way (`mmode`); `depth_mm` and `sector_deg` are taken as given.
    pub fn slice_frame(
        &self,
        anatomy: &EchoAnatomy,
        frame: &ProbeFrame,
        depth_mm: f32,
        sector_deg: f32,
        out: &mut LabelImage,
    ) {
        let d: &DesignedParams = &anatomy.designed;
        out.depth_mm = depth_mm;
        out.sector_deg = sector_deg;
        out.contact = frame.contact;
        let (lines, samples) = (out.lines, out.samples);
        if out.labels.len() != lines * samples {
            *out = LabelImage::new(lines, samples);
            out.depth_mm = depth_mm;
            out.sector_deg = sector_deg;
            out.contact = frame.contact;
        }
        if !frame.contact {
            out.labels.fill(Label::Outside as u8);
            out.hidden_from.fill(samples as u16);
            return;
        }
        let thorax_box = BoxMm::of(anatomy.thorax_volume());
        let mut rests: Vec<Option<Vec3>> = Vec::with_capacity(samples);
        for line in 0..lines {
            let dir = frame.line_direction(line, lines, sector_deg);
            let base = line * samples;
            let row = &mut out.labels[base..base + samples];
            let rest_row = &mut out.rest_mm[base..base + samples];
            let heart_row = &mut out.from_heart[base..base + samples];
            let mut hidden = samples;
            let mut skin_reached = false;
            self.line_rests(dir, frame.apex_mm, depth_mm, samples, &mut rests);
            for s in 0..samples {
                // the same expression as `LabelImage::depth_of`, bit for bit
                let r = (s as f32 + 0.5) / samples as f32 * depth_mm;
                let x = frame.apex_mm + dir * r;
                // past the thorax box (convex) nothing more is inside the body on this line
                if skin_reached && !thorax_box.contains_point(x - self.thorax_shift) {
                    row[s..].fill(Label::Outside as u8);
                    heart_row[s..].fill(false);
                    for (k, r) in rest_row[s..].iter_mut().enumerate() {
                        let rk = ((s + k) as f32 + 0.5) / samples as f32 * depth_mm;
                        *r = frame.apex_mm + dir * rk - self.thorax_shift;
                    }
                    break;
                }
                let heart =
                    rests[s].and_then(|rest| anatomy.heart_label_posed(rest, &self.posed).map(|l| (l, rest)));
                let mut l = if let Some((l, rest)) = heart {
                    rest_row[s] = rest;
                    heart_row[s] = true;
                    l
                } else {
                    rest_row[s] = x - self.thorax_shift;
                    heart_row[s] = false;
                    anatomy.thorax_label_retracted(x - self.thorax_shift, self.retraction)
                };
                if !skin_reached {
                    if l == Label::Outside && r <= d.skin_gap_mm {
                        l = Label::SoftTissue;
                    } else {
                        skin_reached = true;
                    }
                }
                row[s] = l as u8;
                if hidden == samples && l.occludes() {
                    hidden = s + 1;
                }
            }
            out.hidden_from[line] = hidden as u16;
        }
    }
}

/// An axis-aligned box, mm.
#[derive(Debug, Clone, Copy)]
struct BoxMm {
    lo: Vec3,
    hi: Vec3,
}

impl BoxMm {
    fn of(v: &heartsim_geometry::LabelVolume) -> Self {
        Self {
            lo: v.origin_mm,
            hi: v.max_mm(),
        }
    }
    fn contains_point(&self, p: Vec3) -> bool {
        p.cmpge(self.lo).all() && p.cmplt(self.hi).all()
    }
}

/// One call for a drawn frame: [`SliceState::update`] then [`SliceState::slice`].
pub fn slice(
    anatomy: &EchoAnatomy,
    state: &mut SliceState,
    model: &MotionModel,
    frame: &MotionFrame,
    input: &MotionInput<'_>,
    pose: &ProbePose,
    out: &mut LabelImage,
) {
    state.update(anatomy, model, frame, input);
    state.slice(anatomy, pose, out);
}
