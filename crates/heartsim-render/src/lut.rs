//! Colour look-up tables as 256×1 RGBA8 textures. Scalars are normalised to [0, 1] by the caller.

/// Voltage map: blue (rest) → yellow → red (depolarised); matches what students see in textbooks.
#[must_use]
pub fn voltage_lut() -> [[u8; 4]; 256] {
    let mut out = [[0u8; 4]; 256];
    for (i, px) in out.iter_mut().enumerate() {
        let t = i as f32 / 255.0;
        let (r, g, b) = if t < 0.5 {
            let u = t * 2.0; // blue → yellow
            (u, u * 0.9, 1.0 - u)
        } else {
            let u = (t - 0.5) * 2.0; // yellow → red
            (1.0, 0.9 * (1.0 - u), 0.0)
        };
        *px = [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, 255];
    }
    out
}

/// Isochrone map (activation time): perceptually ordered viridis-like ramp.
#[must_use]
pub fn isochrone_lut() -> [[u8; 4]; 256] {
    let mut out = [[0u8; 4]; 256];
    for (i, px) in out.iter_mut().enumerate() {
        let t = i as f32 / 255.0;
        let r = (0.27 + 0.9 * t * t).min(1.0);
        let g = (0.0 + 1.1 * t).min(0.9);
        let b = (0.33 + 0.6 * (1.0 - t) * (0.5 + 0.5 * t)).min(1.0);
        *px = [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, 255];
    }
    out
}

/// Action-potential-duration map: short APD (hot, red) → long APD (cool, blue), so that regions
/// that repolarise early — the substrate of re-entry — stand out as warm colours.
#[must_use]
pub fn apd_lut() -> [[u8; 4]; 256] {
    let mut out = [[0u8; 4]; 256];
    for (i, px) in out.iter_mut().enumerate() {
        let t = i as f32 / 255.0;
        let r = (1.0 - 1.4 * t).clamp(0.1, 1.0);
        let g = (1.6 * (0.5 - (t - 0.5).abs())).clamp(0.1, 0.9) + 0.1 * t;
        let b = (0.25 + 1.2 * t).clamp(0.0, 1.0);
        *px = [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, 255];
    }
    out
}

/// Solid colour used to highlight the His–Purkinje system (the scalar is ignored): a saturated
/// green-cyan that no other map uses, so the conduction tissue is never confused with myocardium.
#[must_use]
pub fn conduction_lut() -> [[u8; 4]; 256] {
    [[26, 230, 191, 255]; 256]
}

/// Colour-blind-safe voltage map (cividis-like: dark blue → grey → yellow). It varies
/// monotonically in lightness and avoids the red–green axis, so rest and depolarised tissue stay
/// distinguishable under deuteranopia and protanopia (Nuñez et al. 2018, *PLoS ONE* 13:e0199239).
#[must_use]
pub fn voltage_colour_blind_lut() -> [[u8; 4]; 256] {
    // Control points of the cividis ramp (sRGB), linearly interpolated.
    const STOPS: [[f32; 3]; 5] = [
        [0.000, 0.135, 0.304],
        [0.268, 0.302, 0.420],
        [0.488, 0.482, 0.471],
        [0.738, 0.682, 0.418],
        [0.995, 0.906, 0.144],
    ];
    let mut out = [[0u8; 4]; 256];
    for (i, px) in out.iter_mut().enumerate() {
        let x = i as f32 / 255.0 * (STOPS.len() - 1) as f32;
        let k = (x.floor() as usize).min(STOPS.len() - 2);
        let f = x - k as f32;
        let c = |j: usize| STOPS[k][j] + (STOPS[k + 1][j] - STOPS[k][j]) * f;
        *px = [
            (c(0) * 255.0).round() as u8,
            (c(1) * 255.0).round() as u8,
            (c(2) * 255.0).round() as u8,
            255,
        ];
    }
    out
}

/// Which family of colour maps the scene uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LutStyle {
    /// Textbook maps (blue → yellow → red voltage).
    #[default]
    Classic,
    /// Maps that stay readable with red–green colour-vision deficiency.
    ColourBlindSafe,
}

/// Identifies one row of the LUT texture. The discriminant is the row index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LutId {
    /// Membrane voltage, blue (rest) → red (depolarised); see [`voltage_lut`].
    Voltage = 0,
    /// Activation-time isochrones; see [`isochrone_lut`].
    Isochrone = 1,
    /// Action-potential duration; see [`apd_lut`].
    Apd = 2,
    /// Solid conduction-system highlight; see [`conduction_lut`].
    Conduction = 3,
}

/// Number of rows in the LUT texture (one per [`LutId`]).
pub const LUT_ROWS: u32 = 4;

impl LutId {
    /// Default contents of this row.
    #[must_use]
    pub fn default_table(self) -> [[u8; 4]; 256] {
        match self {
            Self::Voltage => voltage_lut(),
            Self::Isochrone => isochrone_lut(),
            Self::Apd => apd_lut(),
            Self::Conduction => conduction_lut(),
        }
    }

    /// Contents of this row in `style`. The isochrone and APD ramps avoid red–green
    /// discrimination already, so only the voltage map has a colour-blind variant.
    #[must_use]
    pub fn table(self, style: LutStyle) -> [[u8; 4]; 256] {
        match (self, style) {
            (Self::Voltage, LutStyle::ColourBlindSafe) => voltage_colour_blind_lut(),
            _ => self.default_table(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_lut_is_opaque_and_ids_index_rows() {
        for id in [LutId::Voltage, LutId::Isochrone, LutId::Apd, LutId::Conduction] {
            assert!(id.default_table().iter().all(|px| px[3] == 255));
            assert!((id as u32) < LUT_ROWS);
        }
    }

    #[test]
    fn colour_blind_voltage_is_monotonic_in_lightness_and_has_no_red_green_swap() {
        let t = voltage_colour_blind_lut();
        let luma =
            |p: [u8; 4]| 0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2]);
        for w in t.windows(2) {
            assert!(luma(w[1]) >= luma(w[0]) - 0.5, "lightness never decreases");
        }
        assert!(
            t[0][2] > t[0][0] && t[255][0] > t[255][2],
            "blue at rest, yellow at peak"
        );
        assert_eq!(LutId::Voltage.table(LutStyle::Classic), voltage_lut());
        assert_eq!(LutId::Apd.table(LutStyle::ColourBlindSafe), apd_lut());
    }

    #[test]
    fn conduction_highlight_is_solid() {
        let t = conduction_lut();
        assert!(t.iter().all(|px| *px == t[0]));
    }

    #[test]
    fn apd_runs_warm_to_cool() {
        let t = apd_lut();
        assert!(t[0][0] > t[0][2], "short APD is red-dominant");
        assert!(t[255][2] > t[255][0], "long APD is blue-dominant");
    }
}
