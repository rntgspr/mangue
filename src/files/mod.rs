//! GPUI-independent file system helpers for the folder browser.

pub mod tree;

use std::path::{Path, PathBuf};

/// The user's home directory from `$HOME`, if set.
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Directory a save dialog opens in: the bound file's parent, else
/// `<home>/Documents` when it exists, else `home`, else ".".
pub fn dialog_start_dir(current: Option<&Path>, home: Option<&Path>) -> PathBuf {
    if let Some(parent) = current.and_then(Path::parent) {
        return parent.to_path_buf();
    }

    let Some(home) = home else {
        return PathBuf::from(".");
    };

    let documents = home.join("Documents");
    if documents.is_dir() {
        documents
    } else {
        home.to_path_buf()
    }
}

/// Accepts only `.bpmn` and `.xml` files (case-insensitive); GPUI's path
/// prompt has no extension filter, so the check happens after the pick.
pub fn check_openable(path: &Path) -> Result<(), String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);

    match ext.as_deref() {
        Some("bpmn" | "xml") => Ok(()),
        _ => Err("Unsupported file type: expected .bpmn or .xml".to_string()),
    }
}

/// Converts an Open Documents URL (`file:///a%20b.bpmn`) into a filesystem
/// path, decoding percent escapes; returns `None` for non-file URLs.
pub fn url_to_path(url: &str) -> Option<PathBuf> {
    let raw = url.strip_prefix("file://")?.as_bytes();
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;

    while i < raw.len() {
        let hex = raw
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());

        match (raw[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }

    String::from_utf8(out).ok().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// File URLs decode percent escapes and other schemes are rejected.
    #[test]
    fn file_url_decodes_path() {
        assert_eq!(
            url_to_path("file:///a%20b.bpmn"),
            Some(PathBuf::from("/a b.bpmn"))
        );
        assert_eq!(url_to_path("https://example.com/a.bpmn"), None);
    }

    /// The save dialog starts beside the bound file.
    #[test]
    fn dialog_start_dir_prefers_file_parent() {
        let dir = dialog_start_dir(Some(Path::new("/a/b/c.bpmn")), Some(Path::new("/home/u")));

        assert_eq!(dir, Path::new("/a/b"));
    }

    /// Without a bound file the dialog starts in `~/Documents`, else the home dir.
    #[test]
    fn dialog_start_dir_falls_back_to_documents_then_home() {
        let home = std::env::temp_dir().join(format!("mangue-home-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();

        assert_eq!(dialog_start_dir(None, Some(&home)), home);

        std::fs::create_dir_all(home.join("Documents")).unwrap();
        assert_eq!(dialog_start_dir(None, Some(&home)), home.join("Documents"));

        std::fs::remove_dir_all(&home).unwrap();
    }

    /// Only `.bpmn` and `.xml` (any case) are openable.
    #[test]
    fn check_openable_accepts_only_bpmn_and_xml() {
        assert!(check_openable(Path::new("a.bpmn")).is_ok());
        assert!(check_openable(Path::new("a.XML")).is_ok());
        assert!(check_openable(Path::new("a.png")).is_err());
        assert!(check_openable(Path::new("noext")).is_err());
    }
}
