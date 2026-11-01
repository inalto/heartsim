//! Application model and eframe `App` implementation. `main.rs` only sets up logging and the
//! native window so the same `HeartSimApp` can be mounted in the web build.

pub mod app;
pub use app::HeartSimApp;
