//! A heart-sound clip drawn whole (brief B28): one ECG lead above the phonocardiogram lane, both
//! fitted to the width so the loop is seen at once, the sound labels from the model's schedule and a
//! playhead moving with what is heard. Used by the Sound Trainer and the auscultation quiz.
//!
//! The horizontal scale is the clip's, not the paper's, so the grid (whose squares mean 0.04 s and
//! 0.1 mV on paper) is left out rather than drawn at a false scale.

use crate::pcg_lane::{PcgLane, draw_pcg_lane, draw_playhead, lane_height};
use crate::strip::{StripStyle, StripTrace, SweepMode, draw_strip};
use egui::{FontId, Rect, Ui, Vec2};
use heartsim_ecg::{LeadRing, SoundLabel};

/// What [`clip_view`] draws.
#[derive(Debug, Clone, Copy)]
pub struct ClipTrace<'a> {
    /// One ECG lead of the clip, from its first sample (500 Hz).
    pub ecg: &'a LeadRing,
    /// The lead's name.
    pub lead_label: &'a str,
    /// The clip's phonocardiogram, from its first sample, an integer multiple of the ECG's rate.
    pub pcg: &'a LeadRing,
    /// Sound labels as `(PCG index, label)`.
    pub marks: &'a [(u64, SoundLabel)],
    /// PCG index being heard now, if the clip plays.
    pub playhead: Option<u64>,
    /// The lane's title (site and chest piece, "model").
    pub title: &'a str,
    /// A tag at the lane's bottom right that says the sound is altered (slow motion, small
    /// speakers), if it is.
    pub tag: Option<&'a str>,
}

/// Draw `clip` into `rect`: the lead in the upper part, the lane under it, the playhead in both.
pub fn clip_view(ui: &mut Ui, rect: Rect, clip: &ClipTrace<'_>, style: &StripStyle) {
    let n = clip.ecg.pushed().max(1);
    let factor = (clip.pcg.sample_rate_hz / clip.ecg.sample_rate_hz.max(1.0))
        .round()
        .max(1.0) as u64;
    // fit the whole clip to the width; keep the screen's millivolt scale
    let px_per_sample = rect.width().max(1.0) / n as f32 * 0.999;
    let px_per_mm = px_per_sample * clip.ecg.sample_rate_hz / style.mm_per_s.max(1.0);
    let px_per_mv = style.mm_per_mv * style.px_per_mm;
    let fitted = StripStyle {
        mode: SweepMode::Scroll,
        px_per_mm,
        mm_per_mv: px_per_mv / px_per_mm.max(1e-3),
        show_grid: false,
        ..*style
    };
    let h = lane_height(rect.height()).max(rect.height() * 0.4);
    let (ecg_rect, lane_rect) = rect.split_top_bottom_at_y(rect.bottom() - h);
    let id = ui.scope_id().with("clip_view");
    let win = draw_strip(
        ui,
        id,
        ecg_rect,
        clip.ecg,
        &fitted,
        &StripTrace {
            label: clip.lead_label,
            invert: false,
            head: clip.ecg.pushed(),
            marks: None,
            show_waves: false,
            show_intervals: false,
        },
    );
    let lane_win = win.rescaled(lane_rect, factor);
    let lane = PcgLane {
        ring: clip.pcg,
        marks: clip.marks,
    };
    draw_pcg_lane(ui, id.with("lane"), &lane_win, &lane, &fitted, clip.title);
    if let Some(p) = clip.playhead {
        draw_playhead(ui, &win, p, factor, &fitted);
        draw_playhead(ui, &lane_win, p, 1, &fitted);
    }
    if let Some(tag) = clip.tag {
        draw_altered_tag(ui, lane_rect, tag);
    }
    ui.allocate_rect(rect, egui::Sense::hover());
}

/// The "altered sound" tag the lane shows while a heart-sound sound is changed for listening (small
/// speakers, slow motion): an amber label on its soft amber fill at the top of `lane_rect`, left of
/// the lane's Play button, so it reads on paper and on the monitor colours alike.
pub fn draw_altered_tag(ui: &Ui, lane_rect: Rect, text: &str) {
    let warn = crate::Palette::of(ui).warn();
    let painter = ui.painter_at(lane_rect);
    let galley = painter.layout_no_wrap(text.to_owned(), FontId::proportional(11.0), warn.on_soft);
    let size = galley.size() + Vec2::new(10.0, 4.0);
    let rect = Rect::from_min_size(
        egui::pos2(lane_rect.right() - 76.0 - size.x, lane_rect.top() + 2.0),
        size,
    );
    painter.rect_filled(rect, 3.0, warn.soft);
    painter.galley(rect.min + Vec2::new(5.0, 2.0), galley, warn.on_soft);
}
