//! BPMN 2.0 semantic model and diagram interchange (DI), GPUI-independent.

mod definitions;
mod details;
mod di;
mod id;
mod kind;

#[cfg(test)]
mod tests;

pub use definitions::{Definitions, Element};
pub use details::{AssociationDirection, ElementDetails, RootElement, RootKind, TimerExpression};
pub use di::{Bounds, Diagram, Edge, Plane, Shape, Waypoint};
pub use id::ElementId;
pub use kind::{ElementKind, EventDefinition};
