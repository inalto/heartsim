//! 12-lead PDF printout: the clinical 3 × 4 page plus a lead II rhythm strip on 1 mm / 5 mm ECG
//! paper, a 1 mV calibration pulse per row and a patient-free header (scenario, paper speed, gain,
//! date). Landscape A4 or US Letter; the grid is 250 mm × 160 mm on both, so at 25 mm/s the page
//! holds 10 s (2.5 s per column) like a standard electrocardiograph printout.
//!
//! The file is a pure function of its inputs: fixed metadata dates, builtin Helvetica (nothing
//! embedded), and a document ID derived from the content, so the app and the CLI write the same
//! bytes.

use crate::ExportInfo;
use crate::record::EcgRecord;
use heartsim_core::Lead;
use heartsim_ui::LeadOrder;
use printpdf::{
    BuiltinFont, Color, Line, LinePoint, Mm, Op, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Point,
    Pt, Rgb, TextItem,
};

/// Paper sizes, landscape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Paper {
    #[default]
    A4,
    Letter,
}

impl Paper {
    /// Width × height in landscape, mm.
    #[must_use]
    pub fn size_mm(self) -> (f32, f32) {
        match self {
            Paper::A4 => (297.0, 210.0),
            Paper::Letter => (279.4, 215.9),
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Paper::A4 => "A4",
            Paper::Letter => "Letter",
        }
    }
}

/// Width of the ECG grid, mm (10 s at 25 mm/s).
pub const GRID_WIDTH_MM: f32 = 250.0;
/// Height of one row (three lead rows and the rhythm strip), mm (±2 mV at 10 mm/mV).
pub const ROW_HEIGHT_MM: f32 = 40.0;
/// Rows on the page: three of four leads each, then the rhythm strip.
pub const ROWS: usize = 4;
/// Space left of the grid for the calibration pulses, mm.
const PULSE_BAND_MM: f32 = 8.0;
/// Lead of the rhythm strip.
pub const RHYTHM_LEAD: Lead = Lead::II;

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(Rgb {
        r: f32::from(r) / 255.0,
        g: f32::from(g) / 255.0,
        b: f32::from(b) / 255.0,
        icc_profile: None,
    })
}

fn polyline(points: impl IntoIterator<Item = (f32, f32)>) -> Op {
    Op::DrawLine {
        line: Line {
            points: points
                .into_iter()
                .map(|(x, y)| LinePoint {
                    p: Point::new(Mm(x), Mm(y)),
                    bezier: false,
                })
                .collect(),
            is_closed: false,
        },
    }
}

/// The builtin fonts only cover WinAnsi: keep printable ASCII, spell out the common symbols.
fn ascii(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '—' | '–' | '−' => '-',
            '×' => 'x',
            '°' => 'o',
            c if c.is_ascii() && !c.is_ascii_control() => c,
            _ => '?',
        })
        .collect()
}

fn text(ops: &mut Vec<Op>, x_mm: f32, y_mm: f32, size_pt: f32, bold: bool, s: &str) {
    let font = if bold {
        BuiltinFont::HelveticaBold
    } else {
        BuiltinFont::Helvetica
    };
    ops.extend([
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(x_mm), Mm(y_mm)),
        },
        Op::SetFont {
            font: PdfFontHandle::Builtin(font),
            size: Pt(size_pt),
        },
        Op::ShowText {
            items: vec![TextItem::Text(ascii(s))],
        },
        Op::EndTextSection,
    ]);
}

/// Seconds of signal across the grid at `mm_per_s`.
#[must_use]
pub fn page_seconds(mm_per_s: f32) -> f64 {
    f64::from(GRID_WIDTH_MM / mm_per_s.max(1.0))
}

/// The page's lead cells: `(lead, inverted, column, row)`; columns are consecutive quarters of the
/// page time, rows 0–2 from the top. The rhythm strip (row 3) is not listed.
#[must_use]
pub fn page_leads(order: LeadOrder) -> Vec<(Lead, bool, usize, usize)> {
    order
        .leads()
        .iter()
        .enumerate()
        .map(|(i, (l, inv))| (*l, *inv, i / 3, i % 3))
        .collect()
}

/// The 12-lead PDF of the last [`page_seconds`] of `rec` (all of it when shorter, left-aligned).
#[must_use]
pub fn write_pdf(rec: &EcgRecord, info: &ExportInfo, order: LeadOrder) -> Vec<u8> {
    let (page_w, page_h) = info.paper.size_mm();
    let grid_h = ROWS as f32 * ROW_HEIGHT_MM;
    let x0 = (page_w - GRID_WIDTH_MM - PULSE_BAND_MM) / 2.0 + PULSE_BAND_MM;
    let y_top = page_h - 26.0;
    let y_bottom = y_top - grid_h;
    let x1 = x0 + GRID_WIDTH_MM;
    let shown = rec.tail(page_seconds(info.mm_per_s));
    let mm_per_sample = info.mm_per_s / shown.sample_rate_hz as f32;
    let mm_per_mv = info.mm_per_mv;

    let mut ops = Vec::new();
    // grid: 1 mm lines, then 5 mm lines on top
    for (major, colour, width_mm) in [
        (false, rgb(250, 205, 205), 0.08_f32),
        (true, rgb(235, 140, 140), 0.18),
    ] {
        ops.push(Op::SetOutlineColor { col: colour });
        ops.push(Op::SetOutlineThickness {
            pt: Mm(width_mm).into(),
        });
        for i in 0..=(GRID_WIDTH_MM as usize) {
            if (i % 5 == 0) == major {
                let x = x0 + i as f32;
                ops.push(polyline([(x, y_bottom), (x, y_top)]));
            }
        }
        for j in 0..=(grid_h as usize) {
            if (j % 5 == 0) == major {
                let y = y_bottom + j as f32;
                ops.push(polyline([(x0, y), (x1, y)]));
            }
        }
    }

    let row_centre = |row: usize| y_top - (row as f32 + 0.5) * ROW_HEIGHT_MM;
    // Traces may swing past their row, but not off the paper.
    let clamp_y = |y: f32| y.clamp(y_bottom - 5.0, y_top + 5.0);
    ops.push(Op::SetOutlineColor { col: rgb(0, 0, 0) });
    ops.push(Op::SetOutlineThickness { pt: Mm(0.25).into() });
    let column_samples = (page_seconds(info.mm_per_s) / 4.0 * shown.sample_rate_hz).round() as usize;
    for (lead, invert, col, row) in page_leads(order) {
        let samples = shown.lead(lead);
        let from = (col * column_samples).min(samples.len());
        let to = ((col + 1) * column_samples).min(samples.len());
        if to <= from {
            continue;
        }
        let sign = if invert { -1.0 } else { 1.0 };
        let yc = row_centre(row);
        ops.push(polyline((from..to).map(|i| {
            (
                x0 + i as f32 * mm_per_sample,
                clamp_y(yc + sign * samples[i] * mm_per_mv),
            )
        })));
    }
    let rhythm = shown.lead(RHYTHM_LEAD);
    let yc = row_centre(ROWS - 1);
    if rhythm.len() > 1 {
        ops.push(polyline(rhythm.iter().enumerate().map(|(i, v)| {
            (x0 + i as f32 * mm_per_sample, clamp_y(yc + v * mm_per_mv))
        })));
    }
    // 1 mV × 200 ms calibration pulse left of each row, and column separators
    let pulse_w = 0.2 * info.mm_per_s;
    for row in 0..ROWS {
        let yc = row_centre(row);
        let px = x0 - PULSE_BAND_MM + 1.0;
        let pw = pulse_w.min(PULSE_BAND_MM - 2.0);
        ops.push(polyline([
            (px - 1.0, yc),
            (px, yc),
            (px, yc + mm_per_mv),
            (px + pw, yc + mm_per_mv),
            (px + pw, yc),
            (x0, yc),
        ]));
        if row < ROWS - 1 {
            for c in 1..4 {
                let x = x0 + (c * column_samples) as f32 * mm_per_sample;
                ops.push(polyline([(x, yc - 3.0), (x, yc + 3.0)]));
            }
        }
    }
    ops.push(Op::SetFillColor { col: rgb(0, 0, 0) });
    for (lead, invert, col, row) in page_leads(order) {
        let label = if invert {
            format!("-{}", lead.label())
        } else {
            lead.label().to_string()
        };
        let x = x0 + (col * column_samples) as f32 * mm_per_sample + 1.5;
        text(
            &mut ops,
            x,
            y_top - row as f32 * ROW_HEIGHT_MM - 5.0,
            9.0,
            true,
            &label,
        );
    }
    text(
        &mut ops,
        x0 + 1.5,
        y_top - (ROWS - 1) as f32 * ROW_HEIGHT_MM - 5.0,
        9.0,
        true,
        RHYTHM_LEAD.label(),
    );

    // header and footer: scenario and recording settings only, never patient fields
    let left = x0 - PULSE_BAND_MM;
    text(
        &mut ops,
        left,
        page_h - 12.0,
        13.0,
        true,
        &format!("{} - simulated 12-lead ECG", info.scenario_title),
    );
    text(
        &mut ops,
        left,
        page_h - 19.0,
        9.0,
        false,
        &format!(
            "Scenario: {}    Speed: {} mm/s    Gain: {} mm/mV    Sampling: {:.0} Hz    Date: {}",
            info.scenario_id, info.mm_per_s, info.mm_per_mv, shown.sample_rate_hz, info.date
        ),
    );
    text(
        &mut ops,
        left,
        y_bottom - 7.0,
        8.0,
        false,
        &format!(
            "Simulation time {:.2}-{:.2} s ({:.1} s shown). {}. {}.",
            shown.start_ms / 1000.0,
            shown.start_ms / 1000.0 + shown.duration_s(),
            shown.duration_s(),
            crate::DISCLAIMER,
            crate::generator()
        ),
    );

    if let Some(note) = &info.trace_note {
        text(
            &mut ops,
            left,
            y_bottom - 11.5,
            8.0,
            true,
            &format!("Trace not as simulated. {note}"),
        );
    }

    let mut doc = PdfDocument::new(&ascii(&format!(
        "{} - simulated 12-lead ECG",
        info.scenario_title
    )));
    doc.metadata.info.creator = crate::generator();
    doc.metadata.info.producer = crate::generator();
    doc.metadata.info.subject = ascii(&format!("Simulated ECG, scenario {}", info.scenario_id));
    if let Some(ts) = crate::unix_time_of_date(&info.date)
        && let Ok(t) = printpdf::OffsetDateTime::from_unix_timestamp(ts)
    {
        doc.metadata.info.creation_date = t;
        doc.metadata.info.modification_date = t;
        doc.metadata.info.metadata_date = t;
    }
    doc.with_pages(vec![PdfPage::new(Mm(page_w), Mm(page_h), ops)]);
    let mut warnings = Vec::new();
    let mut bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
    stable_document_id(&mut bytes);
    bytes
}

/// FNV-1a, 64 bit.
fn fnv1a(bytes: &[u8], seed: u64) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325 ^ seed, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// printpdf fills the trailer `/ID` with two strings from a process-global counter; replace their
/// contents (same length, so the cross-reference offsets stay valid) with a hash of the rest of
/// the file.
fn stable_document_id(bytes: &mut [u8]) {
    let Some(id_at) = bytes.windows(3).rposition(|w| w == b"/ID") else {
        return;
    };
    let mut spans = Vec::new();
    let mut i = id_at;
    while spans.len() < 2 && i < bytes.len() {
        if bytes[i] == b'(' {
            let start = i + 1;
            let Some(len) = bytes[start..].iter().position(|b| *b == b')') else {
                return;
            };
            spans.push(start..start + len);
            i = start + len;
        }
        i += 1;
    }
    let digest = {
        let mut masked = bytes.to_vec();
        for s in &spans {
            masked[s.clone()].fill(b'0');
        }
        [
            fnv1a(&masked, 1),
            fnv1a(&masked, 2),
            fnv1a(&masked, 3),
            fnv1a(&masked, 4),
        ]
    };
    let hex: Vec<u8> = digest
        .iter()
        .flat_map(|d| format!("{d:016x}").into_bytes())
        .collect();
    for s in spans {
        for (k, b) in bytes[s].iter_mut().enumerate() {
            *b = hex[k % hex.len()];
        }
    }
}
