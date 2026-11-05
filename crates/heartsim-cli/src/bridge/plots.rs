//! Report figures: plotters drawing into an in-memory bitmap, encoded with `image`. The font is embedded
//! (egui's default Ubuntu Light), so the PNG bytes do not depend on the fonts of the machine.

use anyhow::{Context, Result, anyhow};
use plotters::prelude::*;
use plotters::style::{FontStyle, register_font};
use std::path::Path;
use std::sync::Once;

const REFERENCE_COLOUR: RGBColor = RGBColor(0x1f, 0x4e, 0x9e);
const TEST_COLOUR: RGBColor = RGBColor(0xc2, 0x4a, 0x1a);
const GRID_COLOUR: RGBColor = RGBColor(0xe4, 0xe4, 0xe4);

fn fonts() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // the embedded font is known good, so this cannot fail in practice
        let _ = register_font(
            "sans-serif",
            FontStyle::Normal,
            epaint_default_fonts::UBUNTU_LIGHT,
        );
    });
}

fn draw<F>(path: &Path, (w, h): (u32, u32), paint: F) -> Result<()>
where
    F: FnOnce(DrawingArea<BitMapBackend<'_>, plotters::coord::Shift>) -> Result<()>,
{
    fonts();
    let mut buffer = vec![255u8; w as usize * h as usize * 3];
    {
        let root = BitMapBackend::with_buffer(&mut buffer, (w, h)).into_drawing_area();
        paint(root)?;
    }
    let image = image::RgbImage::from_raw(w, h, buffer).ok_or_else(|| anyhow!("bitmap size mismatch"))?;
    image
        .save(path)
        .with_context(|| format!("writing {}", path.display()))
}

fn range(values: impl Iterator<Item = f64>, pad: f64) -> (f64, f64) {
    let (lo, hi) = values
        .filter(|v| v.is_finite())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), v| {
            (l.min(v), h.max(v))
        });
    if !lo.is_finite() {
        return (0.0, 1.0);
    }
    let margin = ((hi - lo) * pad).max(1e-3);
    (lo - margin, hi + margin)
}

fn err(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow!("plot: {e}")
}

/// Activation-time scatter, test against reference, with the identity line.
pub fn activation_scatter(
    path: &Path,
    title: &str,
    x_label: &str,
    y_label: &str,
    points: &[(f64, f64)],
) -> Result<()> {
    draw(path, (900, 900), |root| {
        root.fill(&WHITE).map_err(err)?;
        let (lo, hi) = range(points.iter().flat_map(|(a, b)| [*a, *b]), 0.05);
        let mut chart = ChartBuilder::on(&root)
            .caption(title, ("sans-serif", 22))
            .margin(16)
            .x_label_area_size(46)
            .y_label_area_size(58)
            .build_cartesian_2d(lo..hi, lo..hi)
            .map_err(err)?;
        chart
            .configure_mesh()
            .light_line_style(GRID_COLOUR)
            .x_desc(x_label)
            .y_desc(y_label)
            .draw()
            .map_err(err)?;
        chart
            .draw_series(LineSeries::new(
                [(lo, lo), (hi, hi)],
                REFERENCE_COLOUR.stroke_width(2),
            ))
            .map_err(err)?;
        chart
            .draw_series(
                points
                    .iter()
                    .filter(|(a, b)| a.is_finite() && b.is_finite())
                    .map(|(a, b)| Circle::new((*a, *b), 3, TEST_COLOUR.filled())),
            )
            .map_err(err)?;
        root.present().map_err(err)
    })
}

/// One lead of [`ecg_overlay`].
#[derive(Debug, Clone)]
pub struct LeadTrace<'a> {
    pub label: &'a str,
    pub reference_mv: &'a [f64],
    pub test_mv: &'a [f64],
}

/// Per-lead overlay, 3 columns × 4 rows, on a shared time axis.
pub fn ecg_overlay(path: &Path, title: &str, times_ms: &[f64], leads: &[LeadTrace<'_>]) -> Result<()> {
    draw(path, (1500, 1100), |root| {
        root.fill(&WHITE).map_err(err)?;
        let root = root.titled(title, ("sans-serif", 22)).map_err(err)?;
        let t = range(times_ms.iter().copied(), 0.0);
        for (area, lead) in root.split_evenly((4, 3)).iter().zip(leads) {
            let v = range(lead.reference_mv.iter().chain(lead.test_mv).copied(), 0.08);
            let mut chart = ChartBuilder::on(area)
                .caption(lead.label, ("sans-serif", 16))
                .margin(6)
                .x_label_area_size(26)
                .y_label_area_size(46)
                .build_cartesian_2d(t.0..t.1, v.0..v.1)
                .map_err(err)?;
            chart
                .configure_mesh()
                .light_line_style(GRID_COLOUR)
                .x_labels(5)
                .y_labels(4)
                .x_desc("ms")
                .y_desc("mV")
                .draw()
                .map_err(err)?;
            for (trace, colour) in [(lead.reference_mv, REFERENCE_COLOUR), (lead.test_mv, TEST_COLOUR)] {
                chart
                    .draw_series(LineSeries::new(
                        times_ms.iter().copied().zip(trace.iter().copied()),
                        colour.stroke_width(1),
                    ))
                    .map_err(err)?;
            }
        }
        root.present().map_err(err)
    })
}
