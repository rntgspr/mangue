/// Identifier of a BPMN element, matching the XML `id` attribute.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ElementId(pub String);

impl ElementId {
    /// Returns the id as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ElementId {
    /// Builds an id from a string slice.
    fn from(value: &str) -> Self {
        ElementId(value.to_string())
    }
}
