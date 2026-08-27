use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::FfmpegEvent;

#[derive(Debug, Clone)]
pub enum ExtractEvent {
    PreparingFfmpeg,
    Progress { current: usize },
    Done { total_frames: usize },
    Error(String),
}

/// Spawn a background thread that runs ffmpeg to write PNG frames to
/// `frames_dir/frame-%04d.png` at the video's native fps. Events stream over
/// the returned receiver; the sender is dropped when the thread exits.
pub fn spawn_extraction(video: PathBuf, frames_dir: PathBuf) -> Receiver<ExtractEvent> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || run_extraction(video, frames_dir, tx));
    rx
}

fn run_extraction(video: PathBuf, frames_dir: PathBuf, tx: Sender<ExtractEvent>) {
    let _ = tx.send(ExtractEvent::PreparingFfmpeg);
    if let Err(e) = ffmpeg_sidecar::download::auto_download() {
        let _ = tx.send(ExtractEvent::Error(format!("prepare ffmpeg: {e}")));
        return;
    }

    let pattern = frame_pattern(&frames_dir);
    let mut child = match FfmpegCommand::new()
        .hide_banner()
        .overwrite()
        .input(video.to_string_lossy().as_ref())
        .args(["-fps_mode", "passthrough"])
        .output(&pattern)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(ExtractEvent::Error(format!("spawn ffmpeg: {e}")));
            return;
        }
    };

    let iter = match child.iter() {
        Ok(i) => i,
        Err(e) => {
            let _ = tx.send(ExtractEvent::Error(format!("read ffmpeg output: {e}")));
            return;
        }
    };

    let mut last_error: Option<String> = None;
    for event in iter {
        match event {
            FfmpegEvent::Progress(p) => {
                let _ = tx.send(ExtractEvent::Progress {
                    current: p.frame as usize,
                });
            }
            FfmpegEvent::Error(msg) => {
                last_error = Some(msg);
            }
            FfmpegEvent::Log(ffmpeg_sidecar::event::LogLevel::Fatal, msg) => {
                last_error = Some(msg);
            }
            _ => {}
        }
    }

    if let Err(e) = child.wait() {
        let _ = tx.send(ExtractEvent::Error(format!("ffmpeg exit: {e}")));
        return;
    }

    match crate::session::count_frames(&frames_dir) {
        Ok(0) => {
            let msg = last_error.unwrap_or_else(|| {
                "ffmpeg produced no frames — is the file a supported video?".to_string()
            });
            let _ = tx.send(ExtractEvent::Error(msg));
        }
        Ok(n) => {
            let _ = tx.send(ExtractEvent::Done { total_frames: n });
        }
        Err(e) => {
            let _ = tx.send(ExtractEvent::Error(format!("count frames: {e}")));
        }
    }
}

fn frame_pattern(frames_dir: &Path) -> String {
    frames_dir
        .join("frame-%04d.png")
        .to_string_lossy()
        .into_owned()
}
