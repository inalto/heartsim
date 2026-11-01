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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lead::LEAD_COUNT;
    use approx::assert_abs_diff_eq;

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
}
