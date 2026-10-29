//! Validates exported XML with bpmn-moddle (the parser bpmn-js uses).
//!
//! Requires `node` and `npm ci` in `tools/validate`; skips with a note otherwise.

use std::path::{Path, PathBuf};
use std::process::Command;

use mangue::io::{export_xml, import_xml, new_diagram};
use mangue::modeling::Command as ModelCommand;

/// Fixtures exported and validated, by file name.
const FIXTURES: [(&str, &str); 6] = [
    ("simple", include_str!("fixtures/simple.bpmn")),
    ("collaboration", include_str!("fixtures/collaboration.bpmn")),
    ("events", include_str!("fixtures/events.bpmn")),
    (
        "message-delivery",
        include_str!("fixtures/message-delivery.bpmn"),
    ),
    (
        "collapsed-subprocess",
        include_str!("fixtures/collapsed-subprocess.bpmn"),
    ),
    ("vertical-pool", include_str!("fixtures/vertical-pool.bpmn")),
];

/// Path of `tools/validate`, or `None` (with a note) when it cannot run.
fn validator() -> Option<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/validate");

    if !dir.join("node_modules").is_dir() {
        println!("skipped: run `npm ci` in tools/validate");
        return None;
    }

    if Command::new("node").arg("--version").output().is_err() {
        println!("skipped: `node` not found");
        return None;
    }

    Some(dir)
}

/// Writes `xml` to a temp file and runs `validate.mjs` on it.
fn assert_valid(dir: &Path, name: &str, xml: &str) {
    let file = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.bpmn"));
    std::fs::write(&file, xml).expect("write temp file");

    let output = Command::new("node")
        .arg(dir.join("validate.mjs"))
        .arg(&file)
        .output()
        .expect("run node");

    assert!(
        output.status.success(),
        "bpmn-moddle rejected {name}:\n{}\n{xml}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Every fixture (also with its DI in bpmn-js order) and `new_diagram()`
/// exports XML bpmn-moddle parses cleanly.
#[test]
fn exported_xml_is_valid_for_bpmn_moddle() {
    let Some(dir) = validator() else {
        return;
    };

    for (name, fixture) in FIXTURES {
        let mut defs = import_xml(fixture).expect("fixture imports");
        assert_valid(&dir, name, &export_xml(&defs));

        mangue::modeling::ordering::order_planes(&mut defs);
        assert_valid(&dir, &format!("{name}_ordered"), &export_xml(&defs));
    }

    assert_valid(&dir, "new_diagram", &export_xml(&new_diagram()));
}

/// A sub-process expanded then collapsed again exports its new
/// `SubProcess_1_plane` diagram in a shape bpmn-moddle accepts.
#[test]
fn collapsed_plane_export_is_valid_for_bpmn_moddle() {
    let Some(dir) = validator() else {
        return;
    };

    let mut defs = import_xml(include_str!("fixtures/collapsed-subprocess.bpmn")).unwrap();
    for _ in 0..2 {
        let toggle = ModelCommand::ToggleCollapse {
            id: "SubProcess_1".into(),
        };
        toggle.execute(&mut defs).expect("toggles");
    }

    let xml = export_xml(&defs);
    assert!(xml.contains("id=\"SubProcess_1_plane\" bpmnElement=\"SubProcess_1\""));
    assert_valid(&dir, "collapsed-toggle", &xml);
}

/// A pasted collapsed sub-process exports its copied plane, and a legacy
/// file with the content on the root plane exports after the import split
/// it into its own plane; bpmn-moddle accepts both.
#[test]
fn pasted_and_legacy_planes_are_valid_for_bpmn_moddle() {
    let Some(dir) = validator() else {
        return;
    };

    let fixture = include_str!("fixtures/collapsed-subprocess.bpmn");
    let mut defs = import_xml(fixture).unwrap();
    let clip = mangue::modeling::clipboard::copy(&defs, &["SubProcess_1".into()]).unwrap();
    let at = mangue::model::Waypoint { x: 320.0, y: 400.0 };
    let (paste, _) = mangue::modeling::clipboard::paste(&defs, &clip, at).unwrap();
    paste.execute(&mut defs).expect("pastes");
    assert_eq!(defs.diagrams.len(), 3);
    assert_valid(&dir, "collapsed-paste", &export_xml(&defs));

    let seam = "    </bpmndi:BPMNPlane>\n  </bpmndi:BPMNDiagram>\n  \
        <bpmndi:BPMNDiagram id=\"BPMNDiagram_0p6a7cm\">\n    \
        <bpmndi:BPMNPlane id=\"BPMNPlane_1h2xq9e\" bpmnElement=\"SubProcess_1\">\n";
    let legacy = mangue::canvas::persist::import_code(&fixture.replace(seam, "")).unwrap();
    assert_eq!(legacy.diagrams.len(), 2);
    assert_valid(&dir, "collapsed-legacy", &export_xml(&legacy));
}
