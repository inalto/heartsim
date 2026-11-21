//! The top bar's page navigation: with room, all thirteen pages are tabs; with less, three group
//! menus; always one line high, and every page is reachable in at most two clicks.

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
fn at_1280_all_thirteen_pages_are_tabs_and_a_click_switches_page() {
    assert_eq!(Tab::ALL.len(), 13);
    let mut h = harness(1280.0);
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
            None
        ]
    );
    assert_eq!(Tab::Simulator.shortcut(), Some("1"));
    assert_eq!(Tab::Tutorials.shortcut(), Some("9"));
    assert_eq!(Tab::EpLab.shortcut(), Some("0"));
}
