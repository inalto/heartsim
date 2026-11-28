# Patient cases

A patient case is a short story told in stages. Each stage gives the history and the observations, sets the simulator up (a
rhythm, a drug, a shock), and asks multiple-choice questions with an explanation; a score closes the case. The cases are for
education only and **no clinician has reviewed them** (see the [disclaimer](disclaimer.md) and the
[known issues](../known-issues.md)). Where the simulator cannot show something (a drug it does not have, a pacemaker that cannot
be reprogrammed, an arrest that starts from an infarct), the text says so and tells it instead. A simulated shock is drawn from a
seeded random generator and can fail; every case that shocks loads the rhythm that follows on the next page, so the story holds.

Every case is a **synthetic** story: the patients are invented, never a real person's record. Whoever writes and shares a case
file is responsible for keeping it that way.

## The Cases page

**Cases** is a page of its own in the *Learn* group, after Tutorials (it has no number key). It lists the cases in three sections:

* **Built-in**: the cases that come with heartsim.
* **My cases**: case files you opened, or built-in cases you saved as a file. They stay there between sessions (see below).
* **Online**: a place for a library of cases to import from the web; it is not there yet.

Each case is a card with its title, the patient line and a summary, the number of stages and questions, and your best score in this
session (scores are not kept after you close heartsim). A case from a file also shows its authors, its licence (or "not stated")
and where it came from. **Start** opens the case in the reader panel on the right, which stays open while you move between pages,
as for tutorials. **Save as…** writes the case as a file. A case under My cases also has **Remove**, which asks first.

A file whose case has the same id as a built-in case is listed under My cases with "(file)" after its title, and keeps its own score.

## Opening a case file

A case file has the extension `.hs` (for example `syncope.hs`). To open one:

* **Desktop**: **Open case file…** on the Cases page opens your system's file dialog; or drop the `.hs` file on the heartsim window,
  on any page (heartsim switches to the Cases page).
* **Browser**: **Open case file…** shows a small panel with your browser's file picker; choose the file and confirm. Dropping the
  file on the page works too.

heartsim checks the file before anything is shown (see [what a case file may contain](#what-a-case-file-may-contain)). A file it
refuses is named on the page with the reason, for example "Could not open notes.hs: not a case file", "the case needs the rhythm
`…`, which this heartsim does not have" or "this is a case file of version 2; … update heartsim". Nothing of a refused file is kept.

A case that brings its own rhythms adds them to the rhythm picker for the session, under the heading **From cases**. These rhythms
never appear in the Rhythm Quiz, the Interval Drill or the Auscultation Quiz, which keep to the built-in library.

## Saving a case file

**Save as…** on any card writes the case as a `.hs` file:

* **Desktop**: your system's save dialog opens with the name `<case id>.hs`.
* **Browser**: a small panel offers the file as a download; your browser saves it where it saves downloads.

A built-in case saved this way is a template: the file names "heartsim authors" and the licence of the shipped texts (MIT OR
Apache-2.0), and the saved case joins My cases. A teacher can unzip it, change the text (see below) and hand it out.

## Where My cases are kept

* **Desktop**: one file per case, `<case id>.hs`, in the `cases` folder of heartsim's user data folder (next to the saved
  scenarios): `~/.local/share/heartsim/cases` on Linux, `~/Library/Application Support/heartsim/cases` on macOS,
  `%APPDATA%\heartsim\data\cases` on Windows. Remove deletes the file there.
* **Browser**: in the browser's storage for the heartsim page, at most 1 MiB in all; when a file does not fit, the page says so and
  the case stays open for the session only. Clearing the site's data in the browser removes them.

## What a case file may contain

A `.hs` file is an ordinary zip archive, so any archive tool can open it. It holds:

* `heartsim-case.ron`: what the file is (format version, case id, title, language, authors, licence, date);
* `case.ron`: the case itself, in the same form as the files under `assets/cases/`;
* `scenarios/<id>.ron`: rhythms the case needs that heartsim does not have built in (optional);
* `README.txt`: a note for whoever unzips it (ignored when the file is opened).

Nothing else: no pictures, sounds, programs or links that open by themselves. heartsim refuses a file with any other entry, a file
larger than 2 MiB, or one that unpacks to more than 4 MiB, and it never writes the contents of a case file anywhere except, as
a whole, into My cases. A link in a case's text is shown as a link and opens only if you click it; images and HTML in the text are
not shown. The full rules are in the [case file format](https://github.com/inalto/heartsim/blob/master/docs/case-file.md).

> **A case file from someone else is teaching text that nobody at heartsim has reviewed.** It can be wrong, out of date or
> misleading. Treat it as you would a handout from a colleague, not as reference material.

To change a case by hand: unzip the file, edit `case.ron` (the format is described in `crates/heartsim-core/src/case.rs` and
shown by the files in `assets/cases/`), and pack it again with `heartsim-cli case pack` (see [the command
line](cli.md#case-files)), which also checks it. A zip made by hand with an archive tool works as long as `heartsim-case.ron` is
its first entry.

## The built-in cases

The nine cases, in the order of the list, with no spoilers:

* **A fast, wide rhythm in an older man** — an older man with an old heart attack and sudden palpitations has a regular rhythm with
  wide complexes.
* **Crushing pain, then a collapse** — a man in his sixties arrives with severe chest pain; read the first ECG and follow the
  heart to the next day.
* **A pulse stuck at 150** — a woman short of breath has a regular narrow tachycardia that does not change with rest.
* **Chest pain on the golf course** — a middle-aged man with crushing chest pain: read the ST segments and see what
  reperfusion changes.
* **Dizzy weeks after a pacemaker** — a woman who depends on her pacemaker becomes dizzy.
* **A racing heart in a student** — a fit young man has sudden fast palpitations that stop abruptly.
* **Sharp chest pain after a cold** — a young woman with chest pain has ST elevation on her ECG.
* **A faint after three days of vomiting** — an older woman with a stomach bug and a slow pulse faints on the ward.
* **Collapse in the supermarket** — an older woman faints twice in a week; follow her ECG from a quiet baseline to the rhythm that explains it.

The files are `assets/cases/case_*.ron`; `heartsim-cli validate-cases assets/cases` checks them (see [the command line](cli.md)).
