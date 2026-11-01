//! Tier 0 synthesis: each timing event spawns a set of Gaussian waves (McSharry et al. 2003
//! morphology) whose dipole directions are projected onto the 12 leads. Because the waves are
//! triggered by the shared timing engine, the strip stays consistent with any 3-D view.

use crate::leadvec::LeadVectors;
use glam::DVec3;
use heartsim_core::{FrontalAxis, LEAD_COUNT, Lead, Millis, Region, TwelveLead};
use heartsim_sim::{Event, EventKind};
use serde::{Deserialize, Serialize};

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
        WaveSpec {
            name: "T",
            amplitude: 0.35,
            width_ms: 55.0,
            offset_ms: 300.0,
            axis_deg: 45.0,
            anterior: 0.6,
        },
    ];
    (atrial, ventricular)
}

#[derive(Debug, Clone, Copy)]
struct ActiveWave {
    spec: WaveSpec,
    t0: Millis,
}

/// Event-driven 12-lead synthesiser.
#[derive(Debug, Clone)]
pub struct GaussianSynth {
    pub leads: LeadVectors,
    pub atrial: Vec<WaveSpec>,
    pub ventricular: Vec<WaveSpec>,
    /// QRS width scale (1 = 90 ms-ish; 1.5 ≈ bundle-branch block).
    pub qrs_width_scale: f64,
    /// Global axis rotation added to every ventricular wave, degrees.
    pub axis_shift_deg: f64,
    active: Vec<ActiveWave>,
}

impl GaussianSynth {
    #[must_use]
    pub fn textbook() -> Self {
        let (atrial, ventricular) = textbook_waves();
        Self {
            leads: LeadVectors::standard(),
            atrial,
            ventricular,
            qrs_width_scale: 1.0,
            axis_shift_deg: 0.0,
            active: Vec::new(),
        }
    }

    /// Feed timing-engine events.
    pub fn on_events(&mut self, events: &[Event]) {
        for ev in events {
            if ev.kind == EventKind::Blocked {
                continue;
            }
            let specs: &[WaveSpec] = match ev.region {
                r if r.is_atrial() => &self.atrial,
                Region::LeftVentricle | Region::RightVentricle | Region::VentricularSeptum => {
                    &self.ventricular
                }
                _ => continue,
            };
            for s in specs {
                self.active.push(ActiveWave { spec: *s, t0: ev.at });
            }
        }
    }

    /// Sample the 12 leads at time `t` and drop finished waves.
    pub fn sample(&mut self, t: Millis) -> TwelveLead {
        let mut dipole = DVec3::ZERO;
        let axis_shift = self.axis_shift_deg;
        let scale = self.qrs_width_scale;
        self.active.retain(|w| {
            let is_qrs = matches!(w.spec.name, "Q" | "R" | "S");
            let width = if is_qrs {
                w.spec.width_ms * scale
            } else {
                w.spec.width_ms
            };
            let offset = if is_qrs {
                w.spec.offset_ms * scale
            } else {
                w.spec.offset_ms
            };
            let dt = t.0 - w.t0.0 - offset;
            if dt > 5.0 * width {
                return false;
            }
            if dt > -5.0 * width {
                let g = w.spec.amplitude * (-(dt * dt) / (2.0 * width * width)).exp();
                let spec = if is_qrs || w.spec.name == "T" {
                    WaveSpec {
                        axis_deg: w.spec.axis_deg + axis_shift,
                        ..w.spec
                    }
                } else {
                    w.spec
                };
                dipole += spec.direction() * g;
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
