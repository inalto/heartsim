//! The rhythm picker keeps one width whatever the rhythm's name, so the bar around it does not
//! reflow when the rhythm changes (no GPU).

use egui::{Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_app::scenarios::{ScenarioLibrary, scenario_picker};
use heartsim_ui::i18n;
use std::path::Path;

struct Model {
    library: ScenarioLibrary,
    current: Option<usize>,
    bar: Rect,
}

fn harness(width: f32, current: Option<usize>) -> Harness<'static, Model> {
    i18n::set_language("en");
    let library =
        ScenarioLibrary::load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios"))
            .unwrap();
    Harness::builder()
        .with_context_setup(heartsim_ui::theme::apply_to_context)
        .with_size(vec2(width, 200.0))
        .build_ui_state(
            |ui, m: &mut Model| {
                let r = ui.horizontal(|ui| {
                    scenario_picker(ui, &m.library, m.current);
                });
                m.bar = r.response.rect;
            },
            Model {
                library,
                current,
                bar: Rect::NOTHING,
            },
        )
}

#[test]
fn the_picker_bar_has_the_same_rect_for_a_short_a_long_and_no_rhythm_name() {
    let probe = harness(1280.0, None);
    let by_len = |f: fn(usize, usize) -> bool| {
        (0..probe.state().library.entries.len())
            .reduce(|a, b| {
                let (la, lb) = (
                    probe.state().library.entries[a].scenario.title.chars().count(),
                    probe.state().library.entries[b].scenario.title.chars().count(),
                );
                if f(la, lb) { a } else { b }
            })
            .unwrap()
    };
    let short = by_len(|a, b| a <= b);
    let long = by_len(|a, b| a >= b);
    let (short_title, long_title) = (
        probe.state().library.entries[short].scenario.title.clone(),
        probe.state().library.entries[long].scenario.title.clone(),
    );
    assert!(long_title.chars().count() > short_title.chars().count() + 10);

    let mut rects = Vec::new();
    for current in [Some(short), Some(long), None] {
        let mut h = harness(1280.0, current);
        h.run();
        rects.push(h.state().bar);
    }
    for r in &rects[1..] {
        assert!(
            (r.width() - rects[0].width()).abs() < 0.5 && (r.height() - rects[0].height()).abs() < 0.5,
            "bar sizes differ: {rects:?}"
        );
    }

    // a long name is cut and still readable whole on hover
    let mut h = harness(1280.0, Some(long));
    h.run();
    let combo = h.get_by_role(egui::accesskit::Role::ComboBox);
    assert!(combo.rect().width() <= heartsim_app::scenarios::PICKER_WIDTH_PX + 40.0);
}
