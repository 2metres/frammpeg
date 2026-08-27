#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod fonts;
mod session;
mod theme;

use app::FrammpegApp;

fn main() -> eframe::Result<()> {
    let session_root = match session::ensure_session_root() {
        Ok(path) => {
            eprintln!("frammpeg: session root ready at {}", path.display());
            Some(path)
        }
        Err(err) => {
            eprintln!("frammpeg: could not prepare session root: {err}");
            None
        }
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Frammpeg")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([960.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Frammpeg",
        native_options,
        Box::new(move |cc| Ok(Box::new(FrammpegApp::new(cc, session_root.clone())))),
    )
}
