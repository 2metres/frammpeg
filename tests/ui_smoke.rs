use eframe::App;
use egui_kittest::Harness;
use frammpeg::app::FrammpegApp;

fn create_test_app(ctx: &egui::Context) -> FrammpegApp {
    let cc = eframe::CreationContext::_new_kittest(ctx.clone());
    FrammpegApp::new(&cc, None)
}

#[test]
fn test_empty_state_renders_without_panic() {
    let mut harness = Harness::new_ui(|ui| {
        let mut app = create_test_app(ui.ctx());
        let mut frame = eframe::Frame::_new_kittest();
        app.ui(ui, &mut frame);
    });

    for _ in 0..3 {
        harness.step();
    }
}

#[test]
fn test_toolbar_renders_in_empty_state() {
    let mut harness = Harness::new_ui(|ui| {
        let mut app = create_test_app(ui.ctx());
        let mut frame = eframe::Frame::_new_kittest();
        app.ui(ui, &mut frame);
    });

    for _ in 0..3 {
        harness.step();
    }
}

#[test]
fn test_app_frame_update_does_not_panic() {
    let mut harness = Harness::new_ui(|ui| {
        let mut app = create_test_app(ui.ctx());
        let mut frame = eframe::Frame::_new_kittest();
        app.ui(ui, &mut frame);
    });

    for _ in 0..5 {
        harness.step();
    }
}
