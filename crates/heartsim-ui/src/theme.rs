//! The one colour palette of the interface, keyed on light or dark: soft pastel fills, white or
//! slate cards, pill labels and a calm blue accent. Widgets take their colours from here, never
//! from literals, so the two themes and the high-contrast setting stay consistent.
//!
//! The ECG paper and the 3-D colour maps keep their own clinical colours (`strip`, `colour_bar`);
//! this palette is for the chrome around them.

use egui::{Color32, Context, CornerRadius, Stroke, Theme, Ui, Vec2};
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

    /// Amber text that reads at 4.5:1 on the panel and on the selected (accent-tinted) fill of a
    /// toggle, in both themes: for a label that flags a change on a toggle button.
    #[must_use]
    pub fn warn_text(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(252, 215, 150)
        } else {
            self.warn().on_soft
        }
    }

    /// Amber, for a warning or an equiphasic lead.
    #[must_use]
    pub fn warn(&self) -> ClassColours {
        self.class(AxisClass::LeftDeviation)
    }

    /// The colours of the one primary action of a panel ([`crate::widgets::primary_button`]): the
    /// accent as the fill, a darker (light theme) or lighter (dark theme) accent hovered, and a text
    /// colour that reads at 4.5:1 or more on all three. The dark theme's accent is light, so its
    /// text is near-black, not white.
    #[must_use]
    pub fn primary(&self) -> PrimaryColours {
        if self.dark {
            PrimaryColours {
                fill: self.accent,
                hover: Color32::from_rgb(150, 195, 255),
                pressed: Color32::from_rgb(86, 146, 236),
                text: Color32::from_rgb(14, 22, 36),
            }
        } else {
            PrimaryColours {
                fill: self.accent,
                hover: Color32::from_rgb(47, 92, 158),
                pressed: self.accent_strong,
                text: WHITE,
            }
        }
    }

    /// Explanatory text ([`crate::widgets::helper_text`]): quieter than body text but at 4.5:1 or
    /// more on the panel, cards, pills and group fills in both themes (`muted` is not: 4.1:1 on
    /// the light panel).
    #[must_use]
    pub fn helper_text(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(160, 172, 190)
        } else {
            Color32::from_rgb(84, 98, 120)
        }
    }

    /// The opacity of the accent in the fill of a selected button, tab or pill.
    #[must_use]
    pub fn selected_alpha(&self) -> f32 {
        if self.dark { 0.45 } else { 0.3 }
    }

    /// The fill of a selected button (`Button::selectable`, a tab, the nav pill) as seen over
    /// `background`: the accent at [`Self::selected_alpha`] blended over it.
    #[must_use]
    pub fn selected_fill_over(&self, background: Color32) -> Color32 {
        let a = self.selected_alpha();
        let mix = |x: u8, y: u8| (f32::from(x) * (1.0 - a) + f32::from(y) * a).round() as u8;
        Color32::from_rgb(
            mix(background.r(), self.accent.r()),
            mix(background.g(), self.accent.g()),
            mix(background.b(), self.accent.b()),
        )
    }

    /// The text of a selected button, tab or pill: it reads at 4.5:1 or more on
    /// [`Self::selected_fill_over`] the panel and a card (the accent itself did not: 3.2:1 light,
    /// 2.9:1 dark, on the panel).
    #[must_use]
    pub fn selected_text(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(235, 242, 255)
        } else {
            self.accent_strong
        }
    }
}

/// The colours of the primary button in one theme: its fill at rest, hovered and pressed, and the
/// text on all three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrimaryColours {
    pub fill: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub text: Color32,
}

/// Corner radius of cards and windows, points.
pub const CARD_RADIUS: u8 = 10;

/// Space between a button's label and its edge, in points (egui's own is 4 × 1, which reads as
/// cramped and gives a small pointer and touch target).
pub const BUTTON_PADDING: Vec2 = Vec2::new(8.0, 4.0);

/// Puts the palette's panel, window and widget colours into the style of the theme the context
/// resolves to now. Called once per frame by the application; it writes only when the style does
/// not carry the palette yet (after a theme change, or after the settings rebuilt the visuals).
/// The high-contrast setting, recognised by its text override, is left as it is: its black and
/// white are the point.
pub fn apply_to_context(ctx: &Context) {
    let theme = ctx.theme();
    let p = Palette::for_theme(theme);
    let style = ctx.style_of(theme);
    // spacing is set for every setting, high contrast included: it is not a colour
    if style.spacing.button_padding != BUTTON_PADDING {
        ctx.style_mut_of(theme, |s| s.spacing.button_padding = BUTTON_PADDING);
    }
    let v = &style.visuals;
    if v.override_text_color.is_some() {
        return;
    }
    if v.panel_fill == p.panel
        && v.window_fill == p.card
        && v.hyperlink_color == p.accent
        && v.selection.stroke.color == p.selected_text()
    {
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
        v.selection.bg_fill = p.accent.gamma_multiply(p.selected_alpha());
        // no outline on a selected pill or tab (zero width): the fill marks it, the colour is the
        // label text, which must read on that fill (the accent did not)
        v.selection.stroke = Stroke::new(0.0, p.selected_text());
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

    /// The amber text of a toggle that counts changes ("Adjust · 2") reads on the panel and on the
    /// selected fill, which is the accent blended over the panel, in both themes.
    #[test]
    fn amber_toggle_text_reads_on_the_panel_and_on_the_selected_fill() {
        for p in [Palette::light(), Palette::dark()] {
            let selected = p.selected_fill_over(p.panel);
            let text = p.warn_text();
            assert!(contrast_ratio(text, p.panel) >= 4.5, "{} panel", theme_name(&p));
            assert!(
                contrast_ratio(text, selected) >= 4.5,
                "{} selected: {:.2}",
                theme_name(&p),
                contrast_ratio(text, selected)
            );
        }
    }

    /// The label of a selected button, tab or nav pill ("Explore: Echo ▾") reads on its fill, over
    /// the panel (the bars) and over a card, in both themes. Measured before the fix (the accent as
    /// the text) on the panel: 3.2:1 light, 2.9:1 dark; after: 7.1:1 and 6.1:1.
    #[test]
    fn the_selected_label_reads_on_the_selected_fill() {
        for p in [Palette::light(), Palette::dark()] {
            let old = contrast_ratio(p.accent, p.selected_fill_over(p.panel));
            assert!(old < 4.5, "the old pairing failed: {old:.2}");
            for bg in [p.panel, p.card] {
                let fill = p.selected_fill_over(bg);
                let r = contrast_ratio(p.selected_text(), fill);
                assert!(r >= 4.5, "{} selected label: {r:.2}", theme_name(&p));
            }
        }
    }

    /// The selection colours [`apply_to_context`] writes are those the test above measures: the
    /// accent at the palette's alpha, the selected text as the label colour.
    #[test]
    fn apply_to_context_uses_the_measured_selection_colours() {
        for theme in [Theme::Light, Theme::Dark] {
            let ctx = Context::default();
            ctx.set_theme(theme);
            apply_to_context(&ctx);
            let p = Palette::for_theme(theme);
            let v = ctx.style_of(theme).visuals.clone();
            assert_eq!(v.selection.stroke.color, p.selected_text());
            assert_eq!(v.selection.bg_fill, p.accent.gamma_multiply(p.selected_alpha()));
            assert_eq!(ctx.style_of(theme).spacing.button_padding, BUTTON_PADDING);
        }
    }

    /// Helper text reads at AA on every surface it is written on, in both themes (the old `muted`
    /// did not on the light panel).
    #[test]
    fn helper_text_reads_on_panels_cards_pills_and_groups() {
        for p in [Palette::light(), Palette::dark()] {
            for bg in [p.panel, p.card, p.pill, p.accent_soft] {
                let r = contrast_ratio(p.helper_text(), bg);
                assert!(r >= 4.5, "{} helper on {bg:?}: {r:.2}", theme_name(&p));
            }
        }
        assert!(contrast_ratio(Palette::light().muted, Palette::light().panel) < 4.5);
    }

    fn theme_name(p: &Palette) -> &'static str {
        if p.dark { "dark" } else { "light" }
    }
}
