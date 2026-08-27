use std::io;
use std::path::{Path, PathBuf};

use chrono::Local;

pub fn sessions_root() -> io::Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no home directory"))?;
    Ok(home.join(".frammpeg").join("sessions"))
}

pub fn ensure_sessions_root() -> io::Result<PathBuf> {
    let root = sessions_root()?;
    std::fs::create_dir_all(&root)?;
    Ok(root)
}

pub struct SessionDirs {
    #[allow(dead_code)]
    pub root: PathBuf,
    pub frames: PathBuf,
    pub export: PathBuf,
}

pub fn create_session(sessions_root: &Path) -> io::Result<SessionDirs> {
    let stamp = Local::now().format("%Y-%m-%d_%H%M%S").to_string();
    let root = sessions_root.join(stamp);
    let frames = root.join("frames");
    let export = root.join("export");
    std::fs::create_dir_all(&frames)?;
    std::fs::create_dir_all(&export)?;
    Ok(SessionDirs {
        root,
        frames,
        export,
    })
}

pub fn frame_path(frames_dir: &Path, index: usize) -> PathBuf {
    frames_dir.join(format!("frame-{:04}.png", index + 1))
}

pub fn count_frames(frames_dir: &Path) -> io::Result<usize> {
    let mut count = 0usize;
    for entry in std::fs::read_dir(frames_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with("frame-") && name.ends_with(".png") {
                    count += 1;
                }
            }
        }
    }
    Ok(count)
}
