use std::fs;
use std::io;
use std::path::Path;

pub fn path_exists(target_path: impl AsRef<Path>) -> bool {
    target_path.as_ref().exists()
}

pub fn read_text_if_exists(target_path: impl AsRef<Path>) -> io::Result<Option<String>> {
    let target_path = target_path.as_ref();
    if !path_exists(target_path) {
        return Ok(None);
    }

    match fs::read_to_string(target_path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn write_text(target_path: impl AsRef<Path>, content: &str) -> io::Result<()> {
    let target_path = target_path.as_ref();
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(target_path, content)
}
