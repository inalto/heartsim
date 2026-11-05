//! B19 acceptance tests: the translation catalogues, every key used in code, the hard-coded
//! string lint, and the accessible names of the toolbar (AccessKit tree through `egui_kittest`).

use std::fs;
use std::path::{Path, PathBuf};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_ui::i18n::{self, CATALOGUES, LANGUAGES};
use heartsim_ui::transport_controls;

fn repo_path(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel)
}

fn ftl_files(lang: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(repo_path(&format!("assets/locales/{lang}")))
        .unwrap_or_else(|e| panic!("assets/locales/{lang}: {e}"))
        .map(|e| e.expect("readable entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "ftl"))
        .collect();
    files.sort();
    files
}

/// Message ids of one `.ftl` source: lines that start with a lower-case identifier followed by ` =`.
/// Comments (`#`), terms (`-term =`) and indented continuation lines are skipped.
fn message_ids(src: &str) -> Vec<String> {
    src.lines()
        .filter_map(|line| {
            let (head, _) = line.split_once(" =")?;
            let mut chars = head.chars();
            let starts_ok = chars.next()?.is_ascii_lowercase();
            let rest_ok = chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            (starts_ok && rest_ok).then(|| head.to_string())
        })
        .collect()
}

fn keys_of(lang: &str) -> Vec<String> {
    ftl_files(lang)
        .iter()
        .flat_map(|p| message_ids(&fs::read_to_string(p).expect("readable ftl")))
        .collect()
}

#[test]
fn every_english_key_exists_in_italian() {
    let it = keys_of("it");
    let missing: Vec<String> = keys_of("en").into_iter().filter(|k| !it.contains(k)).collect();
    assert!(missing.is_empty(), "keys missing from it/: {missing:?}");
}

#[test]
fn stub_languages_only_define_english_keys() {
    let en = keys_of("en");
    for lang in ["de", "fr", "es"] {
        let orphans: Vec<String> = keys_of(lang).into_iter().filter(|k| !en.contains(k)).collect();
        assert!(
            orphans.is_empty(),
            "{lang}/ defines keys unknown to en/: {orphans:?}"
        );
    }
}

#[test]
fn no_language_defines_a_key_twice() {
    for lang in LANGUAGES {
        let keys = keys_of(lang.code);
        let mut seen = std::collections::BTreeSet::new();
        let dupes: Vec<&String> = keys.iter().filter(|k| !seen.insert(k.as_str())).collect();
        assert!(dupes.is_empty(), "{}: duplicate keys {dupes:?}", lang.code);
    }
}

#[test]
fn embedded_catalogues_match_the_files_on_disk() {
    for lang in LANGUAGES {
        for path in ftl_files(lang.code) {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            assert!(
                CATALOGUES.iter().any(|(l, n, _)| *l == lang.code && *n == name),
                "{}/{name} is not embedded in i18n.rs CATALOGUES",
                lang.code
            );
        }
    }
    for (lang, name, _) in CATALOGUES {
        assert!(
            repo_path(&format!("assets/locales/{lang}/{name}")).exists(),
            "{lang}/{name} missing"
        );
    }
}

/// The keys named by `t!("…")` calls in one source file.
fn keys_used_in(full: &str) -> Vec<String> {
    // Doc comments (`///`, `//!`) show example calls such as `t!("key")`; only code counts.
    let code: String = full
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let src = code.as_str();
    let mut keys = Vec::new();
    for (at, _) in src.match_indices("t!(") {
        // `format!(` also ends in `t!(`: a macro call of `t` must not follow an identifier character
        let before = src[..at].chars().next_back();
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let rest = src[at + 3..].trim_start();
        if let Some(body) = rest.strip_prefix('"')
            && let Some(end) = body.find('"')
        {
            keys.push(body[..end].to_string());
        }
    }
    keys
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("source dir") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn every_key_used_in_code_is_defined_in_english() {
    let en = keys_of("en");
    let mut files = Vec::new();
    for krate in ["heartsim-ui", "heartsim-app"] {
        rust_sources(&repo_path(&format!("crates/{krate}/src")), &mut files);
    }
    let mut undefined = Vec::new();
    for f in files {
        let src = fs::read_to_string(&f).expect("readable source");
        for key in keys_used_in(&src) {
            if !en.contains(&key) {
                undefined.push(format!("{} -> {key}", f.display()));
            }
        }
    }
    assert!(
        undefined.is_empty(),
        "t! keys with no English text: {undefined:#?}"
    );
}

/// Literal text passed to a label-like egui call. Covers the single-line forms used in the panels;
/// `heartsim` is the product name and stays as it is.
const LITERAL_CALLS: [&str; 14] = [
    ".label(\"",
    ".button(\"",
    ".strong(\"",
    ".weak(\"",
    ".heading(\"",
    ".small_button(\"",
    ".on_hover_text(\"",
    ".text(\"",
    ".hint_text(\"",
    ".suffix(\"",
    ".prefix(\"",
    "Window::new(\"",
    "CollapsingHeader::new(\"",
    "Button::new(\"",
];

#[test]
fn no_hard_coded_label_text_in_panels() {
    let mut files = Vec::new();
    for krate in ["heartsim-ui", "heartsim-app"] {
        rust_sources(&repo_path(&format!("crates/{krate}/src")), &mut files);
    }
    let mut hits = Vec::new();
    for f in files {
        let src = fs::read_to_string(&f).expect("readable source");
        for (n, line) in src.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            if line.trim_start().starts_with("#[cfg(test)]") {
                break; // unit tests below this point may use literals
            }
            for call in LITERAL_CALLS {
                if let Some(at) = line.find(call) {
                    let literal = &line[at + call.len()..];
                    let text = literal.split('"').next().unwrap_or_default();
                    if text.chars().any(char::is_alphabetic) && text != "heartsim" {
                        hits.push(format!("{}:{}: {}", f.display(), n + 1, line.trim()));
                    }
                }
            }
        }
    }
    assert!(hits.is_empty(), "hard-coded text, use t!(): {hits:#?}");
}

#[test]
fn toolbar_controls_have_accessible_names() {
    i18n::set_language("en");
    let mut speed = 1.0;
    let mut harness = Harness::new_ui(move |ui| {
        transport_controls(ui, false, &mut speed);
    });
    harness.run();
    assert!(
        harness.query_by_label("Pause").is_some(),
        "pause button has no accessible name"
    );
    // The speed slider and its drag-value box are both labelled "speed", so accept several matches.
    assert!(
        harness.query_all_by_label("speed").next().is_some(),
        "speed slider has no accessible name"
    );
}

#[test]
fn switching_language_relabels_the_toolbar() {
    i18n::set_language("en");
    let mut speed = 1.0;
    let mut harness = Harness::new_ui(move |ui| {
        transport_controls(ui, false, &mut speed);
    });
    harness.run();
    assert!(harness.query_by_label("Pause").is_some());

    i18n::set_language("it");
    harness.run();
    assert!(
        harness.query_by_label("Pausa").is_some(),
        "button did not relabel to Italian"
    );
    assert!(
        harness.query_by_label("Pause").is_none(),
        "English label still present after switch"
    );
    assert!(
        harness.query_all_by_label("velocità").next().is_some(),
        "slider did not relabel to Italian"
    );

    i18n::set_language("en");
}

#[test]
fn unknown_keys_show_the_key_and_fall_back_to_english() {
    i18n::set_language("it");
    assert_eq!(i18n::tr("no-such-key"), "no-such-key");
    assert_eq!(i18n::tr("transport-run"), "Avvia");
    i18n::set_language("en");
}
