# The `.hs` patient case file

A `.hs` file is one heartsim patient case in one file: the story, its questions, and any rhythm the case needs that heartsim
does not already have. A teacher writes a case, saves it, and hands the file to students; they open it on the **Cases** page of
the desktop or the browser version.

This page describes version 1 of the format, as heartsim 0.26.0 and later read and write it. The reference implementation is
the crate `crates/heartsim-casefile`; the developer document with every rule's constant and test is
[`docs/case-file.md`](../case-file.md).

> Cases are **synthetic teaching cases**. A case file must never contain a real person's record. Whoever writes and shares a
> file is responsible for its content and its licence; heartsim only reviews the cases it ships.

**Contents**

1. [At a glance](#at-a-glance)
2. [Three ways to make a case file](#three-ways-to-make-a-case-file)
3. [The container](#the-container)
4. [The manifest: `heartsim-case.ron`](#the-manifest-heartsim-caseron)
5. [The case: `case.ron`](#the-case-caseron)
6. [Actions](#actions)
7. [Bundled rhythms: `scenarios/<id>.ron`](#bundled-rhythms-scenariosidron)
8. [What the reader checks](#what-the-reader-checks)
9. [What the writer guarantees](#what-the-writer-guarantees)
10. [Versions](#versions)
11. [Online libraries](#online-libraries)
12. [The command line](#the-command-line)
13. [A worked example by hand](#a-worked-example-by-hand)
14. [Questions people ask](#questions-people-ask)

## At a glance

| | |
|---|---|
| Extension | `.hs` |
| Media type | `application/vnd.heartsim.case+zip` |
| Container | a zip archive; entries stored or deflated |
| Text encoding | UTF-8, in [RON](https://github.com/ron-rs/ron) (Rusty Object Notation) |
| Format version | 1 |
| Largest file | 2 MiB |
| Opens in | heartsim desktop and browser versions (Cases page), `heartsim-cli case` |

A case file holds exactly these entries and nothing else:

| Entry | Required | Content |
|---|---|---|
| `heartsim-case.ron` | yes, and it must be the **first** entry | the manifest: what the file is, who wrote it, its licence |
| `case.ron` | yes | the case: stages, vignettes, actions, questions |
| `scenarios/<id>.ron` | no | a rhythm the case loads that is not built into heartsim |
| `README.txt` | no | a note for a person who unzips the file; ignored on open |

Images, audio, scripts or any other entry make the reader refuse the file and name the entry.

## Three ways to make a case file

1. **In the application.** On the Cases page, **New case** (or **Edit a copy** on any card) opens the case editor. Fill in the
   form, press **Try it** to play the draft, then **Save as…**. No knowledge of this format is needed. The user guide's chapter
   "Patient cases" walks through it.
2. **From a `case.ron` with the command line.** Write the case as text (see [below](#a-worked-example-by-hand)) and run
   `heartsim-cli case pack`.
3. **By hand with a zip tool.** Possible, since the container is a plain zip, but the manifest must be the first entry and the
   names must be exact. `heartsim-cli case check` tells you whether heartsim will accept the result.

Any built-in case can also be saved as a file with **Save as…** on its card: a good starting point to see what a complete file
looks like (`heartsim-cli case unpack` takes it apart).

## The container

* A zip archive. Each entry is stored or deflated; any other compression method, and any encrypted entry, is refused.
* At most 16 entries. Each entry unpacks to at most 1 MiB and all entries together to at most 4 MiB. The sizes are counted
  while unpacking; the sizes written in the zip headers are not trusted.
* Entry names are compared exactly, with `/` as the separator. A name with a leading `/`, a `\`, a `:`, a `.` or `..` part or a
  NUL character is refused, as is a name that appears twice.
* A `scenarios/` folder entry, which archive tools add, is accepted and skipped.
* Every text entry is UTF-8 without NUL characters.

## The manifest: `heartsim-case.ron`

```ron
CaseFile(
    format_version: 1,
    kind: "heartsim-case",
    id: "case_syncope_heart_block",
    title: "Collapse in the supermarket",
    language: "en",
    authors: ["heartsim authors"],
    licence: "MIT OR Apache-2.0",
    created: "2026-11-27",
    generator: "heartsim 0.26.0",
    scenarios: [],
)
```

| Field | Required | Rule |
|---|---|---|
| `format_version` | yes | `1`. A larger number is refused with a message to update heartsim. |
| `kind` | yes | always `"heartsim-case"` |
| `id` | yes | 1 to 64 characters of `a-z`, `0-9` and `_`; must equal the `id` in `case.ron` |
| `title` | yes | at most 200 characters |
| `language` | yes | a BCP 47 tag such as `en` or `pt-BR`: letters, digits and `-`, at most 35 characters |
| `authors` | no | a list of free text, at most 32 entries of at most 200 characters each |
| `licence` | no | an SPDX id (`CC-BY-4.0`) or free text, at most 200 characters; empty means "not stated" |
| `created` | no | an ISO date, for information only, at most 32 characters |
| `generator` | no | the program that wrote the file, at most 200 characters |
| `scenarios` | no | the ids of the bundled rhythms; must match the `scenarios/<id>.ron` entries exactly, none twice |

A field that is not in this table is an error.

The `id` identifies the case: heartsim keeps one case per id in **My cases** and keys the saved score by it. Opening a file
with the id of a case you already have replaces that case.

## The case: `case.ron`

The same text as the cases heartsim ships in `assets/cases/`.

```ron
Case(
    format_version: 1,
    id: "case_syncope_heart_block",
    title: "Collapse in the supermarket",
    summary: "An older woman faints twice in a week. …",
    patient: "74-year-old woman, two episodes of sudden loss of consciousness in a week, …",
    references: [
        "Kusumoto FM et al. 2018 ACC/AHA/HRS guideline on … bradycardia and cardiac conduction delay. Circulation 2019;140:e382-e482.",
    ],
    stages: [
        Stage(
            title: "The first ECG",
            markdown: r#"
**Mrs Rossi** is brought to the emergency department after fainting in a supermarket. …
"#,
            actions: [
                Ui(LoadScenario("bifascicular")),
                Ui(FocusPanel(TwelveLead)),
            ],
            questions: [
                Question(
                    prompt: "What conduction abnormality does the ECG show?",
                    choices: [
                        "Right bundle-branch block with left anterior fascicular block (bifascicular block)",
                        "Left bundle-branch block",
                        "First-degree AV block only",
                        "A normal ECG for her age",
                    ],
                    answer: 0,
                    explanation: "A wide QRS with an rSR' in V1 is right bundle-branch block. …",
                ),
            ],
        ),
    ],
)
```

**`Case`**

| Field | Content |
|---|---|
| `format_version` | `1` (the version of the case text, separate from the container's) |
| `id` | as in the manifest |
| `title` | shown on the card and in the reader |
| `summary` | one or two sentences for the list, without giving the diagnosis away |
| `patient` | one line: age, sex, setting, presenting complaint |
| `references` | the literature the clinical statements were checked against, one citation per entry |
| `stages` | the story, in order; at least one |

**`Stage`**

| Field | Content |
|---|---|
| `title` | short, for the stage index |
| `markdown` | the vignette in CommonMark: what the clinician knows and sees at this point |
| `actions` | what the simulator does when the stage opens, in order (optional) |
| `questions` | asked in order; each is answered once, then the explanation is shown (optional) |

**`Question`**

| Field | Content |
|---|---|
| `prompt` | the question |
| `choices` | the answers offered |
| `answer` | the position of the right one in `choices`, counted from 0 |
| `explanation` | shown once answered (CommonMark) |

**What makes a case unreadable.** A file is refused when the case has no stages, or a question has no choices or an `answer`
that points outside its choices.

**What is only advice.** The cases heartsim ships also keep these editorial rules, which `heartsim-cli case check` and the case
editor report as warnings for your own cases:

* title, summary, patient line, every stage title and every vignette are filled in;
* there is at least one reference;
* the first stage loads a rhythm;
* at least five questions in all, each with three to five different choices, a prompt and an explanation.

Vary the position of the right answer: a case whose right answer is always the first teaches the position, not the ECG.

**Markdown.** Text is shown as CommonMark. Raw HTML is not rendered as HTML. An image link shows as an image that did not load:
heartsim fetches nothing for a case. A link is drawn as a link and opens in the browser only when the reader clicks it.

## Actions

An action is one line of RON. `Ui(…)` drives the application; `Sim(…)` is an intervention on the running simulation, in the
same syntax as the scenario timeline editor.

| Action | What it does |
|---|---|
| `Ui(LoadScenario("sinus_normal"))` | load a rhythm by id (built in, or bundled in the file) |
| `Ui(StartReentry("af_persistent"))` | start a re-entry scenario by id |
| `Ui(FocusPanel(TwelveLead))` | show a page |
| `Ui(SelectLead(II))` | select an ECG lead |
| `Ui(WaitForEvent(ElapsedMs(3000.0)))`, `Ui(WaitForEvent(Qrs))` | wait before the next action |
| `Ui(Pause)`, `Ui(Resume)`, `Ui(SetSpeed(0.5))` | control the run |
| `Ui(Highlight("pause"))` | point at a control |
| `Ui(CameraPreset(Lao))` | set the 3-D view |
| `Ui(SetQrsAxis(60.0))` | set the axis on the axis pages |
| `Ui(HeartSoundAudio(true))`, `Ui(HeartSoundLane(true))`, `Ui(SoundCheck)` | heart sounds |
| `Ui(Listen(site: Mitral, chest: Diaphragm))` | place the stethoscope |
| `Ui(ProbeAtWindow(Apical))`, `Ui(ProbeAtView("a4c"))`, `Ui(SetEchoView("plax"))`, `Ui(EchoMeasure("lvid"))` | echo page |
| `Sim(Block(region: AvNodeCompact, blocked: true))` | an intervention: a block, a drug, a pacing stimulus, a shock, … |

The case editor's **Other action** list inserts a complete, valid example of every kind, including each `Sim(…)` intervention,
so you never have to remember the spelling. The ids of the built-in rhythms are the file names in `assets/scenarios/`; the
editor's **Load rhythm** helper lists them.

A case that changes the rhythm during a stage reads best when it acts on the running simulation (a block, a drug, a shock)
rather than loading another rhythm, and says in the vignette what was done.

## Bundled rhythms: `scenarios/<id>.ron`

A case usually loads rhythms that every copy of heartsim has. When it loads one of your own, the file carries it:

* the entry is `scenarios/<id>.ron`, a scenario file as heartsim's scenario editor saves it;
* `<id>` is 1 to 64 characters of `a-z`, `0-9` and `_`, and the scenario's own `id` must be the same;
* the manifest's `scenarios` list names it;
* the id must not be the id of a built-in rhythm: a case file can add a rhythm, never replace one.

In the application, a bundled rhythm is added for the session, listed under **From cases** in the rhythm picker, and never
drawn by a quiz or a drill. A file is also refused when it would change a rhythm that another open case, or you in this
session, use under the same id.

A file whose case loads a rhythm that is neither bundled nor built in is refused with a message that names the rhythm.

## What the reader checks

A case file comes from someone else, so heartsim treats it as hostile input. Everything is read in memory; nothing is ever
written to disk under a name taken from the archive.

| Check | Limit |
|---|---|
| File size | at most 2 MiB |
| Number of entries | at most 16 |
| One unpacked entry | at most 1 MiB, counted while unpacking |
| All unpacked entries | at most 4 MiB |
| Compression | stored or deflate; never encrypted |
| Entry names | safe, unique, and only the four kinds above |
| Entry order | the manifest first |
| Text | UTF-8, no NUL; RON nesting at most 128 levels deep |
| Manifest | the fields and limits of the table above; no unknown field |
| Case | parses, has stages, every answer points at a choice, its id equals the manifest's |
| Rhythms | each listed one is present and each present one is listed; each parses and has the id of its entry |
| Against the running heartsim | no bundled rhythm has a built-in id; every loaded rhythm exists |

Each refusal has its own message. The reader is tested with random and deliberately damaged files, including a zip bomb, and
must return an error without a crash and without a large allocation.

## What the writer guarantees

* **The same case gives the same bytes.** Entry order is fixed (manifest, case, rhythms in id order, README), every timestamp
  is the zip epoch (1980-01-01 00:00), permissions are 0644, compression is deflate. So the SHA-256 of a file identifies it.
* **The writer never makes a file the reader refuses**: it reads its own output back before returning it.
* A file opened and saved again unchanged is the same file, byte for byte.

## Versions

* The manifest's `format_version` is the version of the container. A reader refuses a newer version and says to update
  heartsim; it looks at `format_version` and `kind` before anything else, so the message is that one and not a parse error.
* Within version 1 nothing may be added: an unknown entry or manifest field is an error. Anything new (images for a vignette,
  audio, translations, signatures) will come with a version 2 that names it and its limits.
* A later reader will keep reading version 1 files.
* `case.ron` has its own `format_version` for the case text.

## Online libraries

A library is a folder of case files on a web server with a catalogue beside it. The Cases page lists a library and imports a
case with a click. Nothing is requested until you press **Browse the online library** or **Import**.

```
<base>/index.ron        the catalogue
<base>/<file>.hs        one case file per entry
```

```ron
CaseIndex(
    format_version: 1,
    title: "heartsim case library",
    updated: "2026-11-27",
    cases: [
        CaseEntry(
            id: "case_syncope_heart_block",
            title: "Collapse in the supermarket",
            summary: "An older woman faints twice in a week. …",
            language: "en",
            authors: ["heartsim authors"],
            licence: "MIT OR Apache-2.0",
            file: "case_syncope_heart_block.hs",
            size_bytes: 3839,
            sha256: "be00f9daa5fc2a8b0075d8f2e7058620a9152518ac392fb5253c2b5a397662a3",
            case_format_version: 1,
            min_app_version: Some("0.27.0"),
        ),
    ],
)
```

| Field of an entry | Rule |
|---|---|
| `id` | as a case id; the downloaded file's id must equal it |
| `title` | not empty, at most 200 characters |
| `summary` | optional, at most 1000 characters |
| `language`, `authors`, `licence` | as in the manifest; `authors` and `licence` optional |
| `file` | a bare file name ending in `.hs`: letters, digits, `.`, `_`, `-`; no `/`, no `..`, no address |
| `size_bytes` | the exact size of the file, 1 byte to 2 MiB |
| `sha256` | the file's SHA-256, 64 lower-case hex characters |
| `case_format_version` | the `format_version` of the case inside |
| `min_app_version` | optional, `MAJOR.MINOR.PATCH`: the oldest heartsim that can play the case |

**Limits on the index.** At most 256 KiB and 500 entries; the library title at most 200 characters; a newer `format_version`
is refused. An entry that breaks a rule, or repeats an earlier id, is dropped and counted ("3 entries ignored"); the rest of
the index is still shown.

**Importing.** The address of a case is the library's address plus the bare file name, never an address taken from the index.
The download must have exactly the listed size and SHA-256, pass every check of [the reader](#what-the-reader-checks), and
carry the listed id. Only then does it join My cases, marked "from <host>". A case you already imported shows as
**Imported**; when the library has a different file under the same id, the page offers **Update…** and replaces yours only
when you confirm.

**Addresses.** `https://host/path/`. Plain `http://` works only for `localhost` and `127.0.0.1`, for testing. The official
library is `https://inalto.github.io/heartsim/cases/`, published with the guide and the browser version.

**What the digest does and does not protect.** The SHA-256 catches a damaged or swapped file, given an honest index. It does
not make a library trustworthy: whoever controls the web site controls the index. Version 1 has no signatures. Every imported
file still passes the same reader as a file opened from disk, and an index cannot loosen any limit.

**Hosting your own library** on GitHub Pages:

1. Make each file (**Save as…** in heartsim, or `heartsim-cli case pack`) and check it with `heartsim-cli case check`.
2. Write `index.ron`. `case pack` prints each file's size and SHA-256; `stat -c %s file.hs` and `sha256sum file.hs` give
   the same numbers.
3. Commit `index.ron` and the `.hs` files in one folder of a repository, for example `docs/cases/`.
4. In **Settings → Pages**, publish that branch and folder. The library's address is then
   `https://<user>.github.io/<repository>/cases/`.
5. In heartsim, type the address in **Library address** and press **Browse the online library**.

A repository's raw files (`https://raw.githubusercontent.com/<user>/<repository>/<branch>/<folder>/`) work without Pages. Any
other web server works for the desktop version; for the browser version it must also send
`Access-Control-Allow-Origin: *` (GitHub Pages and raw files already do).

To update a case, replace its file and correct `size_bytes` and `sha256` in the index; keep the `id`.

## The command line

```bash
heartsim-cli case pack my_case.ron -o my_case.hs --author "A. Teacher" --licence "CC-BY-4.0"
```

Makes a case file. `--scenario my_rhythm.ron` (repeatable) bundles a rhythm; `--created 2026-11-27` fixes the date so that
the same input gives the same file on any day. It prints the file's size and SHA-256.

```bash
heartsim-cli case check my_case.hs
```

Says whether heartsim will open the file (it fails when the application would refuse it), then lists the editorial advice and
runs every stage's actions without a window, as warnings.

```bash
heartsim-cli case unpack my_case.hs -o my_case
```

Writes the manifest, the case and the bundled rhythms into a folder, under the allowed names only.

```bash
cargo xtask case-library --cases my_cases --out site/cases
```

From a heartsim checkout: packs every `*.ron` case of a folder and writes the `index.ron` of a library.

## A worked example by hand

Save this as `case_first_block.ron`. It is the smallest useful case: one stage, one built-in rhythm, one question.

```ron
Case(
    format_version: 1,
    id: "case_first_block",
    title: "A long PR interval",
    summary: "A routine ECG before minor surgery. Measure before you decide.",
    patient: "58-year-old man, no symptoms, ECG taken before a hernia repair.",
    references: [
        "Kusumoto FM et al. 2018 ACC/AHA/HRS guideline on the evaluation and management of patients with bradycardia and cardiac conduction delay. Circulation 2019;140:e382-e482.",
    ],
    stages: [
        Stage(
            title: "The pre-operative ECG",
            markdown: "He feels well and takes no medicines. Look at lead II and measure the **PR interval**.",
            actions: [
                Ui(LoadScenario("avb_1st")),
                Ui(SelectLead(II)),
            ],
            questions: [
                Question(
                    prompt: "What does the ECG show?",
                    choices: [
                        "A normal PR interval",
                        "Second-degree AV block, Mobitz I",
                        "First-degree AV block",
                    ],
                    answer: 2,
                    explanation: "Every P wave is followed by a QRS, and the PR interval is constant and longer than 200 ms: first-degree AV block.",
                ),
            ],
        ),
    ],
)
```

Pack it and check it:

```bash
heartsim-cli case pack case_first_block.ron -o case_first_block.hs --author "A. Teacher" --licence "CC-BY-4.0"
```

```bash
heartsim-cli case check case_first_block.hs
```

`check` answers `OK, case file version 1: case_first_block "A long PR interval" (en), 1 stages, 1 questions, 0 bundled rhythms`
and one warning, `1 questions (need ≥ 5)`: advice for a case worth sharing, not an error. Open `case_first_block.hs` on the Cases page, or drop it on the
window.

## Questions people ask

**Can I open a `.hs` file with an archive tool?** Yes. It is a zip; rename it to `.zip` if your tool insists. `README.txt`
inside says what the file is.

**Can I edit the text inside and zip it again?** Yes, if the manifest stays the first entry and no other file is added.
`heartsim-cli case unpack`, edit, `case pack` is the safer route.

**Why is my file refused with "not part of a version-1 case file"?** The archive holds an entry the format does not allow.
macOS's Finder adds `__MACOSX/` and `.DS_Store` entries when it compresses a folder; use `heartsim-cli case pack` instead.

**Where does heartsim keep the cases I opened?** On the desktop, as `<id>.hs` files in the `cases` folder of heartsim's user
data folder (the Cases page shows the path). In the browser, in the site's local storage, up to 1 MiB for all cases together;
the page says when a file does not fit.

**Does a case file run code?** No. It is text that the simulator reads: a story, questions, and a list of actions from a fixed
vocabulary. It cannot read files, open addresses or start programs.

**Which licence should I put?** Yours to choose. heartsim's own cases are MIT OR Apache-2.0. For teaching material, CC BY 4.0
is a common choice. Cite your sources in `references` and write the text in your own words: a licence on your file does not
cover text or figures copied from a paper.

**`.hs` is also the extension of Haskell source files. Is that a problem?** Only for file associations: an editor may offer to
open a case file as source code. heartsim picks case files by the extension in its dialogs and on a drop, then decides by the
content: a `.hs` file that is not a case file is refused with a message, and nothing happens to it.
