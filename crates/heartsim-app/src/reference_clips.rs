//! Real reference recordings for the Sound Trainer (brief B28, Part C): a handful of third-party
//! clips (CC0 and CC BY, each listed in `ASSETS-LICENSES.md`) kept as 16-bit mono 4 kHz WAV under
//! `assets/sounds/reference/`, with an RON index that carries the title, the trainer finding it
//! belongs to and the attribution the page must show while the clip is selected.
//!
//! A recording is not the model: it has no ECG and no labels (the sources provide none), it is
//! never time-stretched or filtered by the page, and it is shown with its source and licence.
//!
//! The decoder here reads exactly the one format the clips are converted to; it lives in this crate
//! (not in `heartsim-io`, which the web build does not compile) so the browser can play them too.

use std::rc::Rc;

use serde::Deserialize;

use crate::assets::AssetBundle;

/// Where the index sits, relative to `assets/`.
pub const INDEX_PATH: &str = "sounds/reference/index.ron";
/// The directory of the clips and the index, relative to `assets/`.
pub const DIR: &str = "sounds/reference";
/// Index format version this reader understands.
pub const FORMAT_VERSION: u32 = 1;
/// The sample rate every clip is stored at: the rate the heart-sound sink plays at
/// (`heartsim_ecg::PCG_RATE_HZ`), so a recording plays without resampling.
pub const SAMPLE_RATE_HZ: u32 = 4000;

/// One entry of the index.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ClipMeta {
    /// File name inside `sounds/reference/`.
    pub file: String,
    /// Id of the trainer finding (`FINDINGS[..].id`) the recording is offered beside.
    pub finding: String,
    /// Short title shown with the recording.
    pub title: String,
    /// What the recording contains, as the source states it.
    pub condition: String,
    /// Author(s) as credited by the source.
    pub author: String,
    /// Primary page of the file and, where there is one, the article.
    pub source: String,
    /// Licence name, e.g. `CC0 1.0` or `CC BY 2.0`.
    pub licence: String,
    /// The credit line shown in the app: authors, work, licence, and that heartsim converted it.
    pub attribution: String,
    /// ISO date the original was retrieved.
    pub retrieved: String,
    /// SHA-256 of the shipped WAV file, lower-case hex.
    pub sha256: String,
}

/// The index file.
#[derive(Debug, Clone, Deserialize)]
pub struct Index {
    pub format_version: u32,
    pub clips: Vec<ClipMeta>,
}

/// A recording with its samples decoded for the player.
#[derive(Debug, Clone)]
pub struct ReferenceClip {
    pub meta: ClipMeta,
    /// Samples in -1..1, 4 kHz mono, played looping.
    pub samples: Rc<[f32]>,
}

/// The recordings found in the assets; empty when there are none (the trainer then offers none).
#[derive(Debug, Clone, Default)]
pub struct ReferenceLibrary {
    pub clips: Vec<ReferenceClip>,
}

/// Parse the index text.
///
/// # Errors
/// Malformed RON, or a `format_version` this reader does not know.
pub fn parse_index(text: &str) -> Result<Index, String> {
    let index: Index = ron::from_str(text).map_err(|e| format!("index.ron: {e}"))?;
    if index.format_version != FORMAT_VERSION {
        return Err(format!(
            "index.ron: format_version {} (expected {FORMAT_VERSION})",
            index.format_version
        ));
    }
    Ok(index)
}

/// Decode a WAV file of exactly the stored format: RIFF/WAVE, PCM, 16-bit, one channel, 4 kHz.
/// Other chunks (`LIST`, ...) are skipped.
///
/// # Errors
/// Anything else: a message naming what differs.
pub fn decode_wav(bytes: &[u8]) -> Result<Vec<f32>, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a RIFF/WAVE file".into());
    }
    let mut pos = 12;
    let mut format_ok = false;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size =
            u32::from_le_bytes([bytes[pos + 4], bytes[pos + 5], bytes[pos + 6], bytes[pos + 7]]) as usize;
        let body = pos + 8;
        let end = body
            .checked_add(size)
            .filter(|e| *e <= bytes.len())
            .ok_or("chunk runs past the end")?;
        match id {
            b"fmt " => {
                if size < 16 {
                    return Err("short fmt chunk".into());
                }
                let rd = |o: usize| u16::from_le_bytes([bytes[body + o], bytes[body + o + 1]]);
                let rate =
                    u32::from_le_bytes([bytes[body + 4], bytes[body + 5], bytes[body + 6], bytes[body + 7]]);
                if rd(0) != 1 {
                    return Err("not PCM".into());
                }
                if rd(2) != 1 {
                    return Err("not mono".into());
                }
                if rd(14) != 16 {
                    return Err("not 16-bit".into());
                }
                if rate != SAMPLE_RATE_HZ {
                    return Err(format!("{rate} Hz, expected {SAMPLE_RATE_HZ} Hz"));
                }
                format_ok = true;
            }
            b"data" => {
                if !format_ok {
                    return Err("data chunk before fmt".into());
                }
                if !size.is_multiple_of(2) {
                    return Err("odd data length".into());
                }
                return Ok(bytes[body..end]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|p| f32::from(i16::from_le_bytes(*p)) / 32768.0)
                    .collect());
            }
            _ => {}
        }
        pos = end + (size & 1);
    }
    Err("no data chunk".into())
}

impl ReferenceLibrary {
    /// Build from file access: `read(path)` gives the bytes of a path relative to `assets/`.
    /// A clip that is missing or does not decode is skipped with a warning, so a damaged install
    /// loses a button, not the page.
    fn load_with(read: impl Fn(&str) -> Option<Vec<u8>>) -> Self {
        let Some(text) = read(INDEX_PATH).map(|b| String::from_utf8_lossy(&b).into_owned()) else {
            return Self::default();
        };
        let index = match parse_index(&text) {
            Ok(i) => i,
            Err(e) => {
                tracing::warn!("{e}");
                return Self::default();
            }
        };
        let mut clips = Vec::new();
        for meta in index.clips {
            let path = format!("{DIR}/{}", meta.file);
            let decoded = read(&path)
                .ok_or_else(|| "missing".to_owned())
                .and_then(|b| decode_wav(&b));
            match decoded {
                Ok(s) if !s.is_empty() => clips.push(ReferenceClip {
                    meta,
                    samples: s.into(),
                }),
                Ok(_) => tracing::warn!("{path}: empty"),
                Err(e) => tracing::warn!("{path}: {e}"),
            }
        }
        Self { clips }
    }

    /// From an in-memory asset tree (the web build, and tests).
    #[must_use]
    pub fn from_bundle(bundle: &AssetBundle) -> Self {
        Self::load_with(|p| bundle.get(p).map(<[u8]>::to_vec))
    }

    /// From the installed or source-tree `assets/` (the `native` feature; empty without it).
    #[must_use]
    pub fn load_default() -> Self {
        #[cfg(feature = "native")]
        {
            crate::asset_dirs::find_asset_subdir(DIR)
                .and_then(|d| {
                    d.parent()
                        .and_then(std::path::Path::parent)
                        .map(std::path::Path::to_path_buf)
                })
                .map_or_else(Self::default, |root| {
                    Self::load_with(|p| std::fs::read(root.join(p)).ok())
                })
        }
        #[cfg(not(feature = "native"))]
        {
            Self::default()
        }
    }

    /// The recordings that belong beside trainer finding `finding`, in index order.
    #[must_use]
    pub fn for_finding(&self, finding: &str) -> Vec<&ReferenceClip> {
        self.clips.iter().filter(|c| c.meta.finding == finding).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(rate: u32, channels: u16, bits: u16, samples: &[i16]) -> Vec<u8> {
        let mut v = b"RIFF".to_vec();
        v.extend(((36 + samples.len() * 2) as u32).to_le_bytes());
        v.extend(b"WAVEfmt ");
        v.extend(16u32.to_le_bytes());
        v.extend(1u16.to_le_bytes());
        v.extend(channels.to_le_bytes());
        v.extend(rate.to_le_bytes());
        v.extend((rate * 2).to_le_bytes());
        v.extend(2u16.to_le_bytes());
        v.extend(bits.to_le_bytes());
        v.extend(b"data");
        v.extend(((samples.len() * 2) as u32).to_le_bytes());
        for s in samples {
            v.extend(s.to_le_bytes());
        }
        v
    }

    #[test]
    fn decodes_the_stored_format() {
        let s = decode_wav(&wav(4000, 1, 16, &[0, 16384, -32768])).unwrap();
        assert_eq!(s, vec![0.0, 0.5, -1.0]);
    }

    #[test]
    fn refuses_other_formats() {
        assert!(decode_wav(&wav(8000, 1, 16, &[0])).unwrap_err().contains("8000"));
        assert!(
            decode_wav(&wav(4000, 2, 16, &[0, 0]))
                .unwrap_err()
                .contains("mono")
        );
        assert!(decode_wav(&wav(4000, 1, 8, &[0])).unwrap_err().contains("16-bit"));
        assert!(decode_wav(b"RIFFxxxxWAVE").is_err());
        assert!(decode_wav(b"nope").is_err());
        let mut cut = wav(4000, 1, 16, &[1, 2, 3]);
        cut.truncate(cut.len() - 2);
        assert!(decode_wav(&cut).is_err());
    }

    #[test]
    fn skips_unknown_chunks() {
        let mut w = wav(4000, 1, 16, &[5]);
        let data = w.split_off(36);
        w.extend(b"LIST");
        w.extend(3u32.to_le_bytes());
        w.extend([1, 2, 3, 0]); // padded to even
        w.extend(data);
        assert_eq!(decode_wav(&w).unwrap().len(), 1);
    }

    #[test]
    fn a_wrong_index_version_gives_no_clips() {
        assert!(parse_index("(format_version: 9, clips: [])").is_err());
        let b = AssetBundle::new();
        assert!(ReferenceLibrary::from_bundle(&b).clips.is_empty());
    }
}
