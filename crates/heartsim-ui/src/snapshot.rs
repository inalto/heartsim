//! PNG snapshot of a 12-lead page, rasterised on the CPU with the same [`StripWindow`] mapping
//! and cell layout as the screen, so the image matches what is shown. The raster path carries no
//! font, only the bitmap glyphs lead labels need. No GPU read-back, so it also works headless and
//! in tests.

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

/// 5 × 7 bitmap of a character used in lead labels (each row's low 5 bits, top row first).
fn glyph(c: char) -> Option<[u8; 7]> {
    Some(match c {
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'a' => [0b00000, 0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        _ => return None,
    })
}

/// A lead label at the top-left of `r`, blocks of about 0.4 mm so it reads like the printed page.
fn draw_label(img: &mut RgbaImage, r: Rect, style: &StripStyle, label: &str) {
    let block = (style.px_per_mm * 0.4).round().max(1.0);
    let top = r.top() + 2.0 * block;
    let mut x = r.left() + 2.0 * block;
    for bitmap in label.chars().filter_map(glyph) {
        for (row, bits) in bitmap.iter().enumerate() {
            for col in 0..5 {
                if bits & (0b10000 >> col) != 0 {
                    let min = Pos2::new(x + col as f32 * block, top + row as f32 * block);
                    fill(img, Rect::from_min_size(min, Vec2::splat(block)), style.trace);
                }
            }
        }
        x += 6.0 * block;
    }
}

/// 5 × 7 bitmap of a caption character: upper-case letters, digits and `. , - + : / = ( )`.
fn caption_glyph(c: char) -> Option<[u8; 7]> {
    Some(match c.to_ascii_uppercase() {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        '.' => [0, 0, 0, 0, 0, 0b01100, 0b01100],
        ',' => [0, 0, 0, 0, 0b01100, 0b00100, 0b01000],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        '+' => [0, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0],
        ':' => [0, 0b01100, 0b01100, 0, 0b01100, 0b01100, 0],
        '/' => [0b00001, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b10000],
        '=' => [0, 0, 0b11111, 0, 0b11111, 0, 0],
        '(' => [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
        ')' => [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
        ' ' => [0; 7],
        _ => return None,
    })
}

/// Stamp `text` (plain ASCII; other characters leave a gap) in small block letters along the
/// bottom edge of `img`, wrapped to the image width. Exports and snapshots of a filtered trace
/// use it to say so on the picture itself, where a shared PNG cannot lose it. `style` supplies the
/// scale (`px_per_mm`) and the colour.
pub fn stamp_caption(img: &mut RgbaImage, style: &StripStyle, text: &str) {
    let block = (style.px_per_mm * 0.3).round().max(1.0);
    let advance = 6.0 * block;
    let line_h = 9.0 * block;
    let (w, h) = (img.width() as f32, img.height() as f32);
    let margin = 2.0 * block;
    let per_line = (((w - 2.0 * margin) / advance).floor() as usize).max(1);
    let chars: Vec<char> = text.chars().collect();
    let lines: Vec<&[char]> = chars.chunks(per_line).collect();
    let top = h - margin - lines.len() as f32 * line_h;
    for (row, line) in lines.iter().enumerate() {
        let y = top + row as f32 * line_h;
        for (col, &c) in line.iter().enumerate() {
            let Some(bitmap) = caption_glyph(c) else { continue };
            let x = margin + col as f32 * advance;
            for (r, bits) in bitmap.iter().enumerate() {
                for k in 0..5 {
                    if bits & (0b10000 >> k) != 0 {
                        let min = Pos2::new(x + k as f32 * block, y + r as f32 * block);
                        fill(img, Rect::from_min_size(min, Vec2::splat(block)), style.trace);
                    }
                }
            }
        }
    }
}

fn draw_cell(
    img: &mut RgbaImage,
    win: &StripWindow,
    ring: &LeadRing,
    style: &StripStyle,
    invert: bool,
    label: &str,
) {
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
    draw_label(img, r, style, label);
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
        let label = if cell.invert {
            format!("-{}", cell.lead.label())
        } else {
            cell.lead.label().to_string()
        };
        draw_cell(&mut img, &win, ring, &style, cell.invert, &label);
    }
    img
}

/// Rasterise one strip labelled `label` of `width_px × height_px` pixels whose newest sample is
/// just before absolute index `head` (scroll or sweep, as `style.mode` says), calibration pulse at
/// the left edge.
#[must_use]
pub fn render_strip(
    ring: &LeadRing,
    style: &StripStyle,
    label: &str,
    head: u64,
    size_px: [u32; 2],
    px_per_point: f32,
) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(size_px[0], size_px[1], px(style.paper));
    let style = StripStyle {
        px_per_mm: style.px_per_mm * px_per_point,
        ..*style
    };
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(size_px[0] as f32, size_px[1] as f32));
    let win = StripWindow::new(rect, &style, ring.sample_rate_hz, head);
    draw_cell(&mut img, &win, ring, &style, false, label);
    img
}

/// Encode an image as PNG bytes.
pub fn encode_png(img: &RgbaImage) -> Result<Vec<u8>, image::ImageError> {
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png)?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use heartsim_core::Lead;

    fn dark_pixels(img: &RgbaImage, x: std::ops::Range<u32>, y: std::ops::Range<u32>) -> usize {
        x.flat_map(|x| y.clone().map(move |y| (x, y)))
            .filter(|&(x, y)| img.get_pixel(x, y).0[0] < 100)
            .count()
    }

    #[test]
    fn captions_cover_the_characters_of_a_filter_statement() {
        let text = "Display filter: monitor 0.5-40 Hz. Artefacts added: baseline wander, respiration, 50 Hz mains, muscle tremor (intensity 1.0, seed 12345678901234567890).";
        for c in text.chars() {
            assert!(caption_glyph(c).is_some(), "no glyph for {c:?}");
        }
    }

    #[test]
    fn a_caption_leaves_ink_along_the_bottom_edge_only() {
        let style = StripStyle {
            px_per_mm: 5.0,
            ..StripStyle::paper()
        };
        let mut img = RgbaImage::from_pixel(600, 200, px(style.paper));
        stamp_caption(&mut img, &style, "Display filter: monitor 0.5-40 Hz.");
        let bottom = dark_pixels(&img, 0..600, 150..200);
        let top = dark_pixels(&img, 0..600, 0..100);
        assert!(bottom > 50, "caption has no ink: {bottom}");
        assert_eq!(top, 0);
    }

    #[test]
    fn a_long_caption_wraps_instead_of_running_off_the_image() {
        let style = StripStyle {
            px_per_mm: 5.0,
            ..StripStyle::paper()
        };
        let long = "MAINS ".repeat(60);
        let mut img = RgbaImage::from_pixel(400, 300, px(style.paper));
        stamp_caption(&mut img, &style, &long);
        // wrapped onto several lines, so ink reaches well above the last line's height
        assert!(dark_pixels(&img, 0..400, 200..260) > 0);
    }

    #[test]
    fn every_lead_label_can_be_drawn() {
        for l in Lead::ALL {
            assert!(l.label().chars().all(|c| glyph(c).is_some()), "{}", l.label());
        }
        assert!(glyph('-').is_some(), "Cabrera's inverted aVR");
    }

    #[test]
    fn page_cells_carry_their_label_above_the_trace() {
        let rings: Vec<LeadRing> = (0..12)
            .map(|_| {
                let mut r = LeadRing::new(5000, 500.0);
                (0..5000).for_each(|_| r.push(0.0));
                r
            })
            .collect();
        let style = StripStyle {
            px_per_mm: 5.0,
            ..StripStyle::paper()
        };
        let img = render_page(
            &rings,
            &style,
            TwelveLeadLayout::Clinical,
            LeadOrder::Standard,
            5000,
            [1250, 800],
            1.0,
        );
        // the top-left corner of the first cell holds "I", of the rhythm strip row "II"
        let i = dark_pixels(&img, 2..40, 2..40);
        let ii = dark_pixels(&img, 2..40, 602..640);
        assert!(i > 0, "no label in the first cell");
        assert!(ii > i, "\"II\" has more ink than \"I\": {ii} vs {i}");
    }
}
