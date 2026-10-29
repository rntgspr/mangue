use quick_xml::NsReader;
use quick_xml::XmlVersion;
use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;

use super::error::ImportError;
use super::ns;

/// Namespace of an XML element, resolved from its URI (prefix-independent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ns {
    Bpmn,
    BpmnDi,
    Dc,
    Di,
    Other,
}

/// Namespace-resolved XML element: unqualified attributes, children, text.
#[derive(Debug, Clone)]
pub struct Node {
    pub ns: Ns,
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
    pub text: String,
}

impl Node {
    /// True when the element has namespace `ns` and local name `name`.
    pub fn is(&self, ns: Ns, name: &str) -> bool {
        self.ns == ns && self.name == name
    }

    /// Value of the unqualified attribute `name`.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// First child with namespace `ns` and local name `name`.
    pub fn child(&self, ns: Ns, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.is(ns, name))
    }
}

/// Parses `xml` into a tree rooted at the document element.
pub fn parse(xml: &str) -> Result<Node, ImportError> {
    let mut reader = NsReader::from_str(xml);
    let mut stack: Vec<Node> = Vec::new();

    loop {
        let (resolved, event) = reader.read_resolved_event().map_err(xml_error)?;
        let ns = namespace(&resolved);

        match event {
            Event::Start(start) => stack.push(node(&reader, ns, &start)?),

            Event::Empty(start) => {
                let leaf = node(&reader, ns, &start)?;
                if let Some(root) = attach(&mut stack, leaf) {
                    return Ok(root);
                }
            }

            Event::End(_) => {
                let done = stack.pop().ok_or_else(|| xml_msg("unbalanced end tag"))?;
                if let Some(root) = attach(&mut stack, done) {
                    return Ok(root);
                }
            }

            Event::Text(text) => push_text(&mut stack, &text.xml10_content()),
            Event::CData(data) => push_text(&mut stack, &data.xml10_content()),

            Event::GeneralRef(entity) => {
                let resolved = match entity.resolve_char_ref().map_err(xml_error)? {
                    Some(ch) => ch.to_string(),
                    None => resolve_predefined_entity(&entity)
                        .ok_or_else(|| xml_msg(&format!("unknown entity &{};", &*entity)))?
                        .to_string(),
                };
                push_text(&mut stack, &resolved);
            }

            Event::Eof => return Err(xml_msg("unexpected end of document")),
            _ => {}
        }
    }
}

/// Maps a resolved namespace to the known `Ns` set.
fn namespace(resolved: &ResolveResult<'_>) -> Ns {
    let ResolveResult::Bound(uri) = resolved else {
        return Ns::Other;
    };

    match uri.0 {
        ns::BPMN => Ns::Bpmn,
        ns::BPMNDI => Ns::BpmnDi,
        ns::DC => Ns::Dc,
        ns::DI => Ns::Di,
        _ => Ns::Other,
    }
}

/// Builds a childless node from a start tag, keeping unqualified attributes.
fn node(reader: &NsReader<&[u8]>, ns: Ns, start: &BytesStart<'_>) -> Result<Node, ImportError> {
    let mut attrs = Vec::new();

    for attr in start.attributes() {
        let attr = attr.map_err(xml_error)?;
        let (resolved, local) = reader.resolver().resolve_attribute(attr.key);

        if matches!(resolved, ResolveResult::Unbound) && local.as_ref() != "xmlns" {
            let value = attr
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(xml_error)?;
            attrs.push((local.as_ref().to_string(), value.into_owned()));
        }
    }

    Ok(Node {
        ns,
        name: start.local_name().as_ref().to_string(),
        attrs,
        children: Vec::new(),
        text: String::new(),
    })
}

/// Appends `node` to the open parent; returns it when it is the root.
fn attach(stack: &mut [Node], node: Node) -> Option<Node> {
    match stack.last_mut() {
        Some(parent) => {
            parent.children.push(node);
            None
        }
        None => Some(node),
    }
}

/// Appends character data to the innermost open element.
fn push_text(stack: &mut [Node], text: &str) {
    if let Some(top) = stack.last_mut() {
        top.text.push_str(text);
    }
}

/// Wraps any displayable parser error.
fn xml_error(err: impl std::fmt::Display) -> ImportError {
    ImportError::Xml(err.to_string())
}

/// Builds an XML error from a message.
fn xml_msg(msg: &str) -> ImportError {
    ImportError::Xml(msg.to_string())
}
