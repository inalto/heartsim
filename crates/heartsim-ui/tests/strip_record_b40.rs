//! B40 acceptance: a finished recording is drawn from the left edge of its strip, at every paper
//! speed and strip width, on the real drawing path (the polyline the painter emits), in the
//! single strip and in the page layouts; live strips keep their right-aligned window.

use egui::{Event, Pos2, RawInput, Rect, Shape, Ui, Vec2};
use heartsim_core::{LEAD_COUNT, Lead};
use heartsim_ecg::LeadRing;
use heartsim_ui::{
    StripStyle, TwelveLeadLayout, ecg_record_strip, ecg_strip, record_column, twelve_lead_record_page,
};

const FS: f32 = 500.0;

/// A recording of `seconds` with a beat every second, so the trace is never a straight line.
fn recording(seconds: f32) -> Vec<LeadRing> {
    let n = (seconds * FS) as usize;
    (0..LEAD_COUNT)
        .map(|_| {
            let mut r = LeadRing::new(n, FS);
            for k in 0..n {
                let d = (k as f32 * 1000.0 / FS) % 1000.0 - 500.0;
                r.push((-0.5 * (d / 8.0).powi(2)).exp() + 0.05 * (k as f32 * 0.05).sin());
            }
            r
        })
        .collect()
}

fn run(rect: Rect, mut ui_fn: impl FnMut(&mut Ui)) -> egui::FullOutput {
    let ctx = egui::Context::default();
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(
            Pos2::ZERO,
            Vec2::new(3000.0, 1200.0).max(rect.max.to_vec2()),
        )),
        time: Some(0.0),
        focused: true,
        events: Vec::<Event>::new(),
        ..RawInput::default()
    };
    let mut out = ctx.run_ui(input, &mut ui_fn);
    out.textures_delta.clear();
    out
}

/// The trace polylines (4 points is the calibration pulse; the rest are traces), as x extents.
fn trace_extents(out: &egui::FullOutput) -> Vec<(f32, f32)> {
    out.shapes
        .iter()
        .filter_map(|s| match &s.shape {
            Shape::Path(p) if p.points.len() > 4 => Some(
                p.points
                    .iter()
                    .fold((f32::MAX, f32::MIN), |(lo, hi), q| (lo.min(q.x), hi.max(q.x))),
            ),
            _ => None,
        })
        .collect()
}

#[test]
fn a_ten_second_recording_starts_at_the_left_at_every_speed_and_width() {
    let rings = recording(10.0);
    for mm_per_s in [12.5, 25.0, 50.0] {
        for width in [400.0_f32, 750.0, 1200.0, 2400.0] {
            let style = StripStyle {
                mm_per_s,
                ..StripStyle::default()
            };
            let rect = Rect::from_min_size(Pos2::new(30.0, 10.0), Vec2::new(width, 200.0));
            let out = run(rect, |ui| {
                ecg_record_strip(ui, rect, &rings[0], &style, "II", 0);
            });
            let ext = trace_extents(&out);
            assert_eq!(ext.len(), 1, "one trace");
            let (lo, hi) = ext[0];
            let px = mm_per_s * style.px_per_mm / FS;
            assert!(
                lo >= rect.left() && lo <= rect.left() + 2.0 * px.max(1.0),
                "{mm_per_s} mm/s, {width} pt: the trace starts at {lo}, the strip at {}",
                rect.left()
            );
            assert!(
                hi <= rect.right() + 1e-3,
                "{mm_per_s} mm/s, {width} pt: ends at {hi}"
            );
        }
    }
}

#[test]
fn the_live_window_still_puts_the_newest_sample_at_the_right() {
    // what the recording used to get, and what the live strips keep
    let rings = recording(10.0);
    let style = StripStyle::default();
    let rect = Rect::from_min_size(Pos2::new(30.0, 10.0), Vec2::new(1200.0, 200.0));
    let out = run(rect, |ui| {
        ecg_strip(ui, rect, &rings[0], &style, "II");
    });
    let (lo, hi) = trace_extents(&out)[0];
    assert!(
        lo > rect.left() + 400.0,
        "10 s are 750 pt of 1200: the left is empty, trace at {lo}"
    );
    assert!((hi - rect.right()).abs() < 2.0, "ends at the right edge, {hi}");
}

#[test]
fn a_longer_recording_shows_its_first_seconds_and_a_later_first_pans_it() {
    let rings = recording(60.0);
    let style = StripStyle::default();
    let rect = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(750.0, 200.0));
    let (secs, out0) = {
        let mut secs = 0.0;
        let out = run(rect, |ui| {
            secs = ecg_record_strip(ui, rect, &rings[0], &style, "II", 0);
        });
        (secs, out)
    };
    assert!((secs - 10.0).abs() < 1e-3, "10 s of the 60 s on 750 pt");
    let (lo, hi) = trace_extents(&out0)[0];
    assert!(lo < 3.0 && hi <= rect.right() + 1e-3, "{lo}..{hi}");
    // panned to 30 s: still starts at the left edge, and the beats sit where the first window's did
    let out30 = run(rect, |ui| {
        ecg_record_strip(ui, rect, &rings[0], &style, "II", 30 * 500);
    });
    let (lo30, hi30) = trace_extents(&out30)[0];
    assert!(lo30 < 3.0 && hi30 <= rect.right() + 1e-3, "{lo30}..{hi30}");
}

#[test]
fn every_cell_of_the_page_and_every_row_of_the_column_starts_at_its_left_edge() {
    let rings = recording(10.0);
    let style = StripStyle::default();
    let rect = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(1600.0, 800.0));
    let out = run(rect, |ui| {
        twelve_lead_record_page(ui, rect, &rings, &style, TwelveLeadLayout::Clinical, 0);
    });
    let ext = trace_extents(&out);
    assert_eq!(ext.len(), 13, "twelve cells and the rhythm strip");
    // cells are 400 pt wide at 3 pt/mm × 25 mm/s: 5.3 s, the rhythm strip 1600 pt: 21 s
    for (lo, hi) in &ext {
        let col = ((lo - rect.left()) / 400.0).floor();
        let left = rect.left() + 400.0 * col + 1.0;
        assert!(
            *lo >= left && *lo < left + 3.0,
            "trace at {lo}, cell edge at {left}"
        );
        assert!(*hi <= rect.right() + 1e-3);
    }
    let out = run(rect, |ui| {
        record_column(ui, rect, &rings, &style, &[Lead::II, Lead::V1], 0);
    });
    let ext = trace_extents(&out);
    assert_eq!(ext.len(), 2);
    for (lo, _) in ext {
        assert!(lo >= rect.left() + 1.0 && lo < rect.left() + 4.0, "{lo}");
    }
}
