# B43 — Importing cases from an online library
**Crates** `heartsim-casefile`, `heartsim-app`, `heartsim-web`, `xtask`, `.github/workflows`, docs · **Model** Opus ·
**Milestone** M13 · **Depends on** B41. Read B41, `docs/case-file.md` and `crates/heartsim-app/src/update_check.rs`
and `update_install.rs` (the existing, reviewed pattern for a network request and a checked download) first.

## Goal
The Cases page can list the cases of an online library and import one with a click, on the desktop and in the
browser. A library is plain static files, so anyone can host one (GitHub Pages, a repository's raw files, a school
web server). Nothing is fetched unless the user asks.

## Design (made; challenge it only with evidence)
**A library is a directory served over HTTPS:**
```
<base>/index.ron            the catalogue
<base>/<id>.hs              one file per case
```
```ron
CaseIndex(
    format_version: 1,
    title: "heartsim case library",
    updated: "2026-11-27",
    cases: [
        CaseEntry(
            id: "case_syncope_heart_block",
            title: "…", summary: "…", language: "en", authors: ["…"], licence: "CC-BY-4.0",
            file: "case_syncope_heart_block.hs",     // relative, a bare file name: no "/", no "..", no scheme
            size_bytes: 5321,
            sha256: "…64 lower-case hex…",
            case_format_version: 1,
            min_app_version: "0.27.0",              // optional
        ),
    ],
)
```
* **The official library is published with the project's GitHub Pages**, next to the book and the browser app:
  `cargo xtask case-library --out <dir>` packs every `assets/cases/*.ron` with `heartsim-casefile` (deterministic)
  and writes `index.ron`; `.github/workflows/docs.yml` runs it into `docs/book/book/cases/` before the Pages
  upload. Default base URL on the desktop: `https://inalto.github.io/heartsim/cases/` (the owner and repository as in
  `update_check::LATEST_RELEASE_URL`; confirm the Pages address from the workflow and the book's `book.toml`
  before hard-coding it). In the browser the app is served from `…/app/`, so the default is the relative
  `../cases/`: same origin, no CORS question.
* **Other libraries**: a "Library address" field on the Cases page (stored in the settings; https only, except
  `http://localhost` and `http://127.0.0.1` for testing). In the browser a foreign library needs
  `Access-Control-Allow-Origin` on its responses: GitHub Pages and `raw.githubusercontent.com` send `*`; when the
  fetch fails with a CORS error the page says so in words ("this server does not allow the web version to read it;
  use the desktop version or download the file and open it").
* **Trust model, to state in the docs and on the page.** The SHA-256 in the index protects against a damaged or
  swapped file *given an honest index*; it does not make the library trustworthy. The index is trusted exactly as
  much as the HTTPS origin it came from. There are no signatures in version 1 (say what a version 2 could add).
  Every imported file goes through the same `CasePackage::read` as a file opened from disk; the index cannot relax
  a limit.

**Client rules (untrusted input; named constants, a test each).**
* Index at most 256 KiB, at most 500 entries, strings bounded (title 200, summary 1000 characters), parsed with a
  bounded reader (`take`), strict types, a newer `format_version` refused with a message. Entries with a bad `file`
  name, a bad digest or a `size_bytes` over the case-file limit are dropped and counted ("3 entries ignored").
* Case download: the URL is `base` joined with the bare file name (never a URL from the index); the body is read
  with a cap of `size_bytes` (and the case-file limit), must have exactly `size_bytes` bytes and the listed
  SHA-256, then `CasePackage::read`, and the package's id must equal the entry's id. Only then is it added to My
  cases (B41's persistence), marked "from <host>".
* No redirects to another scheme or to a non-https host; a 5 s connect and 20 s total timeout on the desktop; one
  request at a time; a Cancel button; errors in words, no panic. No cookies, no identifiers: a
  `User-Agent: heartsim/<version>` on the desktop and nothing added in the browser. Nothing is requested at start:
  only the "Browse the online library" button and "Import" do it. Say this in the privacy note of the book.
* A case already in My cases with the same digest is shown as imported; a different digest under the same id is
  offered as an update, never replaced silently.

**Transport.** A small trait in `heartsim-app` (`CaseFetch`: start a GET with a byte cap, poll for
`Pending / Done(bytes) / Failed(text)`, cancel). Desktop: `ureq` on a thread, as `update_check` does; put it behind
a cargo feature `online-cases` that is on by default with `native` (reuse the optional `ureq` and `sha2`
dependencies; do not tie it to `update-check`). Browser: `fetch` with `web-sys` in `heartsim-web` (an
`AbortController` for cancel; add the `web-sys` features it needs), injected into the app like the Web Audio sink.
Tests use an in-memory implementation of the trait: **no test touches the network.**

## If something blocks
If the browser transport cannot be made to work under `trunk` here, or the Pages layout turns out different, ship
the desktop side and the `xtask` generator with their tests, leave the browser button disabled with an explanation,
and report precisely what blocks and what was tried. Do not create accounts, repositories or tokens; do not push.

## Acceptance tests
* `heartsim-casefile` (or the app's pure module): index round trip; each client rule with a crafted index (too
  large, too many entries, `file: "../x.hs"`, `file: "https://evil/x.hs"`, a 63-character digest, upper-case hex,
  an oversized `size_bytes`, a newer version); URL joining never leaves the base.
* With the in-memory transport: browse lists the entries; import of a good entry lands in My cases and plays;
  wrong length, wrong digest, id mismatch and a file the reader refuses each give their own message and add
  nothing; cancel during the download adds nothing; an already-imported digest is recognised; an http base is
  refused, `http://localhost` accepted.
* `xtask`: `case-library` on `assets/cases/` writes an index whose digests match the files it wrote; two runs are
  byte-identical; every file passes `CasePackage::read`. The workflow change is checked with `actionlint` if it is
  installed (otherwise say it was not linted).
* `egui_kittest`: the Online section shows nothing and makes no request until the button is pressed (the in-memory
  transport counts requests).
* `nice -n 10 cargo deny check`, `nice -n 10 cargo xtask ci`.

## Docs
`docs/case-file.md` (a chapter "Libraries": the index format, hosting one on GitHub Pages or from a repository step
by step, CORS, the trust model), `docs/book/src/user/cases.md` (browse, import, a foreign library, the privacy
note), `docs/web.md`, `docs/ARCHITECTURE.md`, `known-issues.md` (**the official library address does not answer
until the next Pages deploy; no request was made against a real server from this machine** unless you did, and
then say which), `CHANGELOG.md` `## [Unreleased]`.

## Rules
One cargo build at a time, always `nice -n 10`, `export PATH="$HOME/.cargo/bin:$PATH"`. Do not commit, stage or
push, do not edit `CLAUDE.md`. Never send the maintainer's name, e-mail or anything from the repository to an outside
service. Report what you did not verify.
