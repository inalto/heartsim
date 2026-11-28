# The heartsim case file (`.hs`)

A case file carries one patient case and the rhythms it needs, so a teacher can hand a case to a student as one file. The
application opens and saves it on the Cases page (desktop and browser, see the user guide's
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

## Where the code is

* `crates/heartsim-casefile`: the format (pure: `heartsim-core`, `heartsim-ecg`, `ron`, `serde`, `zip`, `sha2`, `thiserror`; no
  file system, no clock, no GUI; builds for `wasm32-unknown-unknown`).
* `crates/heartsim-app/src/case_io.rs`: the platform part (file dialogs with `rfd`, dropped files, where My cases are kept).
* `crates/heartsim-app/src/case_ui.rs`: the Cases page, `Cases::open_bytes` (open a package from any source, with a provenance
  label), Save as…, Remove.
* `crates/heartsim-cli/src/casefile.rs`: `case pack | unpack | check`.
