# FAQ

**Is heartsim a medical device? Can I use it to read a patient's ECG?**
No. It is a teaching simulation; see the [medical disclaimer](disclaimer.md). The importer displays recordings for practice
with calipers but makes no interpretation.

**Why does the printed PDF not measure 25 mm per second?**
Print at 100 % ("actual size"). "Fit to page" scales the grid. The PDF page is landscape A4 or Letter; pick the one your
printer uses in the export window.

**The strip on screen is not 25 mm/s against a ruler.**
Screens differ in pixel density. Adjust the *pt/mm* calibration next to the strip until ten small squares measure 1 cm.

**The heart is a grey sphere, or the window does not open.**
heartsim could not load the heart model or find a GPU. Reinstall from the release page; on Linux without a GPU install Mesa's
Vulkan driver (`mesa-vulkan-drivers`). Start heartsim from a terminal with `RUST_LOG=warn` to see the reason.

**Why do my measured intervals differ by a few milliseconds from the read-out?**
The read-out is measured automatically over the beats in the last seconds; a caliper measures the one beat you placed it on.
Samples are 2 ms apart, so caliper readings move in 2 ms steps.

**Can I open the CSV in Excel?**
Yes. The first column is time in milliseconds from the start of the excerpt, then one column per lead in millivolts.

**How do I read the WFDB files in Python?**

```python
import wfdb
record = wfdb.rdrecord("heartsim_sinus_normal")
wfdb.plot_wfdb(record)
```

**Does heartsim send any data over the network?**
No. It needs no account and makes no network connections.

**Where do I report a mistake in a rhythm?**
Open an issue on [GitHub](https://github.com/inalto/heartsim/issues) with the scenario name and, ideally, a share bundle
exported at the moment you saw it.
