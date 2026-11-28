//! B43 acceptance, with the in-memory transport (`MemoryFetch`): browsing lists a library's
//! entries; a good entry is imported into My cases and plays; a wrong length, a wrong digest, an
//! id mismatch and a file the reader refuses each give their own message and add nothing; Cancel
//! during a download adds nothing; an imported digest is recognised and a different one is offered
//! as an update, never replaced silently; an http address is refused and http://localhost accepted;
//! the page makes no request until its button is pressed. **No test touches the network.**

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_app::case_fetch::{FetchError, MemoryFetch};
use heartsim_app::case_online::{EntryState, ImportError, IndexFailure, OnlineLibrary};
use heartsim_app::case_ui::{CaseLibrary, CaseRef, Cases};
use heartsim_app::scenarios::ScenarioLibrary;
use heartsim_app::tutorial::UiAction;
use heartsim_casefile::library::{CaseEntry, CaseIndex, INDEX_FORMAT_VERSION, IndexError};
use heartsim_casefile::{CaseFileError, CasePackage, Manifest, sha256_hex};
use heartsim_core::case::{CASE_FORMAT_VERSION, Case, Stage};
use heartsim_core::tutorial::{Question, Step};
use std::path::{Path, PathBuf};

const BASE: &str = "https://cases.example/lib/";
const INDEX_URL: &str = "https://cases.example/lib/index.ron";

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn scenarios() -> ScenarioLibrary {
    ScenarioLibrary::load_dir(&assets().join("scenarios")).unwrap()
}

/// A case file of a case that loads the built-in sinus rhythm; `authors` changes the bytes.
fn case_file(id: &str, title: &str, authors: &str) -> Vec<u8> {
    let q = |n: usize| Question {
        prompt: format!("Question {n}?"),
        choices: vec!["one".into(), "two".into(), "three".into()],
        answer: n % 3,
        explanation: "Because.".into(),
    };
    let case = Case {
        format_version: CASE_FORMAT_VERSION,
        id: id.into(),
        title: title.into(),
        summary: "A library case.".into(),
        patient: "A synthetic patient".into(),
        references: vec!["Somebody 2026".into()],
        stages: vec![Stage {
            title: "Only".into(),
            markdown: "Look at the strip.".into(),
            actions: vec![Step::Ui(UiAction::LoadScenario("sinus_normal".into()))],
            questions: (0..5).map(q).collect(),
        }],
    };
    let manifest = Manifest::for_case(&case, vec![authors.into()], "CC-BY-4.0", "2026-11-27", "test");
    CasePackage::new(manifest, case, Vec::new()).write().unwrap()
}

fn entry(id: &str, title: &str, bytes: &[u8]) -> CaseEntry {
    CaseEntry {
        id: id.into(),
        title: title.into(),
        summary: "From the library.".into(),
        language: "en".into(),
        authors: vec!["A Teacher".into()],
        licence: "CC-BY-4.0".into(),
        file: format!("{id}.hs"),
        size_bytes: bytes.len() as u64,
        sha256: sha256_hex(bytes),
        case_format_version: 1,
        min_app_version: None,
    }
}

fn index_bytes(cases: Vec<CaseEntry>) -> Vec<u8> {
    CaseIndex {
        format_version: INDEX_FORMAT_VERSION,
        title: "Test library".into(),
        updated: "2026-11-27".into(),
        cases,
    }
    .to_ron()
    .unwrap()
    .into_bytes()
}

/// A library with one good case, `case_lib_a`, served at [`BASE`].
fn library() -> (MemoryFetch, Vec<u8>) {
    heartsim_ui::i18n::set_language("en");
    let mem = MemoryFetch::default();
    let a = case_file("case_lib_a", "Library case A", "A Teacher");
    mem.serve(
        INDEX_URL,
        index_bytes(vec![entry("case_lib_a", "Library case A", &a)]),
    );
    mem.serve(&format!("{BASE}case_lib_a.hs"), a.clone());
    (mem, a)
}

fn cases_with(mem: &MemoryFetch) -> Cases {
    let mut c = Cases::new(CaseLibrary::default());
    c.set_online(Box::new(mem.clone()), Some(BASE));
    c
}

/// Browse and wait for the list.
fn browse(c: &mut Cases, s: &mut ScenarioLibrary) {
    c.online_mut().browse();
    assert!(!c.poll_online(s));
    assert!(!c.online().busy(), "{:?}", c.online().message());
}

/// Import `id` (without asking to replace) and take the answer.
fn import(c: &mut Cases, s: &mut ScenarioLibrary, id: &str, update: bool) -> bool {
    c.online_mut().import(id, update);
    c.poll_online(s)
}

#[test]
fn browse_lists_the_entries() {
    let (mem, _) = library();
    let mut c = cases_with(&mem);
    let mut s = scenarios();
    assert_eq!(mem.requests().len(), 0);
    browse(&mut c, &mut s);
    assert_eq!(mem.requests(), [INDEX_URL]);
    let l = c.online().listing().expect("a list");
    assert_eq!(l.index.title, "Test library");
    assert_eq!(l.index.cases.len(), 1);
    assert_eq!(l.ignored, 0);
    assert_eq!(l.base.as_str(), BASE);
}

#[test]
fn a_good_entry_lands_in_my_cases_marked_with_its_host_and_plays() {
    let (mem, a) = library();
    let mut c = cases_with(&mem);
    let mut s = scenarios();
    browse(&mut c, &mut s);
    assert!(import(&mut c, &mut s, "case_lib_a", false));
    assert_eq!(mem.requests()[1], format!("{BASE}case_lib_a.hs"));
    assert_eq!(c.user.len(), 1);
    assert_eq!(c.user[0].provenance, "from cases.example");
    assert_eq!(c.user[0].sha256, sha256_hex(&a));
    let (ok, text) = c.online().message().unwrap();
    assert!(ok, "{text}");
    assert!(text.contains("Library case A"), "{text}");
    c.start(CaseRef::User(0));
    assert!(c.is_open());
    // the same digest is recognised as imported
    let e = c.online().listing().unwrap().index.cases[0].clone();
    assert_eq!(OnlineLibrary::entry_state(&e, &c.user), EntryState::Imported);
}

#[test]
fn each_bad_download_has_its_own_message_and_adds_nothing() {
    heartsim_ui::i18n::set_language("en");
    let good = case_file("case_lib_a", "Library case A", "A Teacher");
    let other = case_file("case_lib_b", "Library case B", "A Teacher");
    let garbage = b"this is not a zip archive".to_vec();
    let mut longer = good.clone();
    longer.push(0);
    let short = good[..good.len() - 1].to_vec();

    let mut wrong_digest = entry("case_digest", "Wrong digest", &good);
    wrong_digest.sha256 = sha256_hex(b"something else");
    // both list the good file's size; the server sends one byte more, or one less
    let long_entry = entry("case_long", "Longer", &good);
    let short_entry = entry("case_short", "Shorter", &good);
    // lists the id `case_mismatch` but serves the file of case_lib_b, with its true size and digest
    let mismatch = entry("case_mismatch", "Mismatch", &other);
    let refused = entry("case_garbage", "Garbage", &garbage);

    let mem = MemoryFetch::default();
    mem.serve(
        INDEX_URL,
        index_bytes(vec![
            wrong_digest,
            long_entry,
            short_entry,
            mismatch,
            refused,
            entry("case_offline", "Offline", &good),
        ]),
    );
    mem.serve(&format!("{BASE}case_digest.hs"), good.clone());
    mem.serve(&format!("{BASE}case_long.hs"), longer);
    mem.serve(&format!("{BASE}case_short.hs"), short);
    mem.serve(&format!("{BASE}case_mismatch.hs"), other);
    mem.serve(&format!("{BASE}case_garbage.hs"), garbage);
    mem.fail(&format!("{BASE}case_offline.hs"), FetchError::Status(503));
    let mut c = cases_with(&mem);
    let mut s = scenarios();
    browse(&mut c, &mut s);

    let mut messages = Vec::new();
    for id in [
        "case_digest",
        "case_long",
        "case_short",
        "case_mismatch",
        "case_garbage",
        "case_offline",
    ] {
        assert!(!import(&mut c, &mut s, id, false), "{id}");
        assert_eq!(c.user.len(), 0, "{id}: nothing is added");
        let (ok, text) = c.online().message().unwrap();
        assert!(!ok, "{id}: {text}");
        let err = c.online().import_error().cloned().unwrap();
        match id {
            "case_digest" => assert_eq!(err, ImportError::WrongDigest),
            "case_long" => assert!(matches!(err, ImportError::WrongLength { got: None, .. })),
            "case_short" => assert!(matches!(err, ImportError::WrongLength { got: Some(_), .. })),
            "case_mismatch" => assert!(matches!(err, ImportError::IdMismatch { .. })),
            "case_garbage" => assert!(matches!(err, ImportError::File(CaseFileError::NotZip(_)))),
            _ => assert_eq!(err, ImportError::Fetch(FetchError::Status(503))),
        }
        messages.push(text.to_owned());
    }
    let mut unique = messages.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), messages.len(), "{messages:#?}");
    assert!(messages[0].contains("SHA-256"), "{}", messages[0]);
    assert!(messages[3].contains("case_mismatch"), "{}", messages[3]);
    assert!(messages[5].contains("503"), "{}", messages[5]);
    assert!(c.status().is_none(), "the messages stay in the Online section");
}

#[test]
fn a_case_this_build_cannot_open_is_refused_with_its_message() {
    heartsim_ui::i18n::set_language("en");
    // a case that loads a rhythm nobody has: the reader accepts it, the resolver does not
    let bytes = {
        let mut case: Case = Case::from_ron(
            &std::fs::read_to_string(assets().join("cases/case_syncope_heart_block.ron")).unwrap(),
        )
        .unwrap();
        case.id = "case_unknown_rhythm".into();
        case.stages[0]
            .actions
            .insert(0, Step::Ui(UiAction::LoadScenario("no_such_rhythm".into())));
        let m = Manifest::for_case(&case, vec![], "", "2026-11-27", "test");
        // written by hand: the writer reads back but does not resolve
        CasePackage::new(m, case, Vec::new()).write().unwrap()
    };
    let mem = MemoryFetch::default();
    mem.serve(
        INDEX_URL,
        index_bytes(vec![entry("case_unknown_rhythm", "Unknown rhythm", &bytes)]),
    );
    mem.serve(&format!("{BASE}case_unknown_rhythm.hs"), bytes);
    let mut c = cases_with(&mem);
    let mut s = scenarios();
    browse(&mut c, &mut s);
    assert!(!import(&mut c, &mut s, "case_unknown_rhythm", false));
    assert_eq!(c.user.len(), 0);
    assert!(matches!(
        c.online().import_error(),
        Some(ImportError::Open(heartsim_app::case_ui::OpenError::File(
            CaseFileError::NeedsRhythm(_)
        )))
    ));
    assert!(c.online().message().unwrap().1.contains("no_such_rhythm"));
}

#[test]
fn cancel_during_the_download_adds_nothing() {
    let (mem, _) = library();
    let mut c = cases_with(&mem);
    let mut s = scenarios();
    browse(&mut c, &mut s);
    mem.hold(true);
    c.online_mut().import("case_lib_a", false);
    for _ in 0..3 {
        assert!(!c.poll_online(&mut s));
        assert!(c.online().busy());
    }
    c.online_mut().cancel();
    assert!(!c.online().busy());
    mem.release();
    for _ in 0..3 {
        assert!(!c.poll_online(&mut s));
    }
    assert_eq!(c.user.len(), 0);
    assert_eq!(
        c.online().message(),
        Some((true, "Cancelled: nothing was added."))
    );
}

#[test]
fn a_different_digest_under_the_same_id_is_an_update_never_a_silent_replacement() {
    let (mem, a) = library();
    let mut c = cases_with(&mem);
    let mut s = scenarios();
    // My cases already has case_lib_a from a file someone gave the user
    let mine = case_file("case_lib_a", "Library case A", "Someone else");
    c.open_bytes(&mine, "mine.hs", &mut s).unwrap();
    browse(&mut c, &mut s);
    let e = c.online().listing().unwrap().index.cases[0].clone();
    assert_eq!(OnlineLibrary::entry_state(&e, &c.user), EntryState::Update);
    // without the user's yes, nothing is replaced
    assert!(!import(&mut c, &mut s, "case_lib_a", false));
    assert!(matches!(
        c.online().import_error(),
        Some(ImportError::DifferentCase { .. })
    ));
    assert_eq!(c.user[0].sha256, sha256_hex(&mine));
    assert_eq!(c.user[0].provenance, "mine.hs");
    // with it, the library's version replaces it
    assert!(import(&mut c, &mut s, "case_lib_a", true));
    assert_eq!(c.user.len(), 1);
    assert_eq!(c.user[0].sha256, sha256_hex(&a));
    assert_eq!(OnlineLibrary::entry_state(&e, &c.user), EntryState::Imported);
}

#[test]
fn an_http_address_is_refused_and_localhost_accepted() {
    let (mem, _) = library();
    let mut c = cases_with(&mem);
    let mut s = scenarios();
    c.online_mut().set_address("http://cases.example/lib/");
    c.online_mut().browse();
    assert_eq!(mem.requests().len(), 0, "no request for an http address");
    let (ok, text) = c.online().message().unwrap();
    assert!(!ok && text.contains("https://"), "{text}");

    c.online_mut().set_address("http://localhost:8000/lib");
    mem.serve("http://localhost:8000/lib/index.ron", index_bytes(vec![]));
    browse(&mut c, &mut s);
    assert_eq!(mem.requests(), ["http://localhost:8000/lib/index.ron"]);
    assert_eq!(c.online().listing().unwrap().index.cases.len(), 0);
    assert_eq!(
        c.online().listing().unwrap().base.as_str(),
        "http://localhost:8000/lib/"
    );
}

#[test]
fn a_bad_index_is_reported_and_bad_entries_are_counted() {
    heartsim_ui::i18n::set_language("en");
    let good = case_file("case_lib_a", "Library case A", "A Teacher");
    let mut bad = entry("case_bad", "Bad", &good);
    bad.file = "../escape.hs".into();
    let mem = MemoryFetch::default();
    mem.serve(INDEX_URL, index_bytes(vec![entry("case_lib_a", "A", &good), bad]));
    let mut c = cases_with(&mem);
    let mut s = scenarios();
    browse(&mut c, &mut s);
    assert_eq!(c.online().listing().unwrap().ignored, 1);

    mem.serve(
        INDEX_URL,
        br#"CaseIndex(format_version: 9, title: "x", cases: [])"#.to_vec(),
    );
    browse(&mut c, &mut s);
    assert_eq!(
        c.online().index_error(),
        Some(&IndexFailure::Index(IndexError::NewerVersion { found: 9 }))
    );
    assert!(c.online().message().unwrap().1.contains("update heartsim"));

    mem.fail(INDEX_URL, FetchError::Blocked { cross_origin: true });
    browse(&mut c, &mut s);
    let text = c.online().message().unwrap().1.to_owned();
    assert!(text.contains("does not allow the web version"), "{text}");
}

struct Model {
    cases: Cases,
    scenarios: ScenarioLibrary,
}

#[test]
fn the_online_section_shows_nothing_and_asks_nothing_until_its_button_is_pressed() {
    let (mem, _) = library();
    let library = CaseLibrary::load_dir(&assets().join("cases")).unwrap();
    let mut cases = Cases::new(library);
    cases.set_online(Box::new(mem.clone()), Some(BASE));
    let mut h = Harness::builder()
        .with_size(egui::vec2(900.0, 8000.0))
        .build_ui_state(
            |ui, m: &mut Model| {
                m.cases.poll_online(&mut m.scenarios);
                m.cases.library_ui(ui, true);
            },
            Model {
                cases,
                scenarios: scenarios(),
            },
        );
    h.run_steps(6);
    assert_eq!(mem.requests().len(), 0, "no request at start");
    assert!(h.query_by_label_contains("Library case A").is_none());
    assert!(h.query_by_role_and_label(Role::Button, "Import").is_none());
    let _ = h.get_by_label("Online");
    h.get_by_role_and_label(Role::Button, "Browse the online library")
        .click();
    h.run_steps(4);
    assert_eq!(mem.requests(), [INDEX_URL]);
    let _ = h.get_by_label_contains("Test library: 1 cases");
    let _ = h.get_by_label("Library case A");
    h.get_by_role_and_label(Role::Button, "Import").click();
    h.run_steps(4);
    assert_eq!(mem.requests().len(), 2);
    assert_eq!(h.state().cases.user.len(), 1);
    let _ = h.get_by_role_and_label(Role::Button, "Imported");
    assert!(h.query_all_by_label_contains("from cases.example").count() >= 1);
}
