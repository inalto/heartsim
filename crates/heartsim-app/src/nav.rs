//! Page navigation: the pages, their four groups and the one-line page bar in the top bar.
//!
//! The bar adapts to the width it is given and never wraps: with room for all fifteen pages they are
//! tabs, grouped with a small caption (or just a separator when the captions would not fit); with
//! less, four group buttons whose menus list the pages (the group of the open page shows its name);
//! the phone-narrow layout is left to the caller (title and hamburger only).

use eframe::egui::{self, Color32, RichText, TextStyle, Ui};
use heartsim_ui::t;

/// The top-level pages, one tab each. The order of [`Tab::ALL`] is the order of the number-key
/// shortcuts (1–9, 0; the last five pages have none), not of the bar.
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
    /// Single heart-sound findings to loop, with a listen counter (B28).
    SoundTrainer,
    /// Features, then diagnosis, of a hidden auscultation case (B28).
    AuscultationQuiz,
    /// Echocardiography and haemodynamics calculators (B29).
    EchoCalculators,
    /// The probe on the figure and the anatomical cross-section it cuts (B31).
    Echo,
}

/// The four groups of pages in the bar: see, practise, learn, and the tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// Pages that show the heart and its signals.
    Explore,
    /// Pages that ask something of the learner.
    Practise,
    /// Reading material.
    Learn,
    /// Reference tools that do not drive the simulation.
    Tools,
}

impl Group {
    /// Every group, in bar order.
    pub const ALL: [Group; 4] = [Group::Explore, Group::Practise, Group::Learn, Group::Tools];

    /// The pages of the group, in bar order.
    #[must_use]
    pub fn tabs(self) -> &'static [Tab] {
        match self {
            Group::Explore => &[
                Tab::Simulator,
                Tab::TwelveLead,
                Tab::HeartVector,
                Tab::Echo,
                Tab::Electrodes,
                Tab::EpLab,
            ],
            Group::Practise => &[
                Tab::AxisTrainer,
                Tab::Quiz,
                Tab::RhythmQuiz,
                Tab::IntervalDrill,
                Tab::SoundTrainer,
                Tab::AuscultationQuiz,
            ],
            Group::Learn => &[Tab::Tutorials, Tab::Help],
            Group::Tools => &[Tab::EchoCalculators],
        }
    }

    /// The caption shown for the group.
    #[must_use]
    pub fn title(self) -> String {
        match self {
            Group::Explore => t!("nav-group-explore"),
            Group::Practise => t!("nav-group-practise"),
            Group::Learn => t!("nav-group-learn"),
            Group::Tools => t!("nav-group-tools"),
        }
    }
}

impl Tab {
    /// Every page in shortcut order (see [`Tab::shortcut`]).
    pub const ALL: [Tab; 15] = [
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
        Tab::SoundTrainer,
        Tab::AuscultationQuiz,
        Tab::EchoCalculators,
        Tab::Echo,
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
            Tab::SoundTrainer => t!("tab-sound-trainer"),
            Tab::AuscultationQuiz => t!("tab-auscultation-quiz"),
            Tab::EchoCalculators => t!("tab-echo-calculators"),
            Tab::Echo => t!("tab-echo"),
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

    /// The number key that opens the page: `1`–`9`, then `0`; the eleventh to fifteenth pages (Help,
    /// the Sound Trainer, the auscultation quiz, the echo calculators and Echo) have none.
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
    /// Every page a tab, with a caption before each group.
    TabsWithCaptions,
    /// Every page a tab, groups only separated.
    Tabs,
    /// Four group buttons with a menu each.
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
    let pad = TAB_BUTTON_PADDING.x * 2.0;
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

/// Padding inside the page tabs of the bar, points: roomier than the global
/// [`heartsim_ui::theme::BUTTON_PADDING`] (8 × 4), so the page names do not touch the edge of the
/// selected tab's pill and each tab is an easier target. [`choose_style`] measures with it.
pub const TAB_BUTTON_PADDING: egui::Vec2 = egui::Vec2::new(10.0, 6.0);

fn tab_button(ui: &mut Ui, current: &mut Tab, tab: Tab) {
    let mut response = ui
        .scope(|ui| {
            ui.spacing_mut().button_padding = TAB_BUTTON_PADDING;
            ui.add(egui::Button::selectable(*current == tab, tab.title()))
        })
        .inner;
    if let Some(key) = tab.shortcut() {
        response = response.on_hover_text(t!("nav-shortcut", key = key));
    }
    if response.clicked() {
        *current = tab;
    }
}

/// Padding inside the group menu buttons (Explore, Practise, Learn, Tools), points: roomier than
/// the global [`heartsim_ui::theme::BUTTON_PADDING`] (8 × 4), which made them read as cramped next
/// to the other buttons. The rows of the opened menu use the same vertical padding, so each is a
/// comfortable target.
pub const GROUP_BUTTON_PADDING: egui::Vec2 = egui::Vec2::new(12.0, 6.0);

fn group_menu(ui: &mut Ui, current: &mut Tab, group: Group) {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = GROUP_BUTTON_PADDING;
        group_menu_inner(ui, current, group);
    });
}

fn group_menu_inner(ui: &mut Ui, current: &mut Tab, group: Group) {
    let selected = current.group() == group;
    let label = format!("{}{}", group_button_label(group, *current), arrow_room(ui));
    let button = egui::Button::selectable(selected, label);
    let (response, _) = egui::containers::menu::MenuButton::from_button(button).ui(ui, |ui| {
        ui.spacing_mut().button_padding.y = GROUP_BUTTON_PADDING.y;
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
