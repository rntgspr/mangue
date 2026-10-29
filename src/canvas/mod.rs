//! Diagram canvas: pure viewport geometry plus the GPUI view that paints and
//! edits it, with its palette, context pad and replace menu.

pub mod actions;
mod append_menu;
mod auto_scroll;
mod breadcrumbs;
mod code_view;
pub mod context_pad;
pub mod demo;
mod doc_header;
mod document;
mod drill_button;
pub mod drilldown;
mod empty_state;
pub mod geometry;
mod grid;
mod handles;
mod icon;
mod menu_shell;
pub mod overlay;
mod overlay_box;
mod paint;
pub mod palette;
pub mod persist;
mod replace_menu;
mod search;
mod sidebar;
mod tab_bar;
mod text_edit;
mod text_element;
mod text_input;
mod view;
mod xml_error;

pub use code_view::init_editor;
pub use geometry::Viewport;
pub use menu_shell::MenuShell;
pub use view::Canvas;
