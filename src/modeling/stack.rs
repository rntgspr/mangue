//! `CommandStack`: linear undo/redo history, after bpmn-js `CommandStack`.

use std::collections::HashSet;

use super::command::Command;
use super::undo::Undo;
use crate::model::{Definitions, Edge, ElementId, Shape};

/// Plane a command ran on: the owning sub-process, `None` for the root.
type PlaneOwner = Option<ElementId>;

/// One history entry: the command, the undo it produced, its plane, and
/// whether a change from outside the stack (a code sync) touched its content.
#[derive(Debug)]
struct Entry {
    command: Command,
    undo: Undo,
    plane: PlaneOwner,
    stale: bool,
}

/// Undo and redo history of executed commands. Each entry remembers the
/// plane it ran on; undo and redo re-open that plane first (DI edits apply
/// to the active plane).
#[derive(Debug, Default)]
pub struct CommandStack {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
}

impl CommandStack {
    /// Executes `command`; records it and clears redo when it changed anything.
    pub fn execute(&mut self, command: Command, defs: &mut Definitions) -> bool {
        let Some(undo) = command.execute(defs) else {
            return false;
        };

        let plane = defs.plane_owner().cloned();
        self.undo.push(Entry {
            command,
            undo,
            plane,
            stale: false,
        });
        self.redo.clear();
        true
    }

    /// Reverts the last command; returns `false` when there is nothing to
    /// undo or the entry is stale (no-op).
    pub fn undo(&mut self, defs: &mut Definitions) -> bool {
        if self.undo.last().is_none_or(|e| e.stale) {
            return false;
        }

        let entry = self.undo.pop().expect("checked above");
        defs.open_plane(entry.plane.as_ref());
        entry.undo.restore(defs);
        self.redo.push(entry);
        true
    }

    /// Re-executes the last undone command; `false` when there is none or
    /// the entry is stale (no-op).
    pub fn redo(&mut self, defs: &mut Definitions) -> bool {
        if self.redo.last().is_none_or(|e| e.stale) {
            return false;
        }

        let entry = self.redo.pop().expect("checked above");
        defs.open_plane(entry.plane.as_ref());
        if let Some(undo) = entry.command.execute(defs) {
            self.undo.push(Entry { undo, ..entry });
        }
        true
    }

    /// Marks stale every entry whose content differs between `before` and
    /// `after` (a model replaced from outside the stack, e.g. a code sync).
    pub fn invalidate(&mut self, before: &Definitions, after: &Definitions) {
        let changed = changed_ids(before, after);

        for entry in self.undo.iter_mut().chain(self.redo.iter_mut()) {
            entry.stale |= entry.undo.touches(&changed);
        }
    }

    /// Whether `undo` would do something.
    pub fn can_undo(&self) -> bool {
        self.undo.last().is_some_and(|e| !e.stale)
    }

    /// Whether `redo` would do something.
    pub fn can_redo(&self) -> bool {
        self.redo.last().is_some_and(|e| !e.stale)
    }

    /// Drops all history (e.g. after opening another file).
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

/// Ids whose element, DI shapes, DI edges or DI ids differ between `a` and
/// `b`, across every diagram.
fn changed_ids(a: &Definitions, b: &Definitions) -> HashSet<ElementId> {
    let mut ids: HashSet<&ElementId> = a.elements.keys().chain(b.elements.keys()).collect();
    for defs in [a, b] {
        for plane in defs.diagrams.iter().map(|d| &d.plane) {
            ids.extend(plane.shapes.iter().map(|s| &s.element_id));
            ids.extend(plane.edges.iter().map(|e| &e.element_id));
            ids.extend(plane.di_ids.keys());
        }
    }

    ids.into_iter()
        .filter(|id| a.elements.get(*id) != b.elements.get(*id) || di_of(a, id) != di_of(b, id))
        .cloned()
        .collect()
}

/// DI entries of `id` in one diagram: its shape, edge and DI id.
type Di<'a> = (Option<&'a Shape>, Option<&'a Edge>, Option<&'a String>);

/// Every DI entry of `id` in `defs`, one per diagram.
fn di_of<'a>(defs: &'a Definitions, id: &ElementId) -> Vec<Di<'a>> {
    let planes = defs.diagrams.iter().map(|d| &d.plane);

    planes
        .map(|p| {
            let shape = p.shapes.iter().find(|s| &s.element_id == id);
            let edge = p.edges.iter().find(|e| &e.element_id == id);
            (shape, edge, p.di_ids.get(id))
        })
        .collect()
}
