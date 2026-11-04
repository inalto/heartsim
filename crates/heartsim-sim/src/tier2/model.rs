//! Phenomenological cell models (research §2.3), all in milliseconds with a normalised
//! membrane variable `u ∈ [0, 1]` (0 = rest, 1 = peak) as the first state variable.
//!
//! Each model exposes two dials so that one parameter set can be mapped onto every region:
//! [`CellModel::scaled_apd`] stretches the plateau (APD ≈ s · APD₀) and
//! [`CellModel::scaled_excitation`] speeds up the upstroke kinetics, which raises the conduction
//! velocity at a fixed diffusivity (`CV ∝ sqrt(s·D)`): on a coarse grid the explicit diffusion
//! limit `dt ≤ h²/(2 tr D)` caps D, so fast tissue (Bachmann's bundle, the Purkinje layer) gets
//! its speed from the kinetics instead.

/// A cell model integrated with explicit Euler. `state[0]` is the normalised membrane variable.
pub trait CellModel: Clone + Send + Sync + std::fmt::Debug {
    /// Number of state variables (membrane variable first).
    const VARS: usize;
    /// Short identifier for logs and scenario files.
    const NAME: &'static str;

    /// Write the resting state into `state`.
    fn rest(&self, state: &mut [f32]);

    /// One forward-Euler step of `dt_ms`. `i_stim` is an applied current in membrane-variable
    /// units per ms; the solver passes the diffusion current here too (operator splitting with
    /// one explicit step for both).
    fn step(&self, state: &mut [f32], i_stim: f32, dt_ms: f32);

    /// Normalised transmembrane potential (0 rest … 1 peak).
    fn vm(&self, state: &[f32]) -> f32;

    /// The slow recovery variable, used with `vm` as the phase-plane coordinates of the
    /// phase-singularity diagnostic.
    fn recovery(&self, state: &[f32]) -> f32;

    /// The same model with its plateau stretched by `s` (APD grows roughly ∝ s).
    #[must_use]
    fn scaled_apd(&self, s: f32) -> Self;

    /// The same model with upstroke kinetics `s` times faster (CV ∝ sqrt(s), APD ≈ unchanged).
    #[must_use]
    fn scaled_excitation(&self, s: f32) -> Self;

    /// Plane-wave conduction velocity in a continuum with diffusivity `d_mm2_ms`, mm/ms (= m/s),
    /// when the model has a closed form (bistable cubic front), else `None`.
    fn analytic_cv(&self, d_mm2_ms: f32) -> Option<f32>;

    /// Largest explicit-Euler step the reaction tolerates, ms (λ_max·dt ≤ 1 on the upstroke).
    fn max_dt_ms(&self) -> f32;
}

/// Speed of the travelling front of `du/dt = D u'' + A·u(u − α)(β − u)`:
/// `c = sqrt(A·D/2)·(β − 2α)` (Nagumo / Zeldovich front; McKean 1970).
fn cubic_front_speed(a_per_ms: f32, d: f32, alpha: f32, beta: f32) -> f32 {
    (a_per_ms * d / 2.0).sqrt() * (beta - 2.0 * alpha)
}

// --- Aliev–Panfilov ---------------------------------------------------------------------------

/// Aliev & Panfilov 1996 (Chaos Solitons Fractals 7:293), in ms:
///
/// ```text
/// du/dt = s·(−k u (u − a)(u − 1) − u v)/T + I
/// dv/dt = r·ε(u, v)·(−v − k u (u − a − 1))/T,   ε = ε₀ + μ₁ v / (u + μ₂)
/// ```
///
/// `T` is the model time unit (≈ 12.9 ms, research §2.3.2), `r` the recovery (APD) dial and
/// `s` the excitation dial. Slow upstroke (≈ 10 ms), hence a ≈ 6 mm wavefront that a 2 mm
/// grid resolves: the Tier-2 default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlievPanfilov {
    pub k: f32,
    pub a: f32,
    pub eps0: f32,
    pub mu1: f32,
    pub mu2: f32,
    /// Model time unit, ms.
    pub t_unit_ms: f32,
    /// Recovery-rate multiplier (APD ∝ 1/r).
    pub recovery: f32,
    /// Upstroke-rate multiplier.
    pub excitation: f32,
}

impl Default for AlievPanfilov {
    /// The standard set: k = 8, a = 0.15, ε₀ = 0.002, μ₁ = 0.2, μ₂ = 0.3.
    fn default() -> Self {
        Self {
            k: 8.0,
            a: 0.15,
            eps0: 0.002,
            mu1: 0.2,
            mu2: 0.3,
            t_unit_ms: 12.9,
            recovery: 1.0,
            excitation: 1.0,
        }
    }
}

impl CellModel for AlievPanfilov {
    const VARS: usize = 2;
    const NAME: &'static str = "aliev_panfilov";

    fn rest(&self, state: &mut [f32]) {
        state[0] = 0.0;
        state[1] = 0.0;
    }

    #[inline]
    fn step(&self, state: &mut [f32], i_stim: f32, dt_ms: f32) {
        let (u, v) = (state[0], state[1]);
        let inv_t = 1.0 / self.t_unit_ms;
        let du = self.excitation * (-self.k * u * (u - self.a) * (u - 1.0) - u * v) * inv_t + i_stim;
        let eps = self.eps0 + self.mu1 * v / (u + self.mu2).max(1e-3);
        let dv = self.recovery * eps * (-v - self.k * u * (u - self.a - 1.0)) * inv_t;
        state[0] = u + dt_ms * du;
        state[1] = v + dt_ms * dv;
    }

    #[inline]
    fn vm(&self, state: &[f32]) -> f32 {
        state[0]
    }

    fn recovery(&self, state: &[f32]) -> f32 {
        state[1]
    }

    fn scaled_apd(&self, s: f32) -> Self {
        Self {
            recovery: self.recovery / s,
            ..*self
        }
    }

    fn scaled_excitation(&self, s: f32) -> Self {
        Self {
            excitation: self.excitation * s,
            ..*self
        }
    }

    fn analytic_cv(&self, d: f32) -> Option<f32> {
        // v ≈ 0 ahead of and in the front (ε ≪ 1): f(u) = (s k / T)·u (u − a)(1 − u)
        Some(cubic_front_speed(
            self.excitation * self.k / self.t_unit_ms,
            d,
            self.a,
            1.0,
        ))
    }

    fn max_dt_ms(&self) -> f32 {
        self.t_unit_ms / (self.excitation * self.k * (1.0 - self.a))
    }
}

// --- Mitchell–Schaeffer -----------------------------------------------------------------------

/// Mitchell & Schaeffer 2003 (Bull Math Biol 65:767):
///
/// ```text
/// dv/dt = h v² (1 − v)/τ_in − v/τ_out + I
/// dh/dt = (1 − h)/τ_open  (v < v_gate),   −h/τ_close  (v ≥ v_gate)
/// ```
///
/// APD restitution is analytic: `APD(DI) = τ_close · ln(h_max/h_min)`,
/// `h_min = 4 τ_in/τ_out`, `h_max = 1 − (1 − h_min)·e^(−DI/τ_open)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MitchellSchaeffer {
    pub tau_in_ms: f32,
    pub tau_out_ms: f32,
    pub tau_open_ms: f32,
    pub tau_close_ms: f32,
    pub v_gate: f32,
}

impl Default for MitchellSchaeffer {
    /// The paper's set: τ_in 0.3, τ_out 6, τ_open 120, τ_close 150 ms, v_gate 0.13.
    fn default() -> Self {
        Self {
            tau_in_ms: 0.3,
            tau_out_ms: 6.0,
            tau_open_ms: 120.0,
            tau_close_ms: 150.0,
            v_gate: 0.13,
        }
    }
}

impl MitchellSchaeffer {
    /// Variant for a coarse grid: τ_in and τ_out scaled together by `s` (front width ∝ τ_in at a
    /// given CV, h_min and hence the restitution curve unchanged). Research §2.3.3 suggests
    /// τ_in ≈ 1–2 ms at 2 mm.
    #[must_use]
    pub fn coarse(s: f32) -> Self {
        let d = Self::default();
        Self {
            tau_in_ms: d.tau_in_ms * s,
            tau_out_ms: d.tau_out_ms * s,
            ..d
        }
    }

    /// `h_min = 4 τ_in / τ_out`: the gate value below which the cell cannot stay excited.
    #[must_use]
    pub fn h_min(&self) -> f32 {
        4.0 * self.tau_in_ms / self.tau_out_ms
    }

    /// Analytic APD after a diastolic interval `di_ms` (Mitchell & Schaeffer 2003, eq. 9).
    #[must_use]
    pub fn analytic_apd_ms(&self, di_ms: f32) -> f32 {
        let h_min = self.h_min();
        let h_max = 1.0 - (1.0 - h_min) * (-di_ms / self.tau_open_ms).exp();
        self.tau_close_ms * (h_max / h_min).max(1.0).ln()
    }
}

impl CellModel for MitchellSchaeffer {
    const VARS: usize = 2;
    const NAME: &'static str = "mitchell_schaeffer";

    fn rest(&self, state: &mut [f32]) {
        state[0] = 0.0;
        state[1] = 1.0;
    }

    #[inline]
    fn step(&self, state: &mut [f32], i_stim: f32, dt_ms: f32) {
        let (v, h) = (state[0], state[1]);
        let dv = h * v * v * (1.0 - v) / self.tau_in_ms - v / self.tau_out_ms + i_stim;
        let dh = if v < self.v_gate {
            (1.0 - h) / self.tau_open_ms
        } else {
            -h / self.tau_close_ms
        };
        state[0] = v + dt_ms * dv;
        state[1] = h + dt_ms * dh;
    }

    #[inline]
    fn vm(&self, state: &[f32]) -> f32 {
        state[0]
    }

    fn recovery(&self, state: &[f32]) -> f32 {
        1.0 - state[1]
    }

    fn scaled_apd(&self, s: f32) -> Self {
        Self {
            tau_close_ms: self.tau_close_ms * s,
            ..*self
        }
    }

    fn scaled_excitation(&self, s: f32) -> Self {
        Self {
            tau_in_ms: self.tau_in_ms / s,
            tau_out_ms: self.tau_out_ms / s,
            ..*self
        }
    }

    fn analytic_cv(&self, d: f32) -> Option<f32> {
        // h = 1: f(v) = (1/τ_in)·v (v − v₋)(v₊ − v), v± roots of v² − v + τ_in/τ_out
        let disc = 1.0 - 4.0 * self.tau_in_ms / self.tau_out_ms;
        if disc <= 0.0 {
            return None;
        }
        let (lo, hi) = (f32::midpoint(1.0, -disc.sqrt()), f32::midpoint(1.0, disc.sqrt()));
        Some(cubic_front_speed(1.0 / self.tau_in_ms, d, lo, hi))
    }

    fn max_dt_ms(&self) -> f32 {
        self.tau_in_ms
    }
}

// --- Fenton–Karma 3V --------------------------------------------------------------------------

/// Fenton & Karma 1998 (Chaos 8:20) three-variable model, research §2.3.4:
///
/// ```text
/// du/dt = −(J_fi + J_so + J_si) + I
/// J_fi = −v H(u − u_c)(1 − u)(u − u_c)/τ_d
/// J_so = u (1 − H(u − u_c))/τ_o + H(u − u_c)/τ_r
/// J_si = −w (1 + tanh(k (u − u_c^si)))/(2 τ_si)
/// dv/dt = (1 − H)(1 − v)/τ_v⁻(u) − H v/τ_v⁺,   τ_v⁻ = τ_v1⁻ if u > u_v else τ_v2⁻
/// dw/dt = (1 − H)(1 − w)/τ_w⁻ − H w/τ_w⁺
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FentonKarma {
    pub tau_v_plus_ms: f32,
    pub tau_v1_minus_ms: f32,
    pub tau_v2_minus_ms: f32,
    pub tau_w_plus_ms: f32,
    pub tau_w_minus_ms: f32,
    pub tau_d_ms: f32,
    pub tau_o_ms: f32,
    pub tau_r_ms: f32,
    pub tau_si_ms: f32,
    pub k: f32,
    pub u_c_si: f32,
    pub u_c: f32,
    pub u_v: f32,
}

impl Default for FentonKarma {
    /// The "BR" set of research §2.3.4 (Fenton & Karma 1998 with the erratum values of τ_r and
    /// τ_si; the table there is flagged *verify* against the paper).
    fn default() -> Self {
        Self {
            tau_v_plus_ms: 3.33,
            tau_v1_minus_ms: 1250.0,
            tau_v2_minus_ms: 19.6,
            tau_w_plus_ms: 870.0,
            tau_w_minus_ms: 41.0,
            tau_d_ms: 0.25,
            tau_o_ms: 12.5,
            tau_r_ms: 33.33,
            tau_si_ms: 29.0,
            k: 10.0,
            u_c_si: 0.85,
            u_c: 0.13,
            u_v: 0.04,
        }
    }
}

impl FentonKarma {
    /// Variant for a coarse grid: the upstroke (τ_d) and the fast-inward gate (τ_v⁺) slowed
    /// together by `s`, which widens the front ∝ sqrt(s) at a given CV (research §2.3.4: τ_d
    /// 0.5–1 ms on a 1–2 mm grid). Raising τ_d alone fails: v closes before the upstroke ends.
    #[must_use]
    pub fn coarse(s: f32) -> Self {
        Self::default().scaled_excitation(1.0 / s)
    }
}

impl CellModel for FentonKarma {
    const VARS: usize = 3;
    const NAME: &'static str = "fenton_karma_3v";

    fn rest(&self, state: &mut [f32]) {
        state[0] = 0.0;
        state[1] = 1.0;
        state[2] = 1.0;
    }

    #[inline]
    fn step(&self, state: &mut [f32], i_stim: f32, dt_ms: f32) {
        let (u, v, w) = (state[0], state[1], state[2]);
        let excited = u >= self.u_c;
        let (j_fi, j_so, dv, dw) = if excited {
            (
                -v * (1.0 - u) * (u - self.u_c) / self.tau_d_ms,
                1.0 / self.tau_r_ms,
                -v / self.tau_v_plus_ms,
                -w / self.tau_w_plus_ms,
            )
        } else {
            let tau_v = if u > self.u_v {
                self.tau_v1_minus_ms
            } else {
                self.tau_v2_minus_ms
            };
            (
                0.0,
                u / self.tau_o_ms,
                (1.0 - v) / tau_v,
                (1.0 - w) / self.tau_w_minus_ms,
            )
        };
        let j_si = -w * (1.0 + (self.k * (u - self.u_c_si)).tanh()) / (2.0 * self.tau_si_ms);
        state[0] = u + dt_ms * (-(j_fi + j_so + j_si) + i_stim);
        state[1] = v + dt_ms * dv;
        state[2] = w + dt_ms * dw;
    }

    #[inline]
    fn vm(&self, state: &[f32]) -> f32 {
        state[0]
    }

    fn recovery(&self, state: &[f32]) -> f32 {
        1.0 - state[1]
    }

    fn scaled_apd(&self, s: f32) -> Self {
        // the plateau ends when J_si (fed by w, decaying with τ_w⁺) can no longer balance J_so
        Self {
            tau_w_plus_ms: self.tau_w_plus_ms * s,
            ..*self
        }
    }

    fn scaled_excitation(&self, s: f32) -> Self {
        // the fast inward gate v must stay open for the whole (slower or faster) upstroke
        Self {
            tau_d_ms: self.tau_d_ms / s,
            tau_v_plus_ms: self.tau_v_plus_ms / s,
            ..*self
        }
    }

    fn analytic_cv(&self, _d: f32) -> Option<f32> {
        None
    }

    fn max_dt_ms(&self) -> f32 {
        // fast inward current: rate ≈ (1 − u_c)/τ_d at its steepest
        self.tau_d_ms / (1.0 - self.u_c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Upstroke-to-repolarisation time (u back below 0.1) of one stimulated cell, ms.
    fn apd<M: CellModel>(m: &M, dt: f32) -> f32 {
        let mut s = vec![0.0; M::VARS];
        m.rest(&mut s);
        let (mut t, mut up) = (0.0f32, None);
        while t < 3000.0 {
            let i = if t < 1.0 { 0.5 } else { 0.0 };
            m.step(&mut s, i, dt);
            t += dt;
            match up {
                None if m.vm(&s) > 0.5 => up = Some(t),
                Some(t0) if m.vm(&s) < 0.1 => return t - t0,
                _ => {}
            }
        }
        f32::NAN
    }

    #[test]
    fn every_model_fires_one_action_potential_and_recovers() {
        let ap = apd(&AlievPanfilov::default(), 0.05);
        let ms = apd(&MitchellSchaeffer::default(), 0.01);
        let fk = apd(&FentonKarma::default(), 0.02);
        assert!((150.0..450.0).contains(&ap), "AP APD {ap}");
        assert!((150.0..400.0).contains(&ms), "MS APD {ms}");
        assert!((100.0..400.0).contains(&fk), "FK APD {fk}");
        // the APD dial works for all three
        let ap2 = apd(&AlievPanfilov::default().scaled_apd(1.3), 0.05);
        assert!(ap2 > ap * 1.15, "{ap} → {ap2}");
        let fk2 = apd(&FentonKarma::default().scaled_apd(1.3), 0.02);
        assert!(fk2 > fk * 1.1, "{fk} → {fk2}");
    }
}
