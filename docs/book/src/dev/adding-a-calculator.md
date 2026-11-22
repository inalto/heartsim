# Adding a calculator

The echo calculators live in the pure crate `heartsim-calc` (no dependencies, no I/O, builds for `wasm32`). The page in the
app and `heartsim-cli calc` only display what the crate returns, so a new calculator needs no UI code.

## 1. Read the source first

A calculator is only as good as the publication behind it. Before writing code:

1. Open the **primary** publication: the guideline (ASE, EACVI, ESC) or the original paper. Not a review, not a website, not
   another calculator. Note the table or equation and the **journal page** of every formula and every threshold.
2. Decide the review status:
   * `Review::Verified` if the formula and every threshold you will show were read there;
   * `Review::FormulaOnly` if only the arithmetic could be checked (a standard relation, or a formula from first principles):
     then the calculator returns **no** findings;
   * if even the formula cannot be confirmed, do not add the calculator; list it in `docs/research/echo-calculators.md`.
3. Add a row to `docs/research/echo-calculators.md`: id, status, the source read, its locator, anything not confirmed.

## 2. Write it

Calculators are grouped by category in `crates/heartsim-calc/src/calcs/<category>.rs`. Each is a `pub static` `Calculator`
beside its compute function:

```rust
pub static LA_VOLUME_INDEX: Calculator = Calculator {
    id: "la-volume-index",                      // stable: favourites and the CLI use it
    category: Category::FillingPressures,
    name: tx!("Left atrial volume index", "Volume atriale sinistro indicizzato"),
    synonyms: tx!("LAVi, left atrium, biplane", "LAVi, atrio sinistro, biplano"),   // for the search
    summary: tx!("What it computes and on what assumptions.", "…"),
    inputs: &[
        num("la_volume_ml", tx!("Maximal LA volume (biplane)", "…"), Unit::Ml, 10.0, 300.0, 0),
        num("bsa_m2", tx!("Body surface area", "Superficie corporea"), Unit::M2, 0.5, 3.0, 2),
    ],
    compute: |i| { /* … */ },
    references: &[cite(&CQ_2015, "LA volume §9.3, p. 25; Table 4, p. 10")],
    review: Review::Verified,
    example: &[("la_volume_ml", N(74.0)), ("bsa_m2", N(1.9))],   // our own plausible values
    sim_link: None,
};
```

* **Ids** of the calculator, its inputs and its outputs carry the unit (`_cm`, `_cm2`, `_m_s`, `_mmhg`, `_ml_m2` …); a test
  checks the output units against the suffix.
* **Inputs** are in the order they are acquired. `num` is required, `opt` optional, `choice` a fixed list; `when(spec,
  "mode", &["a"])` shows an input only for some choices. The range is the *plausible* range of the measurement: a value
  outside it is refused (most often a typing or unit error), so make it wide enough for real disease and narrow enough that
  nothing in it divides by zero.
* **Compute** receives inputs that are present and in range. Use `i.n("id")` for required numbers and `o(i, "id")` for
  optional ones. Refuse impossible combinations with `check(bad, "input_id", tx!(...))` (a consistency error shown beside
  that field). Add each result with `out.push(q(id, label, value, unit, decimals), "symbolic formula", substituted)`, so the page
  can show the worked formula. Reuse `crate::blocks` (Bernoulli, `circle_area_cm2`, `stroke_volume_ml`, `continuity_area_cm2`,
  `pisa_flow_ml_s`, `indexed`, `fraction_percent`).
* **Findings.** A graded table is data: a `Scale` of `band(upto, inclusive, severity, label)` copied row by row from the source,
  with `above` for the band over the last limit and the `Cite` of the table. `SCALE.finding(value)` writes the finding with
  its criterion. A single criterion is `criterion_met(severity, text, "criterion", cite)`. Compare with `grade::lt/le/gt/ge`,
  which treat a value within rounding of a limit as equal to it. Word findings as criteria ("in the range the source grades
  *severe*"), never as a diagnosis.
* **Text** is written fresh in English and Italian (`tx!`). Use only characters the bundled egui fonts have: a test fails on a
  missing glyph (write e' with an apostrophe, not a prime).
* **The simulator** may fill an input only if it truly has that quantity: today only the heart rate (`SimLink::HeartRate`).

Register the calculator in `crates/heartsim-calc/src/registry.rs` and add its id to the list in
`crates/heartsim-calc/tests/registry.rs` (ids are a stable interface).

## 3. Test it

In the category's `tests` module:

* a hand-computed case, with the arithmetic in a comment, from the publication's own worked example where it has one;
* every grading boundary at the limit and one step either side, on the side the source puts it;
* the consistency errors.

The crate-wide tests then check the rest for every calculator: the worked example validates and computes, every output has
a unit and is finite for any in-range input (a property test), out-of-range and missing inputs are refused against their
field, formula-only calculators return no findings, names exist in both languages, and the CLI prints the library's outcome.

```bash
cargo test -p heartsim-calc
cargo test --workspace --test echo_calc     # the page, including the glyph check
cargo test --workspace --test calc          # the command line
```

## Decision trees (brief B29 part B)

A guideline algorithm is a calculator too, built on `crates/heartsim-calc/src/tree.rs`:

* **Data.** `Node`s (a question, the `Cite` of its figure or table and page, the facts it looks at) and `Leaf`s (a result
  worded as a criterion met, its `Severity`, its rule in symbols, its `Cite`), gathered in a `Tree` with the arity of each
  fact, an `open` leaf (indeterminate) and a pure `walk` from a complete set of answers to a leaf, which records each question
  asked with `Walk::ask`.
* **Facts.** The compute function turns the measurements into `Fact`s: a criterion met or not (`Fact::of(&test(...), value)`),
  one of several alternatives that were measured (`Fact::any_measured`), or a class out of a few ranges; an empty field is
  `None`. A branch the figure draws for a missing measurement ("LARS not available") is a known class, not `None`.
* **Deciding.** `tree::decide` walks the tree for every value the unknown facts could take. One leaf for all: that is the
  result; several: the `open` leaf, listing them. The findings are the steps common to every walk, then the result; the
  compute function may put graded-table findings before them, but the result must stay last.
* **Registering.** Add the tree to `calcs::TREES` (in registry order). `tests/trees.rs` then enumerates every combination of
  met, not met and not measured for it, checks that each ends at a leaf, that every leaf is reachable and that every node and
  leaf has a locator with a page; the property test checks that every evaluation ends at a leaf.
* Results that conclude nothing graded use `Severity::Abnormal` (for example "elevated filling pressure") or
  `Severity::Indeterminate`.
