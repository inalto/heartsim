//! Acceptance tests of brief B28 Part C: the reference recordings. The index and
//! `ASSETS-LICENSES.md` list exactly the files in `assets/sounds/reference/`; every entry has its
//! author, source, licence, retrieval date and a checksum that matches the file; each file is the
//! stored format; the trainer offers the button only where a recording exists and shows the credit.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_app::AssetBundle;
use heartsim_app::heart_sound_player::HeartSoundPlayer;
use heartsim_app::reference_clips::{ReferenceLibrary, decode_wav, parse_index};
use heartsim_app::scenarios::ScenarioLibrary;
use heartsim_app::sound_out::FakeSink;
use heartsim_app::sound_trainer::{FINDINGS, Selection, SoundTrainer};
use heartsim_ui::{StripStyle, i18n};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn dir() -> PathBuf {
    assets().join("sounds/reference")
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    for b in Sha256::digest(bytes) {
        write!(s, "{b:02x}").unwrap();
    }
    s
}

fn is_wav(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("wav"))
}

fn library() -> ReferenceLibrary {
    ReferenceLibrary::from_bundle(&AssetBundle::from_dir(&assets()).unwrap())
}

#[test]
fn index_licences_and_directory_list_the_same_files() {
    let on_disk: BTreeSet<String> = std::fs::read_dir(dir())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| is_wav(n))
        .collect();
    let index = parse_index(&std::fs::read_to_string(dir().join("index.ron")).unwrap()).unwrap();
    let indexed: BTreeSet<String> = index.clips.iter().map(|c| c.file.clone()).collect();
    assert_eq!(indexed.len(), index.clips.len(), "a file is listed twice");
    assert_eq!(indexed, on_disk, "index.ron and the directory differ");
    let licences = std::fs::read_to_string(assets().join("../ASSETS-LICENSES.md")).unwrap();
    let listed: BTreeSet<String> = licences
        .split("`assets/sounds/reference/")
        .skip(1)
        .filter_map(|rest| rest.split('`').next())
        .filter(|n| is_wav(n))
        .map(str::to_owned)
        .collect();
    assert_eq!(listed, on_disk, "ASSETS-LICENSES.md and the directory differ");
    // and nothing else sits in the directory but the index
    let others = std::fs::read_dir(dir())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| !is_wav(n) && n != "index.ron")
        .count();
    assert_eq!(others, 0);
}

#[test]
fn every_entry_has_its_credit_and_a_matching_checksum() {
    let index = parse_index(&std::fs::read_to_string(dir().join("index.ron")).unwrap()).unwrap();
    let licences = std::fs::read_to_string(assets().join("../ASSETS-LICENSES.md")).unwrap();
    assert!(index.clips.len() >= 7);
    for c in &index.clips {
        for (name, v) in [
            ("title", &c.title),
            ("condition", &c.condition),
            ("author", &c.author),
            ("source", &c.source),
            ("licence", &c.licence),
            ("attribution", &c.attribution),
            ("retrieved", &c.retrieved),
        ] {
            assert!(!v.trim().is_empty(), "{}: empty {name}", c.file);
        }
        assert!(c.source.starts_with("https://"), "{}", c.file);
        assert!(
            c.licence == "CC0 1.0" || c.licence == "CC BY 2.0",
            "{}: {} is not an approved licence",
            c.file,
            c.licence
        );
        assert_eq!(c.retrieved, "2026-11-21");
        if c.licence.starts_with("CC BY") {
            for must in ["Kudriavtsev", "Heart energy signature spectrogram", "CC BY 2.0"] {
                assert!(c.attribution.contains(must), "{}: credit lacks {must}", c.file);
            }
        }
        let bytes = std::fs::read(dir().join(&c.file)).unwrap();
        assert_eq!(hex(&bytes), c.sha256, "{}: checksum", c.file);
        assert!(
            licences.contains(&c.sha256),
            "{}: checksum missing in ASSETS-LICENSES.md",
            c.file
        );
        assert!(
            licences.contains(&c.attribution),
            "{}: credit missing in ASSETS-LICENSES.md",
            c.file
        );
        assert!(
            FINDINGS.iter().any(|f| f.id == c.finding),
            "{}: unknown finding",
            c.file
        );
    }
}

#[test]
fn each_file_is_16_bit_mono_4_khz() {
    for e in std::fs::read_dir(dir()).unwrap() {
        let path = e.unwrap().path();
        if path.extension().is_some_and(|x| x == "wav") {
            let samples = decode_wav(&std::fs::read(&path).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let secs = samples.len() as f32 / 4000.0;
            assert!((1.0..=6.0).contains(&secs), "{}: {secs} s", path.display());
            assert!(
                samples.iter().any(|s| s.abs() > 0.1),
                "{} is silent",
                path.display()
            );
        }
    }
    assert_eq!(
        library().clips.len(),
        7,
        "every clip decodes through the bundle loader"
    );
}

fn page(player: HeartSoundPlayer, finding: &str) -> Harness<'static, Page> {
    i18n::set_language("en");
    let mut trainer = SoundTrainer::default();
    trainer.references = library();
    let i = FINDINGS.iter().position(|f| f.id == finding).unwrap();
    let mut p = HeartSoundPlayer::without_output();
    trainer.select(Selection::Finding(i), &mut p);
    Harness::builder()
        .with_size(egui::vec2(1200.0, 800.0))
        .build_ui_state(
            |ui, p: &mut Page| {
                p.trainer
                    .ui(ui, &p.scenarios, &mut p.player, &StripStyle::default());
            },
            Page {
                trainer,
                player,
                scenarios: ScenarioLibrary::load_dir(&assets().join("scenarios")).unwrap(),
            },
        )
}

struct Page {
    trainer: SoundTrainer,
    player: HeartSoundPlayer,
    scenarios: ScenarioLibrary,
}

#[test]
fn the_button_is_offered_only_where_a_recording_exists() {
    let offered: BTreeSet<&str> = [
        "s1_s2",
        "murmur_innocent",
        "midsystolic_click",
        "murmur_as",
        "murmur_ps",
        "murmur_vsd",
    ]
    .into();
    let lib = library();
    for f in &FINDINGS {
        assert_eq!(
            !lib.for_finding(f.id).is_empty(),
            offered.contains(f.id),
            "{}",
            f.id
        );
    }
    let mut h = page(
        HeartSoundPlayer::with_sink(Box::new(FakeSink::new())),
        "murmur_as",
    );
    h.run();
    let _ = h.get_by_label("Real recording");
    let mut h = page(
        HeartSoundPlayer::with_sink(Box::new(FakeSink::new())),
        "murmur_ms",
    );
    h.run();
    assert!(h.query_by_label("Real recording").is_none());
    // two recordings of the innocent murmur: numbered
    let mut h = page(
        HeartSoundPlayer::with_sink(Box::new(FakeSink::new())),
        "murmur_innocent",
    );
    h.run();
    let _ = h.get_by_label("Real recording 2");
}

#[test]
fn a_selected_recording_shows_its_credit_and_is_not_counted() {
    let sink = FakeSink::new();
    let mut h = page(HeartSoundPlayer::with_sink(Box::new(sink.clone())), "murmur_vsd");
    h.run();
    assert!(
        h.query_by_label_contains("Kudriavtsev").is_none(),
        "credit only while selected"
    );
    h.get_by_label("Real recording").click();
    h.run();
    h.run();
    let t = &h.state().trainer;
    assert_eq!(t.real, Some(0));
    assert!(t.playing && h.state().player.clip_active());
    let credit = t.selected_recording().unwrap().meta.attribution.clone();
    let _ = h.get_by_label_contains("Kudriavtsev V, Polyshchuk V, Roy D (2007)");
    let _ = h.get_by_label_contains(
        "Licence: CC BY 2.0. Source: https://commons.wikimedia.org/wiki/File:Heart-energy",
    );
    let _ = h.get_by_label_contains("not the heartsim model");
    assert!(credit.contains("CC BY 2.0"));
    // many loops heard: nothing is counted
    let mut sounds = heartsim_app::heart_sounds::HeartSounds::new(10.0);
    for _ in 0..(60 * 8) {
        let s = h.state_mut();
        s.player.frame(
            &mut sounds,
            &heartsim_app::heart_sound_player::PlayerInput {
                enabled: false,
                volume: 1.0,
                offset_ms: 0.0,
                paused: false,
                speed: 1.0,
                sim_now_ms: 0.0,
                dt_s: 1.0 / 60.0,
                muted: true,
            },
        );
        sink.drain_ms(1000.0 / 60.0);
        s.trainer.update(&s.player);
    }
    assert!(h.state().player.clip_loops() >= 1, "it did loop");
    assert_eq!(
        h.state().trainer.total(),
        0,
        "recordings are not counted as listens"
    );
    // back to the model
    h.get_by_label("Real recording").click();
    h.run();
    assert_eq!(h.state().trainer.real, None);
    assert!(h.query_by_label_contains("Kudriavtsev").is_none());
}

#[test]
fn without_sound_the_button_explains_that_a_recording_can_only_be_heard() {
    let mut h = page(HeartSoundPlayer::without_output(), "murmur_as");
    h.run();
    let _ = h.get_by_label("A recording can only be heard: there is no sound output here.");
    assert_eq!(h.state().trainer.real, None);
}

#[test]
fn the_texts_exist_in_both_languages() {
    for lang in ["en", "it"] {
        i18n::set_language(lang);
        for key in [
            "st-real",
            "st-real-hint",
            "st-real-no-sound",
            "st-real-note",
            "st-real-heading",
            "st-real-no-trace",
            "st-real-contains",
            "st-real-credit",
            "st-real-licence",
        ] {
            assert_ne!(i18n::tr(key), key, "{lang}: {key}");
        }
    }
    i18n::set_language("en");
}
