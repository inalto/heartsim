//! The answer options of the quizzes mark the right answer and the learner's wrong one with a
//! painted icon as well as a colour (WCAG 1.4.1), and read at 4.5:1 in both themes.

use egui::epaint::Shape;
use heartsim_ui::Palette;
use heartsim_ui::cards::{AnswerState, answer_option};
use heartsim_ui::theme::contrast_ratio;

/// (path shapes, line segments) in a frame that draws one option in `state`.
fn icon_shapes(state: AnswerState) -> (usize, usize) {
    fn count(shape: &Shape, paths: &mut usize, lines: &mut usize) {
        match shape {
            Shape::Vec(v) => v.iter().for_each(|s| count(s, paths, lines)),
            Shape::Path(_) => *paths += 1,
            Shape::LineSegment { .. } => *lines += 1,
            _ => {}
        }
    }
    let ctx = egui::Context::default();
    let mut out = egui::FullOutput::default();
    for _ in 0..2 {
        out = ctx.run_ui(egui::RawInput::default(), |ui| {
            answer_option(ui, state, "Atrial flutter");
        });
        out.textures_delta.clear();
    }
    let (mut paths, mut lines) = (0, 0);
    for clipped in &out.shapes {
        count(&clipped.shape, &mut paths, &mut lines);
    }
    (paths, lines)
}

#[test]
fn correct_gets_a_tick_and_wrong_a_cross_and_the_others_neither() {
    let (tick_paths, _) = icon_shapes(AnswerState::Correct);
    let (_, cross_lines) = icon_shapes(AnswerState::Wrong);
    assert!(tick_paths >= 1, "tick is a painted path");
    assert!(cross_lines >= 2, "cross is two painted strokes");
    for state in [AnswerState::Open, AnswerState::Inactive] {
        let (paths, lines) = icon_shapes(state);
        assert_eq!((paths, lines), (0, 0), "{state:?} carries no mark");
    }
}

#[test]
fn answer_text_reads_at_aa_on_its_background_in_both_themes() {
    for p in [Palette::light(), Palette::dark()] {
        for (c, name) in [(p.good(), "correct"), (p.bad(), "wrong")] {
            assert!(contrast_ratio(c.on_soft, c.soft) >= 4.5, "{name} dark={}", p.dark);
            // the icon's tick / cross on its disc
            assert!(
                contrast_ratio(c.on_strong, c.strong) >= 4.5,
                "{name} icon dark={}",
                p.dark
            );
        }
        assert!(
            contrast_ratio(p.text, p.card) >= 4.5,
            "inactive text dark={}",
            p.dark
        );
    }
}
