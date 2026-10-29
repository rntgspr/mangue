//! `Undo`: before-images of every element, shape and edge a command touched.

use std::collections::HashSet;

use crate::model::{Definitions, Diagram, Edge, Element, ElementId, Shape};

/// Before-image of one entry: its index and value, or `None` if it did not exist.
type Before<T> = (ElementId, Option<(usize, T)>);

/// Records the state of touched entries before a command mutates them, so the
/// command can be reverted exactly (values and document/paint order).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Undo {
    elements: Vec<Before<Element>>,
    shapes: Vec<Before<Shape>>,
    edges: Vec<Before<Edge>>,
    di_ids: Vec<Before<String>>,
    /// Plane `element_id` before the command, when it changed it.
    plane: Option<Option<ElementId>>,
    /// Every diagram before the command, when it added, removed or moved
    /// content between planes (collapse/expand, collapsed sub-processes).
    diagrams: Option<Vec<Diagram>>,
    /// Undos of the sub-commands of a compound command, in execution order.
    steps: Vec<Undo>,
}

impl Undo {
    /// Snapshots element `id` (first touch only; later touches are ignored).
    pub fn element(&mut self, defs: &Definitions, id: &ElementId) {
        if self.elements.iter().any(|(i, _)| i == id) {
            return;
        }

        let before = defs
            .elements
            .get_full(id)
            .map(|(index, _, e)| (index, e.clone()));
        self.elements.push((id.clone(), before));
    }

    /// Snapshots the DI shape of `id` (first touch only).
    pub fn shape(&mut self, defs: &Definitions, id: &ElementId) {
        if self.shapes.iter().any(|(i, _)| i == id) {
            return;
        }

        let shapes = &defs.plane().shapes;
        let before = shapes
            .iter()
            .position(|s| &s.element_id == id)
            .map(|index| (index, shapes[index].clone()));
        self.shapes.push((id.clone(), before));
    }

    /// Snapshots the DI edge of `id` (first touch only).
    pub fn edge(&mut self, defs: &Definitions, id: &ElementId) {
        if self.edges.iter().any(|(i, _)| i == id) {
            return;
        }

        let edges = &defs.plane().edges;
        let before = edges
            .iter()
            .position(|e| &e.element_id == id)
            .map(|index| (index, edges[index].clone()));
        self.edges.push((id.clone(), before));
    }

    /// Snapshots element `id` together with its shape, edge and DI id.
    pub fn all(&mut self, defs: &Definitions, id: &ElementId) {
        self.element(defs, id);
        self.shape(defs, id);
        self.edge(defs, id);

        if !self.di_ids.iter().any(|(i, _)| i == id) {
            let before = defs
                .plane()
                .di_ids
                .get_full(id)
                .map(|(index, _, di)| (index, di.clone()));
            self.di_ids.push((id.clone(), before));
        }
    }

    /// Snapshots the plane's bound element (first touch only).
    pub fn plane(&mut self, defs: &Definitions) {
        if self.plane.is_none() {
            self.plane = Some(defs.plane().element_id.clone());
        }
    }

    /// Snapshots all diagrams (first touch only); restored after every
    /// other entry, so it wins over the per-shape before-images.
    pub fn diagrams(&mut self, defs: &Definitions) {
        if self.diagrams.is_none() {
            self.diagrams = Some(defs.diagrams.clone());
        }
    }

    /// Appends the undo of a sub-command; restored before this undo's own entries.
    pub fn step(&mut self, undo: Undo) {
        self.steps.push(undo);
    }

    /// Returns whether nothing was recorded (the command was a no-op).
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
            && self.shapes.is_empty()
            && self.edges.is_empty()
            && self.di_ids.is_empty()
            && self.plane.is_none()
            && self.diagrams.is_none()
            && self.steps.is_empty()
    }

    /// Returns whether this undo recorded any id in `changed`; a plane or
    /// whole-diagram snapshot counts as touching everything.
    pub fn touches(&self, changed: &HashSet<ElementId>) -> bool {
        if changed.is_empty() {
            return false;
        }

        if self.plane.is_some() || self.diagrams.is_some() {
            return true;
        }

        let ids = self.elements.iter().map(|(id, _)| id);
        let ids = ids.chain(self.shapes.iter().map(|(id, _)| id));
        let ids = ids.chain(self.edges.iter().map(|(id, _)| id));
        let mut ids = ids.chain(self.di_ids.iter().map(|(id, _)| id));
        ids.any(|id| changed.contains(id)) || self.steps.iter().any(|s| s.touches(changed))
    }

    /// Restores every touched entry: removes the current versions, then
    /// re-inserts the before-images in ascending index order. Sub-command
    /// undos are restored first, last one first.
    pub fn restore(&self, defs: &mut Definitions) {
        for step in self.steps.iter().rev() {
            step.restore(defs);
        }

        if let Some(element_id) = &self.plane {
            defs.plane_mut().element_id = element_id.clone();
        }

        for (id, _) in &self.elements {
            defs.elements.shift_remove(id);
        }

        let mut elements: Vec<_> = self
            .elements
            .iter()
            .filter_map(|(_, b)| b.as_ref())
            .collect();
        elements.sort_by_key(|(index, _)| *index);
        for (index, element) in elements {
            let at = (*index).min(defs.elements.len());
            defs.elements
                .shift_insert(at, element.id.clone(), element.clone());
        }

        let plane = defs.plane_mut();
        restore_vec(&mut plane.shapes, &self.shapes, |s| &s.element_id);
        restore_vec(&mut plane.edges, &self.edges, |e| &e.element_id);

        for (id, _) in &self.di_ids {
            plane.di_ids.shift_remove(id);
        }

        let mut di_ids: Vec<_> = self
            .di_ids
            .iter()
            .filter_map(|(id, b)| b.as_ref().map(|(index, di)| (*index, id, di)))
            .collect();
        di_ids.sort_by_key(|(index, _, _)| *index);
        for (index, id, di) in di_ids {
            let at = index.min(plane.di_ids.len());
            plane.di_ids.shift_insert(at, id.clone(), di.clone());
        }

        if let Some(diagrams) = &self.diagrams {
            let owner = defs.plane_owner().cloned();
            defs.diagrams = diagrams.clone();
            defs.open_plane(owner.as_ref());
        }
    }
}

/// Restores a DI list from before-images keyed by element id.
fn restore_vec<T: Clone>(list: &mut Vec<T>, before: &[Before<T>], key: fn(&T) -> &ElementId) {
    list.retain(|item| !before.iter().any(|(id, _)| id == key(item)));

    let mut items: Vec<_> = before.iter().filter_map(|(_, b)| b.as_ref()).collect();
    items.sort_by_key(|(index, _)| *index);
    for (index, item) in items {
        let at = (*index).min(list.len());
        list.insert(at, item.clone());
    }
}
