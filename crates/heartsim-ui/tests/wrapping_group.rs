//! `cards::wrapping_group`: a framed group that would not fit the rest of a wrapping row starts a
//! new row instead of overflowing the window (the rhythm bar on a narrow screen). No GPU.

use egui::{Rect, Vec2};
use egui_kittest::Harness;
use heartsim_ui::cards;

const WINDOW_W: f32 = 400.0;
/// Two groups fit one row of the window, a third does not.
const GROUP_W: f32 = 150.0;

fn harness() -> Harness<'static, Vec<Rect>> {
    Harness::builder()
        .with_size(Vec2::new(WINDOW_W, 300.0))
        .build_ui_state(
            |ui, rects: &mut Vec<Rect>| {
                rects.clear();
                ui.horizontal_wrapped(|ui| {
                    for i in 0..3 {
                        let r = cards::wrapping_group(ui, i, |ui| {
                            ui.add_sized(Vec2::new(GROUP_W, 20.0), egui::Label::new(format!("group {i}")));
                        });
                        rects.push(r.response.rect);
                    }
                });
            },
            Vec::new(),
        )
}

#[test]
fn a_group_that_does_not_fit_the_row_starts_a_new_one() {
    let mut h = harness();
    h.run();
    h.step();
    let rects = h.state().clone();
    assert_eq!(rects.len(), 3);
    for r in &rects {
        assert!(
            r.max.x <= WINDOW_W + 0.5,
            "every group inside the window: {rects:?}"
        );
    }
    assert!(
        (rects[1].min.y - rects[0].min.y).abs() < 0.5,
        "the first two groups share a row: {rects:?}"
    );
    assert!(
        rects[2].min.y >= rects[0].max.y - 0.5,
        "the third group is on the next row: {rects:?}"
    );
    assert!(
        (rects[2].min.x - rects[0].min.x).abs() < 0.5,
        "the new row starts at the left edge: {rects:?}"
    );
}

#[test]
fn groups_that_fit_stay_on_one_row() {
    let mut h = Harness::builder()
        .with_size(Vec2::new(1000.0, 300.0))
        .build_ui_state(
            |ui, rects: &mut Vec<Rect>| {
                rects.clear();
                ui.horizontal_wrapped(|ui| {
                    for i in 0..3 {
                        let r = cards::wrapping_group(ui, i, |ui| {
                            ui.add_sized(Vec2::new(GROUP_W, 20.0), egui::Label::new(format!("group {i}")));
                        });
                        rects.push(r.response.rect);
                    }
                });
            },
            Vec::new(),
        );
    h.run();
    h.step();
    let rects = h.state().clone();
    assert!(
        rects.iter().all(|r| (r.min.y - rects[0].min.y).abs() < 0.5),
        "one row: {rects:?}"
    );
}
