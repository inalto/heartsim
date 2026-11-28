//! B41 acceptance: the writer is deterministic and round-trips; every rule of the reader refuses a
//! crafted archive with its own error.

mod common;

use common::*;
use heartsim_casefile::{
    CASE_NAME, CaseFileError, CasePackage, MANIFEST_NAME, MAX_ENTRIES, MAX_ENTRY_BYTES, MAX_FILE_BYTES,
    MAX_TOTAL_BYTES, sha256_hex,
};

fn err(bytes: &[u8]) -> CaseFileError {
    CasePackage::read(bytes).expect_err("the reader accepted a bad file")
}

/// Replace the entry `name` of the fixture by `data` (or drop it with `None`).
fn with_entry(name: &str, data: Option<&[u8]>) -> Vec<u8> {
    let mut e = entries();
    if let Some(i) = e.iter().position(|(n, _)| n == name) {
        match data {
            Some(d) => e[i].1 = d.to_vec(),
            None => {
                e.remove(i);
            }
        }
    } else if let Some(d) = data {
        e.push((name.to_owned(), d.to_vec()));
    }
    archive_of(&e)
}

fn manifest_text() -> String {
    String::from_utf8(entries()[0].1.clone()).unwrap()
}

#[test]
fn round_trip() {
    let p = package();
    let bytes = p.write().unwrap();
    let back = CasePackage::read(&bytes).unwrap();
    assert_eq!(back, p);
    back.resolve(builtin).unwrap();
    // write(read(x)) reproduces a file the writer made
    assert_eq!(back.write().unwrap(), bytes);
    let names: Vec<String> = entries().into_iter().map(|(n, _)| n).collect();
    assert_eq!(
        names,
        [MANIFEST_NAME, CASE_NAME, "scenarios/my_rhythm.ron", "README.txt"],
        "fixed entry order, manifest first"
    );
    assert_eq!(p.file_name(), "case_fixture.hs");
    assert_eq!(back.tier0_scenarios().unwrap()[0].scenario.id, "my_rhythm");
}

#[test]
fn deterministic_bytes_and_a_pinned_digest() {
    let a = package().write().unwrap();
    let b = package().write().unwrap();
    assert_eq!(a, b);
    // a package with scenarios listed in another order writes the same bytes
    let mut p = package();
    p.scenarios.push(("zz_other".into(), scenario_text("zz_other")));
    p.case.stages[1].actions.push(heartsim_core::tutorial::Step::Ui(
        heartsim_core::tutorial::UiAction::LoadScenario("zz_other".into()),
    ));
    let mut q = p.clone();
    q.scenarios.reverse();
    assert_eq!(p.write().unwrap(), q.write().unwrap());
    // the fixture's digest is pinned: a change here means files written by other builds differ
    assert_eq!(
        sha256_hex(&a),
        "e82b833b3743c78a0a4a31ef445d3f2e279bcd97648c6e129a3951d7c71d4b4a",
        "the fixture's bytes changed (writer, zip or deflate); if intended, update the pin"
    );
    assert_eq!(sha256_hex(b"").len(), 64);
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn too_large_input() {
    let big = vec![0u8; MAX_FILE_BYTES + 1];
    assert_eq!(
        err(&big),
        CaseFileError::FileTooLarge {
            size: MAX_FILE_BYTES + 1
        }
    );
}

#[test]
fn garbage_is_not_a_zip() {
    assert!(matches!(err(b"not a zip at all"), CaseFileError::NotZip(_)));
    assert!(matches!(err(&[]), CaseFileError::NotZip(_)));
}

#[test]
fn too_many_entries() {
    let names: Vec<String> = (0..=MAX_ENTRIES).map(|i| format!("scenarios/s{i}.ron")).collect();
    let mut e = vec![(MANIFEST_NAME.to_owned(), manifest_text().into_bytes())];
    e.extend(names.into_iter().map(|n| (n, b"x".to_vec())));
    assert_eq!(
        err(&archive_of(&e)),
        CaseFileError::TooManyEntries {
            count: MAX_ENTRIES + 2
        }
    );
}

#[test]
fn a_zip_bomb_whose_header_lies_is_stopped_while_inflating() {
    // 3 MiB of blanks deflate to a few KiB; the headers claim 100 bytes
    let bomb = vec![b' '; 3 * 1024 * 1024];
    let mut bytes = archive(&[(MANIFEST_NAME, &bomb)]);
    assert!(bytes.len() < MAX_FILE_BYTES);
    patch_uncompressed_size(&mut bytes, 100);
    // refused while inflating: by the zip reader when the data outgrows the declared size, by the
    // reader's own count (`Read::take`) when the declared size is itself over the limit
    let e = err(&bytes);
    assert!(
        matches!(&e, CaseFileError::Corrupt { message, .. } if message.contains("larger than its declared"))
            || matches!(&e, CaseFileError::EntryTooLarge { .. }),
        "{e:?}"
    );
    let mut bytes = archive(&[(MANIFEST_NAME, &bomb)]);
    patch_uncompressed_size(&mut bytes, u32::MAX);
    assert_eq!(
        err(&bytes),
        CaseFileError::EntryTooLarge {
            name: MANIFEST_NAME.into()
        }
    );
}

#[test]
fn an_entry_over_the_limit_and_a_total_over_the_limit() {
    let just_over = vec![b' '; MAX_ENTRY_BYTES as usize + 1];
    assert_eq!(
        err(&with_entry(CASE_NAME, Some(&just_over))),
        CaseFileError::EntryTooLarge {
            name: CASE_NAME.into()
        }
    );
    let full = vec![b' '; MAX_ENTRY_BYTES as usize];
    let mut e = vec![(MANIFEST_NAME.to_owned(), manifest_text().into_bytes())];
    let n = (MAX_TOTAL_BYTES / MAX_ENTRY_BYTES) as usize + 1;
    e.extend((0..n).map(|i| (format!("scenarios/s{i}.ron"), full.clone())));
    assert_eq!(err(&archive_of(&e)), CaseFileError::TotalTooLarge);
}

#[test]
fn unsafe_names_are_refused() {
    for name in [
        "../x",
        "scenarios/../case.ron",
        "/abs",
        "/case.ron",
        "a\\b",
        "scenarios\\x.ron",
        "C:/x",
    ] {
        let bytes = with_entry(name, Some(b"x"));
        assert!(
            matches!(err(&bytes), CaseFileError::UnsafeName { .. }),
            "{name}: {:?}",
            err(&bytes)
        );
    }
}

#[test]
fn a_duplicate_name_is_refused() {
    let mut e = entries();
    e.push(("casX.ron".into(), b"x".to_vec()));
    let mut bytes = archive_of(&e);
    replace_all(&mut bytes, b"casX.ron", b"case.ron");
    assert_eq!(err(&bytes), CaseFileError::DuplicateName);
}

#[test]
fn an_unknown_entry_is_refused_and_named() {
    for name in ["picture.png", "scenarios/x.txt", "scripts/run.sh", "Case.ron"] {
        assert_eq!(
            err(&with_entry(name, Some(b"x"))),
            CaseFileError::UnknownEntry { name: name.into() },
            "{name}"
        );
    }
    assert!(matches!(
        err(&with_entry("scenarios/Bad-Id.ron", Some(b"x"))),
        CaseFileError::BadScenarioId(_)
    ));
}

#[test]
fn an_encrypted_entry_is_refused() {
    let mut bytes = package().write().unwrap();
    // general-purpose flag bit 0: "encrypted"
    patch_u16(&mut bytes, 6, 8, 1);
    assert_eq!(
        err(&bytes),
        CaseFileError::Encrypted {
            name: MANIFEST_NAME.into()
        }
    );
}

#[test]
fn a_method_other_than_stored_or_deflate_is_refused() {
    let mut bytes = package().write().unwrap();
    // method 12 = bzip2
    patch_u16(&mut bytes, 8, 10, 12);
    assert_eq!(
        err(&bytes),
        CaseFileError::UnsupportedMethod {
            name: MANIFEST_NAME.into()
        }
    );
}

#[test]
fn the_manifest_must_come_first_and_both_required_entries_exist() {
    let mut e = entries();
    e.swap(0, 1);
    assert_eq!(err(&archive_of(&e)), CaseFileError::ManifestNotFirst);
    assert_eq!(
        err(&with_entry(CASE_NAME, None)),
        CaseFileError::MissingEntry(CASE_NAME)
    );
    let e: Vec<(String, Vec<u8>)> = entries().into_iter().skip(1).collect();
    assert_eq!(err(&archive_of(&e)), CaseFileError::ManifestNotFirst);
}

#[test]
fn a_newer_format_version_is_refused_with_a_clear_message() {
    let m = manifest_text().replace(
        "format_version: 1,",
        "format_version: 2,\n    images: [\"x.png\"],",
    );
    let e = err(&with_entry(MANIFEST_NAME, Some(m.as_bytes())));
    assert_eq!(e, CaseFileError::NewerVersion { found: 2 });
    assert!(e.to_string().contains("update heartsim"), "{e}");
}

#[test]
fn a_wrong_kind_is_not_a_case_file() {
    let m = manifest_text().replace("kind: \"heartsim-case\"", "kind: \"heartsim-tutorial\"");
    assert_eq!(
        err(&with_entry(MANIFEST_NAME, Some(m.as_bytes()))),
        CaseFileError::WrongKind("heartsim-tutorial".into())
    );
}

#[test]
fn an_unknown_manifest_field_in_version_1_is_refused() {
    let m = manifest_text().replace("format_version: 1,", "format_version: 1,\n    script: \"x\",");
    assert!(matches!(
        err(&with_entry(MANIFEST_NAME, Some(m.as_bytes()))),
        CaseFileError::Manifest(_)
    ));
}

#[test]
fn manifest_field_rules() {
    let m = manifest_text().replace("id: \"case_fixture\"", "id: \"Case Fixture\"");
    assert!(matches!(
        err(&with_entry(MANIFEST_NAME, Some(m.as_bytes()))),
        CaseFileError::BadId(_)
    ));
    let long = "x".repeat(heartsim_casefile::MAX_FIELD_CHARS + 1);
    let m = manifest_text().replace("title: \"A fixture case\"", &format!("title: \"{long}\""));
    assert!(matches!(
        err(&with_entry(MANIFEST_NAME, Some(m.as_bytes()))),
        CaseFileError::FieldTooLong { field: "title", .. }
    ));
    let m = manifest_text().replace("language: \"en\"", "language: \"en; drop\"");
    assert!(matches!(
        err(&with_entry(MANIFEST_NAME, Some(m.as_bytes()))),
        CaseFileError::BadLanguage(_)
    ));
    let mut p = package();
    p.manifest.authors = vec!["a".into(); heartsim_casefile::MAX_AUTHORS + 1];
    assert!(matches!(
        p.write(),
        Err(CaseFileError::FieldTooLong { field: "authors", .. })
    ));
}

#[test]
fn the_manifest_id_must_equal_the_case_id() {
    let m = manifest_text().replace("id: \"case_fixture\"", "id: \"case_other\"");
    assert_eq!(
        err(&with_entry(MANIFEST_NAME, Some(m.as_bytes()))),
        CaseFileError::IdMismatch {
            manifest: "case_other".into(),
            case: "case_fixture".into()
        }
    );
}

#[test]
fn a_case_with_problems_is_refused() {
    let mut p = package();
    p.case.stages[0].questions[0].answer = 7;
    let text = p.case.to_ron().unwrap();
    assert!(matches!(
        err(&with_entry(CASE_NAME, Some(text.as_bytes()))),
        CaseFileError::Case(_)
    ));
}

#[test]
fn a_scenario_listed_but_missing_and_present_but_not_listed() {
    assert_eq!(
        err(&with_entry("scenarios/my_rhythm.ron", None)),
        CaseFileError::ScenarioMissing("my_rhythm".into())
    );
    let extra = scenario_text("extra");
    assert_eq!(
        err(&with_entry("scenarios/extra.ron", Some(extra.as_bytes()))),
        CaseFileError::ScenarioNotListed("extra".into())
    );
    // the entry name and the scenario's own id must agree
    let m = manifest_text().replace("\"my_rhythm\"", "\"my_rhythm\", \"other\"");
    let mut e = entries();
    e[0].1 = m.into_bytes();
    e.push((
        "scenarios/other.ron".into(),
        scenario_text("not_other").into_bytes(),
    ));
    assert!(matches!(
        err(&archive_of(&e)),
        CaseFileError::ScenarioIdMismatch { .. }
    ));
    let m = manifest_text().replace("\"my_rhythm\"", "\"my_rhythm\", \"my_rhythm\"");
    assert_eq!(
        err(&with_entry(MANIFEST_NAME, Some(m.as_bytes()))),
        CaseFileError::ScenarioListedTwice("my_rhythm".into())
    );
    assert!(matches!(
        err(&with_entry("scenarios/my_rhythm.ron", Some(b"Scenario(nonsense"))),
        CaseFileError::Scenario { .. }
    ));
}

#[test]
fn an_id_clash_with_a_built_in_rhythm_and_a_missing_rhythm() {
    let p = package();
    assert_eq!(
        p.resolve(|id| id == "my_rhythm" || id == "sinus_normal"),
        Err(CaseFileError::ScenarioClash("my_rhythm".into()))
    );
    let e = p.resolve(|_| false).unwrap_err();
    assert_eq!(e, CaseFileError::NeedsRhythm(vec!["sinus_normal".into()]));
    assert!(e.to_string().contains("needs the rhythm `sinus_normal`"), "{e}");
    assert!(CasePackage::read_for(&p.write().unwrap(), builtin).is_ok());
    assert_eq!(p.library_scenario_ids().collect::<Vec<_>>(), ["sinus_normal"]);
}

#[test]
fn invalid_utf8_and_a_nul_are_refused() {
    assert_eq!(
        err(&with_entry(CASE_NAME, Some(&[0x43, 0x61, 0xff, 0xfe]))),
        CaseFileError::NotUtf8 {
            name: CASE_NAME.into()
        }
    );
    let text = package().case.to_ron().unwrap().replacen("Some", "So\0me", 1);
    assert_eq!(
        err(&with_entry(CASE_NAME, Some(text.as_bytes()))),
        CaseFileError::Nul {
            name: CASE_NAME.into()
        }
    );
}

#[test]
fn a_folder_entry_from_an_archive_tool_and_the_readme_are_accepted() {
    let mut e = entries();
    e.insert(1, ("scenarios/".into(), Vec::new()));
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default();
    for (n, d) in &e {
        if n.ends_with('/') {
            w.add_directory(n.as_str(), opts).unwrap();
        } else {
            w.start_file(n.as_str(), opts).unwrap();
            std::io::Write::write_all(&mut w, d).unwrap();
        }
    }
    let bytes = w.finish().unwrap().into_inner();
    assert_eq!(CasePackage::read(&bytes).unwrap(), package());
    // the README is ignored: any text, or none
    assert!(CasePackage::read(&with_entry("README.txt", Some(b"\xff\x00 anything"))).is_ok());
    assert!(CasePackage::read(&with_entry("README.txt", None)).is_ok());
}

#[test]
fn ron_nesting_is_bounded() {
    // ron's recursion limit (128) is on: a deeply nested value is an error, not a stack overflow
    let deep = format!("{}{}", "[".repeat(10_000), "]".repeat(10_000));
    let m = manifest_text().replace("authors: [", &format!("authors: [{deep},"));
    assert!(matches!(
        err(&with_entry(MANIFEST_NAME, Some(m.as_bytes()))),
        CaseFileError::Manifest(_)
    ));
    let c = package()
        .case
        .to_ron()
        .unwrap()
        .replacen("references: [", &format!("references: [{deep},"), 1);
    assert!(matches!(
        err(&with_entry(CASE_NAME, Some(c.as_bytes()))),
        CaseFileError::Case(_)
    ));
}
