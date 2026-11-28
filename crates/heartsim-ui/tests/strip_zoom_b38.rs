//! B38 acceptance tests: the time zoom of the ECG strips. The zoom is a drawing matter: the strips
//! are drawn at the effective paper speed (preset × zoom), the exports keep the preset, a paused
//! strip keeps the sample under the pointer and its calipers on their samples, a preset resets the
//! zoom, and the plain wheel zooms only where no scroll area would take it. Headless, no GPU.

use egui::{Event, Modifiers, MouseWheelSource, MouseWheelUnit, Pos2, RawInput, Rect, TouchPhase, Ui, Vec2};
use egui_kittest::Harness;
use heartsim_core::{LEAD_COUNT, Lead};
use heartsim_ecg::LeadRing;
use heartsim_ui::strip::SPEEDS_MM_S;
use heartsim_ui::{EcgView, StripWindow, TwelveLeadLayout};

const FS: f32 = 500.0;

/// Twelve rings holding `seconds` of a 60 bpm spike train (R at k·1000 + 500 ms).
fn spike_rings(seconds: f32) -> Vec<LeadRing> {
    let n = (seconds * FS) as usize;
    (0..LEAD_COUNT)
        .map(|_| {
            let mut r = LeadRing::new((60.0 * FS) as usize, FS);
            for k in 0..n {
                let d = (k as f32 * 1000.0 / FS % 1000.0) - 500.0;
                r.push((-0.5 * (d / 8.0).powi(2)).exp());
            }
            r
        })
        .collect()
}

/// Headless egui driver: one `run_ui` per frame, time advancing 1/60 s.
struct Frames {
    ctx: egui::Context,
    time: f64,
}

impl Frames {
    fn new() -> Self {
        Self {
            ctx: egui::Context::default(),
            time: 0.0,
        }
    }

    fn frame(&mut self, events: Vec<Event>, ui_fn: impl FnMut(&mut Ui)) {
        self.time += 1.0 / 60.0;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 1000.0))),
            time: Some(self.time),
            focused: true,
            events,
            ..RawInput::default()
        };
        let mut out = self.ctx.run_ui(input, ui_fn);
        out.textures_delta.clear();
    }

    /// Moves the pointer to `at` and lets two frames settle, so the strip is hovered.
    fn hover(&mut self, at: Pos2, mut ui_fn: impl FnMut(&mut Ui)) {
        self.frame(vec![Event::PointerMoved(at)], &mut ui_fn);
        self.frame(vec![], &mut ui_fn);
    }
}

fn strip_rect() -> Rect {
    Rect::from_min_size(Pos2::new(20.0, 100.0), Vec2::new(1500.0, 200.0))
}

/// Draws the strip, returning its window.
fn draw(view: &mut EcgView, rings: &[LeadRing], ui: &mut Ui) -> StripWindow {
    view.strip(ui, strip_rect(), rings, Lead::II)
}

#[test]
fn the_effective_speed_stays_within_6_25_and_400_mm_per_s_for_every_preset() {
    for preset in SPEEDS_MM_S {
        let mut view = EcgView::new();
        view.set_paper_speed(preset);
        view.set_time_zoom(1e6);
        assert!(
            (view.effective_mm_per_s() - 400.0).abs() < 1e-3,
            "{preset}: {}",
            view.effective_mm_per_s()
        );
        view.set_time_zoom(1e-6);
        assert!(
            (view.effective_mm_per_s() - 6.25).abs() < 1e-3,
            "{preset}: {}",
            view.effective_mm_per_s()
        );
    }
}

#[test]
fn the_window_of_the_effective_style_scales_with_the_zoom() {
    let rect = strip_rect();
    let mut view = EcgView::new();
    let base = StripWindow::new(rect, &view.effective_style(), FS, 10_000);
    view.set_time_zoom(2.0);
    let zoomed = StripWindow::new(rect, &view.effective_style(), FS, 10_000);
    assert!((zoomed.px_per_ms() / base.px_per_ms() - 2.0).abs() < 1e-5);
    let ratio = base.visible_samples as f64 / zoomed.visible_samples as f64;
    assert!(
        (ratio - 2.0).abs() < 0.01,
        "{} vs {}",
        base.visible_samples,
        zoomed.visible_samples
    );
    assert!((view.effective_mm_per_s() - 50.0).abs() < 1e-4);
}

#[test]
fn a_zoom_gesture_over_the_strip_changes_the_speed_and_one_away_from_it_does_not() {
    let rings = spike_rings(30.0);
    let mut view = EcgView::new();
    let mut f = Frames::new();
    let over = strip_rect().center();
    f.hover(over, |ui| {
        draw(&mut view, &rings, ui);
    });
    f.frame(vec![Event::Zoom(2.0)], |ui| {
        draw(&mut view, &rings, ui);
    });
    assert!(
        (view.effective_mm_per_s() - 50.0).abs() < 1e-3,
        "{}",
        view.effective_mm_per_s()
    );

    let mut view = EcgView::new();
    let away = Pos2::new(over.x, 600.0);
    f.hover(away, |ui| {
        draw(&mut view, &rings, ui);
    });
    f.frame(vec![Event::Zoom(2.0)], |ui| {
        draw(&mut view, &rings, ui);
    });
    assert!(
        (view.effective_mm_per_s() - 25.0).abs() < 1e-3,
        "pointer elsewhere: no zoom"
    );
}

#[test]
fn paused_zoom_about_the_pointer_keeps_the_sample_under_it() {
    let rings = spike_rings(60.0);
    for (factor, x) in [(2.0_f32, 1100.0_f32), (1.5, 300.0), (0.5, 900.0), (4.0, 1400.0)] {
        let mut view = EcgView::new();
        view.toggle_pause();
        view.scrub_back_by(12_000.0, &rings);
        let mut f = Frames::new();
        let at = Pos2::new(x, strip_rect().center().y);
        let mut before = None;
        f.hover(at, |ui| before = Some(draw(&mut view, &rings, ui)));
        let before = before.unwrap();
        let s0 = before.sample_at_x(x).expect("a sample under the pointer");
        let mut after = None;
        f.frame(vec![Event::Zoom(factor)], |ui| {
            after = Some(draw(&mut view, &rings, ui));
        });
        let after = after.unwrap();
        assert!(
            (after.px_per_ms() / before.px_per_ms() - factor).abs() < 1e-3,
            "zoomed by {factor}"
        );
        let s1 = after
            .sample_at_x(x)
            .expect("a sample under the pointer after the zoom");
        assert!(
            s0.abs_diff(s1) <= 1,
            "factor {factor} at x {x}: sample {s0} became {s1}"
        );
    }
}

#[test]
fn paused_zoom_keeps_a_caliper_line_on_its_sample() {
    let rings = spike_rings(60.0);
    for factor in [2.0_f32, 0.6] {
        let mut view = EcgView::new();
        view.toggle_pause();
        view.scrub_back_by(12_000.0, &rings);
        let mut f = Frames::new();
        let at = Pos2::new(1000.0, strip_rect().center().y);
        let mut win = None;
        f.hover(at, |ui| win = Some(draw(&mut view, &rings, ui)));
        let win = win.unwrap();
        view.strip_calipers_mut()
            .add_pair(win.ms_from_left(800.0), win.ms_from_left(1200.0));
        let samples = |w: &StripWindow, view: &EcgView| {
            let c = view.strip_calipers().pairs[0];
            [c.a_ms, c.b_ms].map(|ms| w.sample_at_x(w.x_from_left_ms(ms)).expect("on screen"))
        };
        let s0 = samples(&win, &view);
        let mut after = None;
        f.frame(vec![Event::Zoom(factor)], |ui| {
            after = Some(draw(&mut view, &rings, ui));
        });
        let after = after.unwrap();
        assert!(
            (after.px_per_ms() / win.px_per_ms() - factor).abs() < 1e-3,
            "zoomed by {factor}"
        );
        let s1 = samples(&after, &view);
        for (a, b) in s0.iter().zip(&s1) {
            assert!(a.abs_diff(*b) <= 1, "factor {factor}: {s0:?} became {s1:?}");
        }
    }
}

#[test]
fn running_zoom_keeps_the_newest_sample_at_the_right_edge_and_the_calipers_their_interval() {
    let rings = spike_rings(30.0);
    let mut view = EcgView::new();
    view.strip_calipers_mut().add_pair(200.0, 1200.0);
    let mut f = Frames::new();
    f.hover(strip_rect().center(), |ui| {
        draw(&mut view, &rings, ui);
    });
    let mut after = None;
    f.frame(vec![Event::Zoom(2.0)], |ui| {
        after = Some(draw(&mut view, &rings, ui));
    });
    let win = after.unwrap();
    assert_eq!(win.head, rings[0].pushed(), "the newest sample is still the head");
    assert_eq!(
        win.sample_at_x(win.rect.right() - 0.1),
        Some(rings[0].pushed() - 1)
    );
    let c = view.strip_calipers().pairs[0];
    assert!((c.interval_ms() - 1000.0).abs() < 1e-3, "{c:?}");
}

#[test]
fn a_preset_resets_the_zoom_and_a_zoom_is_not_the_preset() {
    let mut view = EcgView::new();
    view.set_time_zoom(1.5);
    assert!((view.effective_mm_per_s() - 37.5).abs() < 1e-4);
    assert!(
        (view.style.mm_per_s - 25.0).abs() < 1e-6,
        "the chosen preset is untouched"
    );
    view.set_paper_speed(50.0);
    assert!((view.time_zoom() - 1.0).abs() < 1e-6);
    assert!((view.effective_mm_per_s() - 50.0).abs() < 1e-6);
}

#[test]
fn the_export_keeps_the_preset_speed() {
    let rings = spike_rings(30.0);
    let mut plain = EcgView::new();
    let mut zoomed = EcgView::new();
    zoomed.set_time_zoom(2.0);
    assert!(
        (zoomed.style.mm_per_s - 25.0).abs() < 1e-6,
        "what exports read: the preset"
    );
    let a = plain
        .page_png(&rings, TwelveLeadLayout::Stacked, [800, 600], 1.0)
        .expect("png");
    let b = zoomed
        .page_png(&rings, TwelveLeadLayout::Stacked, [800, 600], 1.0)
        .expect("png");
    assert!(a == b, "the exported page does not depend on the screen zoom");
    plain.update(&rings);
}

fn wheel(delta_y: f32, modifiers: Modifiers) -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Point,
        delta: Vec2::new(0.0, delta_y),
        phase: TouchPhase::Move,
        source: MouseWheelSource::Wheel,
        modifiers,
    }
}

struct Page {
    view: EcgView,
    rings: Vec<LeadRing>,
    in_scroll_area: bool,
    strip: Rect,
    scrolled: f32,
}

fn page(in_scroll_area: bool, plain_wheel_zoom: bool) -> Harness<'static, Page> {
    let mut view = EcgView::new();
    view.plain_wheel_zoom = plain_wheel_zoom;
    Harness::builder()
        .with_size(Vec2::new(900.0, 500.0))
        .build_ui_state(
            |ui, p: &mut Page| {
                let rect =
                    Rect::from_min_size(ui.cursor().min + Vec2::new(0.0, 20.0), Vec2::new(800.0, 160.0));
                p.strip = rect;
                if p.in_scroll_area {
                    let out = egui::ScrollArea::vertical().show(ui, |ui| {
                        p.view.strip(ui, rect, &p.rings, Lead::II);
                        ui.allocate_space(Vec2::new(800.0, 2000.0));
                    });
                    p.scrolled = out.state.clamped_offset().y;
                } else {
                    p.view.strip(ui, rect, &p.rings, Lead::II);
                }
            },
            Page {
                view,
                rings: spike_rings(30.0),
                in_scroll_area,
                strip: Rect::NOTHING,
                scrolled: 0.0,
            },
        )
}

#[test]
fn ctrl_wheel_over_a_strip_zooms_and_the_same_event_away_from_it_does_not() {
    let mut h = page(false, true);
    h.run();
    let over = h.state().strip.center();
    h.hover_at(over);
    h.run();
    h.event_modifiers(wheel(100.0, Modifiers::CTRL), Modifiers::CTRL);
    h.run();
    let fast = h.state().view.effective_mm_per_s();
    assert!(fast > 30.0, "Ctrl + wheel up zooms in, got {fast} mm/s");

    let mut h = page(false, true);
    h.run();
    h.hover_at(Pos2::new(450.0, 400.0));
    h.run();
    h.event_modifiers(wheel(100.0, Modifiers::CTRL), Modifiers::CTRL);
    h.run();
    assert!(
        (h.state().view.effective_mm_per_s() - 25.0).abs() < 1e-3,
        "pointer elsewhere"
    );
}

#[test]
fn the_plain_wheel_zooms_a_free_strip_and_scrolls_one_inside_a_scroll_area() {
    // a strip outside any scroll area takes the plain wheel
    let mut h = page(false, true);
    h.run();
    h.hover_at(h.state().strip.center());
    h.run();
    h.event(wheel(80.0, Modifiers::NONE));
    h.run();
    assert!(
        h.state().view.effective_mm_per_s() > 26.0,
        "{}",
        h.state().view.effective_mm_per_s()
    );

    // inside a scroll area (the host says so) the wheel scrolls and does not zoom
    let mut h = page(true, false);
    h.run();
    h.hover_at(h.state().strip.center());
    h.run();
    h.event(wheel(-120.0, Modifiers::NONE));
    h.run();
    assert!(h.state().scrolled > 10.0, "scrolled {}", h.state().scrolled);
    assert!(
        (h.state().view.effective_mm_per_s() - 25.0).abs() < 1e-3,
        "no zoom"
    );

    // a pinch (zoom event) still zooms there
    h.event(Event::Zoom(2.0));
    h.run();
    assert!((h.state().view.effective_mm_per_s() - 50.0).abs() < 1e-3);
}

#[test]
fn the_twelve_lead_page_zooms_too_running_and_paused() {
    let rings = spike_rings(60.0);
    let rect = Rect::from_min_size(Pos2::new(20.0, 20.0), Vec2::new(1500.0, 900.0));
    for pause in [false, true] {
        let mut view = EcgView::new();
        if pause {
            view.toggle_pause();
            view.scrub_back_by(8000.0, &rings);
        }
        let mut f = Frames::new();
        f.hover(rect.center(), |ui| {
            view.page(ui, rect, &rings, TwelveLeadLayout::TwoColumn);
        });
        f.frame(vec![Event::Zoom(2.0)], |ui| {
            view.page(ui, rect, &rings, TwelveLeadLayout::TwoColumn);
        });
        assert!((view.effective_mm_per_s() - 50.0).abs() < 1e-3, "paused: {pause}");
    }
}
