//! The top bar's page navigation: with room, all fifteen pages are tabs; with less, four group
//! menus (Explore, Practise, Learn, Tools); always one line high, and every page is reachable in at
//! most two clicks.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use heartsim_app::nav::{self, BarStyle, Group, Tab};
use heartsim_ui::i18n;

struct Model {
    tab: Tab,
    style: Option<BarStyle>,
}

/// The bar as the app lays it out: the product on the left, the pages after it, room kept at the
/// right for the menu and theme buttons.
fn harness(width: f32) -> Harness<'static, Model> {
    i18n::set_language("en");
    Harness::builder()
        .with_context_setup(heartsim_ui::theme::apply_to_context)
        .with_size(egui::vec2(width, 300.0))
        .build_ui_state(
            |ui, m: &mut Model| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.heading("heartsim");
                    ui.weak("v0.0.0");
                    ui.separator();
                    let available = ui.available_width() - nav::RIGHT_CLUSTER_PX;
                    m.style = Some(nav::page_bar(ui, &mut m.tab, available));
                });
            },
            Model {
                tab: Tab::Simulator,
                style: None,
            },
        )
}

#[test]
fn at_1520_all_fifteen_pages_are_tabs_and_a_click_switches_page() {
    // fifteen English tabs need about 1420 px (fourteen fitted in 1380): below that, group menus
    assert_eq!(Tab::ALL.len(), 15);
    assert_eq!(Group::ALL.len(), 4);
    let mut h = harness(1520.0);
    h.run();
    assert!(matches!(
        h.state().style,
        Some(BarStyle::TabsWithCaptions | BarStyle::Tabs)
    ));
    for tab in Tab::ALL {
        let _ = h.get_by_label(&tab.title());
    }
    h.get_by_label("Axis Quiz").click();
    h.run();
    assert_eq!(h.state().tab, Tab::Quiz);
    h.get_by_label("Help").click();
    h.run();
    assert_eq!(h.state().tab, Tab::Help);
    h.get_by_label("Echo calculators").click();
    h.run();
    assert_eq!(h.state().tab, Tab::EchoCalculators);
}

#[test]
fn the_tools_group_holds_the_echo_calculators_in_both_languages() {
    assert_eq!(Tab::EchoCalculators.group(), Group::Tools);
    assert_eq!(Group::Tools.tabs(), [Tab::EchoCalculators]);
    i18n::set_language("en");
    assert_eq!(Group::Tools.title(), "Tools");
    assert_eq!(Tab::EchoCalculators.title(), "Echo calculators");
    i18n::set_language("it");
    assert_eq!(Group::Tools.title(), "Strumenti");
    assert_eq!(Tab::EchoCalculators.title(), "Calcolatori eco");
    i18n::set_language("en");
}

#[test]
fn at_the_narrowest_desktop_width_four_group_menus_fit_on_one_line() {
    // 700 px is the narrowest width that is not the phone layout (title and hamburger only)
    for lang in ["en", "it"] {
        i18n::set_language(lang);
        let mut h = harness(700.0);
        h.state_mut().tab = Tab::AuscultationQuiz;
        h.run();
        assert_eq!(h.state().style, Some(BarStyle::GroupMenus), "{lang}");
        let rects: Vec<egui::Rect> = Group::ALL
            .iter()
            .map(|g| h.get_by_label_contains(&g.title()).rect())
            .collect();
        assert_eq!(rects.len(), 4);
        assert!(
            rects.windows(2).all(|w| (w[0].top() - w[1].top()).abs() < 1.0),
            "{lang}: {rects:?}"
        );
        let right = rects.iter().map(egui::Rect::right).fold(0.0, f32::max);
        assert!(
            right <= 700.0 - nav::RIGHT_CLUSTER_PX + 1.0,
            "{lang}: the bar overflows to {right}"
        );
    }
    i18n::set_language("en");
}

#[test]
fn at_900_the_bar_is_one_line_and_every_page_is_within_two_clicks() {
    let mut h = harness(900.0);
    h.run();
    assert_eq!(h.state().style, Some(BarStyle::GroupMenus));
    // one line: every button of the bar sits at the same height
    let tops: Vec<f32> = Group::ALL
        .iter()
        .map(|g| h.get_by_label_contains(&g.title()).rect().top())
        .collect();
    assert!(tops.windows(2).all(|w| (w[0] - w[1]).abs() < 1.0), "{tops:?}");
    let bar_bottom = Group::ALL
        .iter()
        .map(|g| h.get_by_label_contains(&g.title()).rect().bottom())
        .fold(0.0, f32::max);
    assert!(bar_bottom < 40.0, "the bar is {bar_bottom} px tall");
    // the open page names its group's button
    let _ = h.get_by_label_contains("Explore: Simulator");

    for tab in Tab::ALL {
        let mut h = harness(900.0);
        h.run();
        // click one: the group menu; click two: the page
        h.get_by_label_contains(&tab.group().title()).click();
        h.run();
        // the entry of the menu (the open page's own group button names it too, with a colon)
        let title = tab.title();
        let entry = h
            .query_all_by_label_contains(&title)
            .find(|n| n.accesskit_node().label().is_some_and(|l| !l.contains(": ")))
            .unwrap_or_else(|| panic!("no menu entry for {tab:?}"));
        entry.click();
        h.run();
        assert_eq!(h.state().tab, tab, "{tab:?} is not reachable in two clicks");
    }
}

#[test]
fn every_page_has_a_group_and_the_groups_hold_each_page_once() {
    let mut all: Vec<Tab> = Group::ALL.iter().flat_map(|g| g.tabs().iter().copied()).collect();
    assert_eq!(all.len(), Tab::ALL.len());
    for t in Tab::ALL {
        assert_eq!(all.iter().filter(|x| **x == t).count(), 1, "{t:?}");
        assert!(t.group().tabs().contains(&t));
    }
    all.sort_by_key(|t| Tab::ALL.iter().position(|x| x == t));
    assert_eq!(all, Tab::ALL);
}

#[test]
fn the_shortcuts_are_unchanged() {
    let keys: Vec<Option<&str>> = Tab::ALL.iter().map(|t| t.shortcut()).collect();
    assert_eq!(
        keys,
        [
            Some("1"),
            Some("2"),
            Some("3"),
            Some("4"),
            Some("5"),
            Some("6"),
            Some("7"),
            Some("8"),
            Some("9"),
            Some("0"),
            None,
            None,
            None,
            None,
            None
        ]
    );
    assert_eq!(Tab::EchoCalculators.shortcut(), None);
    assert_eq!(
        Tab::Echo.shortcut(),
        None,
        "the Echo page is new and takes no number key"
    );
    assert_eq!(Tab::Simulator.shortcut(), Some("1"));
    assert_eq!(Tab::Tutorials.shortcut(), Some("9"));
    assert_eq!(Tab::EpLab.shortcut(), Some("0"));
}

#[test]
fn the_echo_page_is_in_explore_right_after_heart_vector_in_both_languages() {
    assert_eq!(Tab::Echo.group(), Group::Explore);
    let explore = Group::Explore.tabs();
    let i = explore
        .iter()
        .position(|t| *t == Tab::HeartVector)
        .expect("Heart Vector");
    assert_eq!(explore[i + 1], Tab::Echo, "Echo follows Heart Vector");
    // nothing else moved: the other groups are as they were
    assert_eq!(Group::Tools.tabs(), [Tab::EchoCalculators]);
    assert_eq!(Group::Learn.tabs(), [Tab::Tutorials, Tab::Help]);
    i18n::set_language("en");
    assert_eq!(Tab::Echo.title(), "Echo");
    i18n::set_language("it");
    assert_eq!(Tab::Echo.title(), "Eco");
    i18n::set_language("en");
}

#[test]
fn the_echo_page_is_a_tab_at_the_wide_width_and_a_menu_entry_when_narrow() {
    for lang in ["en", "it"] {
        i18n::set_language(lang);
        let mut h = harness(1700.0);
        h.run();
        h.get_by_label(&Tab::Echo.title()).click();
        h.run();
        assert_eq!(h.state().tab, Tab::Echo, "{lang}");
    }
    i18n::set_language("en");
}

/// The group menu buttons are roomier than ordinary buttons (12 × 6 against 8 × 4): taller by the
/// extra vertical padding, wider by the extra horizontal one, and the bar still fits one line.
#[test]
fn the_group_buttons_have_their_own_roomier_padding() {
    assert_eq!(nav::GROUP_BUTTON_PADDING, egui::vec2(12.0, 6.0));
    const { assert!(nav::GROUP_BUTTON_PADDING.x > heartsim_ui::theme::BUTTON_PADDING.x) };
    const { assert!(nav::GROUP_BUTTON_PADDING.y > heartsim_ui::theme::BUTTON_PADDING.y) };
    i18n::set_language("en");
    let mut h = harness(900.0);
    h.run();
    for g in Group::ALL {
        let r = h.get_by_label_contains(&g.title()).rect();
        // text line (at least 12 pt) plus 2 x 6 pt of padding
        assert!(
            r.height() >= 12.0 + 2.0 * nav::GROUP_BUTTON_PADDING.y - 0.5,
            "{g:?} {r:?}"
        );
        // the label plus two 12 pt paddings and the room for the triangle
        assert!(
            r.width() >= 2.0 * nav::GROUP_BUTTON_PADDING.x + 12.0,
            "{g:?} {r:?}"
        );
    }
}
