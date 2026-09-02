use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use eframe::CreationContext;
use egui::{
    Align, Align2, Area, CentralPanel, Color32, ColorImage, CornerRadius, DragValue, Event, Frame,
    Key, Layout, Order, Panel, Pos2, Rect, RichText, Sense, Stroke, TextEdit, TextureHandle,
    TextureOptions, Vec2,
};

use crate::extract::{spawn_extraction, ExtractEvent};
use crate::filmstrip::{self, FilmstripDrawParams, FilmstripGeometry};
use crate::history::{Action, History, HistoryState, HISTORY_CAP};
use crate::model::{
    Annotation, Moment, DEFAULT_FONT_SIZE, DEFAULT_STROKE_RGBA, DEFAULT_STROKE_WIDTH,
    DEFAULT_TEXT_RGBA, MAX_BUFFER,
};
use crate::session::{self, SessionDirs};
use crate::thumbs::{self, ThumbCache};
use crate::transport::{self, TransportAction, TransportView};
use crate::{export, fonts, theme};

const FRAME_CACHE_SIZE: usize = 5;
const COPY_TOAST_MS: u64 = 1200;
const TRANSPORT_ROW_H: f32 = 40.0;
const BOTTOM_PANEL_H: f32 = 200.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tool {
    Rect,
    Text,
}

struct DragRect {
    start_frame_px: (f32, f32),
    current_frame_px: (f32, f32),
}

enum Phase {
    Empty,
    Extracting(Box<ExtractionState>),
    Ready(Box<VideoState>),
    Error(String),
}

struct ExtractionState {
    session: SessionDirs,
    video_path: PathBuf,
    current: usize,
    preparing: bool,
    rx: Receiver<ExtractEvent>,
}

struct VideoState {
    session: SessionDirs,
    video_path: PathBuf,
    total_frames: usize,
    current_frame: usize,
    prev_current_frame: usize,
    fps: f32,
    playing: bool,
    last_play_tick: Option<Instant>,
    play_leftover: Duration,
    annotations: HashMap<usize, Vec<Annotation>>,
    moments: Vec<Moment>,
    selected_moment: Option<usize>,
    tool: Tool,
    drag: Option<DragRect>,
    editing_text: Option<usize>,
    text_buffer: String,
    text_focus_pending: bool,
    frame_cache: VecDeque<(usize, TextureHandle)>,
    frame_pixel_size: Option<(u32, u32)>,
    last_copy_at: Option<Instant>,
    last_export_msg: Option<(Instant, String)>,
    thumbs: ThumbCache,
    filmstrip_geom: FilmstripGeometry,
    history: History,
    note_edit: Option<NoteEditSnapshot>,
    buffer_edit: Option<BufferEditSnapshot>,
}

struct NoteEditSnapshot {
    moment_index: usize,
    original: String,
}

struct BufferEditSnapshot {
    moment_index: usize,
    original: usize,
}

impl VideoState {
    fn new(
        session: SessionDirs,
        video_path: PathBuf,
        total_frames: usize,
        fps: f32,
        ctx: &egui::Context,
    ) -> Self {
        let thumbs = ThumbCache::new(
            thumbs::DEFAULT_CAPACITY,
            session.frames.clone(),
            thumbs::DEFAULT_THUMB_W,
            thumbs::DEFAULT_THUMB_H,
            ctx.clone(),
        );
        Self {
            session,
            video_path,
            total_frames,
            current_frame: 0,
            prev_current_frame: 0,
            fps,
            playing: false,
            last_play_tick: None,
            play_leftover: Duration::ZERO,
            annotations: HashMap::new(),
            moments: Vec::new(),
            selected_moment: None,
            tool: Tool::Rect,
            drag: None,
            editing_text: None,
            text_buffer: String::new(),
            text_focus_pending: false,
            frame_cache: VecDeque::new(),
            frame_pixel_size: None,
            last_copy_at: None,
            last_export_msg: None,
            thumbs,
            filmstrip_geom: FilmstripGeometry::default(),
            history: History::new(HISTORY_CAP),
            note_edit: None,
            buffer_edit: None,
        }
    }

    fn seek(&mut self, target: usize) {
        let last = self.total_frames.saturating_sub(1);
        let clamped = target.min(last);
        if clamped != self.current_frame {
            self.commit_text_edit();
            self.current_frame = clamped;
        }
    }

    fn set_playing(&mut self, playing: bool) {
        if self.playing == playing {
            return;
        }
        self.playing = playing;
        if playing {
            self.last_play_tick = Some(Instant::now());
            self.play_leftover = Duration::ZERO;
        } else {
            self.last_play_tick = None;
            self.play_leftover = Duration::ZERO;
        }
    }

    fn tick_play(&mut self) {
        if !self.playing || self.total_frames == 0 || self.fps <= 0.0 {
            return;
        }
        let Some(prev) = self.last_play_tick else {
            self.last_play_tick = Some(Instant::now());
            return;
        };
        let now = Instant::now();
        let elapsed = now.duration_since(prev) + self.play_leftover;
        let (frames, leftover) = transport::advance_frames(self.fps, elapsed);
        if frames > 0 {
            let next = transport::step_play(self.current_frame, frames, self.total_frames);
            if next != self.current_frame {
                self.commit_text_edit();
                self.current_frame = next;
            }
        }
        self.last_play_tick = Some(now);
        self.play_leftover = leftover;
    }

    fn commit_text_edit(&mut self) {
        let frame = self.current_frame;
        if let Some(idx) = self.editing_text.take() {
            let ann_list = self.annotations.entry(frame).or_default();
            if let Some(Annotation::Text { text, .. }) = ann_list.get_mut(idx) {
                if self.text_buffer.trim().is_empty() {
                    ann_list.remove(idx);
                    if ann_list.is_empty() {
                        self.annotations.remove(&frame);
                    }
                } else {
                    *text = self.text_buffer.clone();
                    let recorded = ann_list[idx].clone();
                    self.history.record(Action::AnnotationCreated {
                        frame,
                        index: idx,
                        annotation: recorded,
                    });
                }
            }
            self.text_buffer.clear();
        }
    }

    fn after_history_change(&mut self, action: &Action) {
        self.drag = None;
        self.editing_text = None;
        self.text_buffer.clear();
        self.text_focus_pending = false;
        if let Some(sel) = self.selected_moment {
            if sel >= self.moments.len() {
                self.selected_moment = None;
            }
        }
        if matches!(
            action,
            Action::AnnotationCreated { .. } | Action::AnnotationDeleted { .. }
        ) {
            if let Some(frame) = action.affected_frame(&self.moments) {
                if frame != self.current_frame && frame < self.total_frames {
                    self.current_frame = frame;
                }
            }
        }
    }

    fn finalize_pending_edits(&mut self) {
        if let Some(snap) = self.note_edit.take() {
            if let Some(m) = self.moments.get(snap.moment_index) {
                if m.note != snap.original {
                    self.history.record(Action::MomentNoteChanged {
                        index: snap.moment_index,
                        old: snap.original,
                        new: m.note.clone(),
                    });
                }
            }
        }
        if let Some(snap) = self.buffer_edit.take() {
            if let Some(m) = self.moments.get(snap.moment_index) {
                if m.buffer != snap.original {
                    self.history.record(Action::MomentBufferChanged {
                        index: snap.moment_index,
                        old: snap.original,
                        new: m.buffer,
                    });
                }
            }
        }
    }

    fn ensure_frame_texture(&mut self, ctx: &egui::Context, index: usize) -> Option<TextureHandle> {
        if let Some(pos) = self.frame_cache.iter().position(|(i, _)| *i == index) {
            let (_, tex) = self.frame_cache.remove(pos).unwrap();
            let handle = tex.clone();
            self.frame_cache.push_front((index, tex));
            return Some(handle);
        }

        let path = session::frame_path(&self.session.frames, index);
        let img = image::open(&path).ok()?.to_rgba8();
        let (w, h) = (img.width() as usize, img.height() as usize);
        self.frame_pixel_size = Some((w as u32, h as u32));
        let color = ColorImage::from_rgba_unmultiplied([w, h], img.as_raw());
        let tex = ctx.load_texture(
            format!("frammpeg-frame-{index}"),
            color,
            TextureOptions::LINEAR,
        );
        let handle = tex.clone();
        self.frame_cache.push_front((index, tex));
        while self.frame_cache.len() > FRAME_CACHE_SIZE {
            self.frame_cache.pop_back();
        }
        Some(handle)
    }
}

pub struct FrammpegApp {
    sessions_root: Option<PathBuf>,
    phase: Phase,
}

impl FrammpegApp {
    pub fn new(cc: &CreationContext<'_>, sessions_root: Option<PathBuf>) -> Self {
        cc.egui_ctx.set_fonts(fonts::definitions());
        cc.egui_ctx.set_visuals(theme::visuals());
        Self {
            sessions_root,
            phase: Phase::Empty,
        }
    }

    fn poll_dropped_files(&mut self, ctx: &egui::Context) -> Option<PathBuf> {
        let files = ctx.input(|i| i.raw.dropped_files.clone());
        files.into_iter().next().map(|f| f.path().to_path_buf())
    }

    fn start_extraction(&mut self, video: PathBuf) {
        let sessions_root = match &self.sessions_root {
            Some(p) => p.clone(),
            None => {
                self.phase = Phase::Error("no sessions root available".into());
                return;
            }
        };
        let session = match session::create_session(&sessions_root) {
            Ok(s) => s,
            Err(e) => {
                self.phase = Phase::Error(format!("create session dir: {e}"));
                return;
            }
        };
        let rx = spawn_extraction(video.clone(), session.frames.clone());
        self.phase = Phase::Extracting(Box::new(ExtractionState {
            session,
            video_path: video,
            current: 0,
            preparing: false,
            rx,
        }));
    }

    fn poll_extraction(&mut self, ctx: &egui::Context) {
        let Phase::Extracting(state) = &mut self.phase else {
            return;
        };
        loop {
            match state.rx.try_recv() {
                Ok(ExtractEvent::PreparingFfmpeg) => state.preparing = true,
                Ok(ExtractEvent::Progress { current }) => {
                    state.preparing = false;
                    state.current = current;
                }
                Ok(ExtractEvent::Done { total_frames, fps }) => {
                    let extracted = std::mem::replace(&mut self.phase, Phase::Empty);
                    if let Phase::Extracting(s) = extracted {
                        self.phase = Phase::Ready(Box::new(VideoState::new(
                            s.session,
                            s.video_path,
                            total_frames,
                            fps,
                            ctx,
                        )));
                    }
                    return;
                }
                Ok(ExtractEvent::Error(msg)) => {
                    self.phase = Phase::Error(msg);
                    return;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn header_label(&self) -> String {
        match &self.phase {
            Phase::Empty => "no video loaded".into(),
            Phase::Extracting(s) => {
                let name = s
                    .video_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "video".into());
                if s.preparing {
                    format!("{name}  -  preparing ffmpeg...")
                } else {
                    format!("{name}  -  extracting frame {}", s.current)
                }
            }
            Phase::Ready(v) => {
                let name = v
                    .video_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "video".into());
                format!(
                    "{name}  -  frame {}/{}",
                    v.current_frame + 1,
                    v.total_frames
                )
            }
            Phase::Error(msg) => format!("error: {msg}"),
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new("Frammpeg").color(theme::TEXT).strong());
            ui.separator();

            if let Phase::Ready(v) = &mut self.phase {
                let mut tool = v.tool;
                if ui
                    .selectable_label(tool == Tool::Rect, "Rect")
                    .on_hover_text("Draw a rectangle (click and drag on the frame)")
                    .clicked()
                {
                    tool = Tool::Rect;
                    v.commit_text_edit();
                }
                if ui
                    .selectable_label(tool == Tool::Text, "Text")
                    .on_hover_text("Add a text label (click on the frame, then type)")
                    .clicked()
                {
                    tool = Tool::Text;
                }
                v.tool = tool;

                ui.separator();
                if ui
                    .button("Mark notable")
                    .on_hover_text("Add the current frame to Moments")
                    .clicked()
                {
                    v.finalize_pending_edits();
                    let frame = v.current_frame;
                    if !v.moments.iter().any(|m| m.frame_index == frame) {
                        let moment = Moment::new(frame);
                        let index = v.moments.len();
                        v.moments.push(moment.clone());
                        v.history.record(Action::MomentCreated { index, moment });
                    }
                    v.selected_moment = v.moments.iter().position(|m| m.frame_index == frame);
                }

                ui.separator();
                if ui
                    .button("Export")
                    .on_hover_text("Write each moment's buffer + annotated frame to disk")
                    .clicked()
                {
                    v.commit_text_edit();
                    match export::export_all(
                        &v.moments,
                        &v.annotations,
                        v.total_frames,
                        &v.session.frames,
                        &v.session.export,
                    ) {
                        Ok(res) => {
                            v.last_export_msg = Some((
                                Instant::now(),
                                format!("Exported {} moment(s)", res.moments_written),
                            ));
                        }
                        Err(e) => {
                            v.last_export_msg =
                                Some((Instant::now(), format!("export failed: {e}")));
                        }
                    }
                }
                if ui
                    .button("Copy export path")
                    .on_hover_text("Copy the export folder path to the clipboard")
                    .clicked()
                {
                    match arboard::Clipboard::new()
                        .and_then(|mut cb| cb.set_text(v.session.export.display().to_string()))
                    {
                        Ok(()) => v.last_copy_at = Some(Instant::now()),
                        Err(e) => {
                            v.last_export_msg = Some((Instant::now(), format!("clipboard: {e}")));
                        }
                    }
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(self.header_label())
                        .color(theme::TEXT_MUTED)
                        .small(),
                );
            });
        });
    }

    fn bottom_panel(&mut self, ui: &mut egui::Ui) {
        match &mut self.phase {
            Phase::Ready(_) => {
                self.draw_bottom_panel_ready(ui);
            }
            _ => {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("timeline").color(theme::TEXT_MUTED));
                });
            }
        }
    }

    fn draw_bottom_panel_ready(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();

        let (enabled, playing, current_frame, prev_current_frame, total_frames) = {
            let Phase::Ready(v) = &self.phase else {
                return;
            };
            (
                v.total_frames > 0,
                v.playing,
                v.current_frame,
                v.prev_current_frame,
                v.total_frames,
            )
        };

        ui.vertical(|ui| {
            // Transport row (centered).
            let action = ui
                .allocate_ui(Vec2::new(ui.available_width(), TRANSPORT_ROW_H), |ui| {
                    transport::draw(ui, TransportView { enabled, playing })
                })
                .inner;
            if let Some(a) = action {
                self.apply_transport(a);
            }

            ui.add_space(2.0);

            // Filmstrip row.
            let strip_top = ui.cursor().top();
            if let Phase::Ready(v) = &mut self.phase {
                v.thumbs.poll(&ctx);
                let action = filmstrip::draw(
                    ui,
                    FilmstripDrawParams {
                        geom: v.filmstrip_geom,
                        total_frames: v.total_frames,
                        current_frame: v.current_frame,
                        prev_current_frame,
                        thumbs: &mut v.thumbs,
                    },
                );
                if let Some(target) = action.seek_to {
                    v.set_playing(false);
                    v.seek(target);
                }
            }

            // Corner overlay: frame counter, top-right of the strip row.
            let panel_rect = ui.max_rect();
            let label_pos = Pos2::new(panel_rect.right() - 8.0, strip_top + 8.0);
            let text = format!("{}/{}", current_frame + 1, total_frames.max(1));
            let galley = ui.painter().layout_no_wrap(
                text.clone(),
                egui::FontId::monospace(11.0),
                theme::TEXT_MUTED,
            );
            let size = galley.size() + Vec2::new(10.0, 4.0);
            let bg_rect = Rect::from_min_size(Pos2::new(label_pos.x - size.x, label_pos.y), size);
            ui.painter().rect_filled(
                bg_rect,
                CornerRadius::same(3),
                Color32::from_rgba_unmultiplied(0x1A, 0x1E, 0x23, 210),
            );
            ui.painter().text(
                bg_rect.center(),
                Align2::CENTER_CENTER,
                &text,
                egui::FontId::monospace(11.0),
                theme::TEXT_MUTED,
            );
        });
    }

    fn apply_transport(&mut self, action: TransportAction) {
        let Phase::Ready(v) = &mut self.phase else {
            return;
        };
        if v.total_frames == 0 {
            return;
        }
        let last = v.total_frames - 1;
        match action {
            TransportAction::Home => {
                v.set_playing(false);
                v.seek(0);
            }
            TransportAction::End => {
                v.set_playing(false);
                v.seek(last);
            }
            TransportAction::Back(n) => {
                v.set_playing(false);
                v.seek(v.current_frame.saturating_sub(n));
            }
            TransportAction::Fwd(n) => {
                v.set_playing(false);
                v.seek((v.current_frame + n).min(last));
            }
            TransportAction::TogglePlay => {
                v.set_playing(!v.playing);
            }
        }
    }

    fn moments_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(2.0);
        ui.label(RichText::new("Moments").color(theme::TEXT).strong());
        ui.separator();

        let Phase::Ready(v) = &mut self.phase else {
            ui.label(RichText::new("no video loaded").color(theme::TEXT_MUTED));
            return;
        };

        if v.moments.is_empty() {
            ui.label(RichText::new("no moments yet").color(theme::TEXT_MUTED));
            ui.add_space(6.0);
            ui.label(
                RichText::new("Hit 'Mark notable' on the toolbar to save the current frame.")
                    .small()
                    .color(theme::TEXT_MUTED),
            );
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut jump_to: Option<usize> = None;
            let mut delete: Option<usize> = None;
            for i in 0..v.moments.len() {
                let selected = v.selected_moment == Some(i);
                let (frame_index, note_preview, buffer) = {
                    let m = &v.moments[i];
                    let preview = m.note.lines().next().unwrap_or("").to_string();
                    let short = truncate(&preview, 32);
                    (m.frame_index, short, m.buffer)
                };
                let header = format!("Frame {}", frame_index + 1);
                let response = ui.selectable_label(
                    selected,
                    RichText::new(format!("{header}\n{note_preview}")).color(theme::TEXT),
                );
                if response.clicked() {
                    jump_to = Some(frame_index);
                    if v.selected_moment != Some(i) {
                        v.finalize_pending_edits();
                    }
                    v.selected_moment = Some(i);
                }
                if selected {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("buffer +/-").small().color(theme::TEXT_MUTED));
                        let mut b = buffer;
                        let drag_response =
                            ui.add(DragValue::new(&mut b).range(0..=MAX_BUFFER).speed(0.25));
                        let interaction_started =
                            drag_response.drag_started() || drag_response.gained_focus();
                        let interaction_ended =
                            drag_response.drag_stopped() || drag_response.lost_focus();
                        if interaction_started
                            && v.buffer_edit
                                .as_ref()
                                .map(|s| s.moment_index != i)
                                .unwrap_or(true)
                        {
                            v.buffer_edit = Some(BufferEditSnapshot {
                                moment_index: i,
                                original: buffer,
                            });
                        }
                        if drag_response.changed() {
                            v.moments[i].buffer = b;
                        }
                        if interaction_ended {
                            if let Some(snap) = v.buffer_edit.take() {
                                if snap.moment_index == i && v.moments[i].buffer != snap.original {
                                    v.history.record(Action::MomentBufferChanged {
                                        index: i,
                                        old: snap.original,
                                        new: v.moments[i].buffer,
                                    });
                                }
                            }
                        }
                    });
                    ui.add_space(4.0);
                    ui.label(RichText::new("note").small().color(theme::TEXT_MUTED));
                    let pre_note = v.moments[i].note.clone();
                    let note_response = ui.add(
                        TextEdit::multiline(&mut v.moments[i].note)
                            .desired_rows(3)
                            .desired_width(f32::INFINITY),
                    );
                    if note_response.gained_focus()
                        && v.note_edit
                            .as_ref()
                            .map(|s| s.moment_index != i)
                            .unwrap_or(true)
                    {
                        v.note_edit = Some(NoteEditSnapshot {
                            moment_index: i,
                            original: pre_note,
                        });
                    }
                    if note_response.lost_focus() {
                        if let Some(snap) = v.note_edit.take() {
                            if snap.moment_index == i && v.moments[i].note != snap.original {
                                v.history.record(Action::MomentNoteChanged {
                                    index: i,
                                    old: snap.original,
                                    new: v.moments[i].note.clone(),
                                });
                            }
                        }
                    }
                    ui.add_space(4.0);
                    if ui.button("Delete moment").clicked() {
                        delete = Some(i);
                    }
                }
                ui.add_space(6.0);
            }
            if let Some(i) = delete {
                v.finalize_pending_edits();
                let removed = v.moments.remove(i);
                v.history.record(Action::MomentDeleted {
                    index: i,
                    moment: removed,
                });
                if v.selected_moment == Some(i) {
                    v.selected_moment = None;
                } else if let Some(sel) = v.selected_moment {
                    if sel > i {
                        v.selected_moment = Some(sel - 1);
                    }
                }
            }
            if let Some(idx) = jump_to {
                v.set_playing(false);
                v.seek(idx);
            }
        });
    }

    fn viewport(&mut self, ui: &mut egui::Ui) {
        match &mut self.phase {
            Phase::Empty => {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("drop a video file here")
                            .color(theme::TEXT_MUTED)
                            .size(18.0),
                    );
                });
            }
            Phase::Error(msg) => {
                let msg = msg.clone();
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new(msg).color(Color32::from_rgb(0xE0, 0x7A, 0x88)));
                });
            }
            Phase::Extracting(s) => {
                let label = if s.preparing {
                    "Preparing ffmpeg...".to_string()
                } else {
                    format!("Extracting frame {}...", s.current)
                };
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new(label).color(theme::TEXT_MUTED).size(16.0));
                });
            }
            Phase::Ready(_) => {
                // Handled in draw_viewport_ready — split out to reduce nesting.
                self.draw_viewport_ready(ui);
            }
        }
    }

    fn draw_viewport_ready(&mut self, ui: &mut egui::Ui) {
        let Phase::Ready(v) = &mut self.phase else {
            return;
        };
        let ctx = ui.ctx().clone();
        let current = v.current_frame;
        let Some(texture) = v.ensure_frame_texture(&ctx, current) else {
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new(format!("missing frame {}", current + 1))
                        .color(theme::TEXT_MUTED),
                );
            });
            return;
        };

        let (frame_w, frame_h) = v
            .frame_pixel_size
            .map(|(w, h)| (w as f32, h as f32))
            .unwrap_or((texture.size()[0] as f32, texture.size()[1] as f32));

        let avail = ui.available_size();
        let scale = (avail.x / frame_w).min(avail.y / frame_h).max(0.001);
        let disp = Vec2::new(frame_w * scale, frame_h * scale);
        let (rect, response) = ui.allocate_exact_size(avail, Sense::click_and_drag());
        let image_rect = Rect::from_center_size(rect.center(), disp);

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, CornerRadius::ZERO, theme::CANVAS);
        painter.image(
            texture.id(),
            image_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );

        let ui_to_frame = |p: Pos2| -> (f32, f32) {
            (
                (p.x - image_rect.min.x) / scale,
                (p.y - image_rect.min.y) / scale,
            )
        };
        let frame_to_ui = |x: f32, y: f32| -> Pos2 {
            Pos2::new(image_rect.min.x + x * scale, image_rect.min.y + y * scale)
        };

        // Handle input for tools.
        let pointer_over = response.hovered();
        let pointer_pos = response.hover_pos();
        let inside_image = pointer_pos.map(|p| image_rect.contains(p)).unwrap_or(false);

        match v.tool {
            Tool::Rect => {
                if response.drag_started() && inside_image {
                    let p = pointer_pos.unwrap();
                    let (fx, fy) = ui_to_frame(p);
                    v.drag = Some(DragRect {
                        start_frame_px: (fx, fy),
                        current_frame_px: (fx, fy),
                    });
                }
                if let Some(drag) = v.drag.as_mut() {
                    if let Some(p) = pointer_pos {
                        let (fx, fy) = ui_to_frame(p);
                        drag.current_frame_px = (fx, fy);
                    }
                }
                if response.drag_stopped() {
                    if let Some(drag) = v.drag.take() {
                        let (x0, y0) = drag.start_frame_px;
                        let (x1, y1) = drag.current_frame_px;
                        let x = x0.min(x1);
                        let y = y0.min(y1);
                        let w = (x1 - x0).abs();
                        let h = (y1 - y0).abs();
                        if w >= 2.0 && h >= 2.0 {
                            let annotation = Annotation::Rect {
                                x,
                                y,
                                w,
                                h,
                                stroke_color: DEFAULT_STROKE_RGBA,
                                stroke_width: DEFAULT_STROKE_WIDTH,
                            };
                            let list = v.annotations.entry(current).or_default();
                            let index = list.len();
                            list.push(annotation.clone());
                            v.history.record(Action::AnnotationCreated {
                                frame: current,
                                index,
                                annotation,
                            });
                        }
                    }
                }
            }
            Tool::Text => {
                if response.clicked() && inside_image {
                    let p = pointer_pos.unwrap();
                    let (fx, fy) = ui_to_frame(p);
                    v.commit_text_edit();
                    let list = v.annotations.entry(current).or_default();
                    list.push(Annotation::Text {
                        x: fx,
                        y: fy,
                        text: String::new(),
                        font_size: DEFAULT_FONT_SIZE,
                        color: DEFAULT_TEXT_RGBA,
                    });
                    v.editing_text = Some(list.len() - 1);
                    v.text_buffer.clear();
                    v.text_focus_pending = true;
                }
            }
        }

        // Draw committed annotations for this frame.
        if let Some(list) = v.annotations.get(&current) {
            for (i, a) in list.iter().enumerate() {
                match a {
                    Annotation::Rect {
                        x,
                        y,
                        w,
                        h,
                        stroke_color,
                        stroke_width,
                    } => {
                        let a_ui = frame_to_ui(*x, *y);
                        let b_ui = frame_to_ui(*x + *w, *y + *h);
                        let rect = Rect::from_two_pos(a_ui, b_ui);
                        painter.rect_stroke(
                            rect,
                            CornerRadius::ZERO,
                            Stroke::new(
                                stroke_width.max(1.0),
                                Color32::from_rgba_unmultiplied(
                                    stroke_color[0],
                                    stroke_color[1],
                                    stroke_color[2],
                                    stroke_color[3],
                                ),
                            ),
                            egui::StrokeKind::Outside,
                        );
                    }
                    Annotation::Text {
                        x,
                        y,
                        text,
                        font_size,
                        color,
                    } => {
                        if Some(i) == v.editing_text {
                            continue;
                        }
                        let pos = frame_to_ui(*x, *y);
                        let px = font_size * scale;
                        painter.text(
                            pos,
                            Align2::LEFT_TOP,
                            text,
                            egui::FontId::proportional(px.max(6.0)),
                            Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3]),
                        );
                    }
                }
            }
        }

        // Draw in-progress drag rect.
        if let Some(drag) = &v.drag {
            let a_ui = frame_to_ui(drag.start_frame_px.0, drag.start_frame_px.1);
            let b_ui = frame_to_ui(drag.current_frame_px.0, drag.current_frame_px.1);
            let rect = Rect::from_two_pos(a_ui, b_ui);
            painter.rect_stroke(
                rect,
                CornerRadius::ZERO,
                Stroke::new(
                    DEFAULT_STROKE_WIDTH,
                    Color32::from_rgba_unmultiplied(
                        DEFAULT_STROKE_RGBA[0],
                        DEFAULT_STROKE_RGBA[1],
                        DEFAULT_STROKE_RGBA[2],
                        (DEFAULT_STROKE_RGBA[3] as u16 * 3 / 4) as u8,
                    ),
                ),
                egui::StrokeKind::Outside,
            );
        }

        // Inline TextEdit for the pending text annotation, anchored at its screen pos.
        if let Some(idx) = v.editing_text {
            let list = v.annotations.entry(current).or_default();
            if let Some(Annotation::Text {
                x,
                y,
                font_size,
                color,
                ..
            }) = list.get(idx).cloned()
            {
                let pos = frame_to_ui(x, y);
                let area = Area::new(egui::Id::new(("frammpeg-text-edit", idx)))
                    .order(Order::Foreground)
                    .fixed_pos(pos);
                let mut request_focus = v.text_focus_pending;
                area.show(ui.ctx(), |ui| {
                    let color =
                        Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3]);
                    let font = egui::FontId::proportional((font_size * scale).max(10.0));
                    let response = ui.add(
                        TextEdit::singleline(&mut v.text_buffer)
                            .desired_width(240.0)
                            .text_color(color)
                            .font(font)
                            .hint_text("label"),
                    );
                    if request_focus {
                        response.request_focus();
                        request_focus = false;
                    }
                    if response.lost_focus()
                        || ui.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Escape))
                    {
                        v.commit_text_edit();
                    }
                });
                v.text_focus_pending = request_focus;
            }
        }

        // Toasts.
        if let Some(t) = v.last_copy_at {
            if t.elapsed() < Duration::from_millis(COPY_TOAST_MS) {
                let toast_pos = Pos2::new(rect.right() - 16.0, rect.top() + 16.0);
                let text = "Copied!";
                let galley = ui.painter().layout_no_wrap(
                    text.to_string(),
                    egui::FontId::proportional(14.0),
                    theme::TEXT,
                );
                let size = galley.size() + Vec2::new(14.0, 8.0);
                let bg_rect =
                    Rect::from_min_size(Pos2::new(toast_pos.x - size.x, toast_pos.y), size);
                painter.rect_filled(
                    bg_rect,
                    CornerRadius::same(4),
                    Color32::from_rgba_unmultiplied(0x1A, 0x1E, 0x23, 235),
                );
                painter.rect_stroke(
                    bg_rect,
                    CornerRadius::same(4),
                    Stroke::new(1.0, theme::ACCENT),
                    egui::StrokeKind::Outside,
                );
                painter.text(
                    bg_rect.center(),
                    Align2::CENTER_CENTER,
                    text,
                    egui::FontId::proportional(14.0),
                    theme::TEXT,
                );
                ui.ctx().request_repaint_after(Duration::from_millis(100));
            } else {
                v.last_copy_at = None;
            }
        }
        if let Some((t, msg)) = v.last_export_msg.clone() {
            if t.elapsed() < Duration::from_millis(3000) {
                let text = msg;
                let galley = ui.painter().layout_no_wrap(
                    text.clone(),
                    egui::FontId::proportional(13.0),
                    theme::TEXT,
                );
                let size = galley.size() + Vec2::new(14.0, 8.0);
                let bg_rect = Rect::from_min_size(
                    Pos2::new(rect.left() + 16.0, rect.bottom() - size.y - 16.0),
                    size,
                );
                painter.rect_filled(
                    bg_rect,
                    CornerRadius::same(4),
                    Color32::from_rgba_unmultiplied(0x1A, 0x1E, 0x23, 235),
                );
                painter.rect_stroke(
                    bg_rect,
                    CornerRadius::same(4),
                    Stroke::new(1.0, theme::STROKE_STRONG),
                    egui::StrokeKind::Outside,
                );
                painter.text(
                    bg_rect.center(),
                    Align2::CENTER_CENTER,
                    &text,
                    egui::FontId::proportional(13.0),
                    theme::TEXT,
                );
                ui.ctx().request_repaint_after(Duration::from_millis(200));
            } else {
                v.last_export_msg = None;
            }
        }

        // Cursor hint while hovering with a tool.
        if pointer_over && inside_image {
            ui.ctx().set_cursor_icon(match v.tool {
                Tool::Rect => egui::CursorIcon::Crosshair,
                Tool::Text => egui::CursorIcon::Text,
            });
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        let Phase::Ready(v) = &mut self.phase else {
            return;
        };
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let total = v.total_frames;
        if total == 0 {
            return;
        }
        let last = total - 1;
        let mut cur = v.current_frame;
        let mut toggle_play = false;
        let mut undo_requested = false;
        let mut redo_requested = false;
        ctx.input(|i| {
            for e in &i.events {
                if let Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = e
                {
                    if modifiers.command {
                        match key {
                            Key::Z if modifiers.shift => redo_requested = true,
                            Key::Z => undo_requested = true,
                            Key::Y if !modifiers.shift => redo_requested = true,
                            _ => {}
                        }
                        continue;
                    }
                    let big = modifiers.shift;
                    let step = if big { 10 } else { 1 };
                    match key {
                        Key::ArrowLeft => cur = cur.saturating_sub(step),
                        Key::ArrowRight => cur = (cur + step).min(last),
                        Key::Home => cur = 0,
                        Key::End => cur = last,
                        Key::Comma => cur = cur.saturating_sub(1),
                        Key::Period => cur = (cur + 1).min(last),
                        Key::Space => toggle_play = true,
                        _ => {}
                    }
                }
            }
        });
        if cur != v.current_frame {
            v.set_playing(false);
            v.current_frame = cur;
            v.commit_text_edit();
        }
        if toggle_play {
            v.set_playing(!v.playing);
        }
        if undo_requested {
            v.finalize_pending_edits();
            v.commit_text_edit();
            let mut state = HistoryState {
                annotations: &mut v.annotations,
                moments: &mut v.moments,
            };
            if let Some(action) = v.history.undo(&mut state) {
                v.after_history_change(&action);
            }
        } else if redo_requested {
            v.finalize_pending_edits();
            v.commit_text_edit();
            let mut state = HistoryState {
                annotations: &mut v.annotations,
                moments: &mut v.moments,
            };
            if let Some(action) = v.history.redo(&mut state) {
                v.after_history_change(&action);
            }
        }
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n).collect();
        out.push_str("...");
        out
    }
}

impl eframe::App for FrammpegApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_extraction(&ctx);

        // Snapshot prev_current_frame at the start of the frame so the
        // filmstrip can auto-scroll on any change made this frame.
        if let Phase::Ready(v) = &mut self.phase {
            v.prev_current_frame = v.current_frame;
        }

        // Drive the play loop before handling keys, so a Space toggle takes
        // effect on the next frame, not this one.
        if let Phase::Ready(v) = &mut self.phase {
            v.tick_play();
        }

        self.handle_keys(&ctx);

        if let Some(video) = self.poll_dropped_files(&ctx) {
            match &self.phase {
                Phase::Extracting(_) => {
                    // Drop-ignored while a previous extraction is in flight.
                }
                _ => self.start_extraction(video),
            }
        }

        let panel_frame = Frame::default().fill(theme::PANEL).inner_margin(6.0);

        Panel::top("toolbar")
            .exact_size(40.0)
            .resizable(false)
            .frame(panel_frame)
            .show(ui, |ui| self.toolbar(ui));

        Panel::bottom("timeline")
            .exact_size(BOTTOM_PANEL_H)
            .resizable(false)
            .frame(panel_frame)
            .show(ui, |ui| self.bottom_panel(ui));

        Panel::right("moments")
            .default_size(260.0)
            .min_size(220.0)
            .frame(Frame::default().fill(theme::PANEL).inner_margin(10.0))
            .show(ui, |ui| self.moments_panel(ui));

        CentralPanel::default()
            .frame(Frame::default().fill(theme::CANVAS).inner_margin(8.0))
            .show(ui, |ui| self.viewport(ui));

        // Schedule the next play-tick repaint. Also throttles idle CPU.
        if let Phase::Ready(v) = &self.phase {
            if v.playing {
                ctx.request_repaint_after(transport::frame_period(v.fps));
            }
        }
    }
}
