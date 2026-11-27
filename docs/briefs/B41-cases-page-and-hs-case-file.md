# B41 — Patient cases get their own page, and a case file format (`.hs`) to open and save
**Crates** `heartsim-casefile` (new), `heartsim-app`, `heartsim-web`, `heartsim-cli`, `xtask`, docs · **Model** Opus ·
**Milestone** M13 · **Depends on** B15 (tutorial engine), B18 (cases), B21 (web build).
B42 (case editor), B43 (online library) and B44 (more cases) build on this brief; leave them the seams named below.

## Goal
1. The patient cases leave the bottom of the Tutorials page and get their own page, **Cases**.
2. A case can travel as one file, `name.hs`: a teacher saves it, a student opens it, on the desktop and in the
   browser. The format is documented and versioned, and a hostile file cannot harm the application.

## What is there today (read first)
* `crates/heartsim-core/src/case.rs`: `Case` (RON, `format_version` 1), `Stage`, `problems`, `content_problems`,
  `scenario_ids`. Steps are the tutorial vocabulary of `crates/heartsim-core/src/tutorial.rs` (`Step`, `UiAction`).
* `crates/heartsim-app/src/case_ui.rs`: `CaseLibrary` (directory or downloaded bundle), `Cases` (list, session,
  reader); `crates/heartsim-app/src/app/tutorials.rs`: `tutorials_tab` draws the case list under the tutorial list,
  `tutorial_frame` shows the reader as a right-hand panel on every page.
* `crates/heartsim-app/src/nav.rs`: `Tab`, `Group`, shortcuts (`Tab::ALL` order is the number-key order; fifteen
  pages, the first ten have keys). Tests in that file and GUI tests under `crates/heartsim-app/tests/` count pages.
* `assets/cases/*.ron` (three cases), `xtask/src/web.rs` (the web archive carries `cases/`),
  `crates/heartsim-cli/src/content.rs` (`validate-cases`).
* Files today: desktop saves go to a typed path or `scenario_file::user_scenario_dir()`; a file is opened by dropping
  it on the window (`app/exports.rs`, `raw.dropped_files`). `rfd` is declared in the workspace and used by no crate.
  The browser build has no files at all (`crates/heartsim-web/src/lib.rs`, "No files").
* `crates/heartsim-io/src/bundle.rs`: how a deterministic zip is written (fixed order, 1980 timestamps, deflate).
  `crates/heartsim-web/src/archive.rs`: how a downloaded zip is unpacked with a size limit and `enclosed_name`.
  `heartsim-io` does not build for `wasm32` (PDF), which is why the case file gets its own crate.

## Part A — the Cases page
* `Tab::Cases` in `Group::Learn`, after Tutorials, titled "Cases" (Fluent `tab-cases`, five languages). Append it to
  `Tab::ALL` so no existing shortcut moves; it has no number key. Update the page-count tests and the docs that
  count pages.
* The page lists the cases in sections: **Built-in**, **My cases** (opened or saved by the user, see part C) and a
  placeholder slot B43 fills (**Online**). Each card shows title, summary, stage and question counts, best score,
  the licence and author when the file has them, and Start. A user case also has Save as… and Remove.
* The Tutorials page no longer lists cases; one line there links to the Cases page. The reader panel is unchanged.
* The best score is still kept per case id for the session only (unchanged).

## Part B — the `.hs` case file
**Container: a zip archive (deflate), extension `.hs`, media type `application/vnd.heartsim.case+zip`.** Reasons to
record in the format document: one file to hand out; `zip` 9 with the pure-Rust deflate is already a dependency of
the desktop and the browser build (no new crate, nothing new for `cargo deny`); a zip can be opened with any archive
tool, so a case stays readable and editable by hand; entries are named, so later versions can add files without
breaking old readers. gzip of a single RON was considered and rejected (no room for bundled rhythms or later
additions); a tar is not seekable and needs a second crate.

Entries (UTF-8 names, `/` separators, no directories needed):

| entry | required | content |
|---|---|---|
| `heartsim-case.ron` | yes, **first entry** | the manifest, see below |
| `case.ron` | yes | a `heartsim_core::case::Case`, as in `assets/cases/` |
| `scenarios/<id>.ron` | no | rhythms the case loads that are not in the built-in library (`Tier0Scenario` files, `<id>` = the scenario id) |
| `README.txt` | no | written by Save for a human who unzips the file; ignored on open |

Manifest (`heartsim-case.ron`), RON with struct names:
```ron
CaseFile(
    format_version: 1,          // of the container; readers refuse a newer one with a clear message
    kind: "heartsim-case",      // constant; anything else is not a case file
    id: "case_syncope_heart_block",   // equals Case.id; [a-z0-9_]{1,64}
    title: "…",
    language: "en",             // BCP 47
    authors: ["…"],             // free text, may be empty
    licence: "CC-BY-4.0",       // SPDX id or free text; shown on the card. Empty = "not stated"
    created: "2026-11-27",      // ISO date, informational
    generator: "heartsim 0.27.0",
    scenarios: ["my_rhythm"],   // ids of the bundled scenarios, must match the entries exactly
)
```
Reserved for later versions and **refused in version 1**: any entry not listed above (so images, audio or scripts
cannot ride along unnoticed); a reader of version 1 rejects the file and names the entry.

**Reader rules (untrusted input; every limit a named constant with a test).**
* Input at most 2 MiB; at most 16 entries; each entry at most 1 MiB unpacked and the total at most 4 MiB, enforced
  while reading (count the bytes actually inflated, never trust the header's size: use `Read::take`); only stored and
  deflate methods; no encrypted entries.
* Names: must be exactly one of the allowed names, matched as strings after checking `enclosed_name()` is `Some`; no
  absolute paths, no `..`, no backslashes, no duplicate names; `<id>` matches `[a-z0-9_]{1,64}`. The reader never
  writes an entry to disk under its archive name: everything is parsed in memory.
* Text is UTF-8 without NUL; RON is parsed with the existing `Case::from_ron` and `Tier0Scenario::from_ron` (check
  how deep RON nesting is bounded; ron has a recursion limit: confirm it is on); `Case::problems` must be empty; the
  manifest id equals the case id; every `LoadScenario` id that is not bundled is reported as "needs the rhythm
  `<id>`" when it is not in the library of the running build (the open succeeds only if all are resolvable).
* A bundled scenario never replaces a built-in one: on an id clash the file is refused.
* Case markdown is rendered by `egui_commonmark` with images and HTML off (check the features in the root
  `Cargo.toml`: `pulldown_cmark` only); links are shown but a case must not be able to open a URL without the user
  clicking it. Confirm and write it into the format document.
* Errors are a `thiserror` enum with one variant per rule; the UI shows the message, never a panic. Add a
  `proptest` that feeds arbitrary bytes and mutated valid files to the reader: it returns `Err` or `Ok`, never
  panics, never allocates beyond the limits.

**Writer rules.** Deterministic like `bundle.rs` (fixed order with the manifest first, 1980 timestamps, fixed
permissions, deflate): the same case gives the same bytes, so a SHA-256 of the file identifies it (B43 relies on
that). `write(read(x))` reproduces a file the writer made.

**Where the code lives.** A new crate `crates/heartsim-casefile` (`heartsim-core`, `heartsim-ecg`, `ron`, `zip`,
`thiserror`, `serde`; `sha2` if you expose the digest here), pure: no egui, no I/O beyond `Read`/`Write` on byte
slices, builds for `wasm32-unknown-unknown`. In the layering it sits beside `echo` (below `render`). Add it to the
workspace members and `[workspace.dependencies]`, to `cargo xtask`'s wasm check if there is a list of pure crates,
and say in your report that `CLAUDE.md`'s layering line needs the maintainer's edit (do not edit `CLAUDE.md`).
API sketch: `CasePackage { manifest, case, scenarios: Vec<(String, String)> }`, `CasePackage::read(&[u8]) ->
Result<Self, CaseFileError>`, `write(&self) -> Result<Vec<u8>, CaseFileError>`, `sha256_hex(&[u8])`, the limits as
`pub const`.

## Part C — open and save in the application
* **Open** on the Cases page: desktop, a file dialog and dropping a `.hs` file on the window (any page; a dropped
  `.hs` goes to the Cases page); browser, an "Open case file…" button that opens the browser's file picker, and
  drag-and-drop if `raw.dropped_files` delivers bytes on the web (check). **Save as…** writes the `.hs`: desktop, a
  file dialog; browser, a download.
* How: prefer `rfd` 0.17 (already chosen in `docs/research/rust-stack.md`; MIT) on both targets if `cargo deny check`
  passes with it, it builds on Linux without GTK (the `xdg-portal` backend) and its `wasm32` path (file input,
  download) works under `trunk`; the async calls need a small executor on each side (a thread with `pollster` on the
  desktop, `wasm-bindgen-futures::spawn_local` in the browser), with the result handed to the UI through a channel
  or an `Arc<Mutex<Option<…>>>` polled each frame, the pattern `update_check.rs` uses. If `rfd` fails any of the
  three checks, fall back to: desktop, drag-and-drop plus a path field (as the timeline editor has); browser, the
  picker and the download written with `web-sys` in `heartsim-web` and injected into the app the way the Web Audio
  sink is (`crates/heartsim-web/src/web.rs`, `WebApp::new`). Say which path you took and why. In both cases keep the
  platform part behind one small trait or function pair in `heartsim-app` (`case_io.rs`) so the page code has no
  `cfg`.
* **My cases** persist: desktop, the opened or saved `.hs` files are copied into `<user data dir>/cases/` (next to
  `user_scenario_dir()`) and listed at start; Remove deletes that copy after a confirmation. Browser, they are kept
  in eframe storage (`localStorage`) as base64 under one key with a total cap of 1 MiB and a clear message when the
  cap refuses a file; if that proves unsound, keep them for the session only and say so on the page.
* A user case's bundled scenarios are added to the scenario library for the session under their ids (see
  `export_panel::add_to_library` for how a custom scenario joins the library); they are listed under a "From cases"
  heading in the rhythm picker (`scenarios::picker_groups`) and never enter the quiz pools (check
  `rhythm_quiz::make_question` and the interval drill: they must keep drawing only from the built-in ECG pool).
* Opening a case whose id equals a built-in case is allowed and listed under My cases with "(file)" after the title;
  scores are keyed so the two do not mix.
* **Save as…** also for the built-in cases, so a teacher can take one as a template; the manifest then carries
  "heartsim authors" and the licence of the shipped texts (see `ASSETS-LICENSES.md`).
* `heartsim-cli`: `case pack <case.ron> [--scenario <file>]… -o <name.hs>`, `case unpack <name.hs> -o <dir>`
  (writes only the allowed names into `<dir>`), `case check <name.hs>` (the reader's verdict plus `content_problems`
  as warnings). `validate-cases` keeps working on the directory.

## Acceptance tests
* `heartsim-casefile`: round trip; deterministic bytes (two writes are identical; a fixed fixture's SHA-256 is
  pinned); every reader rule has a test with a crafted archive (too large, too many entries, a zip bomb whose
  header lies about its size, `../x`, `/abs`, a backslash name, a duplicate name, an unknown entry, an encrypted
  entry, a newer `format_version`, a wrong `kind`, id mismatch, a scenario listed but missing and present but not
  listed, an id clash with a built-in rhythm, invalid UTF-8, a NUL); the `proptest` above; the crate builds with
  `cargo build -p heartsim-casefile --target wasm32-unknown-unknown` (if the target is installed; if not, say so).
* The three shipped cases pack, read back equal and pass `case check`; a test packs a case with one bundled
  scenario and plays its first stage through the same `TutorialHost` the CLI's `validate-cases` uses.
* `nav.rs`: sixteen pages, Cases in Learn, the first ten shortcuts unchanged. GUI test (`egui_kittest`): the Cases
  page lists the three built-in cases; the Tutorials page lists none; a package given to the page's `open_bytes`
  appears under My cases, starts, and is offered for saving; a rejected file shows its error text.
* Quiz pools are unchanged by an opened case with a bundled rhythm (assert the pool before and after).
* `nice -n 10 cargo deny check` (if new crates entered the tree, list them with their licences),
  `nice -n 10 cargo xtask ci`, and `cargo xtask web-budget` or the web build's check if it can run here.

## Docs
* `docs/case-file.md` (new, linked from `docs/README.md`): the format, the limits, the security rules, the version
  policy (what a version 2 may add and how readers of version 1 behave), a worked example made with the CLI.
* `docs/book/src/dev/file-formats.md` (a section and a link), `docs/book/src/user/cases.md` (new, in `SUMMARY.md`:
  the Cases page, opening and saving a file on the desktop and in the browser, what a case file may contain, a
  warning that a case file from someone else is teaching text nobody at heartsim reviewed), `tutorials.md` and
  `ui-tour.md` (cases moved), `cli.md`, `docs/web.md` and the `heartsim-web` crate doc ("No files" is no longer
  true for cases), `docs/ARCHITECTURE.md` (the new crate, the format), `known-issues.md` (the dialogs and the
  browser picker **were not seen on a real screen or in a real browser** unless you ran them), `CHANGELOG.md`
  `## [Unreleased]`, `docs/briefs/README.md` is maintained by the planner: do not touch it.

## Legal and content rules
Nothing from `epicardio/`, `epicardio-extracted/` or echosuite.app. Cases are synthetic teaching cases, never a real
person's record; say so in the user page and in the format document (authors' responsibility for files they share).

## Rules
One cargo build at a time, always `nice -n 10`, `export PATH="$HOME/.cargo/bin:$PATH"`. Pure crates first; for one GUI
test keep the workspace selection (`cargo test --workspace --test <name>`). Do not commit or stage, do not touch the
untracked `assets/images/*` and `assets/objects/`, do not edit `CLAUDE.md`. Report what you did not verify.
