//! BPMN 2.0 XML import/export, GPUI-independent.
//!
//! Elements are matched by namespace URI, so any prefix (or a default
//! namespace) is accepted on import. Export writes the prefixes bpmn-js uses.

mod error;
mod export;
mod import;
mod load;
mod new_diagram;
mod ns;
mod tree;

pub use error::ImportError;
pub use export::export_xml;
pub use import::import_xml;
pub use load::load;
pub use new_diagram::new_diagram;
