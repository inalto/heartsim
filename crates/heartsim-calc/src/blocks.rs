//! Shared building blocks: the few relations nearly every Doppler calculation is made of.

use std::f64::consts::PI;

/// Simplified Bernoulli equation: pressure drop across a narrowing from the peak jet velocity,
/// ΔP = 4v² (mmHg for v in m/s), neglecting the velocity proximal to it, viscous losses and flow
/// acceleration.
#[must_use]
pub fn bernoulli_mmhg(v_m_s: f64) -> f64 {
    4.0 * v_m_s * v_m_s
}

/// Bernoulli with the proximal velocity kept: ΔP = 4(v₂² − v₁²), used when the proximal velocity
/// is not negligible (over about 1.5 m/s).
#[must_use]
pub fn bernoulli_proximal_mmhg(v2_m_s: f64, v1_m_s: f64) -> f64 {
    4.0 * (v2_m_s * v2_m_s - v1_m_s * v1_m_s)
}

/// Area of a circle from its diameter, cm²: π(d/2)². Outflow tracts and annuli are assumed circular.
#[must_use]
pub fn circle_area_cm2(d_cm: f64) -> f64 {
    PI * (d_cm / 2.0).powi(2)
}

/// Stroke volume through a circular site, mL: area × velocity–time integral.
#[must_use]
pub fn stroke_volume_ml(d_cm: f64, vti_cm: f64) -> f64 {
    circle_area_cm2(d_cm) * vti_cm
}

/// Continuity equation: the same volume crosses both sites, so area₂ = area₁ × VTI₁ ÷ VTI₂ (cm²).
#[must_use]
pub fn continuity_area_cm2(area1_cm2: f64, vti1_cm: f64, vti2_cm: f64) -> f64 {
    area1_cm2 * vti1_cm / vti2_cm
}

/// Flow through a hemispheric proximal isovelocity surface, mL/s: 2πr² × aliasing velocity.
#[must_use]
pub fn pisa_flow_ml_s(r_cm: f64, alias_cm_s: f64) -> f64 {
    2.0 * PI * r_cm * r_cm * alias_cm_s
}

/// Orifice area from the PISA flow and the peak orifice velocity, cm²: flow ÷ v (v in m/s
/// converted to cm/s).
#[must_use]
pub fn pisa_orifice_cm2(r_cm: f64, alias_cm_s: f64, vmax_m_s: f64) -> f64 {
    pisa_flow_ml_s(r_cm, alias_cm_s) / (vmax_m_s * 100.0)
}

/// Correction of a PISA area for a funnel narrower than a flat plane: × α/180°.
#[must_use]
pub fn pisa_angle_corrected(area_cm2: f64, angle_deg: f64) -> f64 {
    area_cm2 * angle_deg / 180.0
}

/// Mosteller body surface area, m²: √(height[cm] × weight[kg] ÷ 3600).
#[must_use]
pub fn bsa_mosteller_m2(height_cm: f64, weight_kg: f64) -> f64 {
    (height_cm * weight_kg / 3600.0).sqrt()
}

/// Du Bois body surface area, m²: 0.007184 × W^0.425 × H^0.725 (the 1916 constant 71.84 for cm²).
#[must_use]
pub fn bsa_du_bois_m2(height_cm: f64, weight_kg: f64) -> f64 {
    0.007_184 * weight_kg.powf(0.425) * height_cm.powf(0.725)
}

/// Haycock body surface area, m²: 0.024265 × W^0.5378 × H^0.3964.
#[must_use]
pub fn bsa_haycock_m2(height_cm: f64, weight_kg: f64) -> f64 {
    0.024_265 * weight_kg.powf(0.5378) * height_cm.powf(0.3964)
}

/// A measurement divided by body surface area.
#[must_use]
pub fn indexed(value: f64, bsa_m2: f64) -> f64 {
    value / bsa_m2
}

/// Change from `from` to `to` as a percentage of `from`: (to − from) ÷ from × 100.
#[must_use]
pub fn percent_change(from: f64, to: f64) -> f64 {
    (to - from) / from * 100.0
}

/// Fraction emptied or shortened, %: (end-diastolic − end-systolic) ÷ end-diastolic × 100.
#[must_use]
pub fn fraction_percent(end_diastolic: f64, end_systolic: f64) -> f64 {
    (end_diastolic - end_systolic) / end_diastolic * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn bernoulli_closed_forms() {
        // 4 × 4² = 64
        assert!(close(bernoulli_mmhg(4.0), 64.0, 1e-12));
        // 4 × (5² − 1²) = 96
        assert!(close(bernoulli_proximal_mmhg(5.0, 1.0), 96.0, 1e-12));
        assert!(close(
            bernoulli_proximal_mmhg(3.0, 0.0),
            bernoulli_mmhg(3.0),
            1e-12
        ));
    }

    #[test]
    fn areas_volumes_and_continuity() {
        // π × 1² = 3.14159…
        assert!(close(circle_area_cm2(2.0), PI, 1e-12));
        // 2.0 cm and 20 cm: π × 20 = 62.83 mL
        assert!(close(stroke_volume_ml(2.0, 20.0), 20.0 * PI, 1e-9));
        // the regurgitation guideline's own SV form 0.785 d² VTI: d 2.3 cm, VTI 14 cm → 58 mL (p. 311)
        assert!(close(stroke_volume_ml(2.3, 14.0), 58.2, 0.1));
        // π × 20 ÷ 80 = 0.785 cm²
        assert!(close(continuity_area_cm2(PI, 20.0, 80.0), PI / 4.0, 1e-12));
    }

    #[test]
    fn pisa_against_the_guideline_example() {
        // ASE 2017 regurgitation, Figure 11 caption (p. 321): r 1.0 cm, Va 29.9 cm/s, peak 527 cm/s → 0.36 cm²
        let a = pisa_orifice_cm2(1.0, 29.9, 5.27);
        assert!(close(a, 0.356, 0.001), "{a}");
        // flow 2π × 0.5² × 40 = 62.83 mL/s
        assert!(close(pisa_flow_ml_s(0.5, 40.0), 20.0 * PI, 1e-9));
        // a 120° funnel keeps two thirds of the area
        assert!(close(pisa_angle_corrected(1.5, 120.0), 1.0, 1e-12));
    }

    #[test]
    fn bsa_formulas_agree_for_an_adult_and_match_the_closed_form() {
        // Mosteller: √(170 × 70 ÷ 3600) = √3.3056 = 1.818
        assert!(close(bsa_mosteller_m2(170.0, 70.0), 1.818, 0.001));
        // √(180 × 80 ÷ 3600) = √4 = 2 exactly
        assert!(close(bsa_mosteller_m2(180.0, 80.0), 2.0, 1e-12));
        // Du Bois exponents 1/2.35 and 1/1.38 as written in the 1916 note
        assert!(close(0.425, 1.0 / 2.35, 0.001) && close(0.725, 1.0 / 1.38, 0.001));
        // the three formulas agree within 2 % for a 170 cm, 70 kg adult
        let m = bsa_mosteller_m2(170.0, 70.0);
        for other in [bsa_du_bois_m2(170.0, 70.0), bsa_haycock_m2(170.0, 70.0)] {
            assert!((other / m - 1.0).abs() < 0.02, "{other} vs {m}");
        }
    }

    #[test]
    fn indexing_and_changes() {
        assert!(close(indexed(100.0, 2.0), 50.0, 1e-12));
        assert!(close(percent_change(20.0, 17.0), -15.0, 1e-12));
        assert!(close(fraction_percent(120.0, 48.0), 60.0, 1e-12));
    }
}
