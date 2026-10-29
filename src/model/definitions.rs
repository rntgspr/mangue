use indexmap::IndexMap;

use super::details::{ElementDetails, RootElement};
use super::di::{Diagram, Edge, Plane, Shape};
use super::id::ElementId;
use super::kind::ElementKind;

/// A semantic BPMN element: shared attributes plus its kind.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub id: ElementId,
    pub kind: ElementKind,
    pub name: Option<String>,
    pub documentation: Option<String>,
    pub parent: Option<ElementId>,
    pub details: ElementDetails,
}

impl Element {
    /// Builds an element without name, documentation or parent.
    pub fn new(id: &str, kind: ElementKind) -> Self {
        Element {
            id: id.into(),
            kind,
            name: None,
            documentation: None,
            parent: None,
            details: ElementDetails::default(),
        }
    }

    /// Sets the parent container (process, sub-process, participant, lane).
    pub fn with_parent(mut self, parent: &str) -> Self {
        self.parent = Some(parent.into());
        self
    }
}

/// Root of a BPMN document (`bpmn:definitions`): all elements in document
/// order, root messages/signals/errors/escalations and the diagram interchange.
///
/// `diagrams` holds the root diagram (process or collaboration) first, then
/// one per collapsed sub-process with its own plane (bpmn-js drill-down).
/// `active` indexes the plane being edited: the DI helpers (`shapes`,
/// `add_shape`, `plane`, ...) work on it, so callers stay plane-agnostic.
#[derive(Debug, Clone, PartialEq)]
pub struct Definitions {
    pub id: String,
    pub elements: IndexMap<ElementId, Element>,
    pub root_elements: Vec<RootElement>,
    pub diagrams: Vec<Diagram>,
    pub active: usize,
}

impl Default for Definitions {
    /// An empty document with one empty root diagram, active.
    fn default() -> Self {
        Definitions {
            id: String::new(),
            elements: IndexMap::new(),
            root_elements: Vec::new(),
            diagrams: vec![Diagram::default()],
            active: 0,
        }
    }
}

impl Definitions {
    /// The active plane (the root one unless drilled into a sub-process).
    pub fn plane(&self) -> &Plane {
        &self.diagrams[self.active].plane
    }

    /// The active plane, mutably.
    pub fn plane_mut(&mut self) -> &mut Plane {
        &mut self.diagrams[self.active].plane
    }

    /// The root plane (process or collaboration), whatever plane is active.
    pub fn root_plane(&self) -> &Plane {
        &self.diagrams[0].plane
    }

    /// Index of the non-root diagram whose plane shows sub-process `id`.
    pub fn plane_index(&self, id: &ElementId) -> Option<usize> {
        self.diagrams
            .iter()
            .skip(1)
            .position(|d| d.plane.element_id.as_ref() == Some(id))
            .map(|i| i + 1)
    }

    /// Sub-process whose plane is active; `None` on the root plane.
    pub fn plane_owner(&self) -> Option<&ElementId> {
        if self.active == 0 {
            return None;
        }

        self.plane().element_id.as_ref()
    }

    /// Activates the plane of sub-process `owner`, or the root plane for
    /// `None`; returns `false` (root activated) when `owner` has no plane.
    pub fn open_plane(&mut self, owner: Option<&ElementId>) -> bool {
        let index = owner.and_then(|id| self.plane_index(id));
        self.active = index.unwrap_or(0);
        index.is_some() || owner.is_none()
    }

    /// Inserts an element; returns `false` and changes nothing if the id exists.
    pub fn add_element(&mut self, element: Element) -> bool {
        if self.elements.contains_key(&element.id) {
            return false;
        }

        self.elements.insert(element.id.clone(), element);
        true
    }

    /// Looks up an element by id.
    pub fn get(&self, id: &ElementId) -> Option<&Element> {
        self.elements.get(id)
    }

    /// Removes an element, every connection attached to it and their DI (in
    /// every plane), plus the planes of removed sub-processes.
    /// Returns the ids actually removed, the requested element first.
    pub fn remove_element(&mut self, id: &ElementId) -> Vec<ElementId> {
        if !self.elements.contains_key(id) {
            return Vec::new();
        }

        let mut removed = vec![id.clone()];
        let attached: Vec<ElementId> = self
            .incoming(id)
            .chain(self.outgoing(id))
            .map(|e| e.id.clone())
            .collect();

        for cid in attached {
            if !removed.contains(&cid) {
                removed.push(cid);
            }
        }

        for rid in &removed {
            self.elements.shift_remove(rid);
        }

        for diagram in &mut self.diagrams {
            let plane = &mut diagram.plane;
            plane.shapes.retain(|s| !removed.contains(&s.element_id));
            plane.edges.retain(|e| !removed.contains(&e.element_id));
            plane.di_ids.retain(|id, _| !removed.contains(id));
        }

        let owner = self.plane_owner().cloned();
        let mut index = 0;
        self.diagrams.retain(|d| {
            index += 1;
            index == 1
                || !d
                    .plane
                    .element_id
                    .as_ref()
                    .is_some_and(|e| removed.contains(e))
        });
        self.open_plane(owner.as_ref());

        removed
    }

    /// Connections whose target is `id`, in document order.
    pub fn incoming<'a>(&'a self, id: &'a ElementId) -> impl Iterator<Item = &'a Element> + 'a {
        self.elements
            .values()
            .filter(move |e| e.kind.endpoints().is_some_and(|(_, t)| t == id))
    }

    /// Connections whose source is `id`, in document order.
    pub fn outgoing<'a>(&'a self, id: &'a ElementId) -> impl Iterator<Item = &'a Element> + 'a {
        self.elements
            .values()
            .filter(move |e| e.kind.endpoints().is_some_and(|(s, _)| s == id))
    }

    /// Elements whose parent is `id`, in document order.
    pub fn children<'a>(&'a self, id: &'a ElementId) -> impl Iterator<Item = &'a Element> + 'a {
        self.elements
            .values()
            .filter(move |e| e.parent.as_ref() == Some(id))
    }

    /// Appends a DI shape to the active plane.
    pub fn add_shape(&mut self, shape: Shape) {
        self.plane_mut().shapes.push(shape);
    }

    /// Appends a DI edge to the active plane.
    pub fn add_edge(&mut self, edge: Edge) {
        self.plane_mut().edges.push(edge);
    }

    /// Iterates the active plane's DI shapes in paint order.
    pub fn shapes(&self) -> impl Iterator<Item = &Shape> {
        self.plane().shapes.iter()
    }

    /// Iterates the active plane's DI edges in paint order.
    pub fn edges(&self) -> impl Iterator<Item = &Edge> {
        self.plane().edges.iter()
    }

    /// Finds the DI shape of an element on the active plane.
    pub fn shape(&self, id: &ElementId) -> Option<&Shape> {
        self.shapes().find(|s| &s.element_id == id)
    }
}
