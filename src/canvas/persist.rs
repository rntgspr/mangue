use std::path::Path;

use crate::model::Definitions;

/// Opens a BPMN file and applies the model normalization used by the canvas.
pub fn open(path: &Path) -> Result<Definitions, String> {
    let mut defs = crate::io::load(path)?;
    crate::modeling::compat_planes(&mut defs);
    crate::modeling::layout::orthogonalize_all(&mut defs);
    Ok(defs)
}

/// Imports code editor XML and applies subprocess compatibility.
pub fn import_code(xml: &str) -> Result<Definitions, crate::io::ImportError> {
    let mut defs = crate::io::import_xml(xml)?;
    crate::modeling::compat_planes(&mut defs);
    Ok(defs)
}

/// Writes a document with its diagram elements in bpmn-js order.
pub fn save(path: &Path, defs: &Definitions) -> Result<(), String> {
    let mut ordered = defs.clone();
    crate::modeling::ordering::order_planes(&mut ordered);
    let xml = crate::io::export_xml(&ordered);
    std::fs::write(path, xml).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opening legacy XML splits subprocess content and normalizes diagonal edges.
    #[test]
    fn open_normalizes_legacy_subprocess_and_edges() {
        let fixture = include_str!("../../tests/fixtures/collapsed-subprocess.bpmn");
        let seam = "    </bpmndi:BPMNPlane>\n  </bpmndi:BPMNDiagram>\n  \
            <bpmndi:BPMNDiagram id=\"BPMNDiagram_0p6a7cm\">\n    \
            <bpmndi:BPMNPlane id=\"BPMNPlane_1h2xq9e\" bpmnElement=\"SubProcess_1\">\n";
        assert!(fixture.contains(seam));
        let legacy = fixture.replace(seam, "").replace(
            "<di:waypoint x=\"270\" y=\"120\" />",
            "<di:waypoint x=\"270\" y=\"130\" />",
        );
        let path =
            std::env::temp_dir().join(format!("mangue-persist-open-{}.bpmn", std::process::id()));
        std::fs::write(&path, &legacy).unwrap();

        let raw = crate::io::import_xml(&legacy).unwrap();
        assert_eq!(raw.diagrams.len(), 1);
        assert!(raw.edges().any(|edge| {
            edge.waypoints
                .windows(2)
                .any(|pair| pair[0].x != pair[1].x && pair[0].y != pair[1].y)
        }));

        let mut defs = open(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(defs.diagrams.len(), 2);
        assert!(defs.shape(&"Task_1".into()).is_none());
        assert!(defs.open_plane(Some(&"SubProcess_1".into())));
        assert!(defs.shape(&"Task_1".into()).is_some());
        assert!(
            defs.diagrams
                .iter()
                .flat_map(|diagram| &diagram.plane.edges)
                .all(|edge| edge
                    .waypoints
                    .windows(2)
                    .all(|pair| pair[0].x == pair[1].x || pair[0].y == pair[1].y))
        );
    }

    /// Saving an opened fixture preserves the previous ordered export bytes.
    #[test]
    fn save_writes_ordered_export_bytes() {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/collapsed-subprocess.bpmn");
        let defs = open(&fixture).unwrap();
        let mut ordered = defs.clone();
        crate::modeling::ordering::order_planes(&mut ordered);
        let expected = crate::io::export_xml(&ordered);
        let path =
            std::env::temp_dir().join(format!("mangue-persist-save-{}.bpmn", std::process::id()));

        save(&path, &defs).unwrap();
        let actual = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(actual, expected.as_bytes());
    }
}
