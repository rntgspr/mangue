use std::fmt;

/// Why a BPMN XML document could not be imported.
#[derive(Debug, Clone, PartialEq)]
pub enum ImportError {
    /// The document is not well-formed XML.
    Xml(String),
    /// The root element is not `bpmn:definitions`.
    NotDefinitions,
    /// A known element lacks an attribute it cannot be modeled without.
    MissingAttribute {
        element: String,
        attribute: &'static str,
    },
    /// Two elements share the same id.
    DuplicateId(String),
    /// A DI coordinate is not a number.
    InvalidNumber(String),
}

impl fmt::Display for ImportError {
    /// Formats the error as a short human-readable message.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Xml(msg) => write!(f, "malformed XML: {msg}"),
            ImportError::NotDefinitions => write!(f, "root element is not bpmn:definitions"),
            ImportError::MissingAttribute { element, attribute } => {
                write!(f, "element {element} is missing attribute {attribute}")
            }
            ImportError::DuplicateId(id) => write!(f, "duplicate element id {id}"),
            ImportError::InvalidNumber(value) => write!(f, "invalid number {value:?}"),
        }
    }
}

impl std::error::Error for ImportError {}
