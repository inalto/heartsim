//! Echocardiography and haemodynamics calculators for teaching (brief B29).
//!
//! Every calculator is a [`Calculator`]: static data (inputs with units and plausible ranges,
//! references with locators, a review status, a worked example) and one pure function from typed
//! inputs to an [`Outcome`]. [`Calculator::evaluate`] validates before computing, so a result never
//! holds NaN or infinity and an invalid input is reported against its field. Interpretations are
//! [`Finding`]s that name the criterion they meet and where it was read; a calculator whose criteria
//! were not read on the primary source is [`Review::FormulaOnly`] and returns none.
//!
//! The crate has no dependencies, does no I/O and compiles for `wasm32`. It is a study aid, not a
//! medical device: findings say which published criterion a value meets, never what a patient has.

pub mod blocks;
pub mod calcs;
pub mod fmt;
pub mod grade;
pub mod model;
pub mod refs;
mod registry;
pub mod tree;

pub use model::{
    Calculator, Category, Choice, Cite, ErrorKind, ExampleValue, Finding, InputError, InputKind, InputSpec,
    Inputs, Message, Outcome, Quantity, Reference, Review, Severity, SimLink, Step, Text, Unit, Value,
};
pub use registry::{ALL, by_id, in_category, search};
