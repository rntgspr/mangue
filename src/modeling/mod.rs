//! Editing operations on a `Definitions`: commands with exact undo, a
//! command stack, connection layout, modeling rules and command builders.
//! GPUI-independent.

pub mod arrange;
pub mod attach;
pub mod auto_resize;
pub mod clipboard;
mod command;
pub mod factory;
pub mod label;
pub mod lanes;
pub mod layout;
pub mod ordering;
mod planes;
pub mod rules;
pub mod snapping;
mod stack;
mod undo;
pub mod waypoints;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_arrange;

#[cfg(test)]
mod tests_factory;

#[cfg(test)]
mod tests_layout;

#[cfg(test)]
mod tests_orthogonal;

#[cfg(test)]
mod tests_vertical;

#[cfg(test)]
mod tests_parity;

#[cfg(test)]
mod tests_planes;

#[cfg(test)]
mod tests_refusal;

#[cfg(test)]
mod tests_rules;

pub use command::{Command, created_id, created_shape};
pub(crate) use planes::compat_planes;
pub use stack::CommandStack;
pub use undo::Undo;
