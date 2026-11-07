//! AHA 17-segment model and coronary territories on ventricular coordinates.
//!
//! Segments are assigned from an apex–base coordinate (0 apex … 1 base) and a circumferential angle
//! measured in the short-axis plane; the "assumed" coronary distribution of the AHA statement
//! (Cerqueira et al. 2002) maps segments to LAD / LCx / RCA. The right ventricular free wall is not
//! part of the 17-segment LV model: it is RCA territory (acute marginal branches) except for its
//! anterior apical zone, [`RV_ANTERIOR_SEGMENT`], which right ventricular branches of the LAD supply.

use heartsim_core::Region;
use serde::{Deserialize, Serialize};

/// Pseudo-segment of the RV free wall supplied by the LAD: its anterior wall next to the septum, in
/// the apical half (the LAD perfuses almost 30 % of the anterior RV wall and most of its apex; Novo
/// et al., *Curr Probl Cardiol* 2022;47:101277). The rest of the RV free wall has segment 0.
pub const RV_ANTERIOR_SEGMENT: u8 = 18;

/// Coronary territory of a cell. `None` for non-ventricular tissue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Territory {
    /// Left anterior descending: anterior wall, anterior septum, apex.
    Lad,
    /// Left circumflex: lateral wall.
    Lcx,
    /// Right coronary: inferior wall, inferior septum, RV free wall.
    Rca,
    None,
}

impl Territory {
    /// Territory from region and AHA segment (0 = no segment).
    #[must_use]
    pub fn for_cell(region: Region, aha_segment: u8) -> Self {
        match (region, aha_segment) {
            (Region::RightVentricle, RV_ANTERIOR_SEGMENT) => Territory::Lad,
            (Region::RightVentricle, _) | (_, 3 | 4 | 9 | 10 | 15) => Territory::Rca,
            (_, 1 | 2 | 7 | 8 | 13 | 14 | 17) => Territory::Lad,
            (_, 5 | 6 | 11 | 12 | 16) => Territory::Lcx,
            _ => Territory::None,
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Territory::Lad => "LAD",
            Territory::Lcx => "LCx",
            Territory::Rca => "RCA",
            Territory::None => "-",
        }
    }
}

/// AHA segment (1–17) for a left-ventricular / septal cell.
///
/// * `apex_base`: 0 at the apex, 1 at the base.
/// * `angle_deg`: short-axis angle with 0° = lateral (patient's left), 90° = anterior,
///   180° = septal, 270° = inferior (the heart frame's +x/+z plane).
#[must_use]
pub fn aha_segment(apex_base: f32, angle_deg: f32) -> u8 {
    let a = angle_deg.rem_euclid(360.0);
    if apex_base < 0.12 {
        return 17;
    }
    if apex_base < 0.38 {
        // apical: 4 × 90° centred on lateral (0), anterior (90), septal (180), inferior (270)
        return match ((a + 45.0).rem_euclid(360.0) / 90.0) as u8 {
            0 => 16, // lateral
            1 => 13, // anterior
            2 => 14, // septal
            _ => 15, // inferior
        };
    }
    // basal / mid: 6 × 60° — anterolateral centred 30°, anterior 90°, anteroseptal 150°,
    // inferoseptal 210°, inferior 270°, inferolateral 330°
    let sector = (a / 60.0) as u8; // 0 = [0,60) anterolateral …
    let order = [6u8, 1, 2, 3, 4, 5];
    let seg = order[usize::from(sector.min(5))];
    if apex_base < 0.69 { seg + 6 } else { seg }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_cover_the_model() {
        assert_eq!(aha_segment(0.05, 0.0), 17);
        assert_eq!(aha_segment(0.9, 90.0), 1);
        assert_eq!(aha_segment(0.9, 150.0), 2);
        assert_eq!(aha_segment(0.9, 270.0), 4);
        assert_eq!(aha_segment(0.9, 30.0), 6);
        assert_eq!(aha_segment(0.5, 90.0), 7);
        assert_eq!(aha_segment(0.2, 180.0), 14);
        assert_eq!(aha_segment(0.2, 0.0), 16);
        assert_eq!(Territory::for_cell(Region::LeftVentricle, 1), Territory::Lad);
        assert_eq!(Territory::for_cell(Region::VentricularSeptum, 3), Territory::Rca);
        assert_eq!(Territory::for_cell(Region::RightVentricle, 0), Territory::Rca);
        assert_eq!(
            Territory::for_cell(Region::RightVentricle, RV_ANTERIOR_SEGMENT),
            Territory::Lad
        );
        assert_eq!(Territory::for_cell(Region::RightAtrium, 0), Territory::None);
    }
}
