//! Flat colours and names of the tissue labels of the echo cross-section (brief B31 §12).
//!
//! One table works on the light and the dark panel (every colour sits in the middle of the
//! lightness range, and the sector is outlined in the theme's text colour), and a second table
//! keeps every pair of labels apart for red–green colour-vision deficiency
//! (`tests/echo_page.rs` simulates deuteranopia and protanopia and bounds the smallest colour
//! difference of either table). The colour-blind table follows the "colour-blind-safe" switch of
//! the 3-D colour maps (`LutStyle::ColourBlindSafe`). The values were found by a random search
//! that maximised the smallest CIELAB distance between any two labels while staying near a
//! tissue-like hue (blood warm on the left side and cool on the right, bone pale, liver brown):
//! designed, no source.

use heartsim_geometry::Label;
use heartsim_render::PALETTE_SLOTS;
use heartsim_ui::t;

/// sRGB of `label` in the classic table (`cvd = false`) or the colour-blind-safe one.
#[must_use]
pub fn rgb(label: Label, cvd: bool) -> [u8; 3] {
    let (classic, safe): ([u8; 3], [u8; 3]) = match label {
        Label::Outside => ([40, 42, 48], [40, 42, 48]),
        Label::LvMyocardium => ([148, 28, 36], [100, 55, 9]),
        Label::RvMyocardium => ([231, 90, 144], [142, 159, 233]),
        Label::LaMyocardium => ([149, 102, 77], [215, 184, 121]),
        Label::RaMyocardium => ([178, 129, 179], [112, 88, 187]),
        Label::LvBlood => ([245, 32, 56], [192, 143, 7]),
        Label::RvBlood => ([23, 85, 244], [1, 69, 202]),
        Label::LaBlood => ([255, 133, 114], [249, 189, 80]),
        Label::RaBlood => ([113, 196, 221], [95, 144, 255]),
        Label::AortaWall => ([117, 19, 87], [188, 68, 0]),
        Label::AortaLumen => ([223, 49, 190], [236, 124, 170]),
        Label::PulmonaryArteryWall => ([1, 121, 89], [14, 69, 60]),
        Label::PulmonaryArteryLumen => ([48, 226, 196], [11, 144, 113]),
        Label::ValveLeaflet => ([223, 255, 119], [237, 243, 143]),
        Label::PapillaryMuscle => ([255, 208, 26], [208, 238, 33]),
        Label::ModeratorBand => ([194, 126, 5], [117, 142, 58]),
        Label::Pericardium => ([255, 231, 251], [206, 243, 236]),
        Label::Bone => ([193, 200, 158], [242, 179, 169]),
        Label::Lung => ([58, 115, 196], [125, 120, 175]),
        Label::Liver => ([99, 116, 24], [94, 93, 62]),
        Label::SoftTissue => ([81, 103, 121], [105, 93, 121]),
        Label::ThoracicBlood => ([61, 33, 133], [40, 54, 117]),
    };
    if cvd { safe } else { classic }
}

/// The shader's palette: the colour of every label byte (zero for unassigned bytes).
#[must_use]
pub fn palette(cvd: bool) -> [[u8; 3]; PALETTE_SLOTS] {
    let mut table = [[0u8; 3]; PALETTE_SLOTS];
    for l in Label::ALL {
        if let Some(slot) = table.get_mut(usize::from(l.to_u8())) {
            *slot = rgb(l, cvd);
        }
    }
    table
}

/// The name of a tissue in the interface language (the legend and the hover read-out).
#[must_use]
pub fn name(label: Label) -> String {
    match label {
        Label::Outside => t!("echo-label-outside"),
        Label::LvMyocardium => t!("echo-label-lv-myocardium"),
        Label::RvMyocardium => t!("echo-label-rv-myocardium"),
        Label::LaMyocardium => t!("echo-label-la-myocardium"),
        Label::RaMyocardium => t!("echo-label-ra-myocardium"),
        Label::LvBlood => t!("echo-label-lv-blood"),
        Label::RvBlood => t!("echo-label-rv-blood"),
        Label::LaBlood => t!("echo-label-la-blood"),
        Label::RaBlood => t!("echo-label-ra-blood"),
        Label::AortaWall => t!("echo-label-aorta-wall"),
        Label::AortaLumen => t!("echo-label-aorta-lumen"),
        Label::PulmonaryArteryWall => t!("echo-label-pa-wall"),
        Label::PulmonaryArteryLumen => t!("echo-label-pa-lumen"),
        Label::ValveLeaflet => t!("echo-label-leaflet"),
        Label::PapillaryMuscle => t!("echo-label-papillary"),
        Label::ModeratorBand => t!("echo-label-moderator"),
        Label::Pericardium => t!("echo-label-pericardium"),
        Label::Bone => t!("echo-label-bone"),
        Label::Lung => t!("echo-label-lung"),
        Label::Liver => t!("echo-label-liver"),
        Label::SoftTissue => t!("echo-label-soft"),
        Label::ThoracicBlood => t!("echo-label-thoracic-blood"),
    }
}
