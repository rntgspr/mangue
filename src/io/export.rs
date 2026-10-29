use quick_xml::escape::escape;

use crate::model::{
    AssociationDirection, Bounds, Definitions, Diagram, Element, ElementId, ElementKind,
    EventDefinition, Plane, RootKind, TimerExpression,
};

use super::ns::{self, EVENT_DEFINITIONS};

/// Serializes the model as BPMN 2.0 XML in the form bpmn-js exports:
/// `bpmn:`/`bpmndi:`/`dc:`/`di:` prefixes, `bpmn:incoming`/`bpmn:outgoing`
/// children on flow nodes, collaboration before processes.
pub fn export_xml(defs: &Definitions) -> String {
    let mut xml = Xml::default();
    let id = if defs.id.is_empty() {
        "Definitions_1"
    } else {
        &defs.id
    };

    xml.out
        .push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.start(
        "bpmn:definitions",
        &[
            ("xmlns:bpmn", ns::BPMN),
            ("xmlns:bpmndi", ns::BPMNDI),
            ("xmlns:dc", ns::DC),
            ("xmlns:di", ns::DI),
            ("xmlns:xsi", ns::XSI),
            ("id", id),
            ("targetNamespace", "http://bpmn.io/schema/bpmn"),
            ("exporter", "mangue"),
            ("exporterVersion", env!("CARGO_PKG_VERSION")),
        ],
    );

    let roots = defs.elements.values().filter(|e| e.parent.is_none());
    let (collaborations, processes): (Vec<_>, Vec<_>) = roots
        .filter(|e| matches!(e.kind, ElementKind::Collaboration | ElementKind::Process))
        .partition(|e| e.kind == ElementKind::Collaboration);

    for root in collaborations.into_iter().chain(processes) {
        let tag = match root.kind {
            ElementKind::Collaboration => "bpmn:collaboration",
            _ => "bpmn:process",
        };

        let mut attrs = vec![("id", root.id.as_str())];
        if let Some(name) = &root.name {
            attrs.push(("name", name));
        }
        if let Some(executable) = root.details.is_executable {
            attrs.push(("isExecutable", bool_str(executable)));
        }

        xml.start(tag, &attrs);
        documentation(&mut xml, root);
        lane_set(&mut xml, defs, &root.id);
        children(&mut xml, defs, &root.id);
        xml.end(tag);
    }

    root_elements(&mut xml, defs);
    diagrams(&mut xml, defs);
    xml.end("bpmn:definitions");
    xml.out
}

/// Writes the root `bpmn:message`/`signal`/`error`/`escalation` elements.
fn root_elements(xml: &mut Xml, defs: &Definitions) {
    for root in &defs.root_elements {
        let tag = match root.kind {
            RootKind::Message => "bpmn:message",
            RootKind::Signal => "bpmn:signal",
            RootKind::Error => "bpmn:error",
            RootKind::Escalation => "bpmn:escalation",
        };

        let mut attrs = vec![("id", root.id.as_str())];
        if let Some(name) = &root.name {
            attrs.push(("name", name));
        }

        xml.start(tag, &attrs);
        xml.end(tag);
    }
}

/// Writes the elements whose semantic container is `container`, in schema
/// order: participants, flow elements, message flows, artifacts.
fn children(xml: &mut Xml, defs: &Definitions, container: &ElementId) {
    let mut members: Vec<&Element> = defs
        .elements
        .values()
        .filter(|e| semantic_parent(defs, e) == Some(container))
        .filter(|e| {
            !matches!(
                e.kind,
                ElementKind::Lane
                    | ElementKind::DataInputAssociation { .. }
                    | ElementKind::DataOutputAssociation { .. }
            )
        })
        .collect();

    members.sort_by_key(|e| match e.kind {
        ElementKind::Participant { .. } => 0,
        ElementKind::MessageFlow { .. } => 2,
        ElementKind::TextAnnotation { .. }
        | ElementKind::Association { .. }
        | ElementKind::Group => 3,
        _ => 1,
    });

    for element in members {
        write_element(xml, defs, element);
    }
}

/// Container of an element ignoring lanes (lane membership is not nesting).
fn semantic_parent<'a>(defs: &'a Definitions, element: &'a Element) -> Option<&'a ElementId> {
    let mut parent = element.parent.as_ref();

    while let Some(id) = parent {
        match defs.get(id) {
            Some(p) if p.kind == ElementKind::Lane => parent = p.parent.as_ref(),
            _ => break,
        }
    }

    parent
}

/// Writes the `bpmn:laneSet` of a process or lane, when it has lanes.
fn lane_set(xml: &mut Xml, defs: &Definitions, owner: &ElementId) {
    let lanes: Vec<&Element> = defs
        .children(owner)
        .filter(|e| e.kind == ElementKind::Lane)
        .collect();

    if lanes.is_empty() {
        return;
    }

    let is_process = defs.get(owner).is_some_and(|o| o.kind != ElementKind::Lane);
    let tag = if is_process {
        "bpmn:laneSet"
    } else {
        "bpmn:childLaneSet"
    };
    let set_id = defs
        .get(owner)
        .and_then(|o| o.details.lane_set_id.clone())
        .unwrap_or_else(|| format!("LaneSet_{}", owner.as_str()));

    xml.start(tag, &[("id", &set_id)]);

    for lane in lanes {
        let mut attrs = vec![("id", lane.id.as_str())];
        if let Some(name) = &lane.name {
            attrs.push(("name", name));
        }

        xml.start("bpmn:lane", &attrs);
        documentation(xml, lane);

        for member in defs
            .children(&lane.id)
            .filter(|e| e.kind != ElementKind::Lane)
        {
            xml.text_element("bpmn:flowNodeRef", &[], member.id.as_str());
        }

        lane_set(xml, defs, &lane.id);
        xml.end("bpmn:lane");
    }

    xml.end(tag);
}

/// Writes one flow element, artifact or collaboration member.
fn write_element(xml: &mut Xml, defs: &Definitions, element: &Element) {
    use ElementKind::*;

    let tag = tag(&element.kind);
    let data_object_id = format!("DataObject_{}", element.id.as_str());
    let default_flow = defs
        .outgoing(&element.id)
        .find(|f| {
            matches!(
                f.kind,
                SequenceFlow {
                    is_default: true,
                    ..
                }
            )
        })
        .map(|f| f.id.as_str());

    let mut attrs: Vec<(&str, &str)> = vec![("id", element.id.as_str())];

    if let Some(name) = &element.name {
        attrs.push(("name", name));
    }

    match &element.kind {
        BoundaryEvent {
            attached_to,
            cancel_activity,
            ..
        } => {
            if !cancel_activity {
                attrs.push(("cancelActivity", "false"));
            }
            attrs.push(("attachedToRef", attached_to.as_str()));
        }

        StartEvent(_) => {
            if let Some(interrupting) = element.details.is_interrupting {
                attrs.push(("isInterrupting", bool_str(interrupting)));
            }
        }

        SubProcess {
            triggered_by_event: true,
        } => attrs.push(("triggeredByEvent", "true")),

        Participant {
            process_ref: Some(process),
        } => attrs.push(("processRef", process.as_str())),

        DataObjectReference => attrs.push(("dataObjectRef", &data_object_id)),

        SequenceFlow { source, target, .. }
        | MessageFlow { source, target }
        | Association { source, target } => {
            attrs.push(("sourceRef", source.as_str()));
            attrs.push(("targetRef", target.as_str()));
        }

        _ => {}
    }

    if let Some(compensation) = element.details.is_for_compensation {
        attrs.push(("isForCompensation", bool_str(compensation)));
    }

    if let Some(direction) = element.details.association_direction {
        attrs.push((
            "associationDirection",
            match direction {
                AssociationDirection::None => "None",
                AssociationDirection::One => "One",
                AssociationDirection::Both => "Both",
            },
        ));
    }

    if let Some(flow) = default_flow {
        attrs.push(("default", flow));
    }

    xml.start(tag, &attrs);
    documentation(xml, element);

    if is_flow_node(&element.kind) {
        for flow in defs.incoming(&element.id).filter(|f| is_sequence_flow(f)) {
            xml.text_element("bpmn:incoming", &[], flow.id.as_str());
        }
        for flow in defs.outgoing(&element.id).filter(|f| is_sequence_flow(f)) {
            xml.text_element("bpmn:outgoing", &[], flow.id.as_str());
        }

        data_associations(xml, defs, &element.id);
    }

    match &element.kind {
        StartEvent(def)
        | EndEvent(def)
        | IntermediateCatchEvent(def)
        | IntermediateThrowEvent(def) => event_definition(xml, element, *def),
        BoundaryEvent { definition, .. } => event_definition(xml, element, *definition),

        TextAnnotation { text: Some(text) } => xml.text_element("bpmn:text", &[], text),

        SequenceFlow {
            condition: Some(condition),
            ..
        } => xml.text_element(
            "bpmn:conditionExpression",
            &[("xsi:type", "bpmn:tFormalExpression")],
            condition,
        ),

        SubProcess { .. } => children(xml, defs, &element.id),

        _ => {}
    }

    xml.end(tag);

    if element.kind == DataObjectReference {
        xml.start("bpmn:dataObject", &[("id", &data_object_id)]);
        xml.end("bpmn:dataObject");
    }
}

/// Writes the data associations owned by a flow node: inputs it is the
/// target of (with the bpmn-js `__targetRef_placeholder` property), then
/// outputs it is the source of.
fn data_associations(xml: &mut Xml, defs: &Definitions, owner: &ElementId) {
    let inputs: Vec<&Element> = defs
        .incoming(owner)
        .filter(|e| matches!(e.kind, ElementKind::DataInputAssociation { .. }))
        .collect();
    let property_id = format!("Property_{}", owner.as_str());

    if !inputs.is_empty() {
        xml.start(
            "bpmn:property",
            &[("id", &property_id), ("name", "__targetRef_placeholder")],
        );
        xml.end("bpmn:property");
    }

    for input in inputs {
        let Some((source, _)) = input.kind.endpoints() else {
            continue;
        };

        xml.start("bpmn:dataInputAssociation", &[("id", input.id.as_str())]);
        documentation(xml, input);
        xml.text_element("bpmn:sourceRef", &[], source.as_str());
        xml.text_element("bpmn:targetRef", &[], &property_id);
        xml.end("bpmn:dataInputAssociation");
    }

    for output in defs
        .outgoing(owner)
        .filter(|e| matches!(e.kind, ElementKind::DataOutputAssociation { .. }))
    {
        let Some((_, target)) = output.kind.endpoints() else {
            continue;
        };

        xml.start("bpmn:dataOutputAssociation", &[("id", output.id.as_str())]);
        documentation(xml, output);
        xml.text_element("bpmn:targetRef", &[], target.as_str());
        xml.end("bpmn:dataOutputAssociation");
    }
}

/// Writes the event definition child of an event (nothing for `None`),
/// with its reference attribute, timer expression or condition.
fn event_definition(xml: &mut Xml, element: &Element, definition: EventDefinition) {
    let Some((_, name)) = EVENT_DEFINITIONS.iter().find(|(d, _)| *d == definition) else {
        return;
    };

    let tag = format!("bpmn:{name}");
    let prefix = name.strip_suffix("EventDefinition").unwrap_or(name);
    let id = format!(
        "{}{}EventDefinition_{}",
        prefix[..1].to_uppercase(),
        &prefix[1..],
        element.id.as_str()
    );

    let ref_attr = match definition {
        EventDefinition::Message => Some("messageRef"),
        EventDefinition::Signal => Some("signalRef"),
        EventDefinition::Error => Some("errorRef"),
        EventDefinition::Escalation => Some("escalationRef"),
        _ => None,
    };

    let mut attrs = vec![("id", id.as_str())];
    if let (Some(attr), Some(target)) = (ref_attr, &element.details.event_ref) {
        attrs.push((attr, target.as_str()));
    }

    xml.start(&tag, &attrs);

    let formal = [("xsi:type", "bpmn:tFormalExpression")];

    if definition == EventDefinition::Timer
        && let Some(timer) = &element.details.timer
    {
        let (tag, text) = match timer {
            TimerExpression::Date(t) => ("bpmn:timeDate", t),
            TimerExpression::Duration(t) => ("bpmn:timeDuration", t),
            TimerExpression::Cycle(t) => ("bpmn:timeCycle", t),
        };
        xml.text_element(tag, &formal, text);
    }

    if definition == EventDefinition::Conditional {
        match &element.details.condition {
            Some(condition) => xml.text_element("bpmn:condition", &formal, condition),
            None => {
                xml.start("bpmn:condition", &formal);
                xml.end("bpmn:condition");
            }
        }
    }

    xml.end(&tag);
}

/// Writes `bpmn:documentation` when the element has some.
fn documentation(xml: &mut Xml, element: &Element) {
    if let Some(doc) = &element.documentation {
        xml.text_element("bpmn:documentation", &[], doc);
    }
}

/// Writes every `bpmndi:BPMNDiagram`, root first. Missing ids default to
/// `BPMNDiagram_1`/`BPMNPlane_1` for the root and, like bpmn-js, to
/// `BPMNDiagram_<sub>`/`<sub>_plane` for a sub-process plane.
fn diagrams(xml: &mut Xml, defs: &Definitions) {
    for (index, diagram) in defs.diagrams.iter().enumerate() {
        diagram_xml(xml, diagram, index == 0);
    }
}

/// Writes one `bpmndi:BPMNDiagram` with its plane.
fn diagram_xml(xml: &mut Xml, diagram: &Diagram, root: bool) {
    let plane = &diagram.plane;
    let owner = plane.element_id.as_ref().map_or("", |e| e.as_str());
    let diagram_id = match (diagram.id.is_empty(), root) {
        (false, _) => diagram.id.clone(),
        (true, true) => "BPMNDiagram_1".into(),
        (true, false) => format!("BPMNDiagram_{owner}"),
    };
    let plane_id = match (plane.id.is_empty(), root) {
        (false, _) => plane.id.clone(),
        (true, true) => "BPMNPlane_1".into(),
        (true, false) => format!("{owner}_plane"),
    };

    xml.start("bpmndi:BPMNDiagram", &[("id", &diagram_id)]);

    let mut plane_attrs = vec![("id", plane_id.as_str())];
    if let Some(element) = &plane.element_id {
        plane_attrs.push(("bpmnElement", element.as_str()));
    }
    xml.start("bpmndi:BPMNPlane", &plane_attrs);

    for shape in &plane.shapes {
        let id = di_id(plane, &shape.element_id);
        let mut attrs = vec![
            ("id", id.as_str()),
            ("bpmnElement", shape.element_id.as_str()),
        ];

        if let Some(horizontal) = shape.is_horizontal {
            attrs.push(("isHorizontal", bool_str(horizontal)));
        }
        if let Some(expanded) = shape.is_expanded {
            attrs.push(("isExpanded", bool_str(expanded)));
        }

        xml.start("bpmndi:BPMNShape", &attrs);
        bounds(xml, &shape.bounds);
        label(xml, shape.label_bounds.as_ref());
        xml.end("bpmndi:BPMNShape");
    }

    for edge in &plane.edges {
        let id = di_id(plane, &edge.element_id);

        xml.start(
            "bpmndi:BPMNEdge",
            &[("id", &id), ("bpmnElement", edge.element_id.as_str())],
        );

        for point in &edge.waypoints {
            let (x, y) = (point.x.to_string(), point.y.to_string());
            xml.start("di:waypoint", &[("x", &x), ("y", &y)]);
            xml.end("di:waypoint");
        }

        label(xml, edge.label_bounds.as_ref());
        xml.end("bpmndi:BPMNEdge");
    }

    xml.end("bpmndi:BPMNPlane");
    xml.end("bpmndi:BPMNDiagram");
}

/// Original DI id of an element, or the bpmn-js style `<element>_di`.
fn di_id(plane: &Plane, element: &ElementId) -> String {
    plane
        .di_ids
        .get(element)
        .cloned()
        .unwrap_or_else(|| format!("{}_di", element.as_str()))
}

/// Writes a `dc:Bounds` element.
fn bounds(xml: &mut Xml, b: &Bounds) {
    let (x, y, w, h) = (
        b.x.to_string(),
        b.y.to_string(),
        b.w.to_string(),
        b.h.to_string(),
    );

    xml.start(
        "dc:Bounds",
        &[("x", &x), ("y", &y), ("width", &w), ("height", &h)],
    );
    xml.end("dc:Bounds");
}

/// Writes a `bpmndi:BPMNLabel` when label bounds exist.
fn label(xml: &mut Xml, label: Option<&Bounds>) {
    if let Some(b) = label {
        xml.start("bpmndi:BPMNLabel", &[]);
        bounds(xml, b);
        xml.end("bpmndi:BPMNLabel");
    }
}

/// XML boolean literal.
fn bool_str(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

/// True for events, activities and gateways (nodes with incoming/outgoing).
fn is_flow_node(kind: &ElementKind) -> bool {
    use ElementKind::*;

    matches!(
        kind,
        StartEvent(_)
            | EndEvent(_)
            | IntermediateCatchEvent(_)
            | IntermediateThrowEvent(_)
            | BoundaryEvent { .. }
            | Task
            | UserTask
            | ServiceTask
            | ScriptTask
            | SendTask
            | ReceiveTask
            | ManualTask
            | BusinessRuleTask
            | CallActivity
            | SubProcess { .. }
            | ExclusiveGateway
            | ParallelGateway
            | InclusiveGateway
            | EventBasedGateway
            | ComplexGateway
    )
}

/// True when the element is a sequence flow.
fn is_sequence_flow(element: &Element) -> bool {
    matches!(element.kind, ElementKind::SequenceFlow { .. })
}

/// Prefixed XML tag of an element kind.
fn tag(kind: &ElementKind) -> &'static str {
    use ElementKind::*;

    match kind {
        Process => "bpmn:process",
        Collaboration => "bpmn:collaboration",
        StartEvent(_) => "bpmn:startEvent",
        EndEvent(_) => "bpmn:endEvent",
        IntermediateCatchEvent(_) => "bpmn:intermediateCatchEvent",
        IntermediateThrowEvent(_) => "bpmn:intermediateThrowEvent",
        BoundaryEvent { .. } => "bpmn:boundaryEvent",
        Task => "bpmn:task",
        UserTask => "bpmn:userTask",
        ServiceTask => "bpmn:serviceTask",
        ScriptTask => "bpmn:scriptTask",
        SendTask => "bpmn:sendTask",
        ReceiveTask => "bpmn:receiveTask",
        ManualTask => "bpmn:manualTask",
        BusinessRuleTask => "bpmn:businessRuleTask",
        CallActivity => "bpmn:callActivity",
        SubProcess { .. } => "bpmn:subProcess",
        ExclusiveGateway => "bpmn:exclusiveGateway",
        ParallelGateway => "bpmn:parallelGateway",
        InclusiveGateway => "bpmn:inclusiveGateway",
        EventBasedGateway => "bpmn:eventBasedGateway",
        ComplexGateway => "bpmn:complexGateway",
        DataObjectReference => "bpmn:dataObjectReference",
        DataStoreReference => "bpmn:dataStoreReference",
        TextAnnotation { .. } => "bpmn:textAnnotation",
        Group => "bpmn:group",
        Participant { .. } => "bpmn:participant",
        Lane => "bpmn:lane",
        SequenceFlow { .. } => "bpmn:sequenceFlow",
        MessageFlow { .. } => "bpmn:messageFlow",
        Association { .. } => "bpmn:association",
        DataInputAssociation { .. } => "bpmn:dataInputAssociation",
        DataOutputAssociation { .. } => "bpmn:dataOutputAssociation",
    }
}

/// Minimal indenting XML writer; empty elements close as `<tag ... />`.
#[derive(Default)]
struct Xml {
    out: String,
    depth: usize,
    pending: bool,
}

impl Xml {
    /// Opens `<tag attrs`; the tag is closed by the first child or `end`.
    fn start(&mut self, tag: &str, attrs: &[(&str, &str)]) {
        self.close_pending();
        self.indent();
        self.out.push('<');
        self.out.push_str(tag);
        self.attrs(attrs);
        self.pending = true;
        self.depth += 1;
    }

    /// Closes `tag`, as `/>` when nothing was written inside it.
    fn end(&mut self, tag: &str) {
        self.depth -= 1;

        if self.pending {
            self.out.push_str(" />\n");
            self.pending = false;
            return;
        }

        self.indent();
        self.out.push_str("</");
        self.out.push_str(tag);
        self.out.push_str(">\n");
    }

    /// Writes `<tag attrs>text</tag>` on one line.
    fn text_element(&mut self, tag: &str, attrs: &[(&str, &str)], text: &str) {
        self.close_pending();
        self.indent();
        self.out.push('<');
        self.out.push_str(tag);
        self.attrs(attrs);
        self.out.push('>');
        self.out.push_str(&escape(text));
        self.out.push_str("</");
        self.out.push_str(tag);
        self.out.push_str(">\n");
    }

    /// Appends escaped ` key="value"` pairs. Line breaks, tabs and carriage
    /// returns become character references because XML attribute value
    /// normalization would otherwise turn them into spaces (bpmn-js writes
    /// multi-line names as `&#10;`).
    fn attrs(&mut self, attrs: &[(&str, &str)]) {
        for (key, value) in attrs {
            self.out.push(' ');
            self.out.push_str(key);
            self.out.push_str("=\"");

            let escaped = escape(*value)
                .replace('\n', "&#10;")
                .replace('\r', "&#13;")
                .replace('\t', "&#9;");
            self.out.push_str(&escaped);
            self.out.push('"');
        }
    }

    /// Terminates an open start tag with `>` before writing content.
    fn close_pending(&mut self) {
        if self.pending {
            self.out.push_str(">\n");
            self.pending = false;
        }
    }

    /// Writes two spaces per nesting level.
    fn indent(&mut self) {
        for _ in 0..self.depth {
            self.out.push_str("  ");
        }
    }
}
