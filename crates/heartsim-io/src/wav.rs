//! The heart-sound signal as a WAV file (brief B28): 16-bit mono PCM at [`WAV_RATE_HZ`], converted
//! from the 4 kHz phonocardiogram with the same band-limited resampler the sound output uses
//! ([`heartsim_ecg::resample::resample_block`]), with a `LIST`/`INFO` chunk naming the scenario, the
//! listening site and that the sound is synthesised by heartsim.
//!
//! Pure: bytes in memory, no clock (the date is an input), the same file for the same input.

use crate::ExportFile;
use heartsim_ecg::resample::resample_block;

/// Sampling rate of the WAV file, Hz: the CD rate every player and editor reads.
pub const WAV_RATE_HZ: u32 = 44_100;
/// Scale from PCG units (a normal S1 peaks at 1.0) to full scale. Designed: the headroom of the
/// sound output at full volume (`HEADROOM_GAIN` of the app), so the loudest beats the model makes
/// stay below full scale and the file sounds as the app does.
pub const WAV_GAIN: f32 = 0.5;

/// What the file's `INFO` chunk says. Plain text; keep it ASCII for old readers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WavInfo {
    /// `INAM`: the scenario's title.
    pub title: String,
    /// `ICMT`: scenario id, listening site and chest piece, and "synthesised by heartsim".
    pub comment: String,
    /// `ICRD`: `YYYY-MM-DD`.
    pub date: String,
}

/// The `ICMT` text of a heart-sound export: what was recorded, where it was heard and that it is a
/// model.
#[must_use]
pub fn wav_comment(scenario_id: &str, site: &str, note: Option<&str>) -> String {
    let mut s = format!(
        "Heart sounds synthesised by heartsim (a model, not a recording). Scenario: {scenario_id}. Site: {site}."
    );
    if let Some(n) = note {
        s.push(' ');
        s.push_str(n);
    }
    s
}

/// The PCG `samples` at `rate_hz` as 16-bit samples at [`WAV_RATE_HZ`] (resampled, times
/// [`WAV_GAIN`], clamped to full scale): exactly what [`wav_bytes`] writes.
#[must_use]
pub fn wav_samples(samples: &[f32], rate_hz: f32) -> Vec<i16> {
    resample_block(samples, f64::from(rate_hz), f64::from(WAV_RATE_HZ))
        .into_iter()
        .map(|v| to_i16(v * WAV_GAIN))
        .collect()
}

fn to_i16(v: f32) -> i16 {
    (v.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16
}

fn info_entry(out: &mut Vec<u8>, id: [u8; 4], text: &str) {
    if text.is_empty() {
        return;
    }
    let mut bytes: Vec<u8> = text
        .bytes()
        .map(|b| if b.is_ascii() { b } else { b'?' })
        .collect();
    bytes.push(0);
    out.extend_from_slice(&id);
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&bytes);
    if bytes.len() % 2 == 1 {
        out.push(0);
    }
}

/// A complete WAV file: `RIFF` / `WAVE`, `fmt ` (PCM, mono, 16 bit, [`WAV_RATE_HZ`]), `LIST`/`INFO`
/// (`INAM`, `ICMT`, `ISFT`, `ICRD`) and `data`.
#[must_use]
pub fn wav_bytes(samples: &[f32], rate_hz: f32, info: &WavInfo) -> Vec<u8> {
    let pcm = wav_samples(samples, rate_hz);
    let mut list = b"INFO".to_vec();
    info_entry(&mut list, *b"INAM", &info.title);
    info_entry(&mut list, *b"ICMT", &info.comment);
    info_entry(&mut list, *b"ISFT", &crate::generator());
    info_entry(&mut list, *b"ICRD", &info.date);
    let data_len = (pcm.len() * 2) as u32;
    let mut out = Vec::with_capacity(64 + list.len() + pcm.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&WAV_RATE_HZ.to_le_bytes());
    out.extend_from_slice(&(WAV_RATE_HZ * 2).to_le_bytes()); // bytes per second
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"LIST");
    out.extend_from_slice(&(list.len() as u32).to_le_bytes());
    out.extend_from_slice(&list);
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in &pcm {
        out.extend_from_slice(&s.to_le_bytes());
    }
    let riff = (out.len() - 8) as u32;
    out[4..8].copy_from_slice(&riff.to_le_bytes());
    out
}

/// The heart-sound export of `samples` as an [`ExportFile`] named `<stem>_heart_sounds.wav`.
#[must_use]
pub fn heart_sound_file(stem: &str, samples: &[f32], rate_hz: f32, info: &WavInfo) -> ExportFile {
    ExportFile {
        name: format!("{stem}_heart_sounds.wav"),
        bytes: wav_bytes(samples, rate_hz, info),
    }
}

/// A WAV file read back (the tests; a 16-bit PCM file only).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WavFile {
    pub rate_hz: u32,
    pub channels: u16,
    pub bits: u16,
    pub samples: Vec<i16>,
    /// The `INFO` entries by their four-letter id, without the terminating zero.
    pub info: Vec<(String, String)>,
}

impl WavFile {
    /// The `INFO` entry `id` (`"INAM"`, `"ICMT"`, …).
    #[must_use]
    pub fn info(&self, id: &str) -> Option<&str> {
        self.info.iter().find(|(k, _)| k == id).map(|(_, v)| v.as_str())
    }
}

/// Parse a 16-bit PCM WAV file.
///
/// # Errors
/// Not a RIFF/WAVE file, a truncated chunk, or not 16-bit PCM.
pub fn read_wav(bytes: &[u8]) -> Result<WavFile, crate::ImportError> {
    let bad = |m: &str| crate::ImportError::Format(format!("WAV: {m}"));
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(bad("not a RIFF/WAVE file"));
    }
    let u32_at = |i: usize| -> Option<u32> {
        bytes
            .get(i..i + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let u16_at = |i: usize| -> Option<u16> { bytes.get(i..i + 2).map(|b| u16::from_le_bytes([b[0], b[1]])) };
    let mut out = WavFile::default();
    let mut format = None;
    let mut i = 12;
    while i + 8 <= bytes.len() {
        let id = &bytes[i..i + 4];
        let len = u32_at(i + 4).ok_or_else(|| bad("chunk header"))? as usize;
        let body = bytes
            .get(i + 8..i + 8 + len)
            .ok_or_else(|| bad("truncated chunk"))?;
        match id {
            b"fmt " => {
                let tag = u16_at(i + 8).ok_or_else(|| bad("fmt"))?;
                out.channels = u16_at(i + 10).ok_or_else(|| bad("fmt"))?;
                out.rate_hz = u32_at(i + 12).ok_or_else(|| bad("fmt"))?;
                out.bits = u16_at(i + 22).ok_or_else(|| bad("fmt"))?;
                format = Some(tag);
            }
            b"data" => {
                out.samples = body
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| i16::from_le_bytes(*c))
                    .collect();
            }
            b"LIST" if body.starts_with(b"INFO") => {
                let mut j = 4;
                while j + 8 <= body.len() {
                    let key = String::from_utf8_lossy(&body[j..j + 4]).into_owned();
                    let n = u32::from_le_bytes([body[j + 4], body[j + 5], body[j + 6], body[j + 7]]) as usize;
                    let text = body.get(j + 8..j + 8 + n).ok_or_else(|| bad("truncated INFO"))?;
                    let text = String::from_utf8_lossy(text).trim_end_matches('\0').to_owned();
                    out.info.push((key, text));
                    j += 8 + n + (n % 2);
                }
            }
            _ => {}
        }
        i += 8 + len + (len % 2);
    }
    if format != Some(1) || out.bits != 16 {
        return Err(bad("not 16-bit PCM"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_entries_are_padded_to_even_length() {
        let mut v = Vec::new();
        info_entry(&mut v, *b"INAM", "ab");
        assert_eq!(v.len(), 8 + 4);
        info_entry(&mut v, *b"ICMT", "abc");
        assert_eq!(v.len(), 12 + 8 + 4);
    }
}
