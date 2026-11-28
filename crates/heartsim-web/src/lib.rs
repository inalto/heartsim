//! heartsim in the browser (brief B21): the same `HeartSimApp` as the desktop build, mounted on a
//! canvas by eframe's web runner and drawn by wgpu on WebGPU, or on WebGL2 where `navigator.gpu`
//! is missing.
//!
//! What differs from the desktop build, and why:
//! * **Assets** come from one downloaded archive ([`archive`]) instead of an installed `assets/`
//!   directory; the page shows a loading screen meanwhile. The wasm itself carries no pack.
//! * **Single thread.** The engines run without rayon (results are bit-identical, see the
//!   determinism tests); Tier 1 and Tier 2 therefore run on the UI thread, one fixed step at a time.
//! * **Sound** (B26) goes through Web Audio ([`audio`] on `wasm32`): the same heart-sound player as
//!   the desktop, a sink that schedules `AudioBuffer`s on the context clock, started by the
//!   learner's first click.
//! * **Files only for patient cases.** No export / import window, PDF, PNG or CSV files, saved
//!   scenario files or electrode-placement files; settings (quiz score, tutorial progress) live in
//!   `localStorage` through eframe's storage. Case files (`.hs`, brief B41) are the exception: the
//!   Cases page opens one with the browser's file picker or a drop on the page, saves one as a
//!   download (`rfd`'s web backend, inside `heartsim-app`'s `case_io`), and keeps My cases in
//!   `localStorage`.
//! * **Online case libraries** (B43): the Cases page's "Browse the online library" and "Import" go
//!   through the page's `fetch` ([`fetch`] on `wasm32`, injected like the sound), by default to
//!   `../cases/`, the library published next to the app on the same site.
//!
//! The pure part ([`archive`]) builds and is tested on every target; the browser glue exists only
//! when compiled for `wasm32`.

pub mod archive;

#[cfg(target_arch = "wasm32")]
mod audio;
#[cfg(target_arch = "wasm32")]
mod fetch;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::start;
