
use std::io::ErrorKind;
use std::path::Path;

use wh_core::Snapshot;

use crate::{Result, StorageError};

pub fn to_json(snapshot: &Snapshot) -> Result<String> {
    serde_json::to_string_pretty(snapshot).map_err(|e| StorageError::Backend(e.to_string()))
}

pub fn from_json(text: &str) -> Result<Snapshot> {
    serde_json::from_str(text).map_err(|e| StorageError::Backend(format!("bad json: {e}")))
}

pub fn save_file(path: &Path, snapshot: &Snapshot) -> Result<()> {
    std::fs::write(path, to_json(snapshot)?).map_err(|e| StorageError::Backend(e.to_string()))
}

pub fn load_file(path: &Path) -> Result<Snapshot> {
    match std::fs::read_to_string(path) {
        Ok(text) => from_json(&text),
        Err(e) if e.kind() == ErrorKind::NotFound => {
            Err(StorageError::NotFound(path.display().to_string()))
        }
        Err(e) => Err(StorageError::Backend(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wh_core::sample::demo_snapshot;

    #[test]
    fn json_roundtrip() {
        let s = demo_snapshot();
        assert_eq!(from_json(&to_json(&s).unwrap()).unwrap(), s);
    }

    #[test]
    fn missing_file_is_not_found() {
        let r = load_file(Path::new("/definitely/not/here.json"));
        assert!(matches!(r, Err(StorageError::NotFound(_))));
    }
}
