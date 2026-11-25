//! The shared design primitives (`heartsim_ui::widgets`) under the app's theme: the primary button
//! (named, disabled when its `Ui` is), slider rows (rails aligned, named by their label, stacked when
//! narrow), helper text and its disclosure (closed by default, remembered), the read-only status mark
//! (named "Met"/"Not met", not a checkbox) and the segmented control (keyboard reachable).

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{By, NodeT, Queryable};
use heartsim_ui::{helper_text, i18n, info_disclosure, primary_button, segmented, slider_row, status_row};

fn themed<S>(width: f32, app: impl FnMut(&mut egui::Ui, &mut S) + 'static, state: S) -> Harness<'static, S> {
    Harness::builder()
        .with_context_setup(heartsim_ui::theme::apply_to_context)
        .with_size(egui::vec2(width, 600.0))
        .build_ui_state(app, state)
}

#[derive(Default)]
struct Clicks {
    enabled: u32,
    disabled: u32,
}

#[test]
fn the_primary_button_is_named_and_disabled_with_its_ui() {
    let mut h = themed(
        400.0,
        |ui, c: &mut Clicks| {
            if primary_button(ui, "Run").clicked() {
                c.enabled += 1;
            }
            ui.add_enabled_ui(false, |ui| {
                if primary_button(ui, "Check answer").clicked() {
                    c.disabled += 1;
                }
            });
        },
        Clicks::default(),
    );
    h.run();
    let run = h.get_by_role_and_label(Role::Button, "Run");
    assert!(!run.accesskit_node().is_disabled());
    run.click();
    h.run();
    let check = h.get_by_label("Check answer");
    assert!(check.accesskit_node().is_disabled());
    check.click();
    h.run();
    assert_eq!((h.state().enabled, h.state().disabled), (1, 0));
}

#[derive(Default)]
struct Rows {
    a: f32,
    b: f32,
}

fn rows(width: f32) -> Harness<'static, Rows> {
    i18n::set_language("en");
    let mut h = themed(
        width,
        |ui, r: &mut Rows| {
            slider_row(ui, "Gain", egui::Slider::new(&mut r.a, 0.0..=10.0).suffix(" dB"));
            slider_row(
                ui,
                "Dynamic range",
                egui::Slider::new(&mut r.b, 0.0..=10.0).suffix(" dB"),
            );
        },
        Rows::default(),
    );
    h.run();
    h
}

#[test]
fn slider_rows_align_their_rails_and_are_named_by_their_labels() {
    let h = rows(400.0);
    let a = h.get_by_role_and_label(Role::Slider, "Gain").rect();
    let b = h.get_by_role_and_label(Role::Slider, "Dynamic range").rect();
    assert!(
        (a.left() - b.left()).abs() < 0.5,
        "rails start together: {a:?} {b:?}"
    );
    assert!((a.width() - b.width()).abs() < 0.5, "and are as long");
    // the label is left of the rail, on the same row
    let label = h
        .query_all(By::new().label("Dynamic range").include_labels())
        .map(|n| n.rect())
        .find(|r| r.right() <= b.left() + 0.5);
    let label = label.expect("a label left of the slider");
    assert!(label.center().y > b.top() && label.center().y < b.bottom());
    assert!(a.right() <= 400.0 && b.right() <= 400.0);
}

#[test]
fn slider_rows_put_the_label_above_the_slider_when_narrow() {
    let h = rows(200.0);
    let slider = h.get_by_role_and_label(Role::Slider, "Dynamic range").rect();
    let label = h
        .query_all(
            By::new()
                .label("Dynamic range")
                .include_labels()
                .role(Role::Label),
        )
        .map(|n| n.rect())
        .next()
        .expect("the label");
    assert!(
        label.bottom() <= slider.top() + 0.5,
        "stacked: {label:?} {slider:?}"
    );
    assert!(slider.right() <= 200.5);
}

#[test]
fn the_disclosure_is_closed_by_default_and_remembers_being_opened() {
    i18n::set_language("en");
    let mut h = themed(
        400.0,
        |ui, (): &mut ()| {
            helper_text(ui, "A short note.");
            info_disclosure(ui, "d", "One line.", "The whole paragraph.");
        },
        (),
    );
    h.run();
    let _ = h.get_by_label("A short note.");
    let _ = h.get_by_label("One line.");
    assert!(h.query_by_label("The whole paragraph.").is_none());
    h.get_by_label("More").click();
    h.run();
    let _ = h.get_by_label("The whole paragraph.");
    h.run_steps(3);
    let _ = h.get_by_label("The whole paragraph.");
    h.get_by_label("Less").click();
    h.run();
    assert!(h.query_by_label("The whole paragraph.").is_none());
}

#[test]
fn status_rows_are_named_met_or_not_met_and_are_not_controls() {
    for (lang, met, not_met) in [("en", "Met", "Not met"), ("it", "Soddisfatto", "Non soddisfatto")] {
        i18n::set_language(lang);
        let mut h = themed(
            400.0,
            |ui, (): &mut ()| {
                status_row(ui, true, "Left ventricle");
                status_row(ui, false, "Right atrium");
            },
            (),
        );
        h.run();
        let done_name = format!("{met}: Left ventricle");
        let done = h.get_by_label(&done_name);
        assert_eq!(done.accesskit_node().role(), Role::Label);
        let not_met_name = format!("{not_met}: Right atrium");
        let _ = h.get_by_label(&not_met_name);
        assert!(h.query_by_role(Role::CheckBox).is_none());
    }
    i18n::set_language("en");
}

#[test]
fn the_segmented_control_switches_and_is_reachable_with_tab() {
    #[derive(Clone, Copy, PartialEq, Debug)]
    enum S {
        A,
        B,
        C,
    }
    let mut h = themed(
        400.0,
        |ui, s: &mut S| {
            segmented(
                ui,
                s,
                &[
                    (S::A, "Probe".into()),
                    (S::B, "Image".into()),
                    (S::C, "Views".into()),
                ],
            );
        },
        S::A,
    );
    h.run();
    h.get_by_label("Image").click();
    h.run();
    assert_eq!(*h.state(), S::B);
    // every segment fits the row
    for name in ["Probe", "Image", "Views"] {
        assert!(h.get_by_label(name).rect().right() <= 400.5);
    }
    // Tab moves the focus through the segments; Space chooses
    let mut focused = Vec::new();
    for _ in 0..3 {
        h.key_press(egui::Key::Tab);
        h.run();
        if let Some(n) = h.query_by(|n: &egui_kittest::kittest::AccessKitNode<'_>| n.is_focused()) {
            focused.push(n.accesskit_node().label().unwrap_or_default());
        }
    }
    assert_eq!(focused, ["Probe", "Image", "Views"]);
    h.key_press(egui::Key::Space);
    h.run();
    assert_eq!(*h.state(), S::C);
}

/// The accessible nodes that are primary actions (named by their label), in tree order.
fn primary_names<S>(h: &Harness<'_, S>) -> Vec<String> {
    h.query_all(By::new())
        .filter(|n| n.accesskit_node().class_name() == Some(heartsim_ui::PRIMARY_CLASS))
        .map(|n| n.accesskit_node().label().unwrap_or_default())
        .collect()
}

#[test]
fn a_primary_button_is_marked_for_tests_also_when_disabled_and_a_plain_one_is_not() {
    let mut h = themed(
        400.0,
        |ui, (): &mut ()| {
            primary_button(ui, "Run");
            ui.add_enabled_ui(false, |ui| primary_button(ui, "Check answer"));
            let _ = ui.button("Restart");
        },
        (),
    );
    h.run();
    assert_eq!(primary_names(&h), ["Run", "Check answer"]);
}

#[test]
fn a_collapsible_section_follows_its_flag_and_a_click_on_the_header_flips_it() {
    let mut h = themed(
        300.0,
        |ui, open: &mut bool| {
            heartsim_ui::collapsible_section(ui, open, "Colour map", |ui| {
                ui.label("The body");
            });
        },
        false,
    );
    h.run();
    assert!(h.query_by_label("The body").is_none());
    h.get_by_label("Colour map").click();
    h.run();
    assert!(*h.state());
    let _ = h.get_by_label("The body");
    h.get_by_label("Colour map").click();
    h.run();
    assert!(!*h.state());
    assert!(h.query_by_label("The body").is_none());
}
