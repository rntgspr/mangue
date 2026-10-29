//! Document tabs of the `Canvas`: opening, activating and resetting the
//! per-diagram interaction state when the active document changes.

use std::path::{Path, PathBuf};

use gpui::{
    AnyElement, Context, Div, Focusable, IntoElement, ParentElement, Styled, Window, div, relative,
};

use super::AfterConfirm;
use super::{Canvas, Gesture, aids};
use crate::canvas::actions::{ActivateLastTab, ActivateTab, NextTab, PrevTab};
use crate::canvas::code_view::new_editor;
use crate::canvas::doc_header::DocHeader;
use crate::canvas::document::Document;
use crate::canvas::tab_bar::TabBar;
use crate::canvas::xml_error::XmlError;
use crate::model::Definitions;
use gpui_component::input::Editor;

impl Canvas {
    /// Commits a pending label edit and drops every interaction state tied
    /// to the diagram being left (gesture, tools, hover, menus, overlays).
    fn reset_transient(&mut self, cx: &mut Context<Self>) {
        self.commit_edit(cx);
        self.gesture = Gesture::Idle;
        self.reset_tools();
        self.hover = None;
        self.search = None;
        self.append_menu = None;
        self.aids = aids::Aids::default();
        self.overlay_store = Default::default();
        self.preview_stale = false;
        self.pending_focus = Some(self.focus.clone());
    }

    /// Makes tab `index` active; out of range or already active is a no-op.
    pub(super) fn activate_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.docs.len() || index == self.active {
            return;
        }

        self.reset_transient(cx);
        self.active = index;
        self.fit_pending = false;
        cx.notify();
    }

    /// Activates the next tab, wrapping from the last to the first.
    pub(super) fn on_next_tab(&mut self, _: &NextTab, _: &mut Window, cx: &mut Context<Self>) {
        if self.docs.is_empty() {
            return;
        }

        self.activate_tab((self.active + 1) % self.docs.len(), cx);
    }

    /// Activates the previous tab, wrapping from the first to the last.
    pub(super) fn on_prev_tab(&mut self, _: &PrevTab, _: &mut Window, cx: &mut Context<Self>) {
        let count = self.docs.len();
        if count == 0 {
            return;
        }

        self.activate_tab((self.active + count - 1) % count, cx);
    }

    /// Activates the tab at the action's index; out of range is a no-op.
    pub(super) fn on_activate_tab(
        &mut self,
        action: &ActivateTab,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_tab(action.index, cx);
    }

    /// Activates the last tab.
    pub(super) fn on_activate_last_tab(
        &mut self,
        _: &ActivateLastTab,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_tab(self.docs.len().saturating_sub(1), cx);
    }

    /// Appends a document loaded from `path` (if any) as a new tab and
    /// activates it, fitting its viewport when it comes from a file.
    pub(super) fn open_tab(
        &mut self,
        definitions: Definitions,
        path: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        self.reset_transient(cx);
        self.fit_pending = path.is_some();
        self.docs.push(Document::new(definitions, path));
        self.active = self.docs.len() - 1;
        self.docs[self.active].plane = self.docs[self.active].definitions.plane_owner().cloned();
        cx.notify();
    }

    /// Closes tab `index` after confirming its unsaved changes (activating
    /// it first); out of range is a no-op.
    pub(super) fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.docs.len() {
            return;
        }

        self.activate_tab(index, cx);
        self.confirm_discard(Box::new(|c, _, cx| c.remove_active(cx)), window, cx);
    }

    /// Drops the active tab, keeping `active` valid; closing the last tab
    /// leaves the canvas with no document.
    fn remove_active(&mut self, cx: &mut Context<Self>) {
        self.reset_transient(cx);
        self.docs.remove(self.active);

        self.active = self.active.min(self.docs.len().saturating_sub(1));
        self.fit_pending = false;
        cx.notify();
    }

    /// Runs `then` once every tab from `start` on has been answered: each
    /// dirty one is activated and prompted in turn; Cancel stops the chain.
    pub(super) fn confirm_all_from(
        &mut self,
        start: usize,
        then: AfterConfirm,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(offset) = self.docs[start.min(self.docs.len())..]
            .iter()
            .position(Document::is_dirty)
        else {
            then(self, window, cx);
            return;
        };

        let index = start + offset;
        self.activate_tab(index, cx);
        self.confirm_discard(
            Box::new(move |c, window, cx| c.confirm_all_from(index + 1, then, window, cx)),
            window,
            cx,
        );
    }

    /// Index of the tab holding `path`, comparing canonicalized paths (the
    /// literal path when it cannot be canonicalized).
    pub(super) fn tab_of(&self, path: &Path) -> Option<usize> {
        let canonical = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        let wanted = canonical(path);

        self.docs
            .iter()
            .position(|d| d.path.as_deref().is_some_and(|p| canonical(p) == wanted))
    }

    /// The tab strip: every document's name and dirty flag, the active one
    /// marked, clicks activating the tab.
    pub(super) fn tab_bar(&self, cx: &mut Context<Self>) -> TabBar {
        let tabs = self
            .docs
            .iter()
            .map(|d| (d.title_name(), d.is_dirty()))
            .collect();

        TabBar::new(
            tabs,
            self.active,
            cx.listener(|c, index: &usize, _, cx| c.activate_tab(*index, cx)),
            cx.listener(|c, index: &usize, window, cx| c.close_tab(*index, window, cx)),
        )
    }

    /// The document header: the active document's name as breadcrumb
    /// placeholder and the mode button toggling its mode.
    pub(super) fn doc_header(&self, cx: &mut Context<Self>) -> DocHeader {
        let doc = &self.docs[self.active];
        let view = cx.entity();

        DocHeader::new(doc.title_name(), doc.code_mode, move |_, cx| {
            view.update(cx, |c, cx| c.toggle_mode(cx));
        })
    }

    /// Flips the active document between preview and code mode; leaving code
    /// mode syncs the edited XML into the model.
    pub(super) fn toggle_mode(&mut self, cx: &mut Context<Self>) {
        if self.docs.is_empty() {
            return;
        }

        let doc = &mut self.docs[self.active];

        if doc.code_mode {
            self.leave_code_mode(cx);
            self.pending_focus = Some(self.focus.clone());
        } else {
            doc.code_mode = true;
            doc.xml_error = None;
        }

        cx.notify();
    }

    /// Leaves code mode: unchanged text keeps the model, importable text
    /// replaces it, anything else keeps the editor and records the error.
    fn leave_code_mode(&mut self, cx: &mut Context<Self>) {
        let doc = &mut self.docs[self.active];
        doc.code_mode = false;

        let Some(editor) = doc.editor.clone() else {
            return;
        };

        let text = editor.read(cx).value().to_string();
        if text == crate::io::export_xml(&doc.definitions) {
            doc.editor = None;
            return;
        }

        match crate::canvas::persist::import_code(&text) {
            Ok(definitions) => {
                doc.editor = None;
                self.replace_definitions(definitions);
            }
            Err(error) => doc.xml_error = Some(error.to_string()),
        }
    }

    /// Swaps the active document's model for `definitions`. The sync is not a
    /// canvas command: canvas history is kept, entries the edit touched turn
    /// stale (no-op), and every revision is renumbered so none matches the
    /// saved one (dirty).
    fn replace_definitions(&mut self, definitions: Definitions) {
        self.docs[self.active].replace_model(definitions);
        self.fit_pending = true;
        self.hover = None;
    }

    /// Discards the code edits and hides the XML error; the model stays as it
    /// was when code mode was entered.
    pub(super) fn revert_code(&mut self, cx: &mut Context<Self>) {
        let doc = &mut self.docs[self.active];
        doc.editor = None;
        doc.xml_error = None;
        cx.notify();
    }

    /// Hides the XML error and returns to code mode with the edited XML.
    pub(super) fn back_to_code(&mut self, cx: &mut Context<Self>) {
        let doc = &mut self.docs[self.active];
        doc.xml_error = None;
        doc.code_mode = true;
        cx.notify();
    }

    /// The document body: the canvas `surface` in preview mode, else the code
    /// editor holding the document's XML (created and focused on first use).
    pub(super) fn body(
        &mut self,
        surface: Div,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let doc = &mut self.docs[self.active];
        if !doc.code_mode {
            let Some(message) = doc.xml_error.clone() else {
                return surface.into_any_element();
            };
            let (revert, back) = (cx.entity(), cx.entity());

            return div()
                .relative()
                .flex()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .child(surface)
                .child(XmlError::new(
                    message,
                    move |_, cx| revert.update(cx, |c, cx| c.revert_code(cx)),
                    move |_, cx| back.update(cx, |c, cx| c.back_to_code(cx)),
                ))
                .into_any_element();
        }

        if doc.editor.is_none() {
            let editor = new_editor(&crate::io::export_xml(&doc.definitions), window, cx);
            window.focus(&editor.read(cx).focus_handle(cx), cx);
            doc.editor = Some(editor);
        }

        let editor = doc.editor.clone().expect("editor created above");

        div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .child(Editor::new(&editor).h(relative(1.)))
            .into_any_element()
    }
}
