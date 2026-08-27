use std::io;
use std::path::PathBuf;

pub fn session_root() -> io::Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no home directory"))?;
    Ok(home.join(".frammpeg").join("sessions"))
}

pub fn ensure_session_root() -> io::Result<PathBuf> {
    let root = session_root()?;
    std::fs::create_dir_all(&root)?;
    Ok(root)
}
