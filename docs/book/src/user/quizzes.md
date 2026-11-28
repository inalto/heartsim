# Quizzes

## Axis Quiz

![Axis quiz](../images/axis-quiz.png)

Press **New question**: heartsim dials a hidden frontal QRS axis into the heart and shows the six limb leads. Read them,
click where you think the axis points on the circle, then press **Check answer**. Answers within 15° (half a hexaxial sector)
count as correct; the score and the streak of correct answers are shown next to the buttons.

The questions are chosen so that none is ambiguous: the drill never asks a "north-west" axis (170° to 280° on the clockwise
wheel, the extreme zone and its borders) and never an axis within 7° of a class boundary (−30°, +90° and the extreme zone), so a
reading that is a few degrees off still falls in the same class.

After you check, the quiz explains the reading in the words of the textbook method: which of leads I, II and aVF are positive or
negative for that axis, and the class that follows ("Lead I is positive and lead II is positive: the net QRS vector points to the
left and down, between −30° and +90°. That is a normal axis.").

A quick method: find the limb lead whose QRS is equiphasic (as much above as below the baseline): the axis is perpendicular to
it. Of the two perpendicular directions, the right one is where the leads are positive. Lead I and aVF both positive means a
normal axis.

## Rhythm Quiz

![Rhythm quiz](../images/rhythm-quiz.png)

A strip from a random scenario of the library is shown without its name. Pick the rhythm from the choices; the wrong choices
are the rhythms it is most often mistaken for. After answering you see the explanation and can open the scenario in the
Simulator to look at it on the heart. The score is kept between sessions.

The strip is a finished recording, drawn like a printout: it starts at the left edge and runs to the right at the paper speed
of the Simulator's ECG view (25 mm/s unless you changed it). The recording is 50 s long, enough to fill a wide window even at
12.5 mm/s; on a very wide window at a fast paper speed the paper to the right of the end of the recording stays empty.

After an answer the right option shows a tick on a green fill and a wrong choice a cross on a rose fill, so the result does not depend on colour. The score reads "Score: 3 of 12 correct".

The explanation is shown after every answer, right or wrong: what the rhythm is, why (the teaching notes of the scenario), the
rhythms it is most often confused with, and, when you chose another one, what that rhythm is.

## Interval Drill

![Interval drill](../images/interval-drill.png)

Measure an interval with the calipers, as you would on paper. Press **New question**: a strip of lead II appears at 50 mm/s
(1 mm is 20 ms) and the question names an interval: the **PR interval** (start of the P wave to start of the QRS), the **QRS
duration**, or the **QT interval** (start of the QRS to end of the T wave). Press **Place calipers**, drag the two lines to the
start and end of the interval (or shift-click the strip for another pair), press **Use caliper reading** or type the value, and
press **Check answer**. Each line has a tab at the top and at the bottom of the strip, and the band between a pair has a grip on its read-out bar: grab a line by its tab (or anywhere along it), or move both lines together by the grip. The pointer shows what it will do (arrows over a line, an open hand over the grip, a closed hand while you move the pair). The recording starts at the left edge of the strip and is 10 s long (1500 points at 50 mm/s), so on a very wide window the paper to its right stays empty. While a question is open and *Check answer* is still disabled, a line under the buttons says what to do
next. The strip uses the height of the window (the paper scale stays 1 mm = 20 ms); the two buttons that are greyed out until you have placed the calipers or entered a value say so when you hover them.

**Zoom and pan.** To place a line precisely, zoom in: the **Zoom in**, **Zoom out** and **Fit** buttons above the strip, Ctrl (or ⌘) + the mouse wheel, the plain wheel over the strip, or a two-finger pinch change the paper speed from 25 to 400 mm/s, keeping the sample under the pointer where it is. A line next to the strip says the scale in words ("1 mm = 20 ms" at 50 mm/s, "1 mm = 5 ms" at 200 mm/s), and the grid stays true at every zoom. **Fit** returns to 50 mm/s from the start of the recording, and so does the next question. Zoomed in, drag the paper sideways, use a horizontal wheel or a two-finger swipe, or drag the thin position bar under the strip to move along the recording; the bar shows which part is in view. The calipers stay on the samples they were placed on while you zoom and pan, and their reading does not depend on the zoom.

**Keys** (click the strip first): Left and Right arrows move the selected line by one sample (with shift, ten), at any zoom; `[` and `]` select the previous and the next line (Tab is left to move the focus out of the strip); `+` and `-` zoom, `0` fits; Delete or Esc clears the calipers.

An answer is correct within ±20 ms for PR and QT and ±15 ms for QRS, about one small square. The reference is the simulator's own
measurement of the same strip, taken only on a regular rhythm with a clean signal and a value inside the scenario's textbook
range; after you answer, the strip shows the waves it delineated, the rhythm is named and your error is given. Real readers
differ a little too, which is why there is a tolerance.
