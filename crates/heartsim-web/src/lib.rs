//! heartsim in the browser (brief B21): the same `HeartSimApp` as the desktop build, mounted on a
//! canvas by eframe's web runner and drawn by wgpu on WebGPU, or on WebGL2 where `navigator.gpu`
//! is missing.
//!
//! What differs from the desktop build, and why:
//! * **Assets** come from one downloaded archive ([`archive`]) instead of an installed `assets/`
//!   directory; the page shows a loading screen meanwhile. The wasm itself carries no pack.
//! * **Single thread.** The engines run without rayon (results are bit-identical, see the
//!   determinism tests); Tier 1 and Tier 2 therefore run on the UI thread, one fixed step at a time.
//! * **No files.** No export / import window, PDF, PNG or CSV files, saved scenario files or
//!   electrode-placement files; settings (quiz score, tutorial progress) live in `localStorage`
//!   through eframe's storage.
//!
//! The pure part ([`archive`]) builds and is tested on every target; the browser glue exists only
//! when compiled for `wasm32`.

pub mod archive;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::start;
