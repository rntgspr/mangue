use std::path::Path;

use crate::model::Definitions;

/// Reads and imports the BPMN file at `path`, returning a displayable error on failure.
pub fn load(path: &Path) -> Result<Definitions, String> {
    let xml = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
    super::import_xml(&xml).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A missing file yields a read error, not a panic.
    #[test]
    fn missing_file_is_an_error() {
        assert!(load(Path::new("/nonexistent/missing.bpmn")).is_err());
    }

    /// Malformed XML yields an import error, not a panic.
    #[test]
    fn invalid_xml_is_an_error() {
        let path = std::env::temp_dir().join("mangue-load-invalid.bpmn");
        std::fs::write(&path, "<not-xml").unwrap();

        let result = load(&path);
        std::fs::remove_file(&path).unwrap();

        assert!(result.is_err());
    }

    /// A valid exported diagram loads back.
    #[test]
    fn valid_file_loads() {
        let path = std::env::temp_dir().join("mangue-load-valid.bpmn");
        std::fs::write(
            &path,
            super::super::export_xml(&super::super::new_diagram()),
        )
        .unwrap();

        let result = load(&path);
        std::fs::remove_file(&path).unwrap();

        assert!(result.is_ok());
    }
}
