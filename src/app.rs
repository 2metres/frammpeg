use std::path::PathBuf;

use eframe::CreationContext;
use egui::{Align, CentralPanel, Frame, Layout, Panel, RichText};

use crate::{fonts, theme};

pub struct FrammpegApp {
    session_root: Option<PathBuf>,
    dropped_video: Option<PathBuf>,
}

impl FrammpegApp {
    pub fn new(cc: &CreationContext<'_>, session_root: Option<PathBuf>) -> Self {
        cc.egui_ctx.set_fonts(fonts::definitions());
        cc.egui_ctx.set_visuals(theme::visuals());
        Self {
            session_root,
            dropped_video: None,
        }
    }

    fn consume_dropped_files(&mut self, ui: &egui::Ui) {
        let files = ui.ctx().input(|i| i.raw.dropped_files.clone());
        for f in files {
            let path = f.path().to_path_buf();
            eprintln!("frammpeg: file dropped: {}", path.display());
            self.dropped_video = Some(path);
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new("Frammpeg").color(theme::TEXT).strong());
            ui.separator();
            ui.label(RichText::new("no video loaded").color(theme::TEXT_MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(root) = &self.session_root {
                    ui.label(
                        RichText::new(root.display().to_string())
                            .monospace()
                            .color(theme::TEXT_MUTED)
                            .small(),
                    );
                }
            });
        });
    }

    fn timeline(&mut self, ui: &mut egui::Ui) {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("timeline").color(theme::TEXT_MUTED));
        });
    }

    fn moments(&mut self, ui: &mut egui::Ui) {
        ui.add_space(2.0);
        ui.label(RichText::new("Moments").color(theme::TEXT).strong());
        ui.separator();
        ui.label(RichText::new("no moments yet").color(theme::TEXT_MUTED));
    }

    fn viewport(&mut self, ui: &mut egui::Ui) {
        ui.centered_and_justified(|ui| {
            let msg = match &self.dropped_video {
                Some(p) => format!("loaded: {}", p.display()),
                None => "drop a video file here".to_string(),
            };
            ui.label(RichText::new(msg).color(theme::TEXT_MUTED));
        });
    }
}

impl eframe::App for FrammpegApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.consume_dropped_files(ui);

        let panel_frame = Frame::default().fill(theme::PANEL).inner_margin(6.0);

        Panel::top("toolbar")
            .exact_size(40.0)
            .resizable(false)
            .frame(panel_frame)
            .show(ui, |ui| self.toolbar(ui));

        Panel::bottom("timeline")
            .exact_size(96.0)
            .resizable(false)
            .frame(panel_frame)
            .show(ui, |ui| self.timeline(ui));

        Panel::right("moments")
            .default_size(260.0)
            .min_size(200.0)
            .frame(Frame::default().fill(theme::PANEL).inner_margin(10.0))
            .show(ui, |ui| self.moments(ui));

        CentralPanel::default()
            .frame(Frame::default().fill(theme::CANVAS))
            .show(ui, |ui| self.viewport(ui));
    }
}
