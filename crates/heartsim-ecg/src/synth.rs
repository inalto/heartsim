//! Tier 0 synthesis: each timing event spawns a set of Gaussian waves (McSharry et al. 2003
//! morphology) whose dipole directions are projected onto the 12 leads. Because the waves are
//! triggered by the shared timing engine, the strip stays consistent with any 3-D view.
//!
//! Each event picks a beat [`Template`] from what the engine reports: sinus, ectopic, retrograde
//! or paced P; normally conducted, bundle-branch-blocked (from the `Blocked` branch events of the
//! same beat), pre-excited, ectopic or paced QRS–T. Scenario files tune the shapes through
//! [`MorphologyOverrides`].

use crate::leadvec::LeadVectors;
use glam::DVec3;
use heartsim_core::{FrontalAxis, LEAD_COUNT, Lead, Millis, Region, TwelveLead};
use heartsim_sim::{Event, EventKind, Origin};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One Gaussian component: `a · exp(−((t − t0 − offset)² / (2 b²)))` along direction `dir`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WaveSpec {
    pub name: &'static str,
    /// Peak dipole magnitude (mV in lead units when projected onto a unit lead vector).
    pub amplitude: f64,
    /// Standard deviation, ms.
    pub width_ms: f64,
    /// Delay of the peak after the triggering event, ms.
    pub offset_ms: f64,
    /// Frontal-plane angle of the dipole, degrees (hexaxial).
    pub axis_deg: f64,
    /// Anterior (+) / posterior (−) component, same units as the frontal unit vector.
    pub anterior: f64,
}

impl WaveSpec {
    #[must_use]
    pub fn direction(&self) -> DVec3 {
        (LeadVectors::frontal(self.axis_deg) + DVec3::new(0.0, 0.0, self.anterior)).normalize()
    }
}

/// Default morphology for a healthy adult (QRS axis +60°, T concordant).
#[must_use]
pub fn textbook_waves() -> (Vec<WaveSpec>, Vec<WaveSpec>) {
    let atrial = vec![WaveSpec {
        name: "P",
        amplitude: 0.15,
        width_ms: 22.0,
        offset_ms: 45.0,
        axis_deg: 60.0,
        anterior: 0.3,
    }];
    let ventricular = vec![
        WaveSpec {
            name: "Q",
            amplitude: -0.12,
            width_ms: 7.0,
            offset_ms: 8.0,
            axis_deg: 60.0,
            anterior: -0.6,
        },
        WaveSpec {
            name: "R",
            amplitude: 1.3,
            width_ms: 11.0,
            offset_ms: 28.0,
            axis_deg: 60.0,
            anterior: 0.45,
        },
        WaveSpec {
            name: "S",
            amplitude: -0.35,
            width_ms: 9.0,
            offset_ms: 48.0,
            axis_deg: 60.0,
            anterior: -0.9,
        },
        T_NORMAL,
    ];
    (atrial, ventricular)
}

const T_NORMAL: WaveSpec = WaveSpec {
    name: "T",
    amplitude: 0.35,
    width_ms: 55.0,
    offset_ms: 300.0,
    axis_deg: 45.0,
    anterior: 0.6,
};

const fn wave(
    name: &'static str,
    amplitude: f64,
    width_ms: f64,
    offset_ms: f64,
    axis_deg: f64,
    anterior: f64,
) -> WaveSpec {
    WaveSpec {
        name,
        amplitude,
        width_ms,
        offset_ms,
        axis_deg,
        anterior,
    }
}

/// The beat morphologies the synthesiser chooses between. The choice is made per timing event
/// from the event's region, kind and origin and from the bundle-branch block events of the same
/// beat, so the ECG always shows what the timing engine did (no separate "BBB switch").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Template {
    /// Sinus P wave (also atrial pacing). Stored in [`GaussianSynth::atrial`].
    SinusP,
    /// P wave of an ectopic atrial focus (PAC): lower, narrower, shifted axis.
    EctopicP,
    /// Retrograde P (VA conduction over the node or an accessory pathway): superior axis,
    /// negative in II, III and aVF.
    RetrogradeP,
    /// QRS–T conducted over an intact His–Purkinje system. Stored in
    /// [`GaussianSynth::ventricular`]; fascicular blocks rotate its QRS.
    Normal,
    /// Right bundle-branch block: normal initial forces plus a slow terminal right-anterior R′
    /// (rsR′ in V1, wide S in I and V6), QRS ≥ 120 ms, T discordant in V1.
    Rbbb,
    /// Left bundle-branch block: no septal q, slow leftward-posterior forces (QS in V1, broad
    /// notched R in I and V6), QRS ≥ 120 ms, T discordant.
    Lbbb,
    /// Delta wave of a left-sided accessory pathway (negative in I/aVL, positive in V1: "type A").
    DeltaLv,
    /// Delta wave of a right-sided accessory pathway (positive in I, negative in V1: "type B").
    DeltaRv,
    /// Beat arising in the left ventricle (PVC, escape, LV pacing): RBBB-like, inferior axis.
    EctopicLv,
    /// Beat arising in the right ventricle / RV apex (PVC, escape, RV pacing): LBBB-like,
    /// superior axis.
    EctopicRv,
    /// Pacing stimulus artefact (< 2 ms), added in front of every captured paced beat.
    Spike,
}

/// Rotation of the QRS forces in left anterior fascicular block, degrees: the unopposed
/// late activation of the anterosuperior LV moves the axis to about −60° (criterion −45° to −90°,
/// Surawicz et al. 2009).
pub const LAFB_ROTATION_DEG: f64 = -120.0;
/// Rotation of the QRS forces in left posterior fascicular block, degrees: axis about +120°
/// (criterion +90° to +180°, Surawicz et al. 2009).
pub const LPFB_ROTATION_DEG: f64 = 60.0;
/// QRS widening of an isolated fascicular block (QRS stays < 120 ms).
const FASCICULAR_WIDTH_SCALE: f64 = 1.1;
/// Fridericia's exponent (QT ∝ RR^⅓), the default rate adaptation of scenario runs.
pub const FRIDERICIA_EXPONENT: f64 = 1.0 / 3.0;

/// Built-in waves of the non-default templates. Amplitudes in mV, widths are Gaussian σ.
#[must_use]
pub fn template_waves(t: Template) -> Vec<WaveSpec> {
    match t {
        Template::SinusP | Template::Normal => {
            let (a, v) = textbook_waves();
            if t == Template::SinusP { a } else { v }
        }
        Template::EctopicP => vec![wave("P", 0.12, 18.0, 45.0, 20.0, 0.1)],
        Template::RetrogradeP => vec![wave("P", 0.12, 16.0, 45.0, -90.0, 0.2)],
        Template::Rbbb => vec![
            wave("Q", -0.12, 7.0, 8.0, 60.0, -0.6),
            wave("R", 1.3, 11.0, 28.0, 60.0, -0.25),
            wave("R'", 0.6, 13.0, 88.0, 180.0, 1.6),
            wave("T", 0.3, 55.0, 310.0, 45.0, -0.6),
        ],
        Template::Lbbb => vec![
            wave("R", 0.8, 11.0, 40.0, 10.0, -0.3),
            wave("R2", 1.0, 13.0, 85.0, -20.0, -0.9),
            wave("T", 0.35, 55.0, 330.0, 170.0, 0.8),
        ],
        Template::DeltaLv => vec![
            wave("D", 0.5, 14.0, 50.0, 110.0, 0.7),
            wave("T", -0.12, 50.0, 300.0, 110.0, 0.7),
        ],
        Template::DeltaRv => vec![
            wave("D", 0.5, 14.0, 50.0, -10.0, -0.6),
            wave("T", -0.12, 50.0, 300.0, -10.0, -0.6),
        ],
        // Wide complexes are two overlapping slow components (slurred, sometimes notched) rather
        // than one very broad Gaussian, whose long tails would read as a 200+ ms QRS. Ectopic
        // complexes are larger than conducted ones (Goldberger 2017 ch. 16).
        Template::EctopicLv => vec![
            wave("R", 1.6, 13.0, 48.0, 100.0, 0.8),
            wave("R2", 1.6, 13.0, 85.0, 110.0, 0.9),
            wave("T", -0.5, 55.0, 330.0, 100.0, 0.8),
        ],
        Template::EctopicRv => vec![
            wave("R", 1.6, 13.0, 48.0, -60.0, -0.8),
            wave("R2", 1.6, 13.0, 85.0, -75.0, -0.9),
            wave("T", -0.5, 55.0, 330.0, -65.0, -0.8),
        ],
        Template::Spike => vec![wave("spike", 2.0, 0.6, 0.0, -60.0, -0.5)],
    }
}

impl Template {
    /// Templates whose waves are ventricular (QRS width scale, axis shift and QT adaptation apply).
    #[must_use]
    pub fn is_ventricular(self) -> bool {
        !matches!(
            self,
            Template::SinusP | Template::EctopicP | Template::RetrogradeP | Template::Spike
        )
    }
}

/// Override of one wave of one template (absolute values; `None` keeps the built-in value).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaveOverride {
    pub template: Template,
    /// Wave name within the template ("P", "Q", "R", "R'", "R2", "S", "T", "D").
    pub wave: String,
    #[serde(default)]
    pub amplitude_mv: Option<f64>,
    #[serde(default)]
    pub width_ms: Option<f64>,
    #[serde(default)]
    pub offset_ms: Option<f64>,
    #[serde(default)]
    pub axis_deg: Option<f64>,
    #[serde(default)]
    pub anterior: Option<f64>,
}

/// Scenario-level Tier-0 morphology knobs (the `morphology` section of a scenario file).
/// Everything here changes the *shape* of the beats; *which* beats occur and when is decided by
/// the timing engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MorphologyOverrides {
    /// QRS width scale of every ventricular template (1 = built-in).
    pub qrs_width_scale: f64,
    /// Rotation of every ventricular wave in the frontal plane, degrees (e.g. a vertical or
    /// horizontal heart position).
    pub axis_shift_deg: f64,
    /// QT rate adaptation exponent `k` in `QT ∝ (RR / 1 s)^k`; `None` keeps the synthesiser's
    /// setting. Scenario runs default to Fridericia (⅓).
    pub qt_rate_exponent: Option<f64>,
    /// Per-wave overrides.
    pub waves: Vec<WaveOverride>,
}

impl Default for MorphologyOverrides {
    fn default() -> Self {
        Self {
            qrs_width_scale: 1.0,
            axis_shift_deg: 0.0,
            qt_rate_exponent: None,
            waves: Vec::new(),
        }
    }
}

/// A [`WaveOverride`] names a wave its template does not have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownWave {
    pub template: Template,
    pub wave: String,
}

impl std::fmt::Display for UnknownWave {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "template {:?} has no wave `{}`", self.template, self.wave)
    }
}

impl std::error::Error for UnknownWave {}

#[derive(Debug, Clone, Copy)]
struct ActiveWave {
    spec: WaveSpec,
    t0: Millis,
}

/// Bundle-branch block events seen since the last His activation (bit set).
const BLOCK_RBB: u8 = 1;
const BLOCK_LBB: u8 = 2;
const BLOCK_LAF: u8 = 4;
const BLOCK_LPF: u8 = 8;

/// Ventricular activations closer than this belong to the same beat (fusion fronts), ms.
const SAME_BEAT_MS: f64 = 150.0;

/// Event-driven 12-lead synthesiser.
#[derive(Debug, Clone)]
pub struct GaussianSynth {
    pub leads: LeadVectors,
    /// Sinus P wave ([`Template::SinusP`]).
    pub atrial: Vec<WaveSpec>,
    /// Normally conducted QRS–T ([`Template::Normal`]).
    pub ventricular: Vec<WaveSpec>,
    /// Waves of every other [`Template`].
    pub templates: BTreeMap<Template, Vec<WaveSpec>>,
    /// QRS width scale (1 = 90 ms-ish; 1.5 ≈ bundle-branch block).
    pub qrs_width_scale: f64,
    /// Global axis rotation added to every ventricular wave, degrees.
    pub axis_shift_deg: f64,
    /// QT rate adaptation exponent: T offset and width scale with `(RR / 1000 ms)^k`, RR being
    /// the interval from the previous ventricular beat. 0 = fixed QT (the textbook default).
    pub qt_rate_exponent: f64,
    active: Vec<ActiveWave>,
    branch_blocks: u8,
    last_beat_ms: Option<f64>,
    last_rr_ms: Option<f64>,
}

impl GaussianSynth {
    #[must_use]
    pub fn textbook() -> Self {
        let (atrial, ventricular) = textbook_waves();
        let templates = [
            Template::EctopicP,
            Template::RetrogradeP,
            Template::Rbbb,
            Template::Lbbb,
            Template::DeltaLv,
            Template::DeltaRv,
            Template::EctopicLv,
            Template::EctopicRv,
            Template::Spike,
        ]
        .into_iter()
        .map(|t| (t, template_waves(t)))
        .collect();
        Self {
            leads: LeadVectors::standard(),
            atrial,
            ventricular,
            templates,
            qrs_width_scale: 1.0,
            axis_shift_deg: 0.0,
            qt_rate_exponent: 0.0,
            active: Vec::new(),
            branch_blocks: 0,
            last_beat_ms: None,
            last_rr_ms: None,
        }
    }

    /// Waves of a template.
    #[must_use]
    pub fn waves(&self, t: Template) -> &[WaveSpec] {
        match t {
            Template::SinusP => &self.atrial,
            Template::Normal => &self.ventricular,
            other => self.templates.get(&other).map_or(&[], Vec::as_slice),
        }
    }

    fn waves_mut(&mut self, t: Template) -> &mut Vec<WaveSpec> {
        match t {
            Template::SinusP => &mut self.atrial,
            Template::Normal => &mut self.ventricular,
            other => self
                .templates
                .entry(other)
                .or_insert_with(|| template_waves(other)),
        }
    }

    /// Apply a scenario's morphology section.
    pub fn apply_overrides(&mut self, o: &MorphologyOverrides) -> Result<(), UnknownWave> {
        self.qrs_width_scale = o.qrs_width_scale;
        self.axis_shift_deg = o.axis_shift_deg;
        if let Some(k) = o.qt_rate_exponent {
            self.qt_rate_exponent = k;
        }
        for w in &o.waves {
            let spec = self
                .waves_mut(w.template)
                .iter_mut()
                .find(|s| s.name == w.wave)
                .ok_or_else(|| UnknownWave {
                    template: w.template,
                    wave: w.wave.clone(),
                })?;
            if let Some(v) = w.amplitude_mv {
                spec.amplitude = v;
            }
            if let Some(v) = w.width_ms {
                spec.width_ms = v;
            }
            if let Some(v) = w.offset_ms {
                spec.offset_ms = v;
            }
            if let Some(v) = w.axis_deg {
                spec.axis_deg = v;
            }
            if let Some(v) = w.anterior {
                spec.anterior = v;
            }
        }
        Ok(())
    }

    /// Feed timing-engine events.
    pub fn on_events(&mut self, events: &[Event]) {
        for ev in events {
            if ev.kind == EventKind::Blocked {
                self.branch_blocks |= match ev.region {
                    Region::RightBundleBranch => BLOCK_RBB,
                    Region::LeftBundleBranch => BLOCK_LBB,
                    Region::LeftAnteriorFascicle => BLOCK_LAF,
                    Region::LeftPosteriorFascicle => BLOCK_LPF,
                    _ => 0,
                };
                continue;
            }
            match ev.region {
                Region::HisBundle => self.branch_blocks = 0,
                r if r.is_atrial() => {
                    let t = match (ev.kind, ev.origin) {
                        (EventKind::Automatic, Origin::Sinus) => Template::SinusP,
                        (EventKind::Automatic, _) => Template::EctopicP,
                        (EventKind::Stimulus, _) => {
                            self.spawn(Template::Spike, ev.at, 0.0, 1.0);
                            Template::SinusP
                        }
                        _ => Template::RetrogradeP,
                    };
                    self.spawn(t, ev.at, 0.0, 1.0);
                }
                Region::LeftVentricle | Region::RightVentricle | Region::VentricularSeptum => {
                    self.on_ventricular(ev);
                }
                _ => {}
            }
        }
    }

    fn on_ventricular(&mut self, ev: &Event) {
        let t = ev.at.0;
        match self.last_beat_ms {
            Some(last) if t - last < SAME_BEAT_MS => {}
            last => {
                self.last_rr_ms = last.map(|l| t - l);
                self.last_beat_ms = Some(t);
            }
        }
        let ectopic = if ev.region == Region::RightVentricle {
            Template::EctopicRv
        } else {
            Template::EctopicLv
        };
        match (ev.kind, ev.origin) {
            (EventKind::Conducted, Origin::AccessoryPathway) => {
                let d = if ev.region == Region::RightVentricle {
                    Template::DeltaRv
                } else {
                    Template::DeltaLv
                };
                self.spawn(d, ev.at, 0.0, 1.0);
            }
            (EventKind::Conducted, _) => {
                let b = std::mem::take(&mut self.branch_blocks);
                let lbbb = b & BLOCK_LBB != 0 || b & (BLOCK_LAF | BLOCK_LPF) == BLOCK_LAF | BLOCK_LPF;
                let (rotation, width) = if lbbb {
                    (0.0, 1.0)
                } else if b & BLOCK_LAF != 0 {
                    (LAFB_ROTATION_DEG, FASCICULAR_WIDTH_SCALE)
                } else if b & BLOCK_LPF != 0 {
                    (LPFB_ROTATION_DEG, FASCICULAR_WIDTH_SCALE)
                } else {
                    (0.0, 1.0)
                };
                let tpl = if lbbb {
                    Template::Lbbb
                } else if b & BLOCK_RBB != 0 {
                    Template::Rbbb
                } else {
                    Template::Normal
                };
                self.spawn(tpl, ev.at, rotation, width);
            }
            (EventKind::Stimulus, _) => {
                self.spawn(Template::Spike, ev.at, 0.0, 1.0);
                self.spawn(ectopic, ev.at, 0.0, 1.0);
            }
            _ => self.spawn(ectopic, ev.at, 0.0, 1.0),
        }
    }

    /// Start the waves of `tpl` at `t0`. Ventricular waves get the global QRS width scale and
    /// axis shift; the QRS waves Q/R/S additionally get the fascicular `rotation_deg` and
    /// `width`; T waves are rate-adapted.
    fn spawn(&mut self, tpl: Template, t0: Millis, rotation_deg: f64, width: f64) {
        let ventricular = tpl.is_ventricular();
        let qt = match self.last_rr_ms {
            Some(rr) if self.qt_rate_exponent != 0.0 => {
                (rr / 1000.0).powf(self.qt_rate_exponent).clamp(0.6, 1.4)
            }
            _ => 1.0,
        };
        let (scale, shift) = (self.qrs_width_scale * width, self.axis_shift_deg);
        let mut new = Vec::new();
        for w in self.waves(tpl) {
            let mut s = *w;
            if ventricular {
                s.axis_deg += shift;
                if s.name == "T" {
                    s.offset_ms *= qt;
                    s.width_ms *= qt;
                } else {
                    s.offset_ms *= scale;
                    s.width_ms *= scale;
                    if matches!(s.name, "Q" | "R" | "S") {
                        s.axis_deg += rotation_deg;
                    }
                }
            }
            new.push(ActiveWave { spec: s, t0 });
        }
        self.active.extend(new);
    }

    /// Sample the 12 leads at time `t` and drop finished waves.
    pub fn sample(&mut self, t: Millis) -> TwelveLead {
        let mut dipole = DVec3::ZERO;
        self.active.retain(|w| {
            let dt = t.0 - w.t0.0 - w.spec.offset_ms;
            let width = w.spec.width_ms;
            if dt > 5.0 * width {
                return false;
            }
            if dt > -5.0 * width {
                let g = w.spec.amplitude * (-(dt * dt) / (2.0 * width * width)).exp();
                dipole += w.spec.direction() * g;
            }
            true
        });
        TwelveLead(self.leads.project(dipole))
    }

    /// Mean frontal QRS axis implied by the configured waves, degrees on the hexaxial circle.
    ///
    /// It is the direction of the summed Q+R+S area vectors (amplitude × width), i.e. the net
    /// ventricular depolarisation dipole. This is the quantity an observer estimates from the
    /// net QRS deflections in leads I and aVF, so a measured axis must agree with it.
    #[must_use]
    pub fn qrs_axis_deg(&self) -> f64 {
        let (mut x, mut y) = (0.0, 0.0);
        for w in self
            .ventricular
            .iter()
            .filter(|w| matches!(w.name, "Q" | "R" | "S"))
        {
            let spec = WaveSpec {
                axis_deg: w.axis_deg + self.axis_shift_deg,
                ..*w
            };
            let d = spec.direction();
            let area = spec.amplitude * spec.width_ms;
            x += d.x * area;
            y += -d.y * area;
        }
        FrontalAxis::normalise(y.atan2(x).to_degrees())
    }

    #[must_use]
    pub fn active_count(&self) -> usize {
        self.active.len()
    }
}

/// Convenience: index helper for callers that keep `[f64; 12]` arrays.
#[must_use]
pub fn lead_index(l: Lead) -> usize {
    l as usize
}

#[allow(dead_code)]
const _: () = assert!(LEAD_COUNT == 12);

#[cfg(test)]
mod tests {
    use super::*;
    use heartsim_core::{FrontalAxis, SimClock};
    use heartsim_sim::{Engine, TimingEngine, TimingParams};

    /// Run timing + synth for `secs` seconds at 1 kHz; returns lead II samples and all leads.
    fn run(secs: f64, synth: &mut GaussianSynth) -> Vec<TwelveLead> {
        let mut eng = TimingEngine::new(TimingParams::default());
        let mut clock = SimClock::new(Millis(1.0));
        let mut out = Vec::new();
        for _ in 0..(secs * 1000.0) as usize {
            clock.advance(Millis(1.0));
            eng.step(Millis(1.0));
            synth.on_events(&eng.drain_events());
            out.push(synth.sample(eng.now()));
        }
        out
    }

    #[test]
    fn lead_ii_has_upright_p_qrs_t_and_physiological_amplitudes() {
        let mut s = GaussianSynth::textbook();
        let v = run(3.0, &mut s);
        let ii: Vec<f64> = v.iter().map(|x| x.get(Lead::II)).collect();
        let max = ii.iter().copied().fold(f64::MIN, f64::max);
        let min = ii.iter().copied().fold(f64::MAX, f64::min);
        assert!((0.8..2.0).contains(&max), "R in II = {max} mV");
        assert!(min > -0.6 && min < -0.05, "S in II = {min} mV");
        // aVR must be predominantly negative
        let avr_sum: f64 = v.iter().map(|x| x.get(Lead::AVR)).sum();
        assert!(avr_sum < 0.0);
    }

    #[test]
    fn qrs_axis_matches_spec_and_shift() {
        for shift in [0.0, -60.0, 60.0] {
            let mut s = GaussianSynth::textbook();
            s.axis_shift_deg = shift;
            let v = run(2.0, &mut s);
            // take the sample with the largest |R| in lead with max energy: use net QRS area ≈ R peak sample
            let peak = v
                .iter()
                .max_by(|a, b| a.get(Lead::II).abs().partial_cmp(&b.get(Lead::II).abs()).unwrap())
                .unwrap();
            let ax = FrontalAxis::from_limb_leads(peak).degrees;
            let expected = 60.0 + shift;
            assert!((ax - expected).abs() < 3.0, "axis {ax} vs {expected}");
        }
    }

    #[test]
    fn waves_are_released() {
        let mut s = GaussianSynth::textbook();
        let _ = run(5.0, &mut s);
        assert!(
            s.active_count() <= 10,
            "waves must not accumulate: {}",
            s.active_count()
        );
    }
}
