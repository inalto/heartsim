# heartsim

heartsim is an open-source simulator of the heart's electrical activity for medical education. It shows activation spreading
over a three-dimensional heart while the 12-lead ECG that activation produces draws in real time. Change the physiology (rate,
conduction blocks, accessory pathways, ectopic beats, pacing, electrode placement) and watch both views change together.

![The Simulator tab: the 3-D heart, the frontal-axis circle and a lead II strip](images/simulator-sinus.png)

The ECG strip and the colours on the heart come from the same simulation step, so they cannot disagree: pause the strip,
drag back to any instant, and the heart shows that instant.

heartsim runs on Windows, macOS and Linux, and [in a web browser](app/) without installing anything (the browser version has
no export or file features and is slower). It is free software (MIT or Apache-2.0) and was written from the published
literature; it contains no code or data from any other simulator.

**This guide has two parts.**

* The [user guide](user/install.md) is for students and teachers: installing, finding your way around the window, the rhythm
  library, tutorials and quizzes, and exporting ECGs for handouts or for review.
* The [developer guide](dev/architecture.md) is for contributors: how the code is organised, how to add a rhythm or a drug
  (no code needed for either), and how to run the tests and benchmarks.

Read the [medical disclaimer](user/disclaimer.md) before using heartsim in teaching.
