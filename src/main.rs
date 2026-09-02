#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod annotate;
mod app;
mod export;
mod extract;
mod filmstrip;
mod fonts;
mod history;
mod icons;
mod model;
mod session;
mod theme;
mod thumbs;
mod transport;

use app::FrammpegApp;

fn main() -> eframe::Result<()> {
    let session_root = match session::ensure_sessions_root() {
        Ok(path) => {
            eprintln!("frammpeg: sessions root ready at {}", path.display());
            Some(path)
        }
        Err(err) => {
            eprintln!("frammpeg: could not prepare sessions root: {err}");
            None
        }
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Frammpeg")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([960.0, 600.0])
            .with_titlebar_shown(false)
            .with_fullsize_content_view(true),
        ..Default::default()
    };

    eframe::run_native(
        "Frammpeg",
        native_options,
        Box::new(move |cc| Ok(Box::new(FrammpegApp::new(cc, session_root.clone())))),
    )
}
