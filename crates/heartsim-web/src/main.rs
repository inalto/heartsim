//! Entry point trunk builds. On a desktop target there is nothing to run: the desktop app is
//! `heartsim-app`'s `heartsim` binary. The stub keeps `cargo build --workspace` working everywhere.

#[cfg(target_arch = "wasm32")]
fn main() {
    heartsim_web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!(
        "heartsim-web runs in a browser: `trunk serve crates/heartsim-web/index.html` (see docs/web.md)."
    );
    std::process::exit(2);
}
