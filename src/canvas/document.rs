//! `Document`: the per-diagram state of a `Canvas` (model, history, saved
//! revision, selection, viewport and drill-down plane).

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use gpui::Entity;
use gpui_component::input::EditorState;

use super::geometry::Viewport;
use crate::model::{Definitions, ElementId};
use crate::modeling::CommandStack;

/// One open diagram: its model, file, command stack and view state.
pub struct Document {
    pub(super) definitions: Rc<Definitions>,
    pub(super) stack: CommandStack,
    pub(super) path: Option<PathBuf>,
    pub(super) viewport: Viewport,
    pub(super) selection: Vec<ElementId>,
    /// Revision ids mirroring the command stack's undo entries.
    history: Vec<u64>,
    /// Shape indices of the active plane in paint order, cached until the
    /// document changes (`defs_mut` drops it).
    pub(super) order: RefCell<Option<Vec<usize>>>,
    /// Revision ids of the undone commands (redo order: last is next).
    undone: Vec<u64>,
    /// Next revision id to assign.
    next_revision: u64,
    /// Revision the file was last loaded or saved at (`None` = empty history).
    saved: Option<u64>,
    /// Sub-process whose plane is shown (`None` = root), as last synced.
    pub(super) plane: Option<ElementId>,
    /// Saved viewport of every plane visited, by owner.
    pub(super) viewports: HashMap<Option<ElementId>, Viewport>,
    /// Whether the document is in code mode (`false` = preview).
    pub(super) code_mode: bool,
    /// The code editor, created on first render in code mode and dropped on leaving it.
    pub(super) editor: Option<Entity<EditorState>>,
    /// Import error of the edited XML left in code mode, shown over the canvas.
    pub(super) xml_error: Option<String>,
}

impl Document {
    /// Creates a document showing `definitions`, loaded from `path` if any.
    pub fn new(definitions: Definitions, path: Option<PathBuf>) -> Self {
        Document {
            definitions: Rc::new(definitions),
            stack: CommandStack::default(),
            path,
            viewport: Viewport::default(),
            selection: Vec::new(),
            history: Vec::new(),
            order: RefCell::new(None),
            undone: Vec::new(),
            next_revision: 0,
            saved: None,
            plane: None,
            viewports: HashMap::new(),
            code_mode: false,
            editor: None,
            xml_error: None,
        }
    }

    /// Whether the document has changes since it was loaded or last saved.
    pub fn is_dirty(&self) -> bool {
        self.history.last().copied() != self.saved
    }

    /// Returns the number of recorded revisions for interaction tests.
    #[cfg(test)]
    pub(super) fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Marks the current revision as saved.
    pub fn mark_saved(&mut self) {
        self.saved = self.history.last().copied();
    }

    /// Records the revision of a command that changed the document.
    pub(super) fn record_execute(&mut self) {
        self.history.push(self.next_revision);
        self.next_revision += 1;
        self.undone.clear();
    }

    /// Moves the current revision to the redo history.
    pub(super) fn record_undo(&mut self) {
        self.undone.extend(self.history.pop());
    }

    /// Restores the next revision from the redo history.
    pub(super) fn record_redo(&mut self) {
        self.history.extend(self.undone.pop());
    }

    /// Replaces the model after a code sync and resets its view state.
    pub(super) fn replace_model(&mut self, definitions: Definitions) {
        self.stack.invalidate(&self.definitions, &definitions);
        self.definitions = Rc::new(definitions);
        self.order.take();

        if self.history.is_empty() {
            self.history.push(0);
        }
        for revision in self.history.iter_mut().chain(self.undone.iter_mut()) {
            *revision = self.next_revision;
            self.next_revision += 1;
        }

        self.selection.clear();
        self.viewports.clear();
        self.plane = self.definitions.plane_owner().cloned();
        self.viewport = Default::default();
    }

    /// File name shown in prompts ("Untitled" for a new diagram).
    pub fn title_name(&self) -> String {
        self.path
            .as_deref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into())
    }
}
