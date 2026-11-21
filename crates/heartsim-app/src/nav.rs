//! Page navigation: the pages, their three groups and the one-line page bar in the top bar.
//!
//! The bar adapts to the width it is given and never wraps: with room for all eleven pages they are
//! tabs, grouped with a small caption (or just a separator when the captions would not fit); with
//! less, three group buttons whose menus list the pages (the group of the open page shows its name);
//! the phone-narrow layout is left to the caller (title and hamburger only).

use eframe::egui::{self, Color32, RichText, TextStyle, Ui};
use heartsim_ui::t;

/// The top-level pages, one tab each. The order of [`Tab::ALL`] is the order of the number-key
/// shortcuts (1–9, 0), not of the bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Simulator,
    TwelveLead,
    HeartVector,
    AxisTrainer,
    Quiz,
    Electrodes,
    RhythmQuiz,
    IntervalDrill,
    Tutorials,
    EpLab,
    Help,
}

/// The three groups of pages in the bar: see, practise, learn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// Pages that show the heart and its signals.
    Explore,
    /// Pages that ask something of the learner.
    Practise,
    /// Reading material.
    Learn,
}

impl Group {
    /// Every group, in bar order.
    pub const ALL: [Group; 3] = [Group::Explore, Group::Practise, Group::Learn];

    /// The pages of the group, in bar order.
    #[must_use]
    pub fn tabs(self) -> &'static [Tab] {
        match self {
            Group::Explore => &[
                Tab::Simulator,
                Tab::TwelveLead,
                Tab::HeartVector,
                Tab::Electrodes,
                Tab::EpLab,
            ],
            Group::Practise => &[Tab::AxisTrainer, Tab::Quiz, Tab::RhythmQuiz, Tab::IntervalDrill],
            Group::Learn => &[Tab::Tutorials, Tab::Help],
        }
    }

    /// The caption shown for the group.
    #[must_use]
    pub fn title(self) -> String {
        match self {
            Group::Explore => t!("nav-group-explore"),
            Group::Practise => t!("nav-group-practise"),
            Group::Learn => t!("nav-group-learn"),
        }
    }
}

impl Tab {
    /// Every page in shortcut order (see [`Tab::shortcut`]).
    pub const ALL: [Tab; 11] = [
        Tab::Simulator,
        Tab::TwelveLead,
        Tab::HeartVector,
        Tab::AxisTrainer,
        Tab::Quiz,
        Tab::Electrodes,
        Tab::RhythmQuiz,
        Tab::IntervalDrill,
        Tab::Tutorials,
        Tab::EpLab,
        Tab::Help,
    ];

    /// The page's name, as in the bar and the menus.
    #[must_use]
    pub fn title(self) -> String {
        match self {
            Tab::Simulator => t!("tab-simulator"),
            Tab::TwelveLead => t!("tab-12-lead"),
            Tab::HeartVector => t!("tab-heart-vector"),
            Tab::AxisTrainer => t!("tab-axis-trainer"),
            Tab::Quiz => t!("tab-axis-quiz"),
            Tab::Electrodes => t!("tab-electrodes"),
            Tab::RhythmQuiz => t!("tab-rhythm-quiz"),
            Tab::IntervalDrill => t!("tab-interval-drill"),
            Tab::Tutorials => t!("tab-tutorials"),
            Tab::EpLab => t!("tab-ep-lab"),
            Tab::Help => t!("tab-help"),
        }
    }

    /// The group the page sits in.
    #[must_use]
    pub fn group(self) -> Group {
        Group::ALL
            .into_iter()
            .find(|g| g.tabs().contains(&self))
            .unwrap_or(Group::Explore)
    }

    /// The number key that opens the page: `1`–`9`, then `0`; the eleventh page (Help) has none.
    #[must_use]
    pub fn shortcut(self) -> Option<&'static str> {
        const LABELS: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
        let i = Tab::ALL.iter().position(|t| *t == self)?;
        LABELS.get(i).copied()
    }
}

/// How the bar draws the pages for a given width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarStyle {
    /// Eleven tabs with a caption before each group.
    TabsWithCaptions,
    /// Eleven tabs, groups only separated.
    Tabs,
    /// Three group buttons with a menu each.
    GroupMenus,
}

/// Room kept free at the right end of the bar for the menu and theme buttons, px.
pub const RIGHT_CLUSTER_PX: f32 = 80.0;

/// A page as one entry of a menu (current one marked, shortcut shown).
fn menu_entry(ui: &mut Ui, current: &mut Tab, tab: Tab) {
    let mut button = egui::Button::new(tab.title()).selected(*current == tab);
    if let Some(key) = tab.shortcut() {
        button = button.shortcut_text(key);
    }
    if ui.add(button).clicked() {
        *current = tab;
        ui.close();
    }
}

/// The page list of the hamburger menu: every page in shortcut order.
pub fn page_list(ui: &mut Ui, current: &mut Tab) {
    for tab in Tab::ALL {
        menu_entry(ui, current, tab);
    }
}

fn text_width(ui: &Ui, text: &str, style: &TextStyle) -> f32 {
    let font = style.resolve(ui.style());
    ui.painter()
        .layout_no_wrap(text.to_owned(), font, Color32::WHITE)
        .size()
        .x
}

/// Trailing blanks of a menu button that leave room for the painted triangle.
fn arrow_room(ui: &Ui) -> String {
    let space = text_width(ui, " ", &TextStyle::Button).max(1.0);
    " ".repeat((ARROW_ROOM_PX / space).ceil() as usize)
}

/// Width reserved at the right of a group button for its triangle, px.
const ARROW_ROOM_PX: f32 = 12.0;

fn group_button_label(group: Group, current: Tab) -> String {
    if current.group() == group {
        format!("{}: {}", group.title(), current.title())
    } else {
        group.title()
    }
}

/// The bar style that fits `available` px: the widest one whose measured width is not more than it.
#[must_use]
pub fn choose_style(ui: &Ui, available: f32) -> BarStyle {
    let spacing = ui.spacing().item_spacing.x;
    let pad = ui.spacing().button_padding.x * 2.0;
    let tabs: f32 = Tab::ALL
        .iter()
        .map(|t| text_width(ui, &t.title(), &TextStyle::Button) + pad)
        .sum();
    let separators = (Group::ALL.len() - 1) as f32 * (spacing * 2.0 + 1.0);
    let items = (Tab::ALL.len() - 1) as f32 * spacing;
    let plain = tabs + items + separators;
    let captions: f32 = Group::ALL
        .iter()
        .map(|g| text_width(ui, &g.title(), &TextStyle::Small) + spacing)
        .sum();
    // a few px of slack so a rounding difference never wraps the bar
    let slack = 8.0;
    if plain + captions + slack <= available {
        BarStyle::TabsWithCaptions
    } else if plain + slack <= available {
        BarStyle::Tabs
    } else {
        BarStyle::GroupMenus
    }
}

/// Draws the pages in the current (horizontal, non-wrapping) layout: tabs or group menus,
/// whichever fits the `available` px, and returns the style used. Clicking a page sets `current`.
pub fn page_bar(ui: &mut Ui, current: &mut Tab, available: f32) -> BarStyle {
    let style = choose_style(ui, available);
    match style {
        BarStyle::TabsWithCaptions | BarStyle::Tabs => {
            for (i, group) in Group::ALL.into_iter().enumerate() {
                if i > 0 {
                    ui.separator();
                }
                if style == BarStyle::TabsWithCaptions {
                    ui.label(RichText::new(group.title()).small().weak());
                }
                for tab in group.tabs() {
                    tab_button(ui, current, *tab);
                }
            }
        }
        BarStyle::GroupMenus => {
            for group in Group::ALL {
                group_menu(ui, current, group);
            }
        }
    }
    style
}

fn tab_button(ui: &mut Ui, current: &mut Tab, tab: Tab) {
    let mut response = ui.add(egui::Button::selectable(*current == tab, tab.title()));
    if let Some(key) = tab.shortcut() {
        response = response.on_hover_text(t!("nav-shortcut", key = key));
    }
    if response.clicked() {
        *current = tab;
    }
}

fn group_menu(ui: &mut Ui, current: &mut Tab, group: Group) {
    let selected = current.group() == group;
    let label = format!("{}{}", group_button_label(group, *current), arrow_room(ui));
    let button = egui::Button::selectable(selected, label);
    let (response, _) = egui::containers::menu::MenuButton::from_button(button).ui(ui, |ui| {
        for tab in group.tabs() {
            menu_entry(ui, current, *tab);
        }
    });
    // painted: egui's bundled fonts have no small-triangle glyph
    let colour = ui
        .style()
        .interact_selectable(&response, selected)
        .fg_stroke
        .color;
    let r = response.rect;
    let c = egui::pos2(
        r.right() - ui.spacing().button_padding.x - 3.0,
        r.center().y + 0.5,
    );
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            c + egui::vec2(-3.5, -2.0),
            c + egui::vec2(3.5, -2.0),
            c + egui::vec2(0.0, 2.5),
        ],
        colour,
        egui::Stroke::NONE,
    ));
}
