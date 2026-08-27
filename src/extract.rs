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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::Duration;

    /// End-to-end sanity check: generate a 3-second testsrc video, extract
    /// all its frames, then run the export pipeline on one moment. Requires
    /// ffmpeg to be downloaded (or downloadable), so it is `#[ignore]` by
    /// default. Run with `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn end_to_end_extract_and_export() {
        use crate::annotate;
        use crate::export;
        use crate::model::{Annotation, Moment, DEFAULT_STROKE_RGBA, DEFAULT_STROKE_WIDTH};

        ffmpeg_sidecar::download::auto_download().expect("auto_download");

        let tmp = std::env::temp_dir().join(format!(
            "frammpeg-e2e-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        let video = tmp.join("testsrc.mp4");
        let frames = tmp.join("frames");
        let export_root = tmp.join("export");
        std::fs::create_dir_all(&frames).unwrap();
        std::fs::create_dir_all(&export_root).unwrap();

        // Generate a 3-second testsrc video at 30fps -> 90 frames.
        let status = FfmpegCommand::new()
            .hide_banner()
            .overwrite()
            .args([
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=3:size=320x240:rate=30",
            ])
            .output(video.to_string_lossy().as_ref())
            .spawn()
            .expect("spawn testsrc")
            .wait()
            .expect("testsrc exit");
        assert!(status.success(), "testsrc generation failed: {status:?}");
        assert!(video.exists(), "no testsrc output at {video:?}");

        // Run the real extraction pipeline.
        let rx = spawn_extraction(video.clone(), frames.clone());
        let total_frames;
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        loop {
            if std::time::Instant::now() > deadline {
                panic!("extraction timed out");
            }
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(ExtractEvent::Done { total_frames: n }) => {
                    total_frames = n;
                    break;
                }
                Ok(ExtractEvent::Error(e)) => panic!("extraction error: {e}"),
                Ok(_) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    panic!("extraction channel closed without Done");
                }
            }
        }
        assert!(
            (60..=100).contains(&total_frames),
            "expected ~90 frames, got {total_frames}"
        );

        // Export one moment mid-clip with a rectangle annotation.
        let moment = Moment {
            frame_index: 30,
            buffer: 3,
            note: "middle glitch".to_string(),
        };
        let mut anns: HashMap<usize, Vec<Annotation>> = HashMap::new();
        anns.insert(
            30,
            vec![Annotation::Rect {
                x: 40.0,
                y: 40.0,
                w: 80.0,
                h: 80.0,
                stroke_color: DEFAULT_STROKE_RGBA,
                stroke_width: DEFAULT_STROKE_WIDTH,
            }],
        );

        let result =
            export::export_all(&[moment], &anns, total_frames, &frames, &export_root).unwrap();
        assert_eq!(result.moments_written, 1);

        // The moment folder must exist with the right filenames.
        let dir = export_root.join("moment-01");
        assert!(dir.exists(), "missing {dir:?}");
        for n in 28..=34 {
            let want = if n == 31 {
                // moment.frame_index is 30 (0-based) -> disk name is 0031.
                dir.join(format!("frame-{n:04}-annotated.png"))
            } else {
                dir.join(format!("frame-{n:04}.png"))
            };
            assert!(want.exists(), "missing {want:?}");
        }
        assert!(dir.join("note.md").exists());

        // The annotated PNG must be a valid image and readable.
        let img = image::open(dir.join("frame-0031-annotated.png"))
            .expect("open annotated")
            .to_rgba8();
        assert_eq!(img.width(), 320);
        assert_eq!(img.height(), 240);

        let _ = annotate::font(); // touched — ensures burn path linked

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
