//! The one colour palette of the interface, keyed on light or dark: soft pastel fills, white or
//! slate cards, pill labels and a calm blue accent. Widgets take their colours from here, never
//! from literals, so the two themes and the high-contrast setting stay consistent.
//!
//! The ECG paper and the 3-D colour maps keep their own clinical colours (`strip`, `colour_bar`);
//! this palette is for the chrome around them.

use egui::{Color32, Context, CornerRadius, Stroke, Theme, Ui};
use heartsim_core::AxisClass;

/// The colours of one axis class: a soft fill for its sector and legend chip, a saturated one
/// for the measured-axis badge, and the text colour that reads on each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassColours {
    /// Soft fill of the class's sector on the circle and of its legend chip.
    pub soft: Color32,
    /// Text written on `soft`.
    pub on_soft: Color32,
    /// Saturated fill of the measured-axis badge and the legend swatch.
    pub strong: Color32,
    /// Text written on `strong`.
    pub on_strong: Color32,
}

/// The palette of one theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub dark: bool,
    /// Window and panel background.
    pub panel: Color32,
    /// Card background.
    pub card: Color32,
    pub card_stroke: Color32,
    /// Shadow colour under a card (alpha included).
    pub card_shadow: Color32,
    /// Pill-shaped label background and outline.
    pub pill: Color32,
    pub pill_stroke: Color32,
    /// The calm blue accent: arrows, links, selected things.
    pub accent: Color32,
    /// The accent darkened for a title band with white text on it.
    pub accent_strong: Color32,
    /// A faint tint of the accent for bands and grouped controls.
    pub accent_soft: Color32,
    pub heading: Color32,
    pub text: Color32,
    pub muted: Color32,
    /// The outer ring and major spokes of the axis circle.
    pub ring: Color32,
    /// Dotted inner rings and minor spokes.
    pub grid: Color32,
    /// The measured-axis arrow at rest and while hovered.
    pub arrow: Color32,
    pub arrow_hover: Color32,
    /// The learner's answer in the quiz.
    pub guess: Color32,
    /// Drop shadow under the arrow (opaque; alpha is applied per pass).
    pub shadow: Color32,
    classes: [ClassColours; 4],
}

const WHITE: Color32 = Color32::WHITE;

/// Badge colours are the same in both themes: white text on a saturated class colour.
const STRONG: [ClassColours; 4] = [
    ClassColours {
        soft: Color32::from_rgb(217, 242, 220),
        on_soft: Color32::from_rgb(24, 88, 46),
        strong: Color32::from_rgb(31, 122, 58),
        on_strong: WHITE,
    },
    ClassColours {
        soft: Color32::from_rgb(253, 232, 200),
        on_soft: Color32::from_rgb(122, 74, 8),
        strong: Color32::from_rgb(180, 83, 9),
        on_strong: WHITE,
    },
    ClassColours {
        soft: Color32::from_rgb(251, 213, 216),
        on_soft: Color32::from_rgb(138, 31, 43),
        strong: Color32::from_rgb(180, 35, 54),
        on_strong: WHITE,
    },
    ClassColours {
        soft: Color32::from_rgb(240, 236, 208),
        on_soft: Color32::from_rgb(92, 88, 38),
        strong: Color32::from_rgb(104, 102, 40),
        on_strong: WHITE,
    },
];

impl Palette {
    /// The light theme: white cards on a very light blue-grey, navy headings.
    #[must_use]
    pub fn light() -> Self {
        Self {
            dark: false,
            panel: Color32::from_rgb(244, 247, 251),
            card: WHITE,
            card_stroke: Color32::from_rgb(217, 225, 236),
            card_shadow: Color32::from_rgba_unmultiplied(30, 50, 80, 22),
            pill: WHITE,
            pill_stroke: Color32::from_rgb(200, 210, 224),
            accent: Color32::from_rgb(59, 111, 182),
            accent_strong: Color32::from_rgb(31, 58, 99),
            accent_soft: Color32::from_rgb(229, 238, 250),
            heading: Color32::from_rgb(31, 58, 99),
            text: Color32::from_rgb(34, 48, 74),
            muted: Color32::from_rgb(107, 122, 144),
            ring: Color32::from_rgb(43, 63, 94),
            grid: Color32::from_rgba_unmultiplied(120, 136, 158, 170),
            arrow: Color32::from_rgb(59, 111, 182),
            arrow_hover: Color32::from_rgb(86, 140, 214),
            guess: Color32::from_rgb(166, 86, 190),
            shadow: Color32::from_rgb(20, 30, 50),
            classes: STRONG,
        }
    }

    /// The dark theme: slate cards on near-black, the same hues muted.
    #[must_use]
    pub fn dark() -> Self {
        let mut classes = STRONG;
        classes[0].soft = Color32::from_rgb(29, 58, 39);
        classes[0].on_soft = Color32::from_rgb(159, 227, 176);
        classes[1].soft = Color32::from_rgb(61, 46, 20);
        classes[1].on_soft = Color32::from_rgb(243, 199, 119);
        classes[2].soft = Color32::from_rgb(63, 31, 37);
        classes[2].on_soft = Color32::from_rgb(245, 163, 173);
        classes[3].soft = Color32::from_rgb(58, 56, 34);
        classes[3].on_soft = Color32::from_rgb(228, 221, 153);
        Self {
            dark: true,
            panel: Color32::from_rgb(23, 28, 36),
            card: Color32::from_rgb(31, 38, 48),
            card_stroke: Color32::from_rgb(47, 58, 72),
            card_shadow: Color32::from_rgba_unmultiplied(0, 0, 0, 70),
            pill: Color32::from_rgb(39, 48, 60),
            pill_stroke: Color32::from_rgb(58, 70, 86),
            accent: Color32::from_rgb(110, 168, 255),
            accent_strong: Color32::from_rgb(46, 84, 140),
            accent_soft: Color32::from_rgb(36, 48, 66),
            heading: Color32::from_rgb(220, 230, 245),
            text: Color32::from_rgb(213, 221, 232),
            muted: Color32::from_rgb(139, 152, 171),
            ring: Color32::from_rgb(195, 207, 224),
            grid: Color32::from_rgba_unmultiplied(120, 136, 158, 170),
            arrow: Color32::from_rgb(110, 168, 255),
            arrow_hover: Color32::from_rgb(150, 195, 255),
            guess: Color32::from_rgb(205, 140, 225),
            shadow: Color32::BLACK,
            classes,
        }
    }

    /// The palette of an egui theme.
    #[must_use]
    pub fn for_theme(theme: Theme) -> Self {
        match theme {
            Theme::Dark => Self::dark(),
            Theme::Light => Self::light(),
        }
    }

    /// The palette of the theme the context resolves to now (system, light or dark).
    #[must_use]
    pub fn of(ui: &Ui) -> Self {
        Self::for_theme(ui.ctx().theme())
    }

    /// The colours of an axis class.
    #[must_use]
    pub fn class(&self, class: AxisClass) -> ClassColours {
        self.classes[match class {
            AxisClass::Normal => 0,
            AxisClass::LeftDeviation => 1,
            AxisClass::RightDeviation => 2,
            AxisClass::ExtremeDeviation => 3,
        }]
    }

    /// Green, for a correct answer or a positive deflection.
    #[must_use]
    pub fn good(&self) -> ClassColours {
        self.class(AxisClass::Normal)
    }

    /// Rose, for a wrong answer or a negative deflection.
    #[must_use]
    pub fn bad(&self) -> ClassColours {
        self.class(AxisClass::RightDeviation)
    }

    /// Amber, for a warning or an equiphasic lead.
    #[must_use]
    pub fn warn(&self) -> ClassColours {
        self.class(AxisClass::LeftDeviation)
    }
}

/// Corner radius of cards and windows, points.
pub const CARD_RADIUS: u8 = 10;

/// Puts the palette's panel, window and widget colours into the style of the theme the context
/// resolves to now. Called once per frame by the application; it writes only when the style does
/// not carry the palette yet (after a theme change, or after the settings rebuilt the visuals).
/// The high-contrast setting, recognised by its text override, is left as it is: its black and
/// white are the point.
pub fn apply_to_context(ctx: &Context) {
    let theme = ctx.theme();
    let p = Palette::for_theme(theme);
    let style = ctx.style_of(theme);
    let v = &style.visuals;
    if v.override_text_color.is_some() {
        return;
    }
    if v.panel_fill == p.panel && v.window_fill == p.card && v.hyperlink_color == p.accent {
        return;
    }
    ctx.style_mut_of(theme, |s| {
        let v = &mut s.visuals;
        v.panel_fill = p.panel;
        v.window_fill = p.card;
        v.window_stroke = Stroke::new(1.0, p.card_stroke);
        v.window_corner_radius = CornerRadius::same(CARD_RADIUS);
        v.menu_corner_radius = CornerRadius::same(8);
        v.hyperlink_color = p.accent;
        v.faint_bg_color = p.accent_soft;
        v.extreme_bg_color = if p.dark { p.panel } else { p.card };
        v.selection.bg_fill = p.accent.gamma_multiply(if p.dark { 0.45 } else { 0.3 });
        v.selection.stroke = Stroke::new(1.0, p.accent);
        for w in [
            &mut v.widgets.noninteractive,
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            w.corner_radius = CornerRadius::same(6);
        }
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.card_stroke);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.accent);
        v.widgets.active.bg_stroke = Stroke::new(1.5, p.accent);
    });
}

/// WCAG relative luminance of an opaque sRGB colour, 0 (black) to 1 (white).
#[must_use]
pub fn relative_luminance(c: Color32) -> f64 {
    let lin = |u: u8| {
        let s = f64::from(u) / 255.0;
        if s <= 0.040_45 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

/// WCAG contrast ratio of two opaque colours, 1 (equal) to 21 (black on white).
#[must_use]
pub fn contrast_ratio(a: Color32, b: Color32) -> f64 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luminance_and_contrast_match_the_wcag_reference_points() {
        assert!((relative_luminance(Color32::WHITE) - 1.0).abs() < 1e-9);
        assert!(relative_luminance(Color32::BLACK).abs() < 1e-9);
        assert!((contrast_ratio(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 1e-9);
        assert!((contrast_ratio(Color32::WHITE, Color32::BLACK) - 21.0).abs() < 1e-9);
        // #767676 on white is the classic 4.5:1 boundary colour
        let grey = Color32::from_rgb(0x76, 0x76, 0x76);
        assert!((contrast_ratio(grey, Color32::WHITE) - 4.54).abs() < 0.02);
    }

    /// Every axis class has one entry in each theme, and the text on its badge and on its soft
    /// sector fill reads at the WCAG AA ratio for body text.
    #[test]
    fn every_axis_class_reads_at_aa_contrast_in_both_themes() {
        for p in [Palette::light(), Palette::dark()] {
            let mut badges = Vec::new();
            for class in AxisClass::ALL {
                let c = p.class(class);
                let badge = contrast_ratio(c.strong, c.on_strong);
                assert!(
                    badge >= 4.5,
                    "{class:?} badge in {} theme: {badge:.2}",
                    theme_name(&p)
                );
                let sector = contrast_ratio(c.soft, c.on_soft);
                assert!(
                    sector >= 4.5,
                    "{class:?} sector in {} theme: {sector:.2}",
                    theme_name(&p)
                );
                badges.push(c.strong);
            }
            badges.dedup();
            assert_eq!(badges.len(), 4, "one distinct badge colour per class");
            // the ordinary text and the headings read on cards and panels
            for (fg, bg) in [
                (p.text, p.card),
                (p.text, p.panel),
                (p.heading, p.card),
                (p.text, p.pill),
            ] {
                assert!(contrast_ratio(fg, bg) >= 4.5, "{} theme text", theme_name(&p));
            }
            assert!(contrast_ratio(WHITE, p.accent_strong) >= 4.5, "title band");
        }
    }

    fn theme_name(p: &Palette) -> &'static str {
        if p.dark { "dark" } else { "light" }
    }
}
