//! Fixtures shared by the reader tests: a small valid case, a bundled rhythm, and helpers that
//! write arbitrary archives and patch their headers (to make files the writer never would).
#![allow(dead_code)]

use heartsim_casefile::{CasePackage, Manifest};
use heartsim_core::case::{CASE_FORMAT_VERSION, Case, Stage};
use heartsim_core::tutorial::{Question, Step, UiAction};
use std::io::{Cursor, Write};
use std::path::Path;

/// A rhythm file of the shipped library, renamed to `id`.
pub fn scenario_text(id: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios/sinus_normal.ron");
    std::fs::read_to_string(path)
        .unwrap()
        .replacen("id: \"sinus_normal\"", &format!("id: \"{id}\""), 1)
}

/// A playable case that loads the bundled rhythm `my_rhythm` and the built-in `sinus_normal`.
pub fn case() -> Case {
    let q = |n: usize| Question {
        prompt: format!("Question {n}?"),
        choices: vec!["one".into(), "two".into(), "three".into()],
        answer: n % 3,
        explanation: "Because.".into(),
    };
    Case {
        format_version: CASE_FORMAT_VERSION,
        id: "case_fixture".into(),
        title: "A fixture case".into(),
        summary: "Used by the tests.".into(),
        patient: "A synthetic patient".into(),
        references: vec!["Nobody 2026".into()],
        stages: vec![
            Stage {
                title: "First".into(),
                markdown: "Some **text**.".into(),
                actions: vec![Step::Ui(UiAction::LoadScenario("my_rhythm".into()))],
                questions: (0..3).map(q).collect(),
            },
            Stage {
                title: "Second".into(),
                markdown: "More text.".into(),
                actions: vec![Step::Ui(UiAction::LoadScenario("sinus_normal".into()))],
                questions: (3..5).map(q).collect(),
            },
        ],
    }
}

pub fn manifest(case: &Case) -> Manifest {
    Manifest::for_case(
        case,
        vec!["Test Author".into()],
        "CC-BY-4.0",
        "2026-11-27",
        "heartsim test",
    )
}

/// The fixture package: the case plus the bundled `my_rhythm`.
pub fn package() -> CasePackage {
    let c = case();
    CasePackage::new(
        manifest(&c),
        c,
        vec![("my_rhythm".into(), scenario_text("my_rhythm"))],
    )
}

pub fn builtin(id: &str) -> bool {
    matches!(id, "sinus_normal" | "lbbb")
}

/// A zip of `entries` in order (deflate unless `stored`), names as given.
pub fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default());
    for (name, data) in entries {
        w.start_file(*name, opts).unwrap();
        w.write_all(data).unwrap();
    }
    w.finish().unwrap().into_inner()
}

/// The valid entries of the fixture, as `(name, bytes)`, to be edited by a test.
pub fn entries() -> Vec<(String, Vec<u8>)> {
    let p = package();
    let mut zip = zip::ZipArchive::new(Cursor::new(p.write().unwrap())).unwrap();
    (0..zip.len())
        .map(|i| {
            let mut f = zip.by_index(i).unwrap();
            let mut data = Vec::new();
            std::io::Read::read_to_end(&mut f, &mut data).unwrap();
            (f.name().unwrap().to_string(), data)
        })
        .collect()
}

pub fn archive_of(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let refs: Vec<(&str, &[u8])> = entries.iter().map(|(n, d)| (n.as_str(), d.as_slice())).collect();
    archive(&refs)
}

/// Offsets of every local file header (`PK\3\4`) and central directory header (`PK\1\2`).
pub fn headers(bytes: &[u8]) -> (Vec<usize>, Vec<usize>) {
    let find = |sig: &[u8; 4]| -> Vec<usize> {
        bytes
            .windows(4)
            .enumerate()
            .filter(|(_, w)| *w == sig)
            .map(|(i, _)| i)
            .collect()
    };
    (find(b"PK\x03\x04"), find(b"PK\x01\x02"))
}

/// Patch a little-endian `u16` at `local + l_off` of the first local header and at
/// `central + c_off` of the first central header.
pub fn patch_u16(bytes: &mut [u8], l_off: usize, c_off: usize, value: u16) {
    let (local, central) = headers(bytes);
    bytes[local[0] + l_off..local[0] + l_off + 2].copy_from_slice(&value.to_le_bytes());
    bytes[central[0] + c_off..central[0] + c_off + 2].copy_from_slice(&value.to_le_bytes());
}

/// Patch the uncompressed size of the first entry in both of its headers.
pub fn patch_uncompressed_size(bytes: &mut [u8], value: u32) {
    let (local, central) = headers(bytes);
    bytes[local[0] + 22..local[0] + 26].copy_from_slice(&value.to_le_bytes());
    bytes[central[0] + 24..central[0] + 28].copy_from_slice(&value.to_le_bytes());
}

/// Replace every occurrence of `from` by `to` (same length).
pub fn replace_all(bytes: &mut [u8], from: &[u8], to: &[u8]) {
    assert_eq!(from.len(), to.len());
    let mut i = 0;
    while i + from.len() <= bytes.len() {
        if &bytes[i..i + from.len()] == from {
            bytes[i..i + from.len()].copy_from_slice(to);
            i += from.len();
        } else {
            i += 1;
        }
    }
}
