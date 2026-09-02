use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use ab_glyph::FontRef;
use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::annotate;
use crate::model::{buffer_range_within, Annotation, Moment};
use crate::session;

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportManifest {
    pub export: ExportMeta,
    pub moments: Vec<MomentEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportMeta {
    pub timestamp: String,
    pub source_range: [usize; 2],
    pub total_frames: usize,
    pub video: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MomentEntry {
    pub frame: usize,
    pub buffer: usize,
    pub note: Option<String>,
    pub annotations: Vec<AnnotationEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
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

    Some(PlannedMoment { dir, files })
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

#[allow(clippy::too_many_arguments)]
pub fn export_all(
    moments: &[Moment],
    annotations: &HashMap<usize, Vec<Annotation>>,
    total_frames: usize,
    trim_start: usize,
    trim_end: usize,
    frames_dir: &Path,
    export_root: &Path,
    video_path: &Path,
) -> io::Result<ExportResult> {
    std::fs::create_dir_all(export_root)?;
    let font = annotate::font();
    let mut written = 0usize;
    let mut moment_entries = Vec::new();

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

        let note = if moment.note.trim().is_empty() {
            None
        } else {
            Some(moment.note.trim().to_string())
        };

        moment_entries.push(MomentEntry {
            frame: moment.frame_index + 1,
            buffer: moment.buffer,
            note,
            annotations: anns.iter().map(annotation_to_entry).collect(),
        });

        written += 1;
    }

    let timestamp = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let video_filename = video_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    let manifest = ExportManifest {
        export: ExportMeta {
            timestamp,
            source_range: [trim_start + 1, trim_end + 1],
            total_frames,
            video: video_filename,
        },
        moments: moment_entries,
    };

    write_moments_yaml(export_root, &manifest)?;

    Ok(ExportResult {
        moments_written: written,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DEFAULT_STROKE_RGBA, DEFAULT_STROKE_WIDTH};

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

        let video = tmp.join("test.mp4");
        let result = export_all(&[moment], &anns, 2, 0, 1, &frames, &export, &video).unwrap();
        assert_eq!(result.moments_written, 1);

        let moment_dir = export.join("moment-01");
        let annotated = moment_dir.join("frame-0001-annotated.png");
        assert!(annotated.exists(), "expected {:?}", annotated);
        // The clean single-frame case emits only the annotated one for that index.
        assert!(!moment_dir.join("frame-0001.png").exists());

        let img = image::open(&annotated).unwrap().to_rgba8();
        assert_eq!(
            img.get_pixel(2, 5).0,
            DEFAULT_STROKE_RGBA,
            "annotation should be burned into export"
        );

        let yaml_path = export.join("moments.yaml");
        assert!(yaml_path.exists(), "expected moments.yaml");
        let yaml_str = std::fs::read_to_string(&yaml_path).unwrap();
        let manifest: ExportManifest = serde_saphyr::from_str(&yaml_str).unwrap();
        assert_eq!(manifest.export.total_frames, 2);
        assert_eq!(manifest.export.source_range, [1, 2]);
        assert_eq!(manifest.export.video, "test.mp4");
        assert_eq!(manifest.moments.len(), 1);
        assert_eq!(manifest.moments[0].frame, 1);
        assert_eq!(manifest.moments[0].buffer, 0);
        assert_eq!(manifest.moments[0].note, Some("boxy".to_string()));
        assert_eq!(manifest.moments[0].annotations.len(), 1);
        match &manifest.moments[0].annotations[0] {
            AnnotationEntry::Rect {
                x,
                y,
                w,
                h,
                stroke,
                stroke_width,
            } => {
                assert_eq!(*x, 2.0);
                assert_eq!(*y, 2.0);
                assert_eq!(*w, 10.0);
                assert_eq!(*h, 10.0);
                assert_eq!(stroke, "#FF3D71");
                assert_eq!(*stroke_width, DEFAULT_STROKE_WIDTH);
            }
            _ => panic!("expected Rect annotation"),
        }

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
        let video = tmp.join("clip.mp4");
        // Trim range [3, 7] excludes the first and third moments.
        let result = export_all(&moments, &anns, 10, 3, 7, &frames, &export, &video).unwrap();
        assert_eq!(result.moments_written, 1);
        assert!(export.join("moment-01").exists());
        assert!(!export.join("moment-02").exists());

        let yaml_str = std::fs::read_to_string(export.join("moments.yaml")).unwrap();
        let manifest: ExportManifest = serde_saphyr::from_str(&yaml_str).unwrap();
        assert_eq!(manifest.export.source_range, [4, 8]);
        assert_eq!(manifest.export.total_frames, 10);
        assert_eq!(manifest.export.video, "clip.mp4");
        assert_eq!(manifest.moments.len(), 1);
        assert_eq!(manifest.moments[0].frame, 6);
        assert_eq!(manifest.moments[0].note, Some("in".to_string()));

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn moments_yaml_round_trips_rect_and_text() {
        let tmp = tempdir();
        let frames = tmp.join("frames");
        let export = tmp.join("export");
        std::fs::create_dir_all(&frames).unwrap();
        std::fs::create_dir_all(&export).unwrap();

        let white = image::RgbaImage::from_pixel(10, 10, image::Rgba([255, 255, 255, 255]));
        white.save(session::frame_path(&frames, 0)).unwrap();

        let moment = Moment {
            frame_index: 0,
            buffer: 0,
            note: "both shapes".to_string(),
        };
        let mut anns = HashMap::new();
        anns.insert(
            0usize,
            vec![
                Annotation::Rect {
                    x: 1.0,
                    y: 2.0,
                    w: 3.0,
                    h: 4.0,
                    stroke_color: [0xAA, 0xBB, 0xCC, 0xFF],
                    stroke_width: 2.5,
                },
                Annotation::Text {
                    x: 5.0,
                    y: 6.0,
                    text: "label".to_string(),
                    font_size: 14.0,
                    color: [0x11, 0x22, 0x33, 0x44],
                },
            ],
        );

        let video = tmp.join("shapes.mp4");
        let result = export_all(&[moment], &anns, 1, 0, 0, &frames, &export, &video).unwrap();
        assert_eq!(result.moments_written, 1);

        let yaml_str = std::fs::read_to_string(export.join("moments.yaml")).unwrap();
        let manifest: ExportManifest = serde_saphyr::from_str(&yaml_str).unwrap();
        assert_eq!(manifest.moments.len(), 1);
        assert_eq!(manifest.moments[0].annotations.len(), 2);

        match &manifest.moments[0].annotations[0] {
            AnnotationEntry::Rect {
                x,
                y,
                w,
                h,
                stroke,
                stroke_width,
            } => {
                assert_eq!(*x, 1.0);
                assert_eq!(*y, 2.0);
                assert_eq!(*w, 3.0);
                assert_eq!(*h, 4.0);
                assert_eq!(stroke, "#AABBCC");
                assert_eq!(*stroke_width, 2.5);
            }
            _ => panic!("expected Rect as first annotation"),
        }

        match &manifest.moments[0].annotations[1] {
            AnnotationEntry::Text {
                x,
                y,
                text,
                font_size,
                color,
            } => {
                assert_eq!(*x, 5.0);
                assert_eq!(*y, 6.0);
                assert_eq!(text, "label");
                assert_eq!(*font_size, 14.0);
                assert_eq!(color, "#11223344");
            }
            _ => panic!("expected Text as second annotation"),
        }

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn empty_note_serializes_as_yaml_null() {
        let tmp = tempdir();
        let frames = tmp.join("frames");
        let export = tmp.join("export");
        std::fs::create_dir_all(&frames).unwrap();
        std::fs::create_dir_all(&export).unwrap();

        let white = image::RgbaImage::from_pixel(10, 10, image::Rgba([255, 255, 255, 255]));
        white.save(session::frame_path(&frames, 0)).unwrap();

        let moment = Moment {
            frame_index: 0,
            buffer: 0,
            note: "   ".to_string(),
        };
        let anns = HashMap::new();
        let video = tmp.join("empty.mp4");
        let result = export_all(&[moment], &anns, 1, 0, 0, &frames, &export, &video).unwrap();
        assert_eq!(result.moments_written, 1);

        let yaml_str = std::fs::read_to_string(export.join("moments.yaml")).unwrap();
        assert!(yaml_str.contains("note: null") || yaml_str.contains("note: ~"));
        let manifest: ExportManifest = serde_saphyr::from_str(&yaml_str).unwrap();
        assert_eq!(manifest.moments[0].note, None);

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
