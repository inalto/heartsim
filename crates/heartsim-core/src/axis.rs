//! Frontal-plane electrical axis: computation from leads and clinical classification.
//!
//! Convention: degrees on the hexaxial (Cabrera) system, 0° = lead I (patient's left),
//! +90° = aVF (inferior), −90° superior; range (−180°, 180°].

use crate::lead::{Lead, TwelveLead};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AxisClass {
    /// −30° … +90°
    Normal,
    /// −90° … −30°
    LeftDeviation,
    /// +90° … +180°
    RightDeviation,
    /// −180° … −90° ("north-west", extreme, indeterminate)
    ExtremeDeviation,
}

impl AxisClass {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            AxisClass::Normal => "Normal axis",
            AxisClass::LeftDeviation => "Left axis deviation",
            AxisClass::RightDeviation => "Right axis deviation",
            AxisClass::ExtremeDeviation => "Extreme axis deviation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FrontalAxis {
    pub degrees: f64,
}

impl FrontalAxis {
    /// From net amplitudes (area or peak-to-peak-signed) of leads I and aVF.
    /// `axis = atan2(aVF, I)` is exact because I and aVF are orthogonal.
    #[must_use]
    pub fn from_i_avf(lead_i: f64, lead_avf: f64) -> Self {
        Self {
            degrees: lead_avf.atan2(lead_i).to_degrees(),
        }
    }

    /// From leads I and II (non-orthogonal, 60° apart). Equivalent to the classic
    /// `aVF = (2·II − I)/√3·(√3/2)…` derivation; we solve the 2×2 projection exactly.
    #[must_use]
    pub fn from_i_ii(lead_i: f64, lead_ii: f64) -> Self {
        // II = x cos60 + y sin60  ⇒ y = (II − 0.5 x) / sin60
        let x = lead_i;
        let y = (lead_ii - 0.5 * x) / (60.0f64.to_radians().sin());
        Self {
            degrees: y.atan2(x).to_degrees(),
        }
    }

    /// Least-squares fit over all six limb leads (robust to a single noisy lead).
    #[must_use]
    pub fn from_limb_leads(sample: &TwelveLead) -> Self {
        // Each limb lead l measures  v_l = x cos θ_l + y sin θ_l. Solve normal equations.
        let (mut sxx, mut sxy, mut syy, mut bx, mut by) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for lead in Lead::ALL.iter().filter(|l| l.is_limb()) {
            let th = lead.frontal_angle_deg().expect("limb lead").to_radians();
            let (s, c) = th.sin_cos();
            let v = sample.get(*lead);
            sxx += c * c;
            sxy += c * s;
            syy += s * s;
            bx += c * v;
            by += s * v;
        }
        let det = sxx * syy - sxy * sxy;
        let x = (syy * bx - sxy * by) / det;
        let y = (sxx * by - sxy * bx) / det;
        Self {
            degrees: y.atan2(x).to_degrees(),
        }
    }

    /// Project a frontal-plane vector of this angle (unit magnitude) onto a limb lead.
    /// This is the formula the open-source ECG axis trainer uses: `A = cos(axis − θ_lead)`.
    #[must_use]
    pub fn projection_onto(&self, lead: Lead) -> Option<f64> {
        lead.frontal_angle_deg()
            .map(|th| (self.degrees - th).to_radians().cos())
    }

    #[must_use]
    pub fn classify(&self) -> AxisClass {
        let d = Self::normalise(self.degrees);
        if (-30.0..=90.0).contains(&d) {
            AxisClass::Normal
        } else if (-90.0..-30.0).contains(&d) {
            AxisClass::LeftDeviation
        } else if d > 90.0 && d <= 180.0 {
            AxisClass::RightDeviation
        } else {
            AxisClass::ExtremeDeviation
        }
    }

    /// Wrap to (−180, 180].
    #[must_use]
    pub fn normalise(deg: f64) -> f64 {
        let mut d = deg % 360.0;
        if d <= -180.0 {
            d += 360.0;
        }
        if d > 180.0 {
            d -= 360.0;
        }
        d
    }
}

/// The four clinical class boundaries on the hexaxial wheel, degrees in (−180°, 180°]: −90°
/// (left/extreme), −30° (normal/left), +90° (normal/right) and ±180° (right/extreme).
pub const CLASS_BOUNDARIES_DEG: [f64; 4] = [-90.0, -30.0, 90.0, 180.0];

/// Half-width of the guard band around every class boundary in the axis drill, degrees. A trace
/// 7° either side of a boundary reads as the neighbouring class to a learner measuring by eye, so
/// the drill never asks one (the ECG-axis trainer the drill follows uses 5–10°).
pub const DRILL_GUARD_DEG: f64 = 7.0;

/// The "north-west" region the drill never asks, on the clockwise 0°…360° wheel of the reference
/// trainer (0° = lead I, 90° = aVF): `[170°, 280°)`, i.e. from just inside the right-axis zone
/// through the extreme zone to 10° past −90°. Extreme deviation is rare and its borders are
/// ambiguous, so it is a topic for the explanation, not the drill.
pub const DRILL_EXCLUDED_DEG: (f64, f64) = (170.0, 280.0);

/// Smallest angle between two directions, degrees (0…180).
#[must_use]
pub fn angular_distance_deg(a: f64, b: f64) -> f64 {
    FrontalAxis::normalise(a - b).abs()
}

/// Whether the axis drill may ask this axis: outside [`DRILL_EXCLUDED_DEG`] and more than
/// [`DRILL_GUARD_DEG`] from every class boundary. Any angle is accepted; it is wrapped first.
#[must_use]
pub fn drill_axis_allowed(deg: f64) -> bool {
    let d = FrontalAxis::normalise(deg);
    let clockwise = d.rem_euclid(360.0);
    if (DRILL_EXCLUDED_DEG.0..DRILL_EXCLUDED_DEG.1).contains(&clockwise) {
        return false;
    }
    CLASS_BOUNDARIES_DEG
        .iter()
        .all(|b| angular_distance_deg(d, *b) > DRILL_GUARD_DEG)
}

/// Draw a drill axis in (−180°, 180°] by rejection sampling from `uniform`, a source of numbers in
/// `[0, 1)`. The result always satisfies [`drill_axis_allowed`]. About 60 % of draws are accepted;
/// after 1000 rejections (a broken source) it returns 45°, a safe normal axis, so it cannot hang.
pub fn draw_drill_axis(mut uniform: impl FnMut() -> f64) -> f64 {
    for _ in 0..1000 {
        let d = FrontalAxis::normalise(uniform() * 360.0 - 180.0);
        if drill_axis_allowed(d) {
            return d;
        }
    }
    45.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lead::LEAD_COUNT;
    use approx::assert_abs_diff_eq;
    use proptest::prelude::*;

    fn synthetic(axis_deg: f64) -> TwelveLead {
        let ax = FrontalAxis { degrees: axis_deg };
        let mut out = [0.0; LEAD_COUNT];
        for l in Lead::ALL {
            if let Some(p) = ax.projection_onto(l) {
                out[l as usize] = p;
            }
        }
        TwelveLead(out)
    }

    #[test]
    fn roundtrip_axis_from_synthetic_leads() {
        for deg in [-170.0, -90.0, -45.0, -30.0, 0.0, 30.0, 60.0, 90.0, 120.0, 179.0] {
            let s = synthetic(deg);
            assert_abs_diff_eq!(
                FrontalAxis::from_i_avf(s.get(Lead::I), s.get(Lead::AVF)).degrees,
                deg,
                epsilon = 1e-9
            );
            assert_abs_diff_eq!(
                FrontalAxis::from_i_ii(s.get(Lead::I), s.get(Lead::II)).degrees,
                deg,
                epsilon = 1e-9
            );
            assert_abs_diff_eq!(FrontalAxis::from_limb_leads(&s).degrees, deg, epsilon = 1e-9);
        }
    }

    #[test]
    fn classification_boundaries() {
        assert_eq!(FrontalAxis { degrees: 0.0 }.classify(), AxisClass::Normal);
        assert_eq!(FrontalAxis { degrees: -30.0 }.classify(), AxisClass::Normal);
        assert_eq!(
            FrontalAxis { degrees: -31.0 }.classify(),
            AxisClass::LeftDeviation
        );
        assert_eq!(
            FrontalAxis { degrees: 91.0 }.classify(),
            AxisClass::RightDeviation
        );
        assert_eq!(
            FrontalAxis { degrees: -120.0 }.classify(),
            AxisClass::ExtremeDeviation
        );
        assert_eq!(
            FrontalAxis { degrees: 240.0 }.classify(),
            AxisClass::ExtremeDeviation
        );
    }
    /// xorshift64*, the generator the quiz uses.
    fn rng(seed: u64) -> impl FnMut() -> f64 {
        let mut state = seed.max(1);
        move || {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            (state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    #[test]
    fn the_guard_bands_cover_the_excluded_regions() {
        // north-west: 170°..280° clockwise is -190°..-80°, i.e. 170°..180° and -180°..-80°
        for d in [170.0, 175.0, 180.0, -179.0, -135.0, -90.0, -81.0] {
            assert!(!drill_axis_allowed(d), "{d}");
        }
        // around -30° and +90°
        for d in [-37.0, -30.0, -23.0, 83.0, 90.0, 97.0] {
            assert!(!drill_axis_allowed(d), "{d}");
        }
        // allowed: clearly inside a class
        for d in [
            -60.0, -45.0, -10.0, 0.0, 30.0, 60.0, 75.0, 100.0, 120.0, 160.0, 169.0,
        ] {
            assert!(drill_axis_allowed(d), "{d}");
        }
        // angles are wrapped first
        assert_eq!(drill_axis_allowed(360.0 + 30.0), drill_axis_allowed(30.0));
        assert_eq!(drill_axis_allowed(-330.0), drill_axis_allowed(30.0));
    }

    #[test]
    fn ten_thousand_draws_never_land_in_a_guard_band() {
        for seed in [1_u64, 7, 0x9E37_79B9_7F4A_7C15, 0xDEAD_BEEF] {
            let mut u = rng(seed);
            for i in 0..10_000 {
                let a = draw_drill_axis(&mut u);
                assert!((-180.0..=180.0).contains(&a), "draw {i}: {a}");
                assert!(
                    !(170.0..280.0).contains(&a.rem_euclid(360.0)),
                    "draw {i}: {a} is north-west"
                );
                for b in CLASS_BOUNDARIES_DEG {
                    assert!(
                        angular_distance_deg(a, b) > DRILL_GUARD_DEG,
                        "draw {i}: {a} is within the guard band of {b}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_drawn_axis_keeps_its_class_when_read_seven_degrees_off() {
        let mut u = rng(42);
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..10_000 {
            let a = draw_drill_axis(&mut u);
            let class = FrontalAxis { degrees: a }.classify();
            for off in [-DRILL_GUARD_DEG, DRILL_GUARD_DEG] {
                let shifted = FrontalAxis { degrees: a + off }.classify();
                assert_eq!(shifted, class, "axis {a}° read {off:+}° off changes class");
            }
            assert_ne!(
                class,
                AxisClass::ExtremeDeviation,
                "{a}° is extreme: the drill excludes it"
            );
            seen.insert(class.label());
        }
        assert_eq!(seen.len(), 3, "normal, left and right are all asked: {seen:?}");
    }

    #[test]
    fn a_broken_source_cannot_hang_the_drill() {
        assert_eq!(draw_drill_axis(|| 0.0), 45.0); // always -180°, which is excluded
    }

    proptest! {
        #[test]
        fn every_allowed_axis_is_clear_of_all_boundaries(deg in -720.0f64..720.0) {
            if drill_axis_allowed(deg) {
                for b in CLASS_BOUNDARIES_DEG {
                    prop_assert!(angular_distance_deg(deg, b) > DRILL_GUARD_DEG);
                }
            }
        }

        #[test]
        fn drill_draws_are_allowed_for_any_seed(seed in 1u64..u64::MAX) {
            let mut u = rng(seed);
            for _ in 0..200 {
                prop_assert!(drill_axis_allowed(draw_drill_axis(&mut u)));
            }
        }
    }
}
