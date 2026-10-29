use crate::model::{
    AssociationDirection, Bounds, Definitions, Diagram, Edge, Element, ElementDetails, ElementId,
    ElementKind, EventDefinition, Plane, RootElement, RootKind, Shape, TimerExpression, Waypoint,
};

use super::error::ImportError;
use super::ns::EVENT_DEFINITIONS;
use super::tree::{self, Node, Ns};

/// Parses a BPMN 2.0 XML document into a `Definitions` model.
///
/// Unknown or unmodeled elements (and the DI that points at them) are skipped.
/// Lane membership (`flowNodeRef`) becomes the node's `parent`; the `default`
/// attribute of a gateway or activity becomes `is_default` on the flow.
pub fn import_xml(xml: &str) -> Result<Definitions, ImportError> {
    let root = tree::parse(xml)?;

    if !root.is(Ns::Bpmn, "definitions") {
        return Err(ImportError::NotDefinitions);
    }

    let mut importer = Importer {
        defs: Definitions {
            id: root.attr("id").unwrap_or_default().to_string(),
            ..Definitions::default()
        },
        defaults: Vec::new(),
        lane_refs: Vec::new(),
        seen_diagram: false,
    };

    for child in &root.children {
        match (child.ns, child.name.as_str()) {
            (Ns::Bpmn, "process") => {
                let id = importer.add(child, ElementKind::Process, None)?;
                importer.container(child, &id)?;
            }

            (Ns::Bpmn, "collaboration") => {
                let id = importer.add(child, ElementKind::Collaboration, None)?;
                importer.container(child, &id)?;
            }

            (Ns::Bpmn, "message" | "signal" | "error" | "escalation") => {
                importer.root_element(child)?;
            }

            (Ns::BpmnDi, "BPMNDiagram") => {
                importer.diagram(child)?;
            }

            _ => {}
        }
    }

    Ok(importer.finish())
}

/// Import state: the model being built plus references resolved at the end.
struct Importer {
    defs: Definitions,
    defaults: Vec<String>,
    lane_refs: Vec<(String, String)>,
    /// Whether the root diagram was read (later ones are sub-process planes).
    seen_diagram: bool,
}

impl Importer {
    /// Adds `node` as an element of `kind` under `parent`; returns its id.
    fn add(
        &mut self,
        node: &Node,
        kind: ElementKind,
        parent: Option<&str>,
    ) -> Result<String, ImportError> {
        let id = required(node, "id")?.to_string();
        let name = match kind {
            ElementKind::TextAnnotation { .. } => None,
            _ => node.attr("name").map(str::to_string),
        };

        let element = Element {
            id: id.as_str().into(),
            kind,
            name,
            documentation: node
                .child(Ns::Bpmn, "documentation")
                .map(|d| d.text.clone()),
            parent: parent.map(ElementId::from),
            details: details_of(node),
        };

        if !self.defs.add_element(element) {
            return Err(ImportError::DuplicateId(id));
        }

        Ok(id)
    }

    /// Records a root `message`, `signal`, `error` or `escalation`.
    fn root_element(&mut self, node: &Node) -> Result<(), ImportError> {
        let kind = match node.name.as_str() {
            "message" => RootKind::Message,
            "signal" => RootKind::Signal,
            "error" => RootKind::Error,
            _ => RootKind::Escalation,
        };

        self.defs.root_elements.push(RootElement {
            id: required(node, "id")?.into(),
            kind,
            name: node.attr("name").map(str::to_string),
        });

        Ok(())
    }

    /// Imports the flow elements, artifacts and lanes contained in `node`.
    fn container(&mut self, node: &Node, parent: &str) -> Result<(), ImportError> {
        for child in node.children.iter().filter(|c| c.ns == Ns::Bpmn) {
            if child.name == "laneSet" {
                self.lanes(child, parent)?;
                continue;
            }

            let Some(kind) = kind_of(child)? else {
                continue;
            };

            let is_sub_process = matches!(kind, ElementKind::SubProcess { .. });
            let id = self.add(child, kind, Some(parent))?;

            if let Some(flow) = child.attr("default") {
                self.defaults.push(flow.to_string());
            }

            self.data_associations(child, &id, parent)?;

            if is_sub_process {
                self.container(child, &id)?;
            }
        }

        Ok(())
    }

    /// Imports the lanes of a `laneSet` (recursing into `childLaneSet`).
    fn lanes(&mut self, lane_set: &Node, parent: &str) -> Result<(), ImportError> {
        if let Some(owner) = self.defs.elements.get_mut(&ElementId::from(parent)) {
            owner.details.lane_set_id = lane_set.attr("id").map(str::to_string);
        }

        for lane in lane_set.children.iter().filter(|c| c.is(Ns::Bpmn, "lane")) {
            let id = self.add(lane, ElementKind::Lane, Some(parent))?;

            for node_ref in lane
                .children
                .iter()
                .filter(|c| c.is(Ns::Bpmn, "flowNodeRef"))
            {
                self.lane_refs
                    .push((node_ref.text.trim().to_string(), id.clone()));
            }

            if let Some(child_set) = lane.child(Ns::Bpmn, "childLaneSet") {
                self.lanes(child_set, &id)?;
            }
        }

        Ok(())
    }

    /// Imports data input/output associations nested in a flow node.
    /// The node is the association's target (input) or source (output).
    fn data_associations(
        &mut self,
        owner: &Node,
        owner_id: &str,
        parent: &str,
    ) -> Result<(), ImportError> {
        for child in owner.children.iter().filter(|c| c.ns == Ns::Bpmn) {
            let kind = match child.name.as_str() {
                "dataInputAssociation" => {
                    ref_text(child, "sourceRef").map(|source| ElementKind::DataInputAssociation {
                        source: source.as_str().into(),
                        target: owner_id.into(),
                    })
                }

                "dataOutputAssociation" => {
                    ref_text(child, "targetRef").map(|target| ElementKind::DataOutputAssociation {
                        source: owner_id.into(),
                        target: target.as_str().into(),
                    })
                }

                _ => None,
            };

            if let Some(kind) = kind {
                self.add(child, kind, Some(parent))?;
            }
        }

        Ok(())
    }

    /// Imports the plane of a `BPMNDiagram`: the first one fills the root
    /// diagram, later ones (collapsed sub-process planes) are appended.
    fn diagram(&mut self, diagram: &Node) -> Result<(), ImportError> {
        let Some(plane) = diagram.child(Ns::BpmnDi, "BPMNPlane") else {
            return Ok(());
        };

        let mut out = Plane {
            id: plane.attr("id").unwrap_or_default().to_string(),
            element_id: plane.attr("bpmnElement").map(ElementId::from),
            ..Plane::default()
        };

        for child in &plane.children {
            let Some(element) = child.attr("bpmnElement") else {
                continue;
            };

            if let Some(di_id) = child.attr("id")
                && di_id != format!("{element}_di")
            {
                out.di_ids.insert(element.into(), di_id.to_string());
            }

            if child.is(Ns::BpmnDi, "BPMNShape") {
                let Some(bounds) = child.child(Ns::Dc, "Bounds") else {
                    continue;
                };

                out.shapes.push(Shape {
                    element_id: element.into(),
                    bounds: bounds_of(bounds)?,
                    is_expanded: flag(child, "isExpanded"),
                    is_horizontal: flag(child, "isHorizontal"),
                    label_bounds: label_bounds(child)?,
                });
            } else if child.is(Ns::BpmnDi, "BPMNEdge") {
                let mut waypoints = Vec::new();

                for point in child.children.iter().filter(|c| c.is(Ns::Di, "waypoint")) {
                    waypoints.push(Waypoint {
                        x: number(point, "x")?,
                        y: number(point, "y")?,
                    });
                }

                out.edges.push(Edge {
                    element_id: element.into(),
                    waypoints,
                    label_bounds: label_bounds(child)?,
                });
            }
        }

        let out = Diagram {
            id: diagram.attr("id").unwrap_or_default().to_string(),
            plane: out,
        };
        if self.seen_diagram {
            self.defs.diagrams.push(out);
        } else {
            self.defs.diagrams[0] = out;
            self.seen_diagram = true;
        }
        Ok(())
    }

    /// Resolves lane membership and default flows, drops DI of skipped elements.
    fn finish(mut self) -> Definitions {
        for (node, lane) in &self.lane_refs {
            if let Some(element) = self.defs.elements.get_mut(&ElementId::from(node.as_str())) {
                element.parent = Some(lane.as_str().into());
            }
        }

        for flow in &self.defaults {
            if let Some(element) = self.defs.elements.get_mut(&ElementId::from(flow.as_str()))
                && let ElementKind::SequenceFlow { is_default, .. } = &mut element.kind
            {
                *is_default = true;
            }
        }

        let elements = &self.defs.elements;
        let mut index = 0;
        self.defs.diagrams.retain(|d| {
            index += 1;
            index == 1
                || d.plane
                    .element_id
                    .as_ref()
                    .is_some_and(|e| elements.contains_key(e))
        });

        for diagram in &mut self.defs.diagrams {
            let plane = &mut diagram.plane;
            plane
                .shapes
                .retain(|s| elements.contains_key(&s.element_id));
            plane.edges.retain(|e| elements.contains_key(&e.element_id));
            plane.di_ids.retain(|id, _| elements.contains_key(id));
        }

        self.defs
    }
}

/// Maps a BPMN element to its kind; `None` for elements mangue does not model.
fn kind_of(node: &Node) -> Result<Option<ElementKind>, ImportError> {
    use ElementKind::*;

    let kind = match node.name.as_str() {
        "startEvent" => StartEvent(event_definition(node)),
        "endEvent" => EndEvent(event_definition(node)),
        "intermediateCatchEvent" => IntermediateCatchEvent(event_definition(node)),
        "intermediateThrowEvent" => IntermediateThrowEvent(event_definition(node)),
        "boundaryEvent" => BoundaryEvent {
            definition: event_definition(node),
            attached_to: required(node, "attachedToRef")?.into(),
            cancel_activity: node.attr("cancelActivity") != Some("false"),
        },

        "task" => Task,
        "userTask" => UserTask,
        "serviceTask" => ServiceTask,
        "scriptTask" => ScriptTask,
        "sendTask" => SendTask,
        "receiveTask" => ReceiveTask,
        "manualTask" => ManualTask,
        "businessRuleTask" => BusinessRuleTask,
        "callActivity" => CallActivity,
        "subProcess" => SubProcess {
            triggered_by_event: node.attr("triggeredByEvent") == Some("true"),
        },

        "exclusiveGateway" => ExclusiveGateway,
        "parallelGateway" => ParallelGateway,
        "inclusiveGateway" => InclusiveGateway,
        "eventBasedGateway" => EventBasedGateway,
        "complexGateway" => ComplexGateway,

        "dataObjectReference" => DataObjectReference,
        "dataStoreReference" => DataStoreReference,
        "textAnnotation" => TextAnnotation {
            text: node.child(Ns::Bpmn, "text").map(|t| t.text.clone()),
        },
        "group" => Group,
        "participant" => Participant {
            process_ref: node.attr("processRef").map(ElementId::from),
        },

        "sequenceFlow" => SequenceFlow {
            source: required(node, "sourceRef")?.into(),
            target: required(node, "targetRef")?.into(),
            is_default: false,
            condition: node
                .child(Ns::Bpmn, "conditionExpression")
                .map(|c| c.text.clone()),
        },
        "messageFlow" => MessageFlow {
            source: required(node, "sourceRef")?.into(),
            target: required(node, "targetRef")?.into(),
        },
        "association" => Association {
            source: required(node, "sourceRef")?.into(),
            target: required(node, "targetRef")?.into(),
        },

        _ => return Ok(None),
    };

    Ok(Some(kind))
}

/// First known event definition child of an event, `None` when absent.
fn event_definition(node: &Node) -> EventDefinition {
    definition_node(node)
        .map(|(def, _)| def)
        .unwrap_or(EventDefinition::None)
}

/// First known event definition child of an event, with its node.
fn definition_node(node: &Node) -> Option<(EventDefinition, &Node)> {
    node.children
        .iter()
        .filter(|c| c.ns == Ns::Bpmn)
        .find_map(|c| {
            EVENT_DEFINITIONS
                .iter()
                .find(|(_, name)| *name == c.name)
                .map(|(def, _)| (*def, c))
        })
}

/// Reads the optional attributes kept in `ElementDetails`: process and
/// start-event flags, event definition payloads, association direction.
fn details_of(node: &Node) -> ElementDetails {
    let mut details = ElementDetails {
        is_executable: (node.name == "process")
            .then(|| flag(node, "isExecutable"))
            .flatten(),
        is_interrupting: (node.name == "startEvent")
            .then(|| flag(node, "isInterrupting"))
            .flatten(),
        is_for_compensation: flag(node, "isForCompensation"),
        ..ElementDetails::default()
    };

    if node.name == "association" {
        details.association_direction = match node.attr("associationDirection") {
            Some("None") => Some(AssociationDirection::None),
            Some("One") => Some(AssociationDirection::One),
            Some("Both") => Some(AssociationDirection::Both),
            _ => None,
        };
    }

    let Some((_, definition)) = definition_node(node) else {
        return details;
    };

    details.event_ref = ["messageRef", "signalRef", "errorRef", "escalationRef"]
        .iter()
        .find_map(|attr| definition.attr(attr))
        .map(ElementId::from);

    details.timer = definition.children.iter().find_map(|c| {
        let text = c.text.trim().to_string();
        match c.name.as_str() {
            "timeDate" => Some(TimerExpression::Date(text)),
            "timeDuration" => Some(TimerExpression::Duration(text)),
            "timeCycle" => Some(TimerExpression::Cycle(text)),
            _ => None,
        }
    });

    details.condition = definition
        .child(Ns::Bpmn, "condition")
        .map(|c| c.text.trim().to_string())
        .filter(|t| !t.is_empty());

    details
}

/// Required attribute, or a `MissingAttribute` error naming the element.
fn required<'a>(node: &'a Node, attribute: &'static str) -> Result<&'a str, ImportError> {
    node.attr(attribute)
        .ok_or_else(|| ImportError::MissingAttribute {
            element: node.attr("id").unwrap_or(&node.name).to_string(),
            attribute,
        })
}

/// Trimmed text of the first `bpmn:<name>` child (e.g. `sourceRef`).
fn ref_text(node: &Node, name: &str) -> Option<String> {
    node.child(Ns::Bpmn, name)
        .map(|c| c.text.trim().to_string())
        .filter(|t| !t.is_empty())
}

/// Optional boolean DI attribute (`isExpanded`, `isHorizontal`).
fn flag(node: &Node, attribute: &str) -> Option<bool> {
    match node.attr(attribute) {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    }
}

/// Numeric attribute of a DI element.
fn number(node: &Node, attribute: &str) -> Result<f64, ImportError> {
    let raw = node.attr(attribute).unwrap_or("0");
    raw.trim()
        .parse()
        .map_err(|_| ImportError::InvalidNumber(raw.to_string()))
}

/// Reads a `dc:Bounds` element.
fn bounds_of(node: &Node) -> Result<Bounds, ImportError> {
    Ok(Bounds {
        x: number(node, "x")?,
        y: number(node, "y")?,
        w: number(node, "width")?,
        h: number(node, "height")?,
    })
}

/// Bounds of the `bpmndi:BPMNLabel` of a shape or edge, if any.
fn label_bounds(node: &Node) -> Result<Option<Bounds>, ImportError> {
    node.child(Ns::BpmnDi, "BPMNLabel")
        .and_then(|label| label.child(Ns::Dc, "Bounds"))
        .map(bounds_of)
        .transpose()
}
