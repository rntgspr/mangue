//! Headless tests of code mode and its synchronization with the canvas.

use super::*;

/// XML highlighting covers nested content and the matching closing tag.
#[gpui::test]
fn selected_xml_range_covers_complete_element(_cx: &mut TestAppContext) {
    let xml = "<root><task id=\"a\"><task><child /></task></task><task id=\"b\" /></root>";
    let range = crate::canvas::code_view::element_range(xml, &ElementId::from("a")).unwrap();
    assert_eq!(&xml[range], "<task id=\"a\"><task><child /></task></task>");

    let range = crate::canvas::code_view::element_range(xml, &ElementId::from("b")).unwrap();
    assert_eq!(&xml[range], "<task id=\"b\" />");
}

/// The document breadcrumb reflects the XML IDs in the active selection.
#[gpui::test]
fn header_breadcrumb_tracks_selection(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    assert_eq!(
        view.read_with(cx, |c, _| c.header_labels()),
        vec!["Untitled"]
    );

    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    assert_eq!(
        view.read_with(cx, |c, _| c.header_labels()),
        vec!["Untitled", "Process_1", START]
    );

    view.update(cx, |c, cx| {
        c.docs[c.active].selection.push(ElementId::from("another"));
        cx.notify();
    });
    assert_eq!(
        view.read_with(cx, |c, _| c.header_labels()),
        vec!["Untitled", "2 selected"]
    );
}

/// Clicking the process ancestor selects it even without a diagram shape.
#[gpui::test]
fn breadcrumb_selects_process_ancestor(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    cx.run_until_parked();

    let crumbs = cx.debug_bounds("breadcrumbs").unwrap();
    for x in (50..180).step_by(5) {
        cx.simulate_click(
            point(crumbs.left() + px(x as f32), crumbs.center().y),
            Modifiers::none(),
        );
        cx.run_until_parked();
        if selection(&view, cx) == vec![ElementId::from("Process_1")] {
            break;
        }
    }

    assert_eq!(selection(&view, cx), vec![ElementId::from("Process_1")]);
}

/// Selecting an ancestor in code mode updates the visual marker without selecting text.
#[gpui::test]
fn code_breadcrumb_updates_highlight(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    cx.dispatch_action(actions::ToggleMode);
    cx.run_until_parked();

    let crumbs = cx.debug_bounds("breadcrumbs").unwrap();
    for x in (50..180).step_by(5) {
        cx.simulate_click(
            point(crumbs.left() + px(x as f32), crumbs.center().y),
            Modifiers::none(),
        );
        cx.run_until_parked();
        if selection(&view, cx) == vec![ElementId::from("Process_1")] {
            break;
        }
    }

    view.read_with(cx, |canvas, cx| {
        let doc = &canvas.docs[canvas.active];
        assert_eq!(doc.selection, vec![ElementId::from("Process_1")]);
        let editor = doc.editor.as_ref().unwrap().read(cx);
        let range = doc
            .selection_highlight
            .as_ref()
            .unwrap()
            .get_ranges(cx)
            .pop()
            .unwrap();
        assert!(editor.value()[range].starts_with("<bpmn:process"));
        assert!(editor.selected_range().is_empty());
    });
}

/// Entering code mode reveals the selected element without selecting text.
#[gpui::test]
fn code_mode_reveals_selected_element(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    cx.simulate_click(at(START_CENTER.0, START_CENTER.1), Modifiers::none());
    cx.dispatch_action(actions::ToggleMode);
    cx.run_until_parked();

    view.read_with(cx, |canvas, cx| {
        let editor = canvas.docs[canvas.active].editor.as_ref().unwrap().read(cx);
        let range = editor.selected_range();
        assert!(range.is_empty());
        let xml = editor.value();
        let element = xml.find("id=\"StartEvent_1\"").unwrap();
        assert!(range.start <= element && element - range.start < 100);
    });
}

/// The header controls share a 16px inset from the window edges.
#[gpui::test]
fn document_header_has_horizontal_insets(cx: &mut TestAppContext) {
    let (_view, cx) = open(cx);
    cx.run_until_parked();

    let width = cx.update(|window, _| window.viewport_size().width);
    let crumb = cx.debug_bounds("breadcrumbs").expect("breadcrumb rendered");
    let button = cx.debug_bounds("doc-header-mode").expect("button rendered");
    assert_eq!(crumb.left(), px(16.));
    assert_eq!(width - button.right(), px(16.));
}

/// Whether the active document is in code mode.
fn code_mode(view: &Entity<Canvas>, cx: &mut VisualTestContext) -> bool {
    view.read_with(cx, |c, _| c.docs[c.active].code_mode)
}

/// The mode button reads "Code" in preview mode and "Preview" in code mode.
#[gpui::test]
fn mode_button_label_follows_mode(_cx: &mut TestAppContext) {
    assert_eq!(crate::canvas::doc_header::label(false), "Code");
    assert_eq!(crate::canvas::doc_header::label(true), "Preview");
}

/// `ToggleMode` flips the active document's mode, and back.
#[gpui::test]
fn toggle_mode_action_flips_mode(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    assert!(!code_mode(&view, cx));

    cx.dispatch_action(actions::ToggleMode);
    assert!(code_mode(&view, cx));

    cx.dispatch_action(actions::ToggleMode);
    assert!(!code_mode(&view, cx));
}

/// Clicking the header's mode button flips the active document's mode.
#[gpui::test]
fn clicking_mode_button_flips_mode(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let width = cx.update(|window, _| window.viewport_size().width);
    let button = point(
        width - px(30.),
        px(crate::canvas::tab_bar::HEIGHT + crate::canvas::doc_header::HEIGHT / 2.0),
    );

    cx.simulate_click(button, Modifiers::none());
    assert!(code_mode(&view, cx));

    cx.simulate_click(button, Modifiers::none());
    assert!(!code_mode(&view, cx));
}

/// Clicking the `doc-header-mode` kit button by its selector flips the mode.
#[gpui::test]
fn clicking_mode_button_by_selector_flips_mode(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let button = cx
        .debug_bounds("doc-header-mode")
        .expect("mode button rendered");

    cx.simulate_click(button.center(), Modifiers::none());
    assert!(code_mode(&view, cx));
}

/// cmd-b flips the active document's mode, and back.
#[gpui::test]
fn cmd_b_toggles_mode(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    assert!(!code_mode(&view, cx));

    cx.simulate_keystrokes("secondary-b");
    assert!(code_mode(&view, cx));

    cx.simulate_keystrokes("secondary-b");
    assert!(!code_mode(&view, cx));
}

/// Entering code mode shows the document's exported XML in the editor;
/// leaving it drops the editor so the canvas shows again.
#[gpui::test]
fn code_mode_shows_xml_in_editor(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    assert!(view.read_with(cx, |c, _| c.docs[c.active].editor.is_none()));

    cx.dispatch_action(actions::ToggleMode);
    let (text, xml) = view.read_with(cx, |c, cx| {
        let doc = &c.docs[c.active];
        let editor = doc.editor.as_ref().expect("editor in code mode");
        (
            editor.read(cx).value().to_string(),
            crate::io::export_xml(&doc.definitions),
        )
    });
    assert!(!xml.is_empty());
    assert_eq!(text, xml);

    cx.dispatch_action(actions::ToggleMode);
    assert!(view.read_with(cx, |c, _| c.docs[c.active].editor.is_none()));
}

/// Replaces the active document's editor text with `text`.
fn edit_xml(view: &Entity<Canvas>, cx: &mut VisualTestContext, text: &str) {
    let editor = view.read_with(cx, |c, _| c.docs[c.active].editor.clone().unwrap());
    let text = text.to_owned();

    cx.update(|window, app| editor.update(app, |s, cx| s.set_value(text, window, cx)));
}

/// Exported XML of the active document's model.
fn model_xml(view: &Entity<Canvas>, cx: &mut VisualTestContext) -> String {
    view.read_with(cx, |c, _| {
        crate::io::export_xml(&c.docs[c.active].definitions)
    })
}

/// Enters code mode and replaces the text with `text`.
fn enter_and_edit(view: &Entity<Canvas>, cx: &mut VisualTestContext, text: &str) {
    cx.dispatch_action(actions::ToggleMode);
    edit_xml(view, cx, text);
}

/// Leaving code mode with unchanged text leaves the model untouched.
#[gpui::test]
fn leaving_code_unchanged_keeps_model(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let before = view.read_with(cx, |c, _| c.docs[0].definitions.clone());

    cx.dispatch_action(actions::ToggleMode);
    cx.dispatch_action(actions::ToggleMode);

    assert!(view.read_with(cx, |c, _| std::rc::Rc::ptr_eq(
        &before,
        &c.docs[0].definitions
    )));
    assert!(!view.read_with(cx, |c, _| c.is_dirty()));
    assert!(view.read_with(cx, |c, _| c.docs[0].xml_error.is_none()));
}

/// A valid edit replaces the definitions, marks the document dirty and drops
/// the editor.
#[gpui::test]
fn leaving_code_with_valid_edit_updates_model(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let edited = model_xml(&view, cx).replace("StartEvent_1", "Renamed_1");

    enter_and_edit(&view, cx, &edited);
    cx.dispatch_action(actions::ToggleMode);

    assert!(model_xml(&view, cx).contains("Renamed_1"));
    assert!(view.read_with(cx, |c, _| c.is_dirty()));
    assert!(!code_mode(&view, cx));
    assert!(view.read_with(cx, |c, _| c.docs[0].editor.is_none()));
    assert!(view.read_with(cx, |c, _| c.docs[0].xml_error.is_none()));
}

/// An invalid edit keeps the model, keeps the editor and records the import
/// error.
#[gpui::test]
fn leaving_code_with_invalid_edit_shows_error(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let before = model_xml(&view, cx);

    enter_and_edit(&view, cx, "<bpmn:definitions");
    cx.dispatch_action(actions::ToggleMode);

    let error = view.read_with(cx, |c, _| c.docs[0].xml_error.clone());
    let expected = crate::io::import_xml("<bpmn:definitions")
        .unwrap_err()
        .to_string();
    assert_eq!(error, Some(expected));
    assert_eq!(model_xml(&view, cx), before);
    assert!(!view.read_with(cx, |c, _| c.is_dirty()));
    assert!(!code_mode(&view, cx));
    assert!(view.read_with(cx, |c, _| c.docs[0].editor.is_some()));
}

/// Revert drops the code edits and hides the error; the model is unchanged.
#[gpui::test]
fn revert_drops_edits_and_hides_error(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let before = model_xml(&view, cx);

    enter_and_edit(&view, cx, "<bpmn:definitions");
    cx.dispatch_action(actions::ToggleMode);
    view.update(cx, |c, cx| c.revert_code(cx));

    assert!(view.read_with(cx, |c, _| c.docs[0].xml_error.is_none()));
    assert!(view.read_with(cx, |c, _| c.docs[0].editor.is_none()));
    assert!(!code_mode(&view, cx));
    assert_eq!(model_xml(&view, cx), before);
}

/// Back to code hides the error and returns to code mode with the edited text.
#[gpui::test]
fn back_to_code_keeps_edited_text(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    enter_and_edit(&view, cx, "<bpmn:definitions");
    cx.dispatch_action(actions::ToggleMode);
    view.update(cx, |c, cx| c.back_to_code(cx));

    assert!(view.read_with(cx, |c, _| c.docs[0].xml_error.is_none()));
    assert!(code_mode(&view, cx));
    let text = view.read_with(cx, |c, cx| {
        c.docs[0]
            .editor
            .as_ref()
            .unwrap()
            .read(cx)
            .value()
            .to_string()
    });
    assert_eq!(text, "<bpmn:definitions");
}

/// Moves the start event by 10px through the view, as a drag would.
fn move_start(view: &Entity<Canvas>, cx: &mut VisualTestContext) {
    view.update(cx, |c, cx| {
        let ids = vec![ElementId::from("StartEvent_1")];
        assert!(c.execute(
            Command::MoveShapes {
                ids,
                dx: 10.0,
                dy: 0.0
            },
            cx
        ));
    });
}

/// A code sync keeps the canvas history: undo still reverts an earlier
/// canvas move the code edit did not touch.
#[gpui::test]
fn code_sync_keeps_canvas_history(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let start = ElementId::from("StartEvent_1");
    let x = view.read_with(cx, |c, _| c.definitions().shape(&start).unwrap().bounds.x);

    move_start(&view, cx);
    let edited = model_xml(&view, cx).replace("Definitions_1", "Definitions_2");
    enter_and_edit(&view, cx, &edited);
    cx.dispatch_action(actions::ToggleMode);
    cx.dispatch_action(actions::Undo);

    let after = view.read_with(cx, |c, _| c.definitions().shape(&start).unwrap().bounds.x);
    assert_eq!(after, x);
    assert!(model_xml(&view, cx).contains("Definitions_2"));
}

/// Undo of a canvas move whose element the code edit changed is a no-op.
#[gpui::test]
fn undo_after_conflicting_code_sync_is_noop(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    move_start(&view, cx);
    let edited = model_xml(&view, cx).replace("StartEvent_1", "Renamed_1");
    enter_and_edit(&view, cx, &edited);
    cx.dispatch_action(actions::ToggleMode);
    let synced = model_xml(&view, cx);
    cx.dispatch_action(actions::Undo);

    assert_eq!(model_xml(&view, cx), synced);
}

/// The invalid-XML banner is a kit `Alert`; its Revert button drops the edits.
#[gpui::test]
fn xml_error_revert_button_reverts(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    enter_and_edit(&view, cx, "<bpmn:definitions");
    cx.dispatch_action(actions::ToggleMode);
    cx.run_until_parked();
    assert!(cx.debug_bounds("xml-error-alert").is_some());

    let revert = cx.debug_bounds("xml-error-revert").expect("revert button");
    cx.simulate_click(revert.center(), Modifiers::none());
    cx.run_until_parked();

    assert!(view.read_with(cx, |c, _| c.docs[0].xml_error.is_none()));
    assert!(view.read_with(cx, |c, _| c.docs[0].editor.is_none()));
    assert!(!code_mode(&view, cx));
}

/// The banner's Back to code button returns to code mode.
#[gpui::test]
fn xml_error_back_button_returns_to_code(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);

    enter_and_edit(&view, cx, "<bpmn:definitions");
    cx.dispatch_action(actions::ToggleMode);
    cx.run_until_parked();

    let back = cx.debug_bounds("xml-error-back").expect("back button");
    cx.simulate_click(back.center(), Modifiers::none());
    cx.run_until_parked();

    assert!(view.read_with(cx, |c, _| c.docs[0].xml_error.is_none()));
    assert!(code_mode(&view, cx));
}
