//! The cardiac EP lab (brief B22, F-33): programmed electrical stimulation from any catheter pair,
//! automatic ERP search, entrainment with post-pacing intervals, and RF lesions that persist.
//!
//! [`EpLab`] sits on top of [`crate::tier2::SwitchingEngine`] and adds no physiology of its own: a
//! stimulus goes through the timing engine's capture decision (Tier 1) or the tissue (Tier 2), a
//! lesion through the cells, the voxels and the edges of the timing graph it destroys, and every
//! measurement below is read back from the engine's own events and activation times. So the strip,
//! the colour map, the electrograms and the protocol results stay one state seen several ways.
//!
//! * [`protocol`] — S1×n / S2 / S3 / continuous / decremental trains, ERP search, entrainment.
//! * [`lesion`] — the RF generator stub and the lesion book.
//! * [`lab`] — [`EpLab`], the procedure.
//! * [`preset`] — the `ep:` section of the procedure files in `assets/scenarios/ep/`.
//!
//! **Reference ranges** used by the tests and the guide, baseline adults: HV 35–55 ms; AH 50–120 ms
//! (published upper limits run from ≈ 105 to 140 ms; Josephson, *Clinical Cardiac Electrophysiology*);
//! refractory periods atrium 170–300, AV node 230–425, ventricle 170–290 ms (Josephson; drive-cycle
//! dependent); a jump of ≥ 50 ms in A2–H2 for a 10 ms shorter A1–A2 marks dual AV-nodal pathways;
//! PPI − TCL ≤ 30 ms puts the pacing site in a macro-re-entrant circuit (Stevenson et al. 1993,
//! Cosío et al. 1993 for flutter); PPI − TCL > 115 ms favours AVNRT over orthodromic AVRT when
//! pacing the RV (Michaud et al. 2001).

pub mod lab;
pub mod lesion;
pub mod preset;
pub mod protocol;

pub use lab::{AblationTarget, EpLab, StimRecord};
pub use lesion::{Lesion, LesionBook, RfApplication, RfParams, RfReadout};
pub use preset::{EpGoal, EpPreset, EpSpec};
pub use protocol::{
    CouplingPoint, EntrainmentResult, ErpResult, ErpTarget, Protocol, ProtocolOutcome, ProtocolRun,
    ProtocolStatus,
};
