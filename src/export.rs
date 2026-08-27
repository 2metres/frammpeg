use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use ab_glyph::FontRef;
use chrono::Local;

use crate::annotate;
use crate::model::{buffer_range, Annotation, Moment};
use crate::session;

/// One planned output file for a moment's export folder.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedFile {
    pub source: PathBuf,
    pub target: PathBuf,
    pub burn_annotated: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlannedMoment {
    pub dir: PathBuf,
    pub note_path: PathBuf,
    pub note_body: String,
    pub files: Vec<PlannedFile>,
}

fn moment_dir_name(one_based: usize) -> String {
    format!("moment-{:02}", one_based)
}

fn frame_filename(disk_index: usize, annotated: bool) -> String {
    if annotated {
        format!("frame-{:04}-annotated.png", disk_index)
    } else {
        format!("frame-{:04}.png", disk_index)
    }
}

pub fn plan_moment(
    moment: &Moment,
    one_based_index: usize,
    total_frames: usize,
    frames_dir: &Path,
    export_root: &Path,
) -> Option<PlannedMoment> {
    let (lo, hi) = buffer_range(moment.frame_index, moment.buffer, total_frames)?;
    let dir = export_root.join(moment_dir_name(one_based_index));

    let mut files = Vec::with_capacity(hi - lo + 1);
    for idx in lo..=hi {
        let disk_index = idx + 1;
        let source = session::frame_path(frames_dir, idx);
        let annotated = idx == moment.frame_index;
        let target = dir.join(frame_filename(disk_index, annotated));
        files.push(PlannedFile {
            source,
            target,
            burn_annotated: annotated,
        });
    }

    let note_path = dir.join("note.md");
    let note_body = note_markdown(moment);

    Some(PlannedMoment {
        dir,
        note_path,
        note_body,
        files,
    })
}

fn note_markdown(moment: &Moment) -> String {
    let stamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let disk_index = moment.frame_index + 1;
    let body = if moment.note.trim().is_empty() {
        "(no note)".to_string()
    } else {
        moment.note.trim().to_string()
    };
    format!(
        "# Frame {disk_index}\n\nExported: {stamp}\nBuffer: +/- {buf} frames\n\n{body}\n",
        buf = moment.buffer,
    )
}

pub fn write_planned(
    plan: &PlannedMoment,
    annotations: &[Annotation],
    font: &FontRef<'_>,
) -> io::Result<()> {
    std::fs::create_dir_all(&plan.dir)?;
    for file in &plan.files {
        if file.burn_annotated {
            burn_and_save(&file.source, &file.target, annotations, font)?;
        } else {
            std::fs::copy(&file.source, &file.target)?;
        }
    }
    std::fs::write(&plan.note_path, &plan.note_body)?;
    Ok(())
}

fn burn_and_save(
    source: &Path,
    target: &Path,
    annotations: &[Annotation],
    font: &FontRef<'_>,
) -> io::Result<()> {
    let img = image::open(source)
        .map_err(|e| io::Error::other(format!("open {}: {e}", source.display())))?;
    let mut rgba = img.to_rgba8();
    annotate::burn(&mut rgba, annotations, font);
    rgba.save(target)
        .map_err(|e| io::Error::other(format!("save {}: {e}", target.display())))?;
    Ok(())
}

pub struct ExportResult {
    pub moments_written: usize,
}

pub fn export_all(
    moments: &[Moment],
    annotations: &HashMap<usize, Vec<Annotation>>,
    total_frames: usize,
    frames_dir: &Path,
    export_root: &Path,
) -> io::Result<ExportResult> {
    std::fs::create_dir_all(export_root)?;
    let font = annotate::font();
    let mut written = 0usize;
    for (i, moment) in moments.iter().enumerate() {
        let plan = match plan_moment(moment, i + 1, total_frames, frames_dir, export_root) {
            Some(p) => p,
            None => continue,
        };
        let empty = Vec::new();
        let anns = annotations.get(&moment.frame_index).unwrap_or(&empty);
        write_planned(&plan, anns, &font)?;
        written += 1;
    }
    Ok(ExportResult {
        moments_written: written,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DEFAULT_BUFFER, DEFAULT_STROKE_RGBA, DEFAULT_STROKE_WIDTH};

    #[test]
    fn plan_layout_for_middle_frame() {
        let moment = Moment {
            frame_index: 40,
            buffer: 5,
            note: "glitch".into(),
        };
        let plan = plan_moment(
            &moment,
            1,
            100,
            Path::new("/tmp/frames"),
            Path::new("/tmp/export"),
        )
        .unwrap();
        assert_eq!(plan.dir, PathBuf::from("/tmp/export/moment-01"));
        assert_eq!(plan.files.len(), 11);
        // First and last are clean, annotated one is at index 40 (disk 0041).
        assert!(plan.files[0].target.ends_with("frame-0036.png"));
        assert!(!plan.files[0].burn_annotated);
        assert!(plan.files[10].target.ends_with("frame-0046.png"));
        assert!(!plan.files[10].burn_annotated);
        let noted = plan.files.iter().find(|f| f.burn_annotated).unwrap();
        assert!(noted.target.ends_with("frame-0041-annotated.png"));
        assert!(noted.source.ends_with("frame-0041.png"));
    }

    #[test]
    fn plan_clamps_low() {
        let moment = Moment {
            frame_index: 2,
            buffer: 5,
            note: String::new(),
        };
        let plan = plan_moment(
            &moment,
            2,
            100,
            Path::new("/tmp/frames"),
            Path::new("/tmp/export"),
        )
        .unwrap();
        assert_eq!(plan.dir, PathBuf::from("/tmp/export/moment-02"));
        // indices 0..=7 -> 8 files
        assert_eq!(plan.files.len(), 8);
        assert!(plan.files[0].target.ends_with("frame-0001.png"));
        assert!(plan.files[7].target.ends_with("frame-0008.png"));
    }

    #[test]
    fn plan_clamps_high() {
        let moment = Moment {
            frame_index: 97,
            buffer: 5,
            note: String::new(),
        };
        let plan = plan_moment(
            &moment,
            3,
            100,
            Path::new("/tmp/frames"),
            Path::new("/tmp/export"),
        )
        .unwrap();
        assert_eq!(plan.files.len(), 8);
        assert!(plan.files[0].target.ends_with("frame-0093.png"));
        assert!(plan.files[7].target.ends_with("frame-0100.png"));
    }

    #[test]
    fn plan_none_for_empty_video() {
        let moment = Moment {
            frame_index: 0,
            buffer: 5,
            note: String::new(),
        };
        assert!(plan_moment(
            &moment,
            1,
            0,
            Path::new("/tmp/frames"),
            Path::new("/tmp/export")
        )
        .is_none());
    }

    #[test]
    fn note_body_includes_frame_and_buffer() {
        let m = Moment {
            frame_index: 40,
            buffer: DEFAULT_BUFFER,
            note: "  a note  ".to_string(),
        };
        let body = note_markdown(&m);
        assert!(body.contains("Frame 41"));
        assert!(body.contains("Buffer: +/- 5"));
        assert!(body.contains("a note"));
    }

    #[test]
    fn note_body_placeholder_when_empty() {
        let m = Moment {
            frame_index: 0,
            buffer: 1,
            note: "   ".to_string(),
        };
        assert!(note_markdown(&m).contains("(no note)"));
    }

    #[test]
    fn end_to_end_export_writes_files() {
        // Build a tiny fake session with two clean frames, export a moment
        // with buffer 0 (single frame) and one rectangle annotation.
        let tmp = tempdir();
        let frames = tmp.join("frames");
        let export = tmp.join("export");
        std::fs::create_dir_all(&frames).unwrap();
        std::fs::create_dir_all(&export).unwrap();

        let white = image::RgbaImage::from_pixel(20, 20, image::Rgba([255, 255, 255, 255]));
        white.save(session::frame_path(&frames, 0)).unwrap();
        white.save(session::frame_path(&frames, 1)).unwrap();

        let moment = Moment {
            frame_index: 0,
            buffer: 0,
            note: "boxy".to_string(),
        };
        let mut anns = HashMap::new();
        anns.insert(
            0usize,
            vec![Annotation::Rect {
                x: 2.0,
                y: 2.0,
                w: 10.0,
                h: 10.0,
                stroke_color: DEFAULT_STROKE_RGBA,
                stroke_width: DEFAULT_STROKE_WIDTH,
            }],
        );

        let result = export_all(&[moment], &anns, 2, &frames, &export).unwrap();
        assert_eq!(result.moments_written, 1);

        let moment_dir = export.join("moment-01");
        let annotated = moment_dir.join("frame-0001-annotated.png");
        let note = moment_dir.join("note.md");
        assert!(annotated.exists(), "expected {:?}", annotated);
        assert!(note.exists(), "expected {:?}", note);
        // The clean single-frame case emits only the annotated one for that index.
        assert!(!moment_dir.join("frame-0001.png").exists());

        let img = image::open(&annotated).unwrap().to_rgba8();
        assert_eq!(
            img.get_pixel(2, 5).0,
            DEFAULT_STROKE_RGBA,
            "annotation should be burned into export"
        );

        let note_body = std::fs::read_to_string(&note).unwrap();
        assert!(note_body.contains("boxy"));
        assert!(note_body.contains("Frame 1"));

        std::fs::remove_dir_all(&tmp).ok();
    }

    fn tempdir() -> PathBuf {
        let base = std::env::temp_dir();
        let unique = format!(
            "frammpeg-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let p = base.join(unique);
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
