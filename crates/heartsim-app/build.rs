//! Embeds the program icon into `heartsim.exe` on Windows, where Explorer, the Start-menu
//! shortcut and a pinned taskbar button take the icon from the executable's resources. The window
//! icon of a running instance is set separately (`main.rs`, `ViewportBuilder::with_icon`).

fn main() {
    println!("cargo:rerun-if-changed=icon/heartsim.rc");
    println!("cargo:rerun-if-changed=icon/heartsim.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("icon/heartsim.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("compiling the Windows icon resource");
    }
}
