# The heartsim case file (`.hs`)

A case file carries one patient case and the rhythms it needs, so a teacher can hand a case to a student as one file. The
application opens and saves it on the Cases page and imports it from an online library ([Libraries](#libraries)) (desktop and browser, see the user guide's
[Patient cases](book/src/user/cases.md)); `heartsim-cli case pack | unpack | check` makes, takes apart and checks one. Both use
the crate `crates/heartsim-casefile` (brief B41): the strict reader `CasePackage::read` and the deterministic writer
`CasePackage::write`. This document is the format; the crate is the reference implementation, and every limit below is a
`pub const` there with a test in `crates/heartsim-casefile/tests/`.

Cases are **synthetic teaching cases**, never a real person's record. Whoever writes and shares a case file is responsible for
its content, for keeping it synthetic, and for its licence; heartsim does not review files it did not ship.

## Container

| | |
|---|---|
| container | a zip archive, entries stored or deflated |
| extension | `.hs` (`heartsim_casefile::EXTENSION`) |
| media type | `application/vnd.heartsim.case+zip` (`MEDIA_TYPE`) |
| format version | 1 (`FORMAT_VERSION`) |

Why a zip: one file to hand out; `zip` 9 with its pure-Rust deflate was already a dependency of the desktop and the browser
build (no new crate, nothing new for `cargo deny`); any archive tool opens a zip, so a case stays readable and editable by hand;
entries are named, so a later version can add files without breaking the readers of this one. A gzip of a single RON file was
considered and rejected (no room for bundled rhythms or later additions); a tar is not seekable and needs another crate.

## Entries

Names are UTF-8 with `/` separators; no directory entries are needed (a `scenarios/` directory entry, which archive tools add, is
accepted and skipped).

| entry | required | content |
|---|---|---|
| `heartsim-case.ron` | yes, **the first entry** | the manifest, below |
| `case.ron` | yes | a `heartsim_core::case::Case`, exactly as the files in `assets/cases/` (`Case(format_version: 1, id: …)`) |
| `scenarios/<id>.ron` | no | a rhythm the case loads that is not in the built-in library: a scenario file as in `assets/scenarios/` (`Tier0Scenario`), whose own `id` is `<id>` |
| `README.txt` | no | written by the writer for a person who unzips the file; its content is ignored on open |

Any other entry is refused in version 1, and the error names it: images, audio, scripts or anything else cannot ride along
unnoticed.

## Manifest

`heartsim-case.ron`, RON with struct names:

```ron
CaseFile(
    format_version: 1,                 // of the container; a newer one is refused with "update heartsim"
    kind: "heartsim-case",             // constant; anything else is not a case file
    id: "case_syncope_heart_block",    // equals Case.id; [a-z0-9_]{1,64}
    title: "Collapse in the supermarket",
    language: "en",                    // BCP 47 tag: letters, digits and '-'
    authors: ["heartsim authors"],     // free text, may be empty
    licence: "MIT OR Apache-2.0",      // SPDX id or free text; shown on the card; empty = "not stated"
    created: "2026-11-27",             // ISO date, informational
    generator: "heartsim 0.26.0",
    scenarios: [],                     // ids of the bundled scenarios; must match the scenarios/ entries exactly
)
```

`authors`, `licence`, `created`, `generator` and `scenarios` may be left out (they default to empty). An unknown field is an error
in version 1.

## Reader rules

A case file comes from someone else, so the reader treats it as hostile input. Everything is parsed in memory; nothing is ever
written to disk under a name from the archive (the application keeps the file as a whole, under the case id, in My cases).

| rule | limit / check | constant | error (`CaseFileError::…`) |
|---|---|---|---|
| file size | at most 2 MiB | `MAX_FILE_BYTES` | `FileTooLarge` |
| a readable zip | the central directory parses | | `NotZip` |
| entries | at most 16 | `MAX_ENTRIES` | `TooManyEntries` |
| unpacked entry | at most 1 MiB, **counted while inflating** (`Read::take`; the header's size is never trusted) | `MAX_ENTRY_BYTES` | `EntryTooLarge` |
| unpacked total | at most 4 MiB, counted the same way | `MAX_TOTAL_BYTES` | `TotalTooLarge` |
| compression | stored or deflate only | | `UnsupportedMethod` |
| encryption | none | | `Encrypted` |
| names | `enclosed_name()` is `Some`; no leading `/`, no `\`, no `:`, no `.` or `..` component, no NUL | | `UnsafeName` |
| names | no name twice | | `DuplicateName` |
| names | exactly one of the names above, compared as strings | | `UnknownEntry` |
| names | `<id>` of `scenarios/<id>.ron` matches `[a-z0-9_]{1,64}` | `MAX_ID_LEN` | `BadScenarioId` |
| order | the manifest is the first entry | | `ManifestNotFirst` |
| required | the manifest and `case.ron` exist | | `MissingEntry` |
| inflating | the data decompresses (a data stream longer than its header says is refused by the zip reader) | | `Corrupt` |
| text | UTF-8 | | `NotUtf8` |
| text | no NUL character | | `Nul` |
| manifest | parses (RON, strict) | | `Manifest` |
| manifest | `kind` is `heartsim-case` | `KIND` | `WrongKind` |
| manifest | `format_version` not newer than 1 | `FORMAT_VERSION` | `NewerVersion` |
| manifest | `id` matches `[a-z0-9_]{1,64}` | `MAX_ID_LEN` | `BadId` |
| manifest | language tag of letters, digits and `-`, at most 35 characters | `MAX_LANGUAGE_CHARS` | `BadLanguage` |
| manifest | title, each author, licence, generator at most 200 characters; at most 32 authors; `created` at most 32 | `MAX_FIELD_CHARS`, `MAX_AUTHORS`, `MAX_DATE_CHARS` | `FieldTooLong` |
| case | parses with `Case::from_ron` (format version, structure) and `Case::problems` is empty | | `Case` |
| case | `Case.id` equals the manifest's `id` | | `IdMismatch` |
| scenarios | the manifest lists no id twice | | `ScenarioListedTwice` |
| scenarios | every listed id has its entry | | `ScenarioMissing` |
| scenarios | every entry is listed | | `ScenarioNotListed` |
| scenarios | each parses with `Tier0Scenario::from_ron` | | `Scenario` |
| scenarios | the scenario's own id equals its entry name | | `ScenarioIdMismatch` |

Two more rules depend on the running build and are checked by `CasePackage::resolve` (the application and `case check` always
apply both):

| rule | error |
|---|---|
| a bundled rhythm never replaces a built-in one: an id clash refuses the file | `ScenarioClash` |
| every rhythm the case loads (`LoadScenario`) is bundled or built in; the message names each missing one ("needs the rhythm `x`") | `NeedsRhythm` |

**RON nesting.** `ron` 0.12 parses with a recursion limit of 128 by default (`ron::Options::default().recursion_limit`), and the
reader uses the default: a deeply nested value is a parse error, not a stack overflow (test `ron_nesting_is_bounded`).

**Robustness.** A `proptest` feeds arbitrary bytes, randomly mutated valid files and mutated zip headers to the reader: it returns
`Ok` or `Err`, never panics, and no single allocation of the test process exceeds twice the entry limit plus the input limit (a
counting allocator; a reader that trusted a size from a header would show as one huge allocation). A 64 MiB zip bomb in a file of a
few kilobytes is refused within the same bound.

**Markdown.** The case text (vignettes, explanations) is rendered by `egui_commonmark` with only its `pulldown_cmark` feature (the
root `Cargo.toml`): no `embedded_image`, no syntax highlighting, and no HTML callback, so raw HTML is not rendered as HTML. An
image (`![](…)`) is given to egui's image loaders, and the application installs none that read files or the network (no
`egui_extras::install_image_loaders`, no `http` or `file` loader), so it shows as an unloaded image and nothing is fetched. A link
is drawn as a hyperlink and opens in the browser **only when the user clicks it**; nothing in a case can open a URL by itself.

## Writer rules

`CasePackage::write` is deterministic, like the share bundle of `heartsim-io`: the manifest first, then `case.ron`, the bundled
scenarios in id order, then `README.txt`; every timestamp the zip epoch (1980-01-01 00:00); permissions 0644; deflate. The
manifest's `scenarios` list is set from the bundled scenarios, so it always matches. The same package gives the same bytes, so the
SHA-256 of a file (`heartsim_casefile::sha256_hex`) identifies it; a test pins the digest of a fixture
(`deterministic_bytes_and_a_pinned_digest`), so a change of the writer, of `zip` or of the deflate implementation is noticed. The
writer reads its own output back before returning it: it never makes a file the reader refuses, and `write(read(x))` reproduces a
file it made. The case and the manifest are written as `to_ron` writes them (pretty RON with struct names); bundled scenarios are
written as the text they were read from.

## Version policy

* A reader of version *n* refuses a file of a newer version with a message that says to update heartsim (`NewerVersion`); it reads
  `format_version` and `kind` first, so a newer manifest with fields this reader does not know still gets that message.
* Within a version nothing is optional beyond what is listed: an unknown entry or manifest field is an error. New entries (images
  for a vignette, audio, a translation of the text, an author's signature) need a new version, which names them, their limits and
  their rules here; version-1 readers refuse such files rather than ignore part of them.
* A version-2 reader reads version-1 files.
* The case inside has its own `format_version` (`heartsim_core::case::CASE_FORMAT_VERSION`); a case format change that old readers
  cannot read also needs a container version, so that the message is the clear one.

## A worked example made with the CLI

Pack a shipped case as a teacher would, check it, and take it apart again (output of heartsim 0.26.0 in this repository; `--created` fixes the date, so the SHA-256 is the same on every run):

```text
$ heartsim-cli case pack assets/cases/case_syncope_heart_block.ron -o syncope.hs \
      --author "heartsim authors" --licence "MIT OR Apache-2.0" --created 2026-11-27
syncope.hs: case_syncope_heart_block (4 stages, 8 questions, 0 bundled rhythms), 3839 bytes, sha256 be00f9daa5fc2a8b0075d8f2e7058620a9152518ac392fb5253c2b5a397662a3

$ heartsim-cli case check syncope.hs
syncope.hs: OK, case file version 1: case_syncope_heart_block "Collapse in the supermarket" (en), 4 stages, 8 questions, 0 bundled rhythms
  authors: heartsim authors; licence: MIT OR Apache-2.0; created 2026-11-27; made by heartsim 0.26.0; sha256 be00f9daa5fc2a8b0075d8f2e7058620a9152518ac392fb5253c2b5a397662a3
0 warnings

$ heartsim-cli case unpack syncope.hs -o syncope
syncope/heartsim-case.ron
syncope/case.ron
(and an empty syncope/scenarios/: this case bundles no rhythm)

$ cat syncope/heartsim-case.ron
CaseFile(
    format_version: 1,
    kind: "heartsim-case",
    id: "case_syncope_heart_block",
    title: "Collapse in the supermarket",
    language: "en",
    authors: [
        "heartsim authors",
    ],
    licence: "MIT OR Apache-2.0",
    created: "2026-11-27",
    generator: "heartsim 0.26.0",
    scenarios: [],
)

$ unzip -l syncope.hs
Archive:  syncope.hs
  Length      Date    Time    Name
---------  ---------- -----   ----
      323  01-01-1980 00:00   heartsim-case.ron
     9179  01-01-1980 00:00   case.ron
      737  01-01-1980 00:00   README.txt
---------                     -------
    10239                     3 files
```

To bundle a rhythm of your own, give its scenario file with `--scenario my_rhythm.ron` (its `id` must not be a built-in one) and
load it from the case with `Ui(LoadScenario("my_rhythm"))`. `case check` prints the reader's verdict (it fails when the
application would refuse the file), then the editorial rules of the shipped cases (`Case::content_problems`: references, at least
five questions, three to five choices each, an explanation) and a headless run of every stage as warnings.

## Libraries

A **case library** is a directory of case files served over HTTPS with a catalogue beside them (brief B43). The Cases page lists
a library's cases and imports one with a click, on the desktop and in the browser; anyone can host one, since it is plain static
files. The pure part (the index types, the reader, the address rules) is `heartsim_casefile::library`; every limit below is a
`pub const` there with a test in `crates/heartsim-casefile/tests/library.rs`.

```
<base>/index.ron            the catalogue
<base>/<file>.hs            one case file per entry
```

### The index

`index.ron`, RON with struct names (`cargo xtask case-library` writes it pretty-printed):

```ron
CaseIndex(
    format_version: 1,                          // of the index; a newer one is refused with "update heartsim"
    title: "heartsim case library",             // at most 200 characters
    updated: "2026-11-27",                      // ISO date, informational, optional
    cases: [
        CaseEntry(
            id: "case_syncope_heart_block",     // [a-z0-9_]{1,64}; the file's manifest id must equal it
            title: "Collapse in the supermarket",   // at most 200 characters, not empty
            summary: "…",                       // at most 1000 characters, optional
            language: "en",                     // BCP 47 tag
            authors: ["heartsim authors"],      // at most 32, each at most 200 characters, optional
            licence: "MIT OR Apache-2.0",       // at most 200 characters, optional
            file: "case_syncope_heart_block.hs",    // a bare file name: [A-Za-z0-9._-], ends in .hs, no "/", no "..", no scheme
            size_bytes: 3839,                   // exact; at most the case-file limit (2 MiB)
            sha256: "be00f9da…62a3",            // 64 lower-case hex characters
            case_format_version: 1,             // heartsim_core::case::CASE_FORMAT_VERSION of the case inside
            min_app_version: Some("0.27.0"),    // optional, MAJOR.MINOR.PATCH
        ),
    ],
)
```

**Client rules** (the index is untrusted input):

| rule | constant | on breach |
|---|---|---|
| index at most 256 KiB, read through a bounded reader (`take`) | `MAX_INDEX_BYTES` | index refused |
| at most 500 entries | `MAX_INDEX_ENTRIES` | index refused |
| `format_version` read first; newer than 1 | `INDEX_FORMAT_VERSION` | index refused, "update heartsim" |
| strict types, no unknown field (within version 1) | | index refused |
| library title at most 200 characters | `MAX_INDEX_TITLE_CHARS` | index refused |
| entry: id, `file` name, digest, `size_bytes` (1 to `MAX_FILE_BYTES`), text bounds (`MAX_SUMMARY_CHARS` …), `min_app_version` syntax | `check_entry` | entry dropped and counted ("3 entries ignored") |
| a second entry with an earlier entry's id | | entry dropped and counted |

An entry whose `case_format_version` or `min_app_version` is newer than the running build is listed with "Needs a newer
heartsim" and cannot be imported.

**Downloading a case.** The URL is the library's base joined with the entry's bare file name (`LibraryBase::join`), never a URL
taken from the index, so a download never leaves the library directory. The body is read with a cap of `size_bytes`, must have
exactly `size_bytes` bytes and the listed SHA-256, then goes through `CasePackage::read` (and the rhythm check of
`CasePackage::resolve`), and the package's id must equal the entry's id. Only then is it added to My cases, marked "from
<host>". Each failure has its own message, and none adds anything. A case already in My cases with the same digest is shown as
imported; a different digest under the same id is offered as an update that the user confirms, never replaced silently.

**Addresses.** `https://host[:port]/path/` (no user name, query, fragment, `.` or `..` segment or `%` escape); `http://` only for
`localhost` and `127.0.0.1`, for testing. On the desktop a redirect of an https request must stay https; an http (localhost)
request follows none. The browser build's default is the relative `../cases/`, resolved by the browser against the page.

**Transport.** One request at a time, with Cancel. Desktop (`ureq`, cargo feature `online-cases`): a 5 s connect and a 20 s total
timeout, `User-Agent: heartsim/<version>` and no other header, no cookies. Browser (`fetch` in `heartsim-web`): no header added,
`credentials: "omit"`, an `AbortController` for Cancel. Nothing is requested at start: only "Browse the online library" and
"Import" make a request.

### The trust model

The SHA-256 in the index protects against a damaged or swapped case file **given an honest index**; it does not make a library
trustworthy. The index is trusted exactly as much as the HTTPS origin it came from: whoever controls that site (or its account,
or its DNS and certificate) controls which files are listed and what their digests are. Version 1 has no signatures. Every imported
file still goes through the same strict reader as a file opened from disk, and the index cannot relax any limit of the case file.

A version 2 could add a detached signature of the index (for example `index.ron.minisig`, Ed25519 with minisign or signify) and let
the user pin a library's public key the first time they add it, so a compromised web host could no longer list a file the
maintainer did not sign; or a signature inside each case file's manifest, so a file keeps its author's signature when it is passed
on. Either needs key management (publishing, rotating, revoking keys) that version 1 leaves out.

### The official library

`cargo xtask case-library --out <dir>` packs every `assets/cases/*.ron` with `CasePackage::write`, checks that each case resolves
against `assets/scenarios/`, writes `index.ron`, deletes `.hs` files of an earlier run that the index no longer lists, and reads
the result back as a client would (every file through `CasePackage::read`, size and digest against the index). It is
deterministic: every manifest has `created: "2026-11-27"` and `generator: "heartsim case-library"` (fixed, with no clock and no
version number, so a file's digest changes only when its case changes, and a user who imported it is offered an update only
then), authors `heartsim authors`, licence `MIT OR Apache-2.0`; the index's `updated` is the `--updated` argument (empty without
it). Two runs with the same arguments write the same bytes (a test in `xtask/src/case_library.rs`).

`.github/workflows/docs.yml` runs it into `docs/book/book/cases/` (with `--updated` set to the date of the commit) before the
Pages upload, so the library is published with GitHub Pages next to the guide (the site root) and the browser app (`/app/`):

| | address |
|---|---|
| guide | `https://inalto.github.io/heartsim/` |
| browser app | `https://inalto.github.io/heartsim/app/` |
| case library | `https://inalto.github.io/heartsim/cases/` (`case_online::DEFAULT_LIBRARY_URL`; `../cases/` from the browser app) |

### Hosting a library

**On GitHub Pages** (the simplest; Pages sends `Access-Control-Allow-Origin: *`, so the browser version can read it too):

1. Make each case file: in heartsim, **Save as…** on the Cases page, or `heartsim-cli case pack my_case.ron -o my_case.hs
   --author "…" --licence "CC-BY-4.0"`. Check it with `heartsim-cli case check my_case.hs`.
2. Write `index.ron` as above. `heartsim-cli case pack` prints each file's size in bytes and its SHA-256; or, from a shell,
   `stat -c %s my_case.hs` and `sha256sum my_case.hs`. A library of cases in the `assets/cases/` layout can reuse
   `cargo xtask case-library --cases <dir> --out <dir>` from a heartsim checkout.
3. Put `index.ron` and the `.hs` files in one folder of a repository, for example `docs/cases/`, and commit them.
4. In the repository's **Settings → Pages**, publish the branch and folder (for example `main`, `/docs`). The library's address is
   then `https://<user>.github.io/<repository>/cases/`.
5. In heartsim, type that address in **Library address** and press **Browse the online library**.

**From a repository's raw files**: `https://raw.githubusercontent.com/<user>/<repository>/<branch>/<folder>/` serves the same
files with `Access-Control-Allow-Origin: *`; no Pages setup is needed. Pin a tag or a commit instead of a branch if the address
should not change under the readers.

**From any web server** (a school's): serve the folder over HTTPS with the files as they are (any `Content-Type`). For the browser
version, add the header `Access-Control-Allow-Origin: *` (or the origin of the heartsim site) to the responses; without it the
browser refuses to hand the files to the page, and the page says so. The desktop version does not need it.

**Updating a case**: replace its `.hs` file and update `size_bytes` and `sha256` in the index; users who imported the old one see
**Update…**. Keep the `id`: an entry with a new id is a new case.

## Where the code is

* `crates/heartsim-casefile`: the format (pure: `heartsim-core`, `heartsim-ecg`, `ron`, `serde`, `zip`, `sha2`, `thiserror`; no
  file system, no clock, no GUI; builds for `wasm32-unknown-unknown`).
* `crates/heartsim-app/src/case_io.rs`: the platform part (file dialogs with `rfd`, dropped files, where My cases are kept).
* `crates/heartsim-app/src/case_ui.rs`: the Cases page, `Cases::open_bytes` (open a package from any source, with a provenance
  label), Save as…, Remove.
* `crates/heartsim-cli/src/casefile.rs`: `case pack | unpack | check`.
* `crates/heartsim-casefile/src/library.rs`: the library index, its reader and the address rules (B43).
* `crates/heartsim-app/src/case_fetch.rs`: the `CaseFetch` trait, the desktop `ureq` transport, `MemoryFetch` for tests;
  `crates/heartsim-app/src/case_online.rs`: the Online section; `crates/heartsim-web/src/fetch.rs`: the browser `fetch`
  transport; `xtask/src/case_library.rs`: `cargo xtask case-library`.
* `crates/heartsim-app/src/case_edit.rs`, `case_edit_ui.rs`: the case editor (brief B42), which writes these files from a form; see
  "Writing a case" in the user book (`docs/book/src/user/cases.md`).
