# B37 — Start-up layers: the four valves on, the valve planes and the torso off
**Crates** `heartsim-app`, docs · **Model** Haiku · **Milestone** M13 (usability and cases) · **Depends on** B35, B36.

## Goal
When the application starts, the *Layers and view* list of the Simulator has **Tricuspid valve**, **Mitral valve**,
**Aortic valve** and **Pulmonary valve** checked, and **Valve planes** and the torso unchecked.

## What is there today (read first)
* `crates/heartsim-app/src/settings.rs`: `Settings.tricuspid_valve`, `mitral_valve`, `aortic_valve`, `pulmonary_valve`,
  all `false` in `Default`; the struct is `#[serde(default)]` and is written whole to eframe storage, so every settings
  file saved by 0.25.0 or 0.26.0 holds an explicit `false` for the valves it knew.
* `crates/heartsim-app/src/heart_view.rs`, `HeartView::new` (about line 570): the `torso` layer and every valve layer are
  switched off after `AnatomyPanelState::new`; `valve_planes` stays on. `valve_planes` and `torso` are not stored.
* `crates/heartsim-app/src/app/scenario_control.rs` (`valve_switches`, and the block that copies the layers back into
  the settings): how the stored switches reach the view.
* The discs of `valve_planes` are already hidden valve by valve while that valve's leaflets are drawn; that stays.

## Work
1. `Settings::default()`: the four valve fields `true`. Update their doc comments (no "off by default").
2. A one-time migration for people who already have a settings file. Add `pub layer_defaults: u32` to `Settings` with a
   **field-level** `#[serde(default)]` that yields `0` (a field-level default wins over the struct-level one, so a file
   without the field reads 0 while `Settings::default()` gives the current value, `LAYER_DEFAULTS: u32 = 1`). In
   `Settings::load`, when the loaded value is below `LAYER_DEFAULTS`, set the four valve fields to `true` and the
   field to `LAYER_DEFAULTS`. After that the user's own choices are stored and respected as before. State this rule in
   the field's doc comment.
3. `HeartView::new`: `valve_planes` starts unchecked, like the torso (keep the torso off and at its 0.2 opacity). The
   valve layers start from the settings as today (do not hard-code them on in the view; the view's own start value
   when there are no settings to apply should also be on).
4. Fix the existing tests in `settings.rs` that assert the old defaults (`the_tricuspid_valve_layer_is_off_by_default…`,
   `the_designed_valve_layers_are_off_by_default…`): rename them and assert the new behaviour.
5. Docs: `docs/book/src/user/ui-tour.md` and any page that says the valve layers are off by default (grep the book and
   `known-issues.md` for "off by default", "Valve planes", "Tricuspid valve"); `CHANGELOG.md` under `## [Unreleased]`
   (`### Changed`), saying that existing installations get the valves switched on once.

## Acceptance tests
* `settings.rs`: defaults have the four valves on; `ron::from_str("(tricuspid_valve: false, mitral_valve: false)")`
  passed through the same migration `load` uses gives all four on and `layer_defaults == LAYER_DEFAULTS`; a file with
  `layer_defaults: 1, aortic_valve: false` keeps the aortic valve off; the round trip through `to_storage_string`
  keeps `layer_defaults`.
* `heart_view.rs` (next to the existing layer tests): a new `HeartView` on the default pack has `valve_planes` and
  `torso` not visible. If constructing a `HeartView` in a test is not already done somewhere, factor the start
  visibility into a small pure function and test that instead.
* `nice -n 10 cargo test --workspace --lib -p heartsim-app settings` style filters are fine while working; finish with
  `nice -n 10 cargo xtask ci`.

## Rules
One cargo build at a time, always `nice -n 10`. Do not commit, do not stage, do not edit `CLAUDE.md`. Report what you
did not verify (nothing here is seen on a real screen: there is no GPU on the build machine).
