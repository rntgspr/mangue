---
human_revised: false
name: modeling
summary: GPUI-independent editing commands with exact undo, a bpmn-js style command stack, manhattan layout with obstacle avoidance, connection/attach/resize rules, lane tools, waypoint edits, label placement and create/append/replace builders.
depends-on: [model]
relates: [canvas]
targets: [platform]
---

# Modeling

## Overview

`mangue::modeling` changes a `Definitions` through `Command`s, after bpmn-js
`CommandStack` (https://github.com/bpmn-io/diagram-js/blob/develop/lib/command/CommandStack.js).
Executing a command returns an `Undo` holding before-images (index + value)
of every element, DI shape, DI edge and DI id it touched; restoring it
reverts the change exactly, document order included.

## Requirements (EARS / RFC 2119)

- The module MUST NOT depend on GPUI.
- WHILE editing THE SYSTEM SHALL support undo/redo of every command; undo MUST restore the model to an equal `Definitions`; undo/redo re-open the plane the command ran on.
- `ToggleCollapse` (bpmn-js `SubProcessPlaneBehavior`) MUST move a collapsing sub-process's content into a new `<id>_plane` diagram with its top-left at (180, 160), and an expanding one's plane content back centered on the shape, dropping the plane; the shape takes its new default size around its center, boundary events keep their relative spot. Creating a collapsed sub-process adds its empty plane; deleting it removes the plane. No pools inside sub-process planes.
- A command that changes nothing MUST NOT be recorded; a new command MUST clear redo.
- WHEN shapes move THE SYSTEM SHALL move their children (a participant's process content, lanes, expanded sub-process content) and attached boundary events.
- WHEN both ends of a connection move THE SYSTEM SHALL translate its waypoints; WHEN one end moves and the path has inner bendpoints THE SYSTEM SHALL keep them, translating the end and its neighbor on the axis that keeps the first segment orthogonal; otherwise (or if that neighbor would land inside the moved shape) it SHALL re-route it.
- Sequence and message flows MUST route manhattan (H-V-H or V-H-V, docked at bounds mids), falling back to V-H-V or an L-shape when the default crosses another flow node or data shape; routes MUST follow the bpmn-js preferred layouts (`layout::Rule`): `h:h` by default and around expanded sub-processes, `v:h` out of gateways, `h:v` into gateways, `v:v` for message flows, boundary event flows leaving the side or corner they are attached on (`s:t` from the attach and target orientations), a boundary event looping to its host around it; self-loops MUST route clockwise with 20px legs from the side they currently leave (`t:r`, `r:b`, `b:l`, `l:t`; new loops `r:b`); inside a vertical pool or lane (`isHorizontal="false"` on the pool, lane or the pool of the element's process, `layout::is_direction_horizontal`) the table MUST swap (`v:v` flows, `h:v` out of and `v:h` into gateways, `h:h` message flows, boundary corner cases on the other axis, loops counter-clockwise `t:l`, `r:t`, `l:b`, `b:r`); a compensation association (compensate boundary event to an `isForCompensation` activity, flagged when the association is created) MUST follow the boundary event rule; associations (and data associations) MUST route with the same manhattan layout and obstacle avoidance. Imported waypoints MUST be kept as-is; only created or re-routed connections are laid out.
- WHEN a moved node's center lands in a participant, lane or expanded sub-process THE SYSTEM SHALL reparent it there (fallback: the plane's process) and update its sequence flows' parent.
- WHEN moved, created, appended or pasted shapes come within the padding (2 top/bottom, 15 left/right) of their participant or expanded sub-process border THE SYSTEM SHALL grow that container on those sides to the content plus the offset (60 / 100), lanes following (`lanes::resize`), in the same undoable step, propagating to outer containers (bpmn-js `BpmnAutoResize`).
- Copy MUST accept pools (with process, lanes and content) and lanes (with sub-lanes and flow nodes); a pool MUST paste only into a collaboration with a fresh process; a lane MUST paste as a new lane below the pool under the paste point, falling back to its content alone.
- Saved DI MUST be ordered like bpmn-js `BpmnDiOrdering` (`ordering::order_planes`).
- WHEN elements are deleted THE SYSTEM SHALL delete their descendants, boundary events and attached connections; deleting a lane MUST delete its child lanes only, keep its flow nodes and let the adjacent lanes absorb its height (half each when lanes remain on both sides), then recompute lane membership (bpmn-js `DeleteLaneBehavior`).
- WHEN a boundary-capable event (boundary, plain intermediate throw, message/timer/signal/conditional catch) is dropped within 15px of an activity border THE SYSTEM SHALL attach it there (center snapped onto the border, parent = host's parent, incoming flows deleted), unless the host is an event sub-process or a receive task after an event-based gateway (bpmn-js `canAttach`, `snapBoundaryEvent`). WHEN a boundary event is dragged off every host THE SYSTEM SHALL detach it into an intermediate catch event (throw when it has no definition).
- Resizing MUST be limited to expanded sub-processes, participants, lanes, text annotations and groups, with minimum sizes 140x120 (sub-process, group), 300x150 (participant), 300x60 (lane), 50x30 (text annotation), keeping the unmoved edges and the padded children box inside (lane padding 20/20/20/50 t/r/b/l, else 20). Resizing a participant or lane MUST drag the lanes sharing a moved edge (bpmn-js `computeLanesResize`: 10px adjacency, 5px flush) and recompute lane membership; it MUST be refused when a lane would drop below 60px.
- WHEN a lane is added above/below THE SYSTEM SHALL create a 120px lane there (bpmn-js `AddLaneHandler`): a pool without lanes first gets one lane filling it (30px indentation); on a lane, the pool and other lanes crossing the insertion line grow and the pool's shapes beyond it move. WHEN a pool or lane is split into N THE SYSTEM SHALL resize existing child lanes and create the missing ones with equal rounded heights, the last one taking the rest; more than N existing child lanes MUST refuse it.
- Lane membership (`flowNodeRef`, stored as the node's `parent`) MUST be the innermost lane under the node's center, else the process (bpmn-js `UpdateFlowNodeRefsBehavior`).
- Bendpoint edits MUST NOT move connection ends; moving a segment that touches an end MUST insert a bendpoint so the end stays docked; diagonal segments MUST NOT move.
- External labels (events, gateways, data, sequence/message flows, groups) MUST default to 90x20 centered below the shape (groups: inside the top edge), flows at the middle segment's mid offset 15px up (vertical segments: 15px right) (bpmn-js `getExternalLabelMid`); moving a shape MUST move its label.
- `UpdateLabel` MUST set `text` on text annotations and `name` otherwise; an empty label clears it.
- `Compound` MUST execute its commands as one undoable step; undo restores sub-commands last first.
- Connection rules (`rules::can_connect`, after bpmn-js `BpmnRules`): a text annotation on either end yields an association; data object/store to activity a data input, activity to data a data output; ends in different participants a message flow (sources: participant, activity, end or intermediate throw event; targets: participant, activity, start, intermediate catch or boundary event); flow nodes in the same scope a sequence flow, never out of an end event nor into a start or boundary event. Compensation boundary events MUST connect to another activity of their scope by an `associationDirection="One"` association and never by sequence flow; data input also accepts throw events, data output catch events; sequence flows MUST NOT leave link throws or event sub-processes nor enter link catches or event sub-processes, and out of an event-based gateway MUST only reach receive tasks and message/timer/signal/conditional catch events. Self-connections MUST be rejected except sequence-flow loops.
- WHEN a shape is created THE SYSTEM SHALL center its default size at the cursor and parent it to the topmost lane, expanded sub-process or pool under the cursor (fallback: the plane's process; artifacts fall back to the plane's element); flow nodes dropped outside every pool of a collaboration MUST NOT be created.
- WHEN the first pool is created on a process diagram THE SYSTEM SHALL wrap the existing content (30px padding + 30px band, at least the default 600x250) and turn the plane into a new collaboration referencing the existing process; later pools get a new process.
- WHEN a shape is appended THE SYSTEM SHALL place it 50px right of the source, vertically centered, shifted below every overlapped flow node/artifact (30px margin), and connect it in the same step; text annotations go 50px right and above, linked by an association.
- `ReplaceElement` MUST keep id, name, DI and connections the rules still allow, delete the others and drop event payloads when the event definition changes.
- Connections MUST use only horizontal and vertical segments (90-degree bends only), for every connection kind and every producer: routing, bendpoint move/insert/remove, segment move, reconnect, move/resize/attach re-routes, paste, align/distribute, create and `UpdateWaypoints`. The commands that write waypoints pass them through `layout::orthogonalize` (diagonals become L/Z elbows following the docking sides, duplicate and collinear points dropped, endpoints kept). WHEN a file is opened THE SYSTEM SHALL normalize diagonal edges with `layout::orthogonalize_all`, so saving rewrites such edges.
- WHEN one end of a connection is reconnected, or its shape moves and the bendpoints cannot be repaired, THE SYSTEM SHALL keep the docking point of the other end when it still lies on its shape's border and the preferred layout can join it (`layout::Keep`, diagram-js `preserveDocking`); otherwise it re-routes both ends.
- `Command::MoveLabel { id, dx, dy }` MUST translate an external label, materializing its default bounds when it has none.
- `Command::Reconnect { id, source, target }` MUST set the connection ends and re-route it; `factory::reconnect` MUST refuse ends the rules reject for that connection kind.
- `arrange::align` / `arrange::distribute` MUST follow the bpmn-js `elements.align` / `elements.distribute` filters (no boundary events, lanes, connections or nested selected shapes; distribution also skips text annotations) and need 2 / 3 shapes.
- `clipboard::copy` / `clipboard::paste` MUST copy shapes with descendants, attached boundary events and inner connections and paste them with fresh ids, reparented to the container at the paste point.
- WHEN a collapsed sub-process with its own plane is replaced by a non-sub-process kind THE SYSTEM SHALL remove its plane and the elements on it in the same undoable step.
- Copy of a collapsed sub-process MUST include its plane content (nested collapsed planes too); paste MUST recreate it with fresh ids on the copy's new plane (`Command::CreatePlaneContent`).
- WHEN `CommandStack::invalidate(before, after)` runs THE SYSTEM SHALL mark stale every undo/redo entry whose recorded ids (elements, DI shapes, DI edges, DI ids) differ between `before` and `after`; an entry holding a plane or whole-diagram snapshot is stale whenever anything differs.
- Undo or redo of a stale entry MUST do nothing and return `false`; `can_undo` / `can_redo` MUST report `false` for a stale top entry; a stale entry blocks the entries below it.

## Decisions

- Orthogonal-only connections deviate from bpmn-js, which allows free-angle associations and keeps imported diagonal waypoints; here imported diagonals are rewritten at load time (the file changes on save). Moving a bendpoint drags its inner neighbors along the shared axis; next to a docked end an elbow is added. Removing a corner replaces it with the opposite corner of its box; inserting a collinear bendpoint alone is a no-op.
- Associations, data input and data output associations route orthogonally (manhattan, obstacle-avoiding), a deliberate deviation from bpmn-js, which routes them straight; requested by the user for visual consistency with flows.
- Commands: `MoveShapes`, `CreateShape` (lanes are painted right after their pool's shape and lanes), `CreateConnection` (empty waypoints = auto-route), `DeleteElements`, `UpdateLabel`, `ResizeShape` (raw, no rules), `ReplaceElement`, `CreateParticipant`, `AttachBoundary { id, host: Option, center }`, `UpdateWaypoints { id, waypoints }`, `UpdateFlowNodeRefs { participant }`, `Compound`.
- Wiring API for the canvas (each returns `Option<Command>`, `None` = not allowed): `attach::attach(defs, event, host, point)`, `attach::move_boundary(defs, event, dx, dy)` (use instead of `MoveShapes` when dragging a boundary event alone), `attach::snap_to_border(point, host_bounds)` (preview), `rules::can_attach`, `rules::boundary_side`; `lanes::resize(defs, id, bounds)` (resize handles; `rules::can_resize` / `rules::min_size` to show handles), `lanes::add_lane(defs, target, LaneSide::Above|Below)`, `lanes::split_lane(defs, target, 2|3)`, `lanes::child_lanes`; `attach::bendpoint_move(defs, id, index, to)`, `attach::bendpoint_insert(defs, id, segment, at)`, `attach::bendpoint_remove(defs, id, index)`, `attach::segment_move(defs, id, segment, delta)` (pure forms in `waypoints`); `label::has_external_label`, `label::shape_label_bounds(kind, bounds)`, `label::flow_label_bounds(waypoints)` for labels without DI bounds. `factory::create_shape`/`append` set default external label bounds and `CreateConnection` sets the flow label bounds after routing.
- Tasks, events, gateways and data are not resizable, as in bpmn-js `canResize` (the plan's "resize a task" criterion follows bpmn-js instead).
- Lanes are horizontal only; delete-lane space is closed by resizing neighbors only (no space tool on contained shapes).
- Builders (`factory.rs`) read the model and return a `Command`, so the canvas stays free of modeling logic. Ids are bpmn-js prefixes plus the first free number (`Activity_1`, `Flow_2`).
- Space tool is simplified: shapes beyond the line move by the dominant drag axis; containers crossing the line are not resized.
- Replace options follow bpmn-js `ReplaceOptions` per category (start/intermediate/end/boundary events, tasks, sub-process, gateways, data).
- Undo by before-images instead of per-command inverse logic; redo re-executes the command.
- Layout ports the `BpmnLayouter` preferred-direction table onto a small orthogonal router (L, Z, or a detour with 20px stubs for explicit sides); when a preferred route crosses a flow node, obstacle avoidance picks among four fixed shapes (H-V-H, V-H-V, two L-shapes); `repairConnection` is reduced to the end segment fix.
- Refused gestures are decided by pure rules (`rules::Refusal`: flow node outside every pool, connection not allowed, boundary event on a host that cannot take it); the canvas turns them into cursor and tooltip feedback (see [canvas](canvas.md)).

## Files

- `src/modeling/mod.rs` — module root.
- `src/modeling/command.rs` — `Command` and its execution (move closure, reparent, delete cascade).
- `src/modeling/undo.rs` — `Undo` before-image recorder.
- `src/modeling/auto_resize.rs` — `with_auto_resize` (container growth).
- `src/modeling/snapping.rs` — `snap_move`, `snap_corner`, `snap_point`.
- `src/modeling/ordering.rs` — `paint_order`, `order_planes`, `gfx_parent`.
- `src/modeling/stack.rs` — `CommandStack`.
- `src/modeling/layout.rs` — `Rule`, `End`, `Side`, `Keep`, `preferred_ends`, `preferred_ends_in`, `preferred`, `preferred_keeping`, `routed`, `routed_in`, `is_direction_horizontal`, `loop_route_from`, `loop_route_in`, `loop_side`, `manhattan`, `manhattan_avoiding`, `loop_route`, `repair_end`, `straight`, `dock`.
- `src/modeling/rules.rs` — `can_connect`, `can_attach`, `boundary_side`, `can_resize`, `min_size`, `is_event_based_target`, `owner`, `participant_of`, `container_at`, `Refusal`, `create_refusal`, `drop_refusal`, `connect_refusal`.
- `src/modeling/attach.rs` — `snap_to_border`, `attach`, `move_boundary`, `bendpoint_move`, `bendpoint_insert`, `bendpoint_remove`, `segment_move`.
- `src/modeling/lanes.rs` — `LaneSide`, `add_lane`, `split_lane`, `resize`, `lanes_resize`, `child_lanes`.
- `src/modeling/waypoints.rs` — pure `move_bendpoint`, `insert_bendpoint`, `remove_bendpoint`, `move_segment`.
- `src/modeling/label.rs` — `LABEL_SIZE`, `has_external_label`, `shape_label_bounds`, `flow_label_bounds`.
- `src/modeling/factory.rs` — `new_id`, `create_shape`, `create_participant`, `connect`, `append`, `append_bounds`, `replace_options`, `kind_label`, `make_space`.
- `src/modeling/tests.rs` — unit tests (commands, layout).
- `src/modeling/tests_factory.rs` — unit tests (rules, builders, compound undo).
- `src/modeling/tests_planes.rs` — unit tests of sub-process planes (toggle, create/delete, undo plane).
- `src/modeling/tests_layout.rs` — unit tests of the preferred-layout rules.
- `src/modeling/tests_vertical.rs` — unit tests of vertical-pool layouts, compensation associations and `preserveDocking`.
- `src/modeling/tests_refusal.rs` — unit tests of the refusal rules.
- `src/modeling/tests_parity.rs` — unit tests (loops/gateway/compensation rules, attach, resize, lanes, waypoints, layout, labels).
- `src/modeling/arrange.rs` — `align`, `distribute`.
- `src/modeling/clipboard.rs` — `copy`, `paste` (pools, lanes, collapsed sub-process planes).
- `src/modeling/tests_arrange.rs` — unit tests of align, distribute, copy/paste, reconnect and label move.
