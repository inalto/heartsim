# Exporting and reviewing ECGs

The **Export / Import** button at the right of the tab row opens the export window. It saves the last seconds of the strip
in several formats at once, and loads a recording back for review with the calipers.

## Exporting

1. Run (or pause at) the rhythm you want. The export ends at the strip cursor, so pausing and scrubbing back exports an
   earlier excerpt.
2. In the export window choose the **length** (default 10 s), the **paper** for the PDF (A4 or US Letter), the formats, and
   the **folder**. Paper speed and gain are the strip's current settings.
3. Press **Export**. The status line lists the files written.

| File | What it is | Open it with |
|---|---|---|
| `heartsim_<scenario>.pdf` | A printable 12-lead page: 3 × 4 leads plus a lead II rhythm strip on 1 mm / 5 mm paper, a 1 mV calibration pulse per row, landscape A4 or Letter. At 25 mm/s the page holds 10 s. | Any PDF reader; print at 100 % ("actual size") so the grid is true millimetres. |
| `heartsim_<scenario>-12lead.png` | The same page as an image, 5 pixels per millimetre. | Slides, worksheets. |
| `heartsim_<scenario>-II.png` | A lead II rhythm strip, 250 mm wide. | Slides, worksheets. |
| `heartsim_<scenario>.csv` | The samples: `t_ms,I,II,III,aVR,aVL,aVF,V1,…,V6`, 500 per second, in millivolts with 1 µV resolution. | Spreadsheets, Python, MATLAB, R. |
| `heartsim_<scenario>.hea` + `.dat` | The same samples as a WFDB record (PhysioNet format 16, 1000 units/mV). | `wfdb` for Python, WFDB tools, LightWAVE. |
| `heartsim_<scenario>.ron` | The scenario file: load it again in heartsim or run it with `heartsim-cli run`. | heartsim, a text editor. |
| `heartsim_<scenario>.zip` | The **share bundle**: all of the above plus a README describing the excerpt. | Send this one to students. |

The header of every file names the scenario, paper speed, gain, sampling rate and date, and says that the ECG is simulated.
There are no patient fields: a simulation has no patient.

With a rhythm from the library loaded, the scenario file is that rhythm as it was loaded: sliders you moved afterwards are not
in it. With "Custom (sliders)" it holds the sliders' current timing settings, axis shift and QRS width, and any pacemaker you
connected.

The **Window screenshot** button saves a PNG of the whole window, including the 3-D heart as currently shown.

The same files come out of the command line, byte for byte, for the same scenario and length; see [Command line](cli.md).

## Reviewing a recording

Type the path of a `.csv`, `.hea` (or `.dat`) or `.zip` file in the export window and press **Import**, or simply drop the file on
the heartsim window. The recording replaces the live strip and the simulation pauses:

* the Simulator strip and the 12-Lead page show the recording; drag the *s back* slider or the strip to move through it;
* calipers, wave letters and the interval read-out work as on a live strip;
* the tab row names the file under review.

The **Export** button is off during a review, because its files would name the running scenario and call the recording
simulated. *Save PNG* on the 12-Lead tab still saves what is on screen.

Press **Back to live** (or *Run*) to return to the simulation where it was paused. Picking a rhythm also ends the review and
starts that rhythm.

The importer reads the formats heartsim writes, and also CSV files with the twelve lead names as column headers in any order
(`t_ms`, or `t_s` for seconds, first) and WFDB format-16 records whose signals are named I … V6, which is the layout of PTB-XL
and of many other PhysioNet 12-lead databases. CSV leads are expected in millivolts; WFDB records may say mV, uV or V. Other
WFDB formats, such as 212, are refused with a message rather than misread. Imported recordings stay on your computer; heartsim sends nothing anywhere.

## Running a scenario file

Import or drop a `.ron` scenario file and it starts running like a rhythm picked from the list. It joins the list until you
close heartsim, replacing any rhythm with the same id for that time; your installed library is not changed.
