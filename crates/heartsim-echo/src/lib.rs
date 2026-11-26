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
//! The wall-motion model itself has no leaflets, papillary muscles or pericardium (the
//! cross-section adds them as designed parts); no pressure, flow or Doppler; no
//! dilatation or hypertrophy; the right-ventricular volume is not computed; nothing here was
//! compared with a real echocardiogram.
//!
//! # The cross-section (brief B31, echo simulator phase 2)
//!
//! An **anatomical cross-section, not an ultrasound image**: the plane a transducer on the
//! figure's skin cuts through the beating heart, as a polar image of tissue labels
//! ([`heartsim_geometry::Label`]). The anatomy is the pack's two label volumes (format 3: the heart
//! box at 1 mm, the thorax box at 2 mm) plus the designed parts the data lacks ([`DesignedParams`]:
//! leaflets, papillary muscles, moderator band, pericardium — each value with its source or
//! "designed"). Each sample point goes back to the rest anatomy through a displacement grid filled
//! from **the same evaluated frame the 3-D heart draws** ([`field`]), so the slice and the mesh
//! cannot disagree and the motion model runs once per drawn frame. Bone and lung hide what lies
//! behind them on each line (geometric occlusion, not acoustics).
//!
//! ```text
//! // once, after the pack and the motion model are built:
//! let anatomy = EchoAnatomy::new(&pack, &model, DesignedParams::default())?;   // ≈ 0.7 s native, once
//! let mut state = SliceState::new(&anatomy);
//! let mut image = LabelImage::new(128, 512);              // lines × samples
//! let mut pose = anatomy.window_pose(EchoWindow::Apical); // or Parasternal, Subcostal, Suprasternal
//!
//! // the UI moves the probe (all values are clamped; NaN never propagates):
//! pose = anatomy.slide_to(&pose, picked_point_on_the_figure);   // drag on the skin
//! pose.tilt_deg += 5.0; pose.rock_deg -= 3.0; pose.rotation_deg += 10.0;
//! pose.depth_mm = 180.0; pose.sector_deg = 80.0;
//! let probe = pose.frame(anatomy.designed.contact_limit_deg); // apex, beam, marker, elevation, contact
//! let clock = pose.clock_hours();                               // the index marker as a clock position
//!
//! // every drawn frame, after model.evaluate(&input, &mut frame) for the 3-D heart:
//! state.update(&anatomy, &model, &frame, &input);   // grid from that frame + valve openings at input.t_ms
//! state.slice(&anatomy, &pose, &mut image);         // or heartsim_echo::slice(…) for both
//! // image.contact == false → "no contact", every label Outside
//! // image.texels(): lines × samples bytes, label | HIDDEN_BIT behind the first bone/lung sample
//! // image.labels_present(): for the legend; image.depth_of(sample) for the depth scale
//! // state.label_at(&anatomy, point_mm): the tissue under a hover point in the heart frame
//! ```
//!
//! With the motion switched off (`input.enabled == false`) `update` puts the grid at rest and
//! the leaflets still open and close by phase. Line 0 of the image is the side away from the
//! marker; the last line is the marker side (the screen's right); sample 0 is at the skin.
//!
//! Cost (`cargo bench -p heartsim-echo --bench slice`, release, the development VM where one
//! [`MotionModel::evaluate`] takes about 18 ms): the slice alone, 128 × 512 from the apical
//! window, about 7 ms; [`SliceState::update`] (grid from the frame, leaflets posed) about 4 ms;
//! evaluate + update + slice about 30 ms. The slice runs no motion evaluation of its own. Along
//! each line the inverse map is solved at every fourth sample (warm-started from the previous
//! one, iterated to 0.05 mm) and interpolated between (≤ 0.3 mm from the exact map); it is skipped
//! wherever the rest heart is out of reach of the instant's largest displacement, and the grid
//! update interpolates only the far nodes such a sample can touch. [`EchoAnatomy::new`] takes
//! about 0.7 s, once.
//!
//! The lung that covers the heart at the left sternal border in this supine subject is pulled
//! back by a **designed** lung-edge retraction (the cardiac notch; [`DesignedParams`]
//! `lung_retraction_*`, narrower in inspiration), applied at lookup time; ribs still hide. The
//! apical start pose is the skin point near the LV long axis's exit whose whole start sector is
//! clearest of bone and lung (B32 step 0).
//!
//! # The grey image (brief B32, echo simulator phase 3)
//!
//! A **simulated B-mode appearance from a simple line model** ([`bmode`]): the label slice of the
//! same frame goes through two-way attenuation, reflection and transmission at label changes
//! (Shams 2008) weighted by the incidence angle (Mattausch 2016), a designed lung reverberation,
//! speckle hashed from each sample's rest position (so it rides with the tissue), a separable
//! polar point-spread function, time-gain compensation, gain and log compression, into a `u8`
//! polar image of the label image's layout. The acoustic numbers are in
//! `assets/echo/acoustic.ron` ([`AcousticTable`], `format_version` 1), each with its source and
//! status (read, unverified or designed).
//!
//! ```text
//! let mut imager = EchoImager::new(AcousticTable::builtin());
//! let settings = ImageSettings::default();                // frequency, depth, sector, focus, gain,
//!                                                         // TGC, dynamic range, lines, samples, seed
//! let pose = settings.apply_to(&pose);                    // depth and sector live in the pose
//! let mut labels = settings.label_image();
//! let mut grey = GreyImage::new(settings.lines, settings.samples);
//! // every drawn frame, after state.update(…) as above:
//! state.slice(&anatomy, &pose, &mut labels);
//! imager.image(&anatomy, &state, &pose, &labels, &settings, &mut grey);
//! // grey.pixels: lines × samples bytes (an R8 texture), grey.contact, grey.focus_mm
//! ```
//!
//! Cost (`cargo bench -p heartsim-echo --bench slice -- grey`, release, the development VM): about
//! 14.7 ms for 128 × 512 from the apical window on top of the slice — line model 2.9 ms, speckle
//! 6.4 ms, point-spread function and envelope 2.8 ms, compensation and compression 0.5 ms. No
//! motion is evaluated again.
//!
//! # M-mode, tissue Doppler and measurements (brief B34, echo simulator phase 5)
//!
//! * [`mmode`]: one scan line of the same cross-section (label and grey) recorded over time at a
//!   fixed [`MMODE_STEP_MS`], independent of the frame rate: column `k` is the heart at
//!   `k · MMODE_STEP_MS`. Sweep speeds 25, 50 and 100 mm/s ([`SweepSpeed`]). A column costs one
//!   evaluation of the heart plus a 17-line thin slice and image, not a whole image.
//! * [`doppler`]: **tissue** Doppler at the septal or lateral mitral and the lateral tricuspid
//!   annulus only ([`AnnulusSite`], [`snap_gate`]): the velocity of the annulus point along the beam,
//!   positive toward the probe, uncorrected for the angle, with S', e' and a' marked from the
//!   model's own timing ([`BeatTimes`]). There is no blood Doppler.
//! * [`measure`]: calipers' arithmetic, the short list of names ([`Named`]) and the model's own
//!   value to compare a measurement with ([`reference_on_plane`], [`reference_on_mmode`],
//!   [`reference_excursion`], [`reference_velocity`]): read from the label line along the learner's
//!   own line, or from the annulus's motion; none where the model has none. Nothing is graded.
//!
//! ```text
//! let mut scan = MModeScan::new(&anatomy, &model, AcousticTable::builtin());
//! let ctx = ScanContext { anatomy: &anatomy, history: &history, pose, cursor_deg, settings: &settings,
//!                         enabled: true, exaggeration: 1.0 };
//! scan.advance(&mut model, &ctx, drawn_time_ms, 2);      // at most two columns this frame
//! let column = scan.buffer.get(step);                    // MModeColumn: grey, labels, depth
//!
//! let mut doppler = DopplerScan::new();
//! doppler.set_gate(snap_gate(&mut model, &input, &probe_frame, clicked_plane_point));
//! doppler.advance(&mut model, &DopplerContext { history: &history, enabled: true, exaggeration: 1.0 },
//!                 drawn_time_ms, 8);
//! let v = doppler.velocity_cm_s(step);                   // central difference of the annulus
//! let beat = model.beat_times(&history, t_ms, site.side());
//! let waves = doppler.markers(&beat.unwrap());           // S', e', a' (no a' without atrial contraction)
//! ```
//!
//! Cost (`cargo bench -p heartsim-echo --bench measure`, release, the development VM): one M-mode
//! column about 33 ms (evaluate, grid update, thin slice and image; the cursor's line alone about
//! 4.5 ms), one Doppler sample about 2.2 ms.
//!
//! The crate is pure: no GPU, GUI, file or clock access; it builds for `wasm32`.

pub mod acoustic;
pub mod anatomy;
pub mod bmode;
pub mod coords;
pub mod designed;
pub mod doppler;
pub mod field;
pub mod history;
pub mod measure;
pub mod mesh_valve;
pub mod mmode;
pub mod model;
pub mod params;
pub mod phases;
pub mod probe;
pub mod slice;
pub mod valves;
pub mod view_hints;
pub mod views;

pub use acoustic::{ACOUSTIC_FORMAT_VERSION, AcousticError, AcousticRow, AcousticTable, Sourced, Status};
pub use anatomy::{Capsule, ECHO_CONE_VALVES, EchoAnatomy, EchoError, EchoWindow, LeafletValve, Posed};
pub use bmode::{EchoImager, GreyImage, ImageSettings, TGC_BANDS};
pub use designed::{DesignedParams, valve_openings};
pub use doppler::{
    DOPPLER_STEP_MS, DopplerContext, DopplerSample, DopplerScan, GATE_SNAP_MM, Gate, Marker, WaveKind,
    snap_gate,
};
pub use field::DisplacementField;
pub use measure::{
    Excursion, ExcursionContext, Feedback, Named, Plane2, Structure, TraceScale, cursor_toward, feedback,
    polygon_area_cm2, reference_excursion, reference_on_mmode, reference_on_plane, reference_velocity,
    slope_cm_s, structure_extent, structure_span,
};
pub use mesh_valve::{MeshValve, PosedMeshValve};
pub use mmode::{
    MMODE_CAPACITY, MMODE_LINES, MMODE_STEP_MS, MModeColumn, MModeSampler, MModeScan, ScanContext,
    StepBuffer, SweepSpeed, TraceRate, cursor_frame, line_angle_deg,
};
pub use probe::{ProbeFrame, ProbePose, Skin};
pub use slice::{HIDDEN_BIT, LabelImage, SliceState, slice};

pub use coords::{AxisCoords, CardiacFrame, CellCoords, ChamberAxis, FrameError, MotionChamber};
pub use history::{ACTIVATIONS_PER_CELL, CellHistory, MotionHistory};
pub use model::{
    AnnulusSite, BeatTimes, MAX_EXAGGERATION, MotionError, MotionFrame, MotionInput, MotionModel,
    MotionReadout, PartFrame, mechanical_injury,
};
pub use params::MotionParams;
pub use phases::{CardiacPhase, CycleTimes, Side, VolumeState};
pub use valves::{ValveDisc, split_valve_discs};
pub use view_hints::{Direction, Hint, HintKind, Movement, PatientDir};
pub use views::{
    FoundTracker, ImageStats, ItemTest, Landmarks, Reach, Reference, ViewId, ViewScore, ViewTarget,
    ViewTargets, ViewTolerances,
};
