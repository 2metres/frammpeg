use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use ab_glyph::FontRef;
use chrono::Local;
use serde::Serialize;

use crate::annotate;
use crate::model::{buffer_range_within, Annotation, Moment};
use crate::session;

#[derive(Debug, Serialize)]
pub struct ExportManifest {
    pub export: ExportMeta,
    pub moments: Vec<MomentEntry>,
}

#[derive(Debug, Serialize)]
pub struct ExportMeta {
    pub timestamp: String,
    pub source_range: [usize; 2],
    pub total_frames: usize,
    pub video: String,
}

#[derive(Debug, Serialize)]
pub struct MomentEntry {
    pub frame: usize,
    pub buffer: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub annotations: Vec<AnnotationEntry>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AnnotationEntry {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        stroke: String,
        stroke_width: f32,
    },
    Text {
        x: f32,
        y: f32,
        text: String,
        font_size: f32,
        color: String,
    },
}

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
    trim_start: usize,
    trim_end: usize,
    frames_dir: &Path,
    export_root: &Path,
) -> Option<PlannedMoment> {
    if total_frames == 0 {
        return None;
    }
    if moment.frame_index < trim_start || moment.frame_index > trim_end {
        return None;
    }
    let (lo, hi) = buffer_range_within(moment.frame_index, moment.buffer, trim_start, trim_end)?;
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
    let note_body = note_markdown(moment, trim_start, trim_end, total_frames);

    Some(PlannedMoment {
        dir,
        note_path,
        note_body,
        files,
    })
}

fn note_markdown(
    moment: &Moment,
    trim_start: usize,
    trim_end: usize,
    total_frames: usize,
) -> String {
    let stamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let disk_index = moment.frame_index + 1;
    let body = if moment.note.trim().is_empty() {
        "(no note)".to_string()
    } else {
        moment.note.trim().to_string()
    };
    format!(
        "# Frame {disk_index}\n\n\
         Exported: {stamp}\n\
         Source range: {r_lo}\u{2013}{r_hi} / {total}\n\
         Buffer: +/- {buf} frames\n\n\
         {body}\n",
        r_lo = trim_start + 1,
        r_hi = trim_end + 1,
        total = total_frames,
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

fn rgba_to_hex(rgba: [u8; 4]) -> String {
    if rgba[3] == 255 {
        format!("#{:02X}{:02X}{:02X}", rgba[0], rgba[1], rgba[2])
    } else {
        format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            rgba[0], rgba[1], rgba[2], rgba[3]
        )
    }
}

fn annotation_to_entry(ann: &Annotation) -> AnnotationEntry {
    match ann {
        Annotation::Rect {
            x,
            y,
            w,
            h,
            stroke_color,
            stroke_width,
        } => AnnotationEntry::Rect {
            x: *x,
            y: *y,
            w: *w,
            h: *h,
            stroke: rgba_to_hex(*stroke_color),
            stroke_width: *stroke_width,
        },
        Annotation::Text {
            x,
            y,
            text,
            font_size,
            color,
        } => AnnotationEntry::Text {
            x: *x,
            y: *y,
            text: text.clone(),
            font_size: *font_size,
            color: rgba_to_hex(*color),
        },
    }
}

pub fn write_moments_yaml(export_root: &Path, manifest: &ExportManifest) -> io::Result<()> {
    let yaml_path = export_root.join("moments.yaml");
    let yaml_str = serde_saphyr::to_string(manifest)
        .map_err(|e| io::Error::other(format!("serialize manifest: {e}")))?;
    std::fs::write(&yaml_path, yaml_str)?;
    Ok(())
}

pub struct ExportResult {
    pub moments_written: usize,
}

pub fn export_all(
    moments: &[Moment],
    annotations: &HashMap<usize, Vec<Annotation>>,
    total_frames: usize,
    trim_start: usize,
    trim_end: usize,
    frames_dir: &Path,
    export_root: &Path,
) -> io::Result<ExportResult> {
    std::fs::create_dir_all(export_root)?;
    let font = annotate::font();
    let mut written = 0usize;
    // Walk moments in order but only assign folder numbers to the in-range
    // ones so `moment-01`, `moment-02` etc. stay contiguous in the export.
    let mut folder_index = 0usize;
    for moment in moments {
        if moment.frame_index < trim_start || moment.frame_index > trim_end {
            continue;
        }
        folder_index += 1;
        let plan = match plan_moment(
            moment,
            folder_index,
            total_frames,
            trim_start,
            trim_end,
            frames_dir,
            export_root,
        ) {
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
            0,
            99,
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
            0,
            99,
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
            0,
            99,
            Path::new("/tmp/frames"),
            Path::new("/tmp/export"),
        )
        .unwrap();
        assert_eq!(plan.files.len(), 8);
        assert!(plan.files[0].target.ends_with("frame-0093.png"));
        assert!(plan.files[7].target.ends_with("frame-0100.png"));
    }

    #[test]
    fn plan_clamps_to_trim_range() {
        let moment = Moment {
            frame_index: 50,
            buffer: 10,
            note: String::new(),
        };
        let plan = plan_moment(
            &moment,
            1,
            100,
            45,
            55,
            Path::new("/tmp/frames"),
            Path::new("/tmp/export"),
        )
        .unwrap();
        // Buffer would have been [40, 60] but trim caps it to [45, 55] = 11 files.
        assert_eq!(plan.files.len(), 11);
        assert!(plan.files[0].target.ends_with("frame-0046.png"));
        assert!(plan.files[10].target.ends_with("frame-0056.png"));
    }

    #[test]
    fn plan_none_when_moment_outside_trim_range() {
        let moment = Moment {
            frame_index: 5,
            buffer: 0,
            note: String::new(),
        };
        assert!(plan_moment(
            &moment,
            1,
            100,
            10,
            30,
            Path::new("/tmp/frames"),
            Path::new("/tmp/export"),
        )
        .is_none());
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
            0,
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
        let body = note_markdown(&m, 0, 99, 100);
        assert!(body.contains("Frame 41"));
        assert!(body.contains("Buffer: +/- 5"));
        assert!(body.contains("a note"));
    }

    #[test]
    fn note_body_includes_source_range() {
        let m = Moment {
            frame_index: 40,
            buffer: DEFAULT_BUFFER,
            note: "n".to_string(),
        };
        let body = note_markdown(&m, 10, 60, 100);
        assert!(
            body.contains("Source range: 11\u{2013}61 / 100"),
            "note body missing source range line: {body}"
        );
    }

    #[test]
    fn note_body_placeholder_when_empty() {
        let m = Moment {
            frame_index: 0,
            buffer: 1,
            note: "   ".to_string(),
        };
        assert!(note_markdown(&m, 0, 9, 10).contains("(no note)"));
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

        let result = export_all(&[moment], &anns, 2, 0, 1, &frames, &export).unwrap();
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

    #[test]
    fn export_all_skips_moments_outside_trim() {
        let tmp = tempdir();
        let frames = tmp.join("frames");
        let export = tmp.join("export");
        std::fs::create_dir_all(&frames).unwrap();
        std::fs::create_dir_all(&export).unwrap();
        let white = image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 255, 255, 255]));
        for i in 0..10 {
            white.save(session::frame_path(&frames, i)).unwrap();
        }

        let moments = vec![
            Moment {
                frame_index: 1,
                buffer: 0,
                note: "out-low".into(),
            },
            Moment {
                frame_index: 5,
                buffer: 0,
                note: "in".into(),
            },
            Moment {
                frame_index: 9,
                buffer: 0,
                note: "out-high".into(),
            },
        ];
        let anns = HashMap::new();
        // Trim range [3, 7] excludes the first and third moments.
        let result = export_all(&moments, &anns, 10, 3, 7, &frames, &export).unwrap();
        assert_eq!(result.moments_written, 1);
        assert!(export.join("moment-01").exists());
        assert!(!export.join("moment-02").exists());
        let note_body = std::fs::read_to_string(export.join("moment-01/note.md")).unwrap();
        assert!(note_body.contains("in"));
        assert!(note_body.contains("Source range: 4\u{2013}8 / 10"));

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
