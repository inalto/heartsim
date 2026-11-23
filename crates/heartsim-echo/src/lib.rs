//! The beating heart: a kinematic model of wall motion driven by the engines' own timing (brief
//! B30, echo simulator phase 1).
//!
//! **A model of wall motion, not electromechanics.** Motion is prescribed in cardiac coordinates
//! (long-axis shortening, inward radial motion with thickening, rotation about the long axis) and
//! scaled so that the LV cavity of the deformed mesh follows a volume curve gated by the same valve
//! schedule that places the heart sounds (`heartsim_ecg::pcg::schedule_with`). There is no
//! pressure and no flow. Every number is in [`MotionParams`] with its source or the word
//! "designed" (`docs/research/echo-simulator.md` §2).
//!
//! **One source of truth.** The motion is a pure function of the record in [`MotionHistory`]
//! (timing-engine events, organised atrial activations, the last few activations of every cell),
//! the injury of the cells, the respiratory phase and the time. It never feeds back into an
//! engine. Same scenario and seed give bit-identical vertices; the frame rate does not matter.
//!
//! # What moves, and why
//!
//! * Each cell contracts from its `activation_ms` plus an electromechanical delay for
//!   `0.9 · apd_ms` ([`phases::contraction`]), gated (fibrillating atria and ventricular
//!   fibrillation keep a small quiver only) and averaged over its neighbours (the myocardium is
//!   mechanically coupled: per chamber, on a voxel grid of [`MotionParams::coupling_voxel_mm`]
//!   with [`MotionParams::coupling_passes`] passes). Each vertex reads its chamber's coupled field
//!   by trilinear interpolation, so the LV, RV and atrial contraction of a vertex near a junction
//!   do not mix.
//! * Each ventricle's displacement is `template · (c + β)`: `c` the hood-interpolated contraction
//!   of the vertex, `β` one global amplitude per ventricle. For the left ventricle `β` is solved
//!   every frame so that the cavity of the deformed LV mesh has exactly the volume of the phase
//!   curve ([`phases::volume_at`]): held through isovolumic contraction and relaxation (so early
//!   regions shorten while late ones stretch — the septal flash of LBBB), falling over ejection to
//!   the beat's ESV, refilling with E, diastasis and the atrial push. The right ventricle has no
//!   closed cavity in the pack; the amplitude at its lateral tricuspid annulus follows its
//!   own curve.
//! * The beat's ejection fraction follows the preceding RR (restitution below 600 ms) and the one
//!   before (post-extrasystolic potentiation); a beat without A2 does not eject, and an early beat
//!   never cuts the previous ejection short.
//! * An ischaemic LV territory ([`MotionModel::set_injury`]) keeps `1 − injury` of its inward
//!   motion in every phase and the LV ejection fraction falls by what it no longer moves, so the
//!   remote wall moves as before.
//! * Atria follow their AV plane and contract on their own clock; valve discs follow an affine fit
//!   to their annulus; great vessels follow their root and fade to rest; the whole heart moves a
//!   few millimetres caudally in inspiration.
//! * **One continuous heart.** The parts are moved in order — LV, RV, valve discs, aortic and
//!   pulmonary roots, left and right atrium, other vessels — and each rests on those before it:
//!   where it touches them its own motion is corrected toward theirs, and the correction fades
//!   with the distance from the contact (the RV free wall over about 3 cm to its own long-axis and
//!   inward motion; the RV has no septum of its own, it lies on the LV). The RV and the discs take
//!   their parents' displacement exactly on the contact; the thin atrial and vessel walls take
//!   part of it exactly and the rest smoothed, because the full local shear of the wall they rest
//!   on folds their sub-millimetre triangles. `tests/continuity.rs` bounds the slip between
//!   touching parts.
//! * TAPSE is read between the right ventricle's own end-diastole and end-systole (tricuspid and
//!   pulmonic closure), MAPSE between the left's: read at A2, TAPSE would change with the
//!   respiratory splitting of S2. The RV's amplitude is set at its lateral tricuspid annulus (its
//!   contraction there plus β follows the RV volume curve), and its long-axis template is scaled
//!   so that the annulus descends by [`MotionParams::tapse_mm`] over the full ejection
//!   ([`MotionModel::rv_long_scale`]). TAPSE is read on the RV's own motion (the correction that
//!   attaches the RV to the LV moved it by less than 0.1 mm).
//!
//! # Using it (the app)
//!
//! ```text
//! // once, from the pack (before HeartView::from_pack consumes it):
//! let mut model = MotionModel::new(&pack, MotionParams::default())?;
//! let mut frame = model.new_frame();                       // one Vec<Vertex> per pack mesh
//! let mut history = MotionHistory::new(PcgParams::default(), respiration_rate_bpm, cell_count)
//!     .with_auscultation(&scenario.auscultation);          // as HeartSounds builds its PcgSynth
//!
//! // every engine step (where HeartView::on_step and HeartSounds::on_events are called):
//! history.set_respiration(engine.respiration());
//! history.on_events(&events);
//! history.record_cells(heart_view.engine().cells());      // or the external (Tier-2) cells
//!
//! // when the lesions change (Action::Stenosis), from the eikonal engine:
//! model.set_injury(&mechanical_injury(heart_view.engine(), &pack.aha_segments));
//!
//! // every drawn frame, at the time the strip and the colour map are drawn:
//! model.evaluate(&MotionInput { history: &history, t_ms, resp_phase, enabled, exaggeration }, &mut frame);
//! // frame.parts[k].vertices: position + recomputed normal, the pack mesh's order and GPU layout
//! // frame.readout: LVEF, EDV / ESV (mL), MAPSE, TAPSE (mm) of the current beat — "model" values
//!
//! // with the motion switched off but the read-out card shown (no vertex written):
//! let r = model.readout(&MotionInput { history: &history, t_ms, resp_phase, enabled: false, exaggeration: 1.0 });
//! ```
//!
//! `enabled = false` returns the rest mesh bit for bit. The valve discs are one part
//! (`valve_planes`); [`MotionModel::valve_discs`] gives each disc's vertex indices in the order the
//! app's `split_valves` uses. Picking still has to be decided by the app (pick on the rest mesh, or
//! freeze motion while the pointer is down).
//!
//! # Cost
//!
//! One evaluation on the default pack (80 135 vertices in 10 parts, 20 051 cells) is about 18 ms
//! native in release on the development VM (`cargo bench -p heartsim-echo`; a generic QEMU CPU):
//! about 8 ms of it attaches the parts to one another (the RV and the atria on control vertices
//! 2 mm apart, the discs and vessels on every vertex), about 4 ms writes the positions and
//! recomputes the normals. [`MotionModel::readout`] alone (the card with the motion off) is about
//! 2.8 ms: the read-outs need no attachment (TAPSE is read on the RV's own motion). Everything that
//! does not depend on the time is precomputed: the vertices read the coupled contraction from a
//! dense voxel lattice by trilinear interpolation, the templates are compact per-vertex records,
//! the attachments their parent vertices, grid samples, controls and interpolation weights, the
//! normals a per-vertex list of incident triangles.
//!
//! # Known limits of the mesh
//!
//! The decimated pack meshes hold near-degenerate slivers (≈ 4–7 % of the ventricular triangles
//! have a smallest altitude under 0.2 × their longest edge); some of those flip under any
//! non-rigid motion and are not counted as faults. One proper RV triangle by the inferior
//! tricuspid annulus folds for about 150 ms per beat in LBBB (no longer in atrial fibrillation). An exaggeration of 2× folds the thin
//! walls at end-systole.
//!
//! # What it cannot show
//!
//! No leaflets, papillary muscles or pericardium (later phases); no pressure, flow or Doppler; no
//! dilatation or hypertrophy; the right-ventricular volume is not computed; nothing here was
//! compared with a real echocardiogram.
//!
//! The crate is pure: no GPU, GUI, file or clock access; it builds for `wasm32`.

pub mod coords;
pub mod history;
pub mod model;
pub mod params;
pub mod phases;
pub mod valves;

pub use coords::{AxisCoords, CardiacFrame, CellCoords, ChamberAxis, FrameError, MotionChamber};
pub use history::{ACTIVATIONS_PER_CELL, CellHistory, MotionHistory};
pub use model::{
    MAX_EXAGGERATION, MotionError, MotionFrame, MotionInput, MotionModel, MotionReadout, PartFrame,
    mechanical_injury,
};
pub use params::MotionParams;
pub use phases::{CardiacPhase, CycleTimes, Side, VolumeState};
pub use valves::{ValveDisc, split_valve_discs};
