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
* **Online**: cases to import from a library on the web, heartsim's own or another one (see
  [Importing from an online library](#importing-from-an-online-library)).

Each case is a card with its title, the patient line and a summary, the number of stages and questions, and your best score in this
session (scores are not kept after you close heartsim). A case from a file also shows its authors, its licence (or "not stated")
and where it came from. **Start** opens the case in the reader panel on the right, which stays open while you move between pages,
as for tutorials. **Save as…** writes the case as a file. A case under My cases also has **Remove**, which asks first.

A file whose case has the same id as a built-in case is listed under My cases with "(file)" after its title, and keeps its own score.

## Writing a case

**New case** on the Cases page opens the case editor in place of the list; **Edit a copy** on any card opens a copy of that case
(the case in the list is never changed; a copy of a built-in case gets the id `<id>_copy` and the title "Copy of …"). You need no
text editor and no knowledge of the file format for the story and the questions. The draft is kept in memory while heartsim
runs, also when you leave the page (the list then shows "Draft in progress" with **Continue editing** and **Discard draft**), but
it is not written anywhere until you save it. heartsim does not ask before it closes: the editor and the list say **not saved**
next to the draft's title while it has changes you have not saved.

The page has a header (title, **Back to cases**, **Try it**, **Save as…**), the **problems panel** under it, and the form. The
header and the panel stay in view while the form scrolls. On a narrow window the stage list stacks above the selected stage and
the preview of the text goes under it.

**A worked example, from an empty form to a file.**

1. Press **New case**. Type the **Title**, say "Chest pain at the fair". The **Case id** follows the title
   (`chest_pain_at_the_fair`): lower-case letters, digits and `_`, at most 64. Type in the id field to set it yourself;
   **From the title** gives the automatic id back. The id names the file and keys the score.
2. Fill the **Summary** (one or two sentences, without the diagnosis), the **Patient** line, the **Language**, the **Authors**
   (one per line), the **Licence** (pick an SPDX id from the list or type your own text; leave it empty for "not stated") and the
   **References**, one per line.
3. In **Stage 1** give a title and write the **Vignette** in Markdown: what the clinician knows and sees. The preview beside it
   is what the reader shows.
4. Add the **Actions**: **Load rhythm** inserts a row for the rhythm you pick, **Show panel** and **Select lead** insert theirs,
   **Add a wait** inserts a pause of the given seconds, and **Other action** lists every other kind with a ready example.
5. Add **Questions**: the question text, three to five choices (**Add choice**, **Remove**), the radio button of the right
   one, and an explanation. Questions and stages can be moved up and down, duplicated (stages) and deleted.
6. Add stages with **Add stage** the same way. The case should load a rhythm in its first stage.
7. Press **Try it**: the reader plays the draft from the selected stage; closing it brings you back to the editor with the draft
   as it was. Nothing is saved and no score is kept.
8. When the problems panel shows no **Must fix** line, **Save as…** is on. It writes the `.hs` file and adds the case to My cases.

**The action syntax.** One action per row, written as in the case files and parsed as you type; a row that does not parse is
shown in red with the reason, and so is a `LoadScenario` of a rhythm that does not exist. Examples:

* `Ui(LoadScenario("stemi_inferior"))`: load a rhythm by its id;
* `Ui(FocusPanel(TwelveLead))`: bring a page forward; `Ui(SelectLead(II))`: lead of the single-lead strip;
* `Ui(WaitForEvent(ElapsedMs(3000.0)))`: hold the next rows for 3 s of simulated time; `Ui(WaitForEvent(Qrs))`: until the next QRS;
* `Ui(SetSpeed(0.5))`, `Ui(Pause)`, `Ui(Resume)`, `Ui(CameraPreset(Lao))`, `Ui(SetQrsAxis(60.0))`, `Ui(StartReentry("af_persistent"))`;
* `Sim(Block(region: AvNodeCompact, blocked: true))`, `Sim(Shock(joules: 200.0, synchronised: true))`, `Sim(Drug(name: "adenosine", dose: 1.0))`:
  an intervention, the same syntax as the scenario timeline editor.

The **Other action** list has one example of each kind of row (every example is checked by a test to parse); insert it and edit
the values.

**Rhythms that are not built in.** If a stage loads a rhythm that came from another case file or that you added in this session
(Simulator, add to library), the page offers **Bundle**: the rhythm travels inside the file. Until you bundle it the problems panel
lists it and Save as… stays off, because the file would not open on other computers. Bundled rhythms are listed in the file
details with **Remove**. A built-in rhythm can never be bundled.

**The problems panel.** Lines marked **Must fix** stop Save as…: no stage, an answer outside its choices, a row that does not
parse, a rhythm that does not exist or is not bundled, an id that breaks the rule, a text longer than the file format allows
(a title, licence or author over 200 characters, more than 32 authors, a case text over 1 MiB). Lines marked **Advice** are the
editorial rules of the shipped cases and do not stop you, since your own case may be shorter: a title, summary and patient line;
references; a first stage that loads a rhythm; text in every stage; at least five questions, each with three to five different
choices and an explanation; an author and a licence. A line that names a stage jumps to it when clicked. The advice lines are in
English.

**Cases are synthetic, and sources are cited, not copied.** Invent the patient; never write a real person's record. Name the
guidelines, textbooks and papers the statements were checked against under **References**, and put the teaching in your own
words. Do not paste text from a copyrighted source, and use a licence you are entitled to give.

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

## Importing from an online library

A case library is a list of case files on a web server: heartsim's own is published with this guide, and a school or a teacher can
publish one the same way (the format and how to host one are in the "Libraries" chapter of `docs/case-file.md` in the repository).

* **Browse the online library** under **Online** downloads the library's list and shows its cases: title, summary, language,
  authors, licence and size. If the list has entries that break a rule of the format, the page says how many were ignored.
* **Import** downloads that case, checks it and adds it to **My cases**, where its card says where it came from ("from
  inalto.github.io"). From then on it is a case like any file you opened: it stays between sessions, **Start**, **Save as…** and
  **Remove** work as for the others.
* A case already in My cases shows **Imported**. If My cases has a *different* file with the same case id (an older version, or a
  file someone gave you), the button reads **Update…** and asks before it replaces yours; heartsim never replaces it silently.
* A case that needs a newer heartsim shows **Needs a newer heartsim** and cannot be imported.
* **Cancel** stops a download that is running; nothing is added. Only one download runs at a time.

Every downloaded file is checked before anything is shown: it must have exactly the size and the SHA-256 the list gives, pass the
same checks as a file opened from disk, and hold the case the list names. A file that fails is named with the reason, for example
"the file's SHA-256 is not the one the library lists" or "the library lists the case `…` but the file holds `…`", and nothing of it
is kept.

**What the checksum does and does not do.** The SHA-256 in the list protects against a damaged or swapped file, *given an honest
list*. It does not make a library trustworthy: the list is trusted exactly as much as the website it comes from. A case from
someone else's library is teaching text nobody at heartsim has reviewed, like any case file you are given.

### Another library

Type its address in **Library address**, for example `https://example.org/heartsim-cases/`, and press **Browse the online
library**; leave the field empty for heartsim's own library. The address is remembered in the settings. Only `https://` addresses
are accepted (and `http://localhost` or `http://127.0.0.1`, for testing a library on your own computer).

* **Desktop**: any https library works.
* **Browser**: heartsim's own library is on the same site as the browser version, so it always works. Another site must allow web
  pages of other sites to read its files (the `Access-Control-Allow-Origin` header of CORS; GitHub Pages and
  `raw.githubusercontent.com` do). When it does not, the page says "this server does not allow the web version to read it, or it
  cannot be reached": use the desktop version, or download the `.hs` file with your browser and open it with **Open case file…**.

### Privacy

Nothing is requested when heartsim starts or while you use the Cases page: only **Browse the online library** (one request for
the list) and **Import** (one request for that file) go to the network, and only to the library address shown. On the desktop the
request carries `User-Agent: heartsim/<version>` and nothing else: no cookies, no account, no identifier. In the browser, the
browser sends its usual headers and no cookies (heartsim asks it to omit them). The library's web server sees your IP address, as
for any web page.

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
