//! B43 acceptance, pure part: the index of an online case library round-trips; each client rule is
//! kept against a crafted index; a library address is https (or http on localhost) and joining a
//! file name never leaves it. No network.

use heartsim_casefile::MAX_FILE_BYTES;
use heartsim_casefile::library::{
    BaseError, CaseEntry, CaseIndex, EntryProblem, INDEX_FORMAT_VERSION, IndexError, LibraryBase,
    MAX_INDEX_BYTES, MAX_INDEX_ENTRIES, MAX_INDEX_TITLE_CHARS, MAX_SUMMARY_CHARS, check_entry, parse_index,
    read_index, valid_file_name,
};

const DIGEST: &str = "be00f9daa5fc2a8b0075d8f2e7058620a9152518ac392fb5253c2b5a397662a3";

fn entry(id: &str) -> CaseEntry {
    CaseEntry {
        id: id.into(),
        title: format!("Case {id}"),
        summary: "A synthetic patient.".into(),
        language: "en".into(),
        authors: vec!["heartsim authors".into()],
        licence: "CC-BY-4.0".into(),
        file: format!("{id}.hs"),
        size_bytes: 5321,
        sha256: DIGEST.into(),
        case_format_version: 1,
        min_app_version: Some("0.27.0".into()),
    }
}

fn index(cases: Vec<CaseEntry>) -> CaseIndex {
    CaseIndex {
        format_version: INDEX_FORMAT_VERSION,
        title: "heartsim case library".into(),
        updated: "2026-11-27".into(),
        cases,
    }
}

fn parse(i: &CaseIndex) -> Result<heartsim_casefile::library::ParsedIndex, IndexError> {
    parse_index(i.to_ron().unwrap().as_bytes())
}

/// The entry with one field changed is dropped and counted; the good one stays.
fn dropped(change: impl FnOnce(&mut CaseEntry)) -> EntryProblem {
    let mut bad = entry("case_bad");
    change(&mut bad);
    let problem = check_entry(&bad).expect_err("the entry breaks a rule");
    let p = parse(&index(vec![entry("case_good"), bad])).expect("the index itself is fine");
    assert_eq!(p.ignored, 1);
    assert_eq!(p.index.cases.len(), 1);
    assert_eq!(p.index.cases[0].id, "case_good");
    problem
}

#[test]
fn an_index_round_trips_through_its_text() {
    let mut e2 = entry("case_two");
    e2.min_app_version = None;
    let i = index(vec![entry("case_one"), e2]);
    let text = i.to_ron().unwrap();
    assert!(text.starts_with("CaseIndex("), "{text}");
    assert!(
        !text.contains("min_app_version: None"),
        "an absent version is left out"
    );
    let p = parse_index(text.as_bytes()).unwrap();
    assert_eq!(p.ignored, 0);
    assert_eq!(p.index, i);
    // the bounded reader gives the same
    assert_eq!(read_index(text.as_bytes()).unwrap().index, i);
}

#[test]
fn the_example_of_the_format_document_reads() {
    let text = format!(
        r#"CaseIndex(
    format_version: 1,
    title: "heartsim case library",
    updated: "2026-11-27",
    cases: [
        CaseEntry(
            id: "case_syncope_heart_block",
            title: "Collapse in the supermarket", summary: "…", language: "en", authors: ["heartsim authors"],
            licence: "MIT OR Apache-2.0",
            file: "case_syncope_heart_block.hs",
            size_bytes: 3839,
            sha256: "{DIGEST}",
            case_format_version: 1,
            min_app_version: Some("0.27.0"),
        ),
    ],
)"#
    );
    let p = parse_index(text.as_bytes()).unwrap();
    assert_eq!(p.index.cases.len(), 1);
    assert_eq!(p.ignored, 0);
}

#[test]
fn an_index_over_the_size_limit_is_refused() {
    let mut i = index(vec![entry("case_a")]);
    i.cases[0].summary = "x".repeat(MAX_SUMMARY_CHARS);
    let mut text = i.to_ron().unwrap();
    text.push_str(&" ".repeat(MAX_INDEX_BYTES + 1 - text.len()));
    assert_eq!(parse_index(text.as_bytes()), Err(IndexError::TooLarge));
    // the bounded reader stops after the limit instead of reading everything
    let endless = std::io::repeat(b' ');
    assert_eq!(read_index(endless).err(), Some(IndexError::TooLarge));
}

#[test]
fn an_index_with_too_many_entries_is_refused() {
    let cases: Vec<CaseEntry> = (0..=MAX_INDEX_ENTRIES)
        .map(|n| {
            let mut e = entry(&format!("c{n}"));
            e.summary.clear();
            e.authors.clear();
            e.min_app_version = None;
            e
        })
        .collect();
    let text = ron::to_string(&index(cases)).unwrap();
    assert!(text.len() < MAX_INDEX_BYTES, "{} bytes", text.len());
    assert_eq!(
        parse_index(text.as_bytes()),
        Err(IndexError::TooManyEntries {
            count: MAX_INDEX_ENTRIES + 1
        })
    );
}

#[test]
fn a_newer_index_is_refused_with_a_message() {
    let text = r#"CaseIndex(format_version: 2, title: "x", cases: [], signature: "new in v2")"#;
    let e = parse_index(text.as_bytes()).unwrap_err();
    assert_eq!(e, IndexError::NewerVersion { found: 2 });
    assert!(e.to_string().contains("update heartsim"), "{e}");
}

#[test]
fn strict_types_and_text_rules() {
    assert!(matches!(
        parse_index(br#"CaseIndex(format_version: 1, title: "x", cases: [], extra: 1)"#),
        Err(IndexError::Parse(_))
    ));
    assert!(matches!(
        parse_index(br#"CaseIndex(format_version: "1", title: "x", cases: [])"#),
        Err(IndexError::Parse(_))
    ));
    assert!(matches!(
        parse_index(br#"CaseIndex(format_version: 0, title: "x", cases: [])"#),
        Err(IndexError::Parse(_))
    ));
    assert_eq!(parse_index(&[0xff, 0xfe]), Err(IndexError::NotUtf8));
    let mut i = index(vec![]);
    i.title = "t".repeat(MAX_INDEX_TITLE_CHARS + 1);
    assert_eq!(parse(&i), Err(IndexError::TitleTooLong));
}

#[test]
fn an_entry_with_a_path_for_its_file_is_dropped() {
    assert_eq!(dropped(|e| e.file = "../x.hs".into()), EntryProblem::BadFileName);
    assert_eq!(
        dropped(|e| e.file = "https://evil/x.hs".into()),
        EntryProblem::BadFileName
    );
    assert_eq!(dropped(|e| e.file = "sub/x.hs".into()), EntryProblem::BadFileName);
    assert_eq!(dropped(|e| e.file = "x.hs?y=1".into()), EntryProblem::BadFileName);
    assert_eq!(dropped(|e| e.file = "x%2f.hs".into()), EntryProblem::BadFileName);
    assert_eq!(dropped(|e| e.file = ".hs".into()), EntryProblem::BadFileName);
    assert_eq!(dropped(|e| e.file = "x.zip".into()), EntryProblem::BadFileName);
}

#[test]
fn an_entry_with_a_bad_digest_is_dropped() {
    assert_eq!(
        dropped(|e| e.sha256 = DIGEST[..63].into()),
        EntryProblem::BadDigest
    );
    assert_eq!(
        dropped(|e| e.sha256 = DIGEST.to_uppercase()),
        EntryProblem::BadDigest
    );
    assert_eq!(
        dropped(|e| e.sha256 = format!("{}g", &DIGEST[..63])),
        EntryProblem::BadDigest
    );
}

#[test]
fn an_entry_larger_than_a_case_file_may_be_is_dropped() {
    assert_eq!(
        dropped(|e| e.size_bytes = MAX_FILE_BYTES as u64 + 1),
        EntryProblem::BadSize
    );
    assert_eq!(dropped(|e| e.size_bytes = 0), EntryProblem::BadSize);
    let mut e = entry("case_max");
    e.size_bytes = MAX_FILE_BYTES as u64;
    assert_eq!(check_entry(&e), Ok(()));
}

#[test]
fn other_entry_rules() {
    assert_eq!(dropped(|e| e.id = "Bad Id".into()), EntryProblem::BadId);
    assert_eq!(
        dropped(|e| e.summary = "s".repeat(MAX_SUMMARY_CHARS + 1)),
        EntryProblem::BadText
    );
    assert_eq!(
        dropped(|e| e.title = "t".repeat(MAX_INDEX_TITLE_CHARS + 1)),
        EntryProblem::BadText
    );
    assert_eq!(dropped(|e| e.language = "e n".into()), EntryProblem::BadText);
    assert_eq!(
        dropped(|e| e.min_app_version = Some("soon".into())),
        EntryProblem::BadVersion
    );
    // a repeated id: the first stays
    let p = parse(&index(vec![entry("case_a"), entry("case_a")])).unwrap();
    assert_eq!((p.index.cases.len(), p.ignored), (1, 1));
}

#[test]
fn three_bad_entries_are_counted() {
    let mut a = entry("case_a");
    a.file = "../a.hs".into();
    let mut b = entry("case_b");
    b.sha256 = "0".repeat(63);
    let mut c = entry("case_c");
    c.size_bytes = u64::MAX;
    let p = parse(&index(vec![a, entry("case_ok"), b, c])).unwrap();
    assert_eq!(p.ignored, 3);
    assert_eq!(p.index.cases.len(), 1);
}

#[test]
fn a_newer_case_or_app_is_recognised() {
    let e = entry("case_a");
    assert!(!e.needs_newer("0.27.0", 1));
    assert!(e.needs_newer("0.26.9", 1));
    assert!(!e.needs_newer("1.0.0", 1));
    let mut f = entry("case_f");
    f.case_format_version = 2;
    assert!(f.needs_newer("9.9.9", 1));
}

#[test]
fn a_library_address_is_https_or_http_on_localhost() {
    let b = LibraryBase::parse("https://inalto.github.io/heartsim/cases").unwrap();
    assert_eq!(b.as_str(), "https://inalto.github.io/heartsim/cases/");
    assert_eq!(b.host(), Some("inalto.github.io"));
    assert_eq!(b.index_url(), "https://inalto.github.io/heartsim/cases/index.ron");
    assert!(!b.is_plain_http());
    assert_eq!(
        LibraryBase::parse("HTTPS://Example.ORG").unwrap().as_str(),
        "https://example.org/"
    );
    assert_eq!(
        LibraryBase::parse("http://example.org/cases/"),
        Err(BaseError::NotHttps)
    );
    assert_eq!(
        LibraryBase::parse("http://localhost:8000/cases/")
            .unwrap()
            .as_str(),
        "http://localhost:8000/cases/"
    );
    assert!(LibraryBase::parse("http://127.0.0.1/").unwrap().is_plain_http());
    assert_eq!(
        LibraryBase::parse("http://localhost.evil.org/"),
        Err(BaseError::NotHttps)
    );
    assert_eq!(LibraryBase::parse("ftp://example.org/"), Err(BaseError::NotHttps));
    assert_eq!(LibraryBase::parse("file:///etc/"), Err(BaseError::NotHttps));
    assert_eq!(LibraryBase::parse("  "), Err(BaseError::Empty));
    for bad in [
        "https://user@example.org/",
        "https://example.org/cases/?x=1",
        "https://example.org/#top",
        "https://example.org/a/../b/",
        "https://example.org/a/./b/",
        "https://example.org/%2e%2e/",
        "https://exa mple.org/",
        "https://example.org:99999/",
        "https://example.org:/",
        "https:///path/",
        "https://exa_mple..org/",
    ] {
        assert!(LibraryBase::parse(bad).is_err(), "{bad}");
    }
    // relative only where the caller allows it (the browser build's own site)
    assert_eq!(LibraryBase::parse("../cases/"), Err(BaseError::NotHttps));
    let rel = LibraryBase::parse_allowing_relative("../cases/").unwrap();
    assert_eq!(rel.host(), None);
    assert_eq!(rel.index_url(), "../cases/index.ron");
    assert!(LibraryBase::parse_allowing_relative("cases/../../x/").is_err());
    assert!(LibraryBase::parse_allowing_relative("//evil.org/").is_err());
    assert!(LibraryBase::parse_allowing_relative("javascript:alert(1)").is_err());
}

#[test]
fn joining_never_leaves_the_base() {
    let bases = [
        LibraryBase::parse("https://example.org/lib/").unwrap(),
        LibraryBase::parse("http://localhost:8080").unwrap(),
        LibraryBase::parse_allowing_relative("../cases/").unwrap(),
    ];
    let names = [
        "case_a.hs",
        "A-b.c_d.hs",
        "../x.hs",
        "..hs",
        "a..b.hs",
        "/x.hs",
        "https://evil/x.hs",
        "//evil/x.hs",
        "x.hs/../../y.hs",
        "x\\y.hs",
        "x.hs#frag",
        "x.hs?q",
        "%2e%2e%2fx.hs",
        "x y.hs",
        "",
        ".",
        "..",
        "é.hs",
    ];
    for b in &bases {
        for n in names {
            match b.join(n) {
                Some(url) => {
                    assert!(valid_file_name(n), "{n}");
                    let rest = url.strip_prefix(b.as_str()).expect("starts with the base");
                    assert_eq!(rest, n);
                    assert!(!rest.contains('/') && !rest.contains(".."), "{url}");
                }
                None => assert!(!valid_file_name(n), "{n}"),
            }
        }
    }
    assert_eq!(
        bases[0].join("case_a.hs").as_deref(),
        Some("https://example.org/lib/case_a.hs")
    );
}
