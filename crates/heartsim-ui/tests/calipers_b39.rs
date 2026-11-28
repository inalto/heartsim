//! B39 acceptance tests: caliper handles, pointer shapes and the recorded strip's pan, headless.
//! The cursor shapes and the touch targets are asserted on the values egui is given, never seen
//! on a screen.

use egui::{
    CursorIcon, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, TouchDeviceId, TouchId, TouchPhase,
    Ui, Vec2,
};
use heartsim_ecg::LeadRing;
use heartsim_ui::calipers::{
    CaliperSet, TOUCH_TARGET, band_grip_rect, draw_calipers, handle_rect, record_interact, target_rect,
};
use heartsim_ui::strip::{StripStyle, StripTrace, StripWindow, draw_record_strip};

const FS: f32 = 500.0;
const LEN: usize = 5000;

fn ring() -> LeadRing {
    let mut r = LeadRing::new(LEN, FS);
    for k in 0..LEN {
        r.push(if k % 500 == 250 { 1.0 } else { 0.0 });
    }
    r
}

fn rect() -> Rect {
    Rect::from_min_size(Pos2::new(20.0, 100.0), Vec2::new(1200.0, 240.0))
}

struct Rig {
    ctx: egui::Context,
    time: f64,
    ring: LeadRing,
    calipers: CaliperSet,
    first: u64,
    mm_per_s: f32,
    win: Option<StripWindow>,
    cursor: CursorIcon,
}

impl Rig {
    fn new(mm_per_s: f32) -> Self {
        let mut calipers = CaliperSet::default();
        calipers.snap = false;
        Self {
            ctx: egui::Context::default(),
            time: 0.0,
            ring: ring(),
            calipers,
            first: 0,
            mm_per_s,
            win: None,
            cursor: CursorIcon::Default,
        }
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.time += 1.0 / 60.0;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 800.0))),
            time: Some(self.time),
            focused: true,
            events,
            ..RawInput::default()
        };
        let style = StripStyle {
            mm_per_s: self.mm_per_s,
            ..StripStyle::default()
        };
        let (ring, calipers, first, win) = (&self.ring, &mut self.calipers, &mut self.first, &mut self.win);
        let mut out = self.ctx.run_ui(input, |ui: &mut Ui| {
            let trace = StripTrace {
                label: "II",
                invert: false,
                head: ring.pushed(),
                marks: None,
                show_waves: false,
                show_intervals: false,
            };
            let id = ui.scope_id().with("strip");
            let w = draw_record_strip(ui, id, rect(), ring, &style, &trace, *first);
            record_interact(ui, id, &w, ring, calipers, first);
            draw_calipers(ui, &w, ring, false, calipers, &style);
            *win = Some(w);
        });
        out.textures_delta.clear();
        self.cursor = out.platform_output.cursor_icon;
    }

    fn win(&self) -> StripWindow {
        self.win.expect("a frame was run")
    }

    fn cursor(&self) -> CursorIcon {
        self.cursor
    }

    fn hover(&mut self, at: Pos2) {
        self.frame(vec![Event::PointerMoved(at)]);
        self.frame(vec![]);
    }

    /// Press at `from`, move to `to` in steps, release.
    fn drag(&mut self, from: Pos2, to: Pos2) {
        let button = |at, pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![button(from, true)]);
        for k in 1..=5 {
            let at = from + (to - from) * (k as f32 / 5.0);
            self.frame(vec![Event::PointerMoved(at)]);
        }
        self.frame(vec![button(to, false)]);
        self.frame(vec![]);
    }
}

fn pair_rig(mm_per_s: f32) -> Rig {
    let mut rig = Rig::new(mm_per_s);
    rig.frame(vec![]);
    let w = rig.win();
    rig.calipers
        .add_pair(w.caliper_ms_at_x(400.0), w.caliper_ms_at_x(700.0));
    rig.frame(vec![]);
    rig
}

#[test]
fn hovering_a_handle_shows_the_resize_arrows_and_the_band_grip_an_open_hand() {
    let mut rig = pair_rig(50.0);
    let w = rig.win();
    let xa = w.x_of_caliper_ms(rig.calipers.pairs[0].a_ms);
    for top in [true, false] {
        rig.hover(handle_rect(&w, xa, top).center());
        assert_eq!(rig.cursor(), CursorIcon::ResizeHorizontal, "handle (top: {top})");
    }
    // along the line, mid-height
    rig.hover(Pos2::new(xa + 4.0, w.rect.center().y));
    assert_eq!(rig.cursor(), CursorIcon::ResizeHorizontal);
    // the grip of the band
    let grip = band_grip_rect(&w, 0, rig.calipers.pairs[0]).expect("on screen");
    rig.hover(grip.center());
    assert_eq!(rig.cursor(), CursorIcon::Grab);
    // free paper, and the band away from its grip: nothing special
    rig.hover(Pos2::new(1000.0, w.rect.center().y));
    assert_eq!(rig.cursor(), CursorIcon::Default);
    rig.hover(Pos2::new(550.0, w.rect.center().y + 40.0));
    assert_eq!(rig.cursor(), CursorIcon::Default);
}

#[test]
fn dragging_a_handle_moves_that_end_by_the_distance_over_px_per_ms_and_not_the_other() {
    let mut ms_per_drag = Vec::new();
    for mm_per_s in [50.0, 100.0] {
        let mut rig = pair_rig(mm_per_s);
        let w = rig.win();
        let before = rig.calipers.pairs[0];
        let from = handle_rect(&w, w.x_of_caliper_ms(before.b_ms), true).center();
        rig.drag(from, from + Vec2::new(50.0, 0.0));
        assert_eq!(
            rig.cursor(),
            CursorIcon::ResizeHorizontal,
            "released over the moved handle"
        );
        let after = rig.calipers.pairs[0];
        assert!((after.a_ms - before.a_ms).abs() < 1e-4, "the other end stays");
        let moved = after.b_ms - before.b_ms;
        assert!(
            (moved - 50.0 / w.px_per_ms()).abs() < 0.05,
            "{mm_per_s} mm/s: moved {moved} ms, expected {}",
            50.0 / w.px_per_ms()
        );
        ms_per_drag.push(moved);
    }
    assert!(
        (ms_per_drag[0] / ms_per_drag[1] - 2.0).abs() < 0.01,
        "twice the zoom, half the milliseconds: {ms_per_drag:?}"
    );
}

#[test]
fn the_cursor_is_held_while_dragging_a_handle_and_the_grip_shows_a_closed_hand() {
    let mut rig = pair_rig(50.0);
    let w = rig.win();
    let a = rig.calipers.pairs[0];
    let from = handle_rect(&w, w.x_of_caliper_ms(a.a_ms), false).center();
    let button = |at, pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    rig.frame(vec![Event::PointerMoved(from)]);
    rig.frame(vec![button(from, true)]);
    // far from any target, still dragging that end
    rig.frame(vec![Event::PointerMoved(from + Vec2::new(300.0, -60.0))]);
    rig.frame(vec![Event::PointerMoved(from + Vec2::new(310.0, -60.0))]);
    assert_eq!(rig.cursor(), CursorIcon::ResizeHorizontal);
    rig.frame(vec![button(from, false)]);

    // the whole pair by its grip: both ends move, the width is kept, the cursor is a closed hand
    let mut rig = pair_rig(50.0);
    let w = rig.win();
    let before = rig.calipers.pairs[0];
    let grip = band_grip_rect(&w, 0, before).unwrap().center();
    rig.frame(vec![Event::PointerMoved(grip)]);
    rig.frame(vec![button(grip, true)]);
    rig.frame(vec![Event::PointerMoved(grip + Vec2::new(30.0, 0.0))]);
    rig.frame(vec![Event::PointerMoved(grip + Vec2::new(60.0, 0.0))]);
    assert_eq!(rig.cursor(), CursorIcon::Grabbing);
    rig.frame(vec![button(grip + Vec2::new(60.0, 0.0), false)]);
    let after = rig.calipers.pairs[0];
    assert!((after.interval_ms() - before.interval_ms()).abs() < 1e-3);
    assert!((after.a_ms - before.a_ms - 60.0 / w.px_per_ms()).abs() < 0.05);
}

#[test]
fn dragging_free_paper_pans_the_record_and_is_clamped() {
    let mut rig = Rig::new(200.0);
    rig.frame(vec![]);
    let w = rig.win();
    assert!(
        w.max_record_first() > 0,
        "zoomed in, part of the record is out of view"
    );
    let y = w.rect.center().y + 70.0;
    // drag left by 100 points: later samples come in, `first` grows by 100 / px_per_sample
    rig.drag(Pos2::new(900.0, y), Pos2::new(800.0, y));
    let expect = (100.0 / w.px_per_sample).round() as u64;
    assert!(rig.first.abs_diff(expect) <= 1, "first {} vs {expect}", rig.first);
    // drag right a long way: back to the start, never before it
    rig.drag(Pos2::new(300.0, y), Pos2::new(1100.0, y));
    rig.drag(Pos2::new(300.0, y), Pos2::new(1100.0, y));
    assert_eq!(rig.first, 0);
    // drag left a long way: stops at the last full strip
    for _ in 0..12 {
        rig.drag(Pos2::new(1100.0, y), Pos2::new(300.0, y));
    }
    assert_eq!(rig.first, w.max_record_first());
}

#[test]
fn a_caliper_line_stays_on_its_sample_while_the_record_is_panned() {
    let mut rig = pair_rig(200.0);
    let w = rig.win();
    let a = rig.calipers.pairs[0];
    let s0 = w.sample_at_x(w.x_of_caliper_ms(a.a_ms)).unwrap();
    rig.first = 300;
    rig.frame(vec![]);
    let w2 = rig.win();
    assert_eq!(rig.calipers.pairs[0], a, "panning does not edit the pair");
    let x = w2.x_of_caliper_ms(a.a_ms);
    if w2.rect.contains(Pos2::new(x, w2.rect.center().y)) {
        assert_eq!(w2.sample_at_x(x), Some(s0));
    }
    assert!(x < w.x_of_caliper_ms(a.a_ms), "it moved left on the screen");
}

#[test]
fn two_fingers_on_the_strip_never_start_a_caliper_drag() {
    let pinch_events = |from: Pos2, to: Pos2| {
        let touch = |id: u64, phase, pos| Event::Touch {
            device_id: TouchDeviceId(0),
            id: TouchId(id),
            phase,
            pos,
            force: None,
        };
        (
            vec![
                touch(1, TouchPhase::Start, from),
                touch(2, TouchPhase::Start, from + Vec2::new(80.0, 0.0)),
            ],
            vec![
                touch(1, TouchPhase::Move, to),
                touch(2, TouchPhase::Move, to + Vec2::new(80.0, 0.0)),
            ],
        )
    };
    let button = |at, pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    let run = |with_second_finger: bool| {
        let mut rig = pair_rig(50.0);
        let w = rig.win();
        let before = rig.calipers.pairs[0];
        let from = handle_rect(&w, w.x_of_caliper_ms(before.a_ms), true).center();
        let to = from + Vec2::new(40.0, 0.0);
        let (start, moved) = pinch_events(from, to);
        rig.frame(vec![Event::PointerMoved(from)]);
        let mut press = vec![button(from, true)];
        if with_second_finger {
            press.splice(0..0, start);
        }
        rig.frame(press);
        for k in 1..=4 {
            let mut ev = vec![Event::PointerMoved(from + (to - from) * (k as f32 / 4.0))];
            if with_second_finger && k == 1 {
                ev.splice(0..0, moved.clone());
            }
            rig.frame(ev);
        }
        rig.frame(vec![button(to, false)]);
        rig.frame(vec![]);
        (before, rig.calipers.pairs[0])
    };
    let (b, a) = run(false);
    assert!(a.a_ms > b.a_ms + 1.0, "control: one pointer drags the line");
    let (b, a) = run(true);
    assert_eq!(a, b, "two fingers: the pair did not move");
}

#[test]
fn the_touch_target_of_a_handle_is_at_least_44_points() {
    let w = StripWindow::record(rect(), &StripStyle::default(), FS, 0, LEN as u64);
    let h = handle_rect(&w, 300.0, true);
    for (touch, min) in [(false, 24.0), (true, TOUCH_TARGET)] {
        let t = target_rect(h, touch);
        assert!(t.width() >= min && t.height() >= min, "{touch}: {t:?}");
        assert_eq!(t.center(), h.center());
    }
}
