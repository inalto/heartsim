#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result<()> {
    // Release builds use the Windows GUI subsystem, so there `--version` only reaches a redirected stdout.
    if heartsim_app::wants_version(std::env::args().skip(1)) {
        println!("{}", heartsim_app::version_line());
        return Ok(());
    }
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title(heartsim_app::version_line()),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "heartsim",
        options,
        Box::new(|cc| Ok(Box::new(heartsim_app::HeartSimApp::new(cc)))),
    )
}
