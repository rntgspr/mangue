//! YAML themes: canvas colors, stroke widths and label font. Built-ins are
//! Solarized light/dark (`assets/themes/`), chosen by window appearance;
//! user themes under `themes/` add named themes; a user file may pin or override them (see `specs/modeler/theme.md`).

mod builtin;
mod component;
mod color;
mod library;
mod schema;
mod select;
mod user;

pub use builtin::{DARK, LIGHT, NAMES, builtin};
pub use color::Color;
pub use library::{Library, load_dir, resolve};
pub use schema::Theme;
pub use select::{apply, for_appearance};
pub use user::{Themes, config_dir, config_path, load, load_picked, names, parse, parse_picked};
