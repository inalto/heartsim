//! PNG snapshot of a 12-lead page, rasterised on the CPU with the same [`StripWindow`] mapping
//! and cell layout as the screen, so the image matches what is shown (minus text labels: the
//! raster path carries no font). No GPU read-back, so it also works headless and in tests.

use crate::strip::{StripStyle, StripWindow};
use crate::twelve_lead::{LeadOrder, TwelveLeadLayout, page_cells};
use egui::{Color32, Pos2, Rect, Vec2};
use heartsim_ecg::LeadRing;
use image::{ImageFormat, Rgba, RgbaImage};

fn px(c: Color32) -> Rgba<u8> {
    Rgba([c.r(), c.g(), c.b(), 255])
}

fn fill(img: &mut RgbaImage, r: Rect, c: Color32) {
    let (w, h) = img.dimensions();
    let x0 = r.left().max(0.0) as u32;
    let y0 = r.top().max(0.0) as u32;
    let x1 = (r.right().ceil().max(0.0) as u32).min(w);
    let y1 = (r.bottom().ceil().max(0.0) as u32).min(h);
    for y in y0..y1 {
        for x in x0..x1 {
            img.put_pixel(x, y, px(c));
        }
    }
}

/// 1-pixel DDA line clipped to `clip`.
fn line(img: &mut RgbaImage, a: Pos2, b: Pos2, clip: Rect, c: Color32) {
    let steps = (b.x - a.x).abs().max((b.y - a.y).abs()).ceil().max(1.0) as u32;
    let (w, h) = img.dimensions();
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let p = a + (b - a) * t;
        if clip.contains(p) && p.x >= 0.0 && p.y >= 0.0 && (p.x as u32) < w && (p.y as u32) < h {
            img.put_pixel(p.x as u32, p.y as u32, px(c));
        }
    }
}

fn draw_cell(img: &mut RgbaImage, win: &StripWindow, ring: &LeadRing, style: &StripStyle, invert: bool) {
    let r = win.rect;
    fill(img, r, style.paper);
    if style.show_grid {
        let mm = style.px_per_mm;
        let mut i = 0;
        let mut x = r.left();
        while x <= r.right() {
            let c = if i % 5 == 0 {
                style.grid_major
            } else {
                style.grid_minor
            };
            line(img, Pos2::new(x, r.top()), Pos2::new(x, r.bottom()), r, c);
            x += mm;
            i += 1;
        }
        let mut j = 0;
        let mut dy = 0.0;
        while dy <= r.height() * 0.5 {
            let c = if j % 5 == 0 {
                style.grid_major
            } else {
                style.grid_minor
            };
            for y in [r.center().y - dy, r.center().y + dy] {
                line(img, Pos2::new(r.left(), y), Pos2::new(r.right(), y), r, c);
            }
            dy += mm;
            j += 1;
        }
    }
    let sign = if invert { -1.0 } else { 1.0 };
    let mut buf = Vec::new();
    for (s, e, x0) in win.segments() {
        let first = ring.range(s, e, &mut buf);
        let pts: Vec<Pos2> = buf
            .iter()
            .enumerate()
            .map(|(k, v)| {
                let x = x0 + ((first - s) as f32 + k as f32 + 0.5) * win.px_per_sample;
                Pos2::new(x, win.y_of_mv(sign * v))
            })
            .collect();
        for w in pts.windows(2) {
            line(img, w[0], w[1], r, style.trace);
        }
    }
    // 1 mV calibration pulse
    let (cx, cy, h) = (r.left() + 4.0, r.center().y, win.px_per_mv);
    let w2 = 2.0 * style.px_per_mm;
    let pulse = [
        Pos2::new(cx, cy),
        Pos2::new(cx, cy - h),
        Pos2::new(cx + w2, cy - h),
        Pos2::new(cx + w2, cy),
    ];
    for w in pulse.windows(2) {
        line(img, w[0], w[1], r, style.trace);
    }
}

/// Rasterise a 12-lead page of `width_px × height_px` pixels whose newest sample is just before
/// absolute index `head`. `px_per_point` scales the on-screen calibration (pass the display's
/// pixels-per-point to get a pixel-exact copy of the screen).
#[must_use]
pub fn render_page(
    rings: &[LeadRing],
    style: &StripStyle,
    layout: TwelveLeadLayout,
    order: LeadOrder,
    head: u64,
    size_px: [u32; 2],
    px_per_point: f32,
) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(size_px[0], size_px[1], px(style.paper.gamma_multiply(0.9)));
    let style = StripStyle {
        px_per_mm: style.px_per_mm * px_per_point,
        ..*style
    };
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(size_px[0] as f32, size_px[1] as f32));
    for cell in page_cells(rect, layout, order) {
        let ring = &rings[cell.lead as usize];
        let probe = StripWindow::new(cell.rect, &style, ring.sample_rate_hz, head);
        let cell_head = head.saturating_sub(cell.columns_before_head as u64 * probe.visible_samples);
        let win = StripWindow::new(cell.rect, &style, ring.sample_rate_hz, cell_head);
        draw_cell(&mut img, &win, ring, &style, cell.invert);
    }
    img
}

/// Encode an image as PNG bytes.
pub fn encode_png(img: &RgbaImage) -> Result<Vec<u8>, image::ImageError> {
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png)?;
    Ok(out.into_inner())
}
