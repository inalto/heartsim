//! B7 acceptance tests: strip performance, calipers, paper speed/gain scaling, pause + scrub and
//! PNG snapshots, driven through a headless `egui::Context` (real pointer events, no window).

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape, Ui, Vec2};
use heartsim_core::{LEAD_COUNT, Lead, Millis};
use heartsim_ecg::{GaussianSynth, LeadRing};
use heartsim_sim::{Engine, TimingEngine, TimingParams};
use heartsim_ui::{EcgView, StripStyle, TwelveLeadLayout, ecg_strip};
use std::time::Instant;

const FS: f32 = 500.0;
const SCREEN: Vec2 = Vec2::new(1600.0, 1000.0);

/// Headless egui driver: one `run_ui` per call, time advancing 1/60 s per frame.
struct Harness {
    ctx: egui::Context,
    time: f64,
    modifiers: Modifiers,
}

impl Harness {
    fn new() -> Self {
        Self {
            ctx: egui::Context::default(),
            time: 0.0,
            modifiers: Modifiers::NONE,
        }
    }

    fn frame(&mut self, mut events: Vec<Event>, ui_fn: impl FnMut(&mut Ui)) -> egui::FullOutput {
        self.time += 1.0 / 60.0;
        events.insert(0, Event::ModifiersChanged(self.modifiers));
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            time: Some(self.time),
            focused: true,
            events,
            ..RawInput::default()
        };
        let mut out = self.ctx.run_ui(input, ui_fn);
        // No renderer here: drop the font-atlas uploads explicitly.
        out.textures_delta.clear();
        out
    }

    fn button(&self, pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: self.modifiers,
        }
    }

    /// Press at `from`, move to `to` in a few steps, release; `ui_fn` runs every frame.
    fn drag(&mut self, from: Pos2, to: Pos2, mut ui_fn: impl FnMut(&mut Ui)) {
        self.frame(vec![Event::PointerMoved(from)], &mut ui_fn);
        let press = self.button(from, true);
        self.frame(vec![press], &mut ui_fn);
        for k in 1..=4 {
            let p = from + (to - from) * (k as f32 / 4.0);
            self.frame(vec![Event::PointerMoved(p)], &mut ui_fn);
        }
        let release = self.button(to, false);
        self.frame(vec![release], &mut ui_fn);
        self.frame(vec![], &mut ui_fn);
    }

    fn click(&mut self, at: Pos2, mut ui_fn: impl FnMut(&mut Ui)) {
        self.frame(vec![Event::PointerMoved(at)], &mut ui_fn);
        let (press, release) = (self.button(at, true), self.button(at, false));
        self.frame(vec![press], &mut ui_fn);
        self.frame(vec![release], &mut ui_fn);
        self.frame(vec![], &mut ui_fn);
    }
}

/// Twelve rings holding `seconds` of a 60 bpm spike train (1 mV, σ = 8 ms, R at k·1000 + 500 ms).
fn spike_rings(seconds: f32) -> Vec<LeadRing> {
    let n = (seconds * FS) as usize;
    (0..LEAD_COUNT)
        .map(|_| {
            let mut r = LeadRing::new((60.0 * FS) as usize, FS);
            for k in 0..n {
                let t_ms = k as f32 * 1000.0 / FS;
                let d = (t_ms % 1000.0) - 500.0;
                r.push((-0.5 * (d / 8.0).powi(2)).exp());
            }
            r
        })
        .collect()
}

/// Twelve rings from the real timing engine + Tier-0 synthesiser (what the app shows).
fn engine_rings(seconds: f64) -> Vec<LeadRing> {
    let mut engine = TimingEngine::new(TimingParams::default());
    let mut synth = GaussianSynth::textbook();
    let mut rings: Vec<LeadRing> = (0..LEAD_COUNT)
        .map(|_| LeadRing::new((60.0 * FS) as usize, FS))
        .collect();
    let steps = (seconds * 1000.0) as usize;
    for k in 0..steps {
        engine.step(Millis(1.0));
        let ev = engine.drain_events();
        synth.on_events(&ev);
        if k % 2 == 1 {
            let s = synth.sample(engine.now());
            for (r, v) in rings.iter_mut().zip(s.0) {
                r.push(v as f32);
            }
        }
    }
    rings
}

#[test]
fn twelve_lead_page_with_thirty_second_rings_renders_fast() {
    let rings = engine_rings(30.0);
    assert_eq!(rings[0].len(), 15_000, "12 rings × 30 s at 500 Hz");
    let mut view = EcgView::new();
    let mut h = Harness::new();
    let rect = Rect::from_min_size(Pos2::ZERO, SCREEN);
    for layout in [TwelveLeadLayout::Stacked, TwelveLeadLayout::Clinical] {
        let mut draw = |ui: &mut Ui| {
            view.update(&rings);
            view.page(ui, rect, &rings, layout);
        };
        for _ in 0..3 {
            h.frame(vec![], &mut draw);
        }
        let frames = 20;
        let t0 = Instant::now();
        for _ in 0..frames {
            let out = h.frame(vec![], &mut draw);
            let prims = h.ctx.tessellate(out.shapes, out.pixels_per_point);
            assert!(!prims.is_empty());
        }
        let per_frame_ms = t0.elapsed().as_secs_f64() * 1000.0 / f64::from(frames);
        eprintln!("{layout:?}: {per_frame_ms:.2} ms/frame (target < 4 ms; CI threshold 20 ms)");
        assert!(per_frame_ms < 20.0, "{layout:?}: {per_frame_ms:.1} ms/frame");
    }
}

#[test]
fn caliper_on_sixty_bpm_reads_one_second_and_sixty_bpm() {
    let rings = spike_rings(30.0);
    let mut view = EcgView::new();
    let mut h = Harness::new();
    let rect = Rect::from_min_size(Pos2::new(0.0, 100.0), Vec2::new(1500.0, 200.0));
    let mut win = None;
    h.frame(vec![], |ui| win = Some(view.strip(ui, rect, &rings, Lead::II)));
    let win = win.unwrap();
    // Two consecutive R peaks in the window.
    let head = win.head;
    let r2 = (head / 500 - 2) * 500 + 250;
    let r1 = r2 - 500;
    let (x1, x2) = (win.x_of_sample(r1).unwrap(), win.x_of_sample(r2).unwrap());
    let y = rect.center().y;
    let ppm = win.px_per_ms();

    // Shift-click 300 ms before R1 adds a 1-s pair; then drag each line roughly (20–30 ms off)
    // onto its R peak and let it snap.
    h.modifiers = Modifiers::SHIFT;
    h.click(Pos2::new(x1 - 300.0 * ppm, y), |ui| {
        view.strip(ui, rect, &rings, Lead::II);
    });
    h.modifiers = Modifiers::NONE;
    assert_eq!(view.strip_calipers().pairs.len(), 1, "shift-click adds a pair");
    let c = view.strip_calipers().pairs[0];
    let (xa, xb) = (win.x_from_left_ms(c.a_ms), win.x_from_left_ms(c.b_ms));
    h.drag(Pos2::new(xa, y), Pos2::new(x1 + 20.0 * ppm, y), |ui| {
        view.strip(ui, rect, &rings, Lead::II);
    });
    h.drag(Pos2::new(xb, y), Pos2::new(x2 - 30.0 * ppm, y), |ui| {
        view.strip(ui, rect, &rings, Lead::II);
    });

    let reading = view
        .strip_calipers()
        .reading(0, &win, &rings[Lead::II as usize], false)
        .unwrap();
    assert!((reading.interval_ms - 1000.0).abs() <= 10.0, "{reading:?}");
    let bpm = reading.rate_bpm.unwrap();
    assert!((bpm - 60.0).abs() <= 0.6, "{bpm}");
    // Both lines sit on R peaks, so the amplitude read-out is ~1 mV each and ΔV ~ 0.
    assert!(
        reading.a_mv.unwrap() > 0.9 && reading.b_mv.unwrap() > 0.9,
        "{reading:?}"
    );
    assert!(reading.text().contains("60 bpm"), "{}", reading.text());

    // Keyboard nudge moves the selected line by one sample; Escape resets.
    let before = view.strip_calipers().pairs[0];
    h.click(Pos2::new(win.x_from_left_ms(before.b_ms), y), |ui| {
        view.strip(ui, rect, &rings, Lead::II);
    });
    let key = |k| Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    h.frame(vec![key(egui::Key::ArrowRight)], |ui| {
        view.strip(ui, rect, &rings, Lead::II);
    });
    let after = view.strip_calipers().pairs[0];
    assert!(
        (after.b_ms - before.b_ms - 2.0).abs() < 1e-3,
        "{before:?} → {after:?}"
    );
    h.frame(vec![key(egui::Key::Escape)], |ui| {
        view.strip(ui, rect, &rings, Lead::II);
    });
    assert_eq!(view.strip_calipers().pairs, Vec::new(), "Escape resets");
}

/// Vertical extent of the longest polyline in the output (the trace), screen points.
fn trace_height(out: &egui::FullOutput) -> f32 {
    let mut best: Option<&egui::epaint::PathShape> = None;
    for s in &out.shapes {
        if let Shape::Path(p) = &s.shape
            && best.is_none_or(|b| p.points.len() > b.points.len())
        {
            best = Some(p);
        }
    }
    let p = best.expect("a trace");
    let (lo, hi) = p
        .points
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), q| (lo.min(q.y), hi.max(q.y)));
    hi - lo
}

#[test]
fn speed_and_gain_scale_the_drawn_strip() {
    let rings = spike_rings(30.0);
    let ring = &rings[Lead::II as usize];
    let rect = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(750.0, 300.0));
    let mut h = Harness::new();
    let mut draw = |style: StripStyle| {
        let mut secs = 0.0;
        let out = h.frame(vec![], |ui| secs = ecg_strip(ui, rect, ring, &style, "II"));
        (secs, trace_height(&out))
    };
    let base = StripStyle::default();
    let (s25, h10) = draw(base);
    let (s50, _) = draw(StripStyle {
        mm_per_s: 50.0,
        ..base
    });
    let (_, h20) = draw(StripStyle {
        mm_per_mv: 20.0,
        ..base
    });
    assert!((s25 - 2.0 * s50).abs() < 1e-3, "{s25} s vs {s50} s");
    assert!((h20 / h10 - 2.0).abs() < 0.02, "{h10} px vs {h20} px");
    assert!(
        (h10 - 30.0).abs() < 1.0,
        "1 mV at 10 mm/mV × 3 pt/mm is 30 pt, got {h10}"
    );
}

#[test]
fn pause_scrub_back_five_seconds_then_resume_without_a_time_jump() {
    let mut rings = spike_rings(30.0);
    let mut view = EcgView::new();
    let mut h = Harness::new();
    let rect = Rect::from_min_size(Pos2::new(0.0, 100.0), Vec2::new(1500.0, 200.0));
    let now_ms = |rings: &[LeadRing]| (rings[0].pushed() - 1) as f64 * 1000.0 / f64::from(FS);
    let sample_ms = 1000.0 / f64::from(FS);

    view.toggle_pause();
    assert!(view.is_paused());
    let now = now_ms(&rings);
    let px_per_ms = heartsim_ui::StripWindow::new(rect, &view.style, FS, 0).px_per_ms();
    let from = Pos2::new(400.0, rect.center().y);
    h.drag(from, from + Vec2::new(5000.0 * px_per_ms, 0.0), |ui| {
        view.strip(ui, rect, &rings, Lead::II);
    });
    let cursor = view.cursor_time_ms(&rings);
    assert!(
        (cursor - (now - 5000.0)).abs() <= sample_ms,
        "cursor {cursor} ms, now {now} ms"
    );
    assert!((view.scrub_offset_ms() - 5000.0).abs() <= sample_ms);

    // Resume: the view returns to "now", which did not move while paused, and continues from it.
    view.toggle_pause();
    assert!(!view.is_paused());
    assert!((view.cursor_time_ms(&rings) - now).abs() < 1e-9);
    for r in &mut rings {
        for _ in 0..50 {
            r.push(0.0);
        }
    }
    assert!((view.cursor_time_ms(&rings) - (now + 50.0 * sample_ms)).abs() < 1e-9);
}

#[test]
fn snapshot_png_has_the_requested_size_and_shows_the_trace() {
    let rings = spike_rings(30.0);
    let view = EcgView::new();
    for layout in [TwelveLeadLayout::Clinical, TwelveLeadLayout::Stacked] {
        let png = view.page_png(&rings, layout, [1200, 800], 1.0).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), (1200, 800));
        let t = view.style.trace;
        let trace_px = img.pixels().filter(|p| p.0 == [t.r(), t.g(), t.b(), 255]).count();
        assert!(trace_px > 1000, "{layout:?}: only {trace_px} trace pixels");
    }
}

#[test]
fn engine_ecg_gets_wave_labels_and_intervals() {
    let rings = engine_rings(12.0);
    let mut view = EcgView::new();
    view.update(&rings);
    let a = view.analysis().expect("analysed");
    let hr = a.hr_bpm.expect("heart rate");
    assert!((40.0..150.0).contains(&hr), "{hr}");
    assert!(a.pr_ms.is_some() && a.qrs_ms.is_some(), "{a:?}");
    // Drawing the labelled page must not panic in any layout or order.
    let mut h = Harness::new();
    for order in [heartsim_ui::LeadOrder::Standard, heartsim_ui::LeadOrder::Cabrera] {
        view.order = order;
        for layout in [
            TwelveLeadLayout::Stacked,
            TwelveLeadLayout::TwoColumn,
            TwelveLeadLayout::Clinical,
        ] {
            let out = h.frame(vec![], |ui| {
                view.page(ui, Rect::from_min_size(Pos2::ZERO, SCREEN), &rings, layout);
            });
            let texts = out
                .shapes
                .iter()
                .filter(|s| matches!(s.shape, Shape::Text(_)))
                .count();
            assert!(texts > 12, "labels drawn: {texts}");
        }
    }
}
