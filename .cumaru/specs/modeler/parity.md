---
human_revised: false
name: parity
summary: Feature-by-feature parity audit of the mangue modeler against the bpmn-js 17.11.1 Modeler module list and diagram-js 14.11.3 features, with status and evidence.
depends-on: [canvas, modeling, io]
relates: [model, theme]
targets: [platform]
---

# Parity with bpmn-js

## Overview

Canonical checklist: the modules imported by `lib/Modeler.js`, `lib/Viewer.js`
and `lib/NavigatedViewer.js` of bpmn-js@17.11.1
(https://github.com/bpmn-io/bpmn-js/blob/v17.11.1/lib/Modeler.js) plus the
diagram-js@14.11.3 features they pull in
(https://github.com/bpmn-io/diagram-js/tree/v14.11.3/lib/features).
Status: **done** (behaves like bpmn-js), **partial** (core behavior, gaps
listed), **missing** (not implemented; rationale given).

Totals: 49 done, 0 partial, 0 missing (49 features).

## Requirements (EARS / RFC 2119)

- This audit MUST be updated whenever a feature listed here changes status.
- Every done or partial row MUST cite code or a test as evidence.

## Feature table

| Feature | bpmn-js / diagram-js module | Status | Evidence / gap |
|---|---|---|---|
| Command stack, undo/redo | diagram-js `command`, bpmn-js `modeling` | done | `src/modeling/stack.rs`, `src/modeling/undo.rs`; `view_tests::move_undo_redo` |
| Modeling commands | `modeling` | done | `src/modeling/command.rs` (`Command` enum); `modeling::tests*` |
| Palette | `palette` | done | `src/canvas/palette.rs`; `view_tests::palette_click_creates`, `palette_drag_creates` |
| Create + preview | diagram-js `create` | done | `paint_ghost` in `src/canvas/paint.rs`; `view_tests::ghost_follows_cursor` |
| Context pad | `context-pad` | done | `src/canvas/context_pad.rs`; `view_tests::pad_append_task`, `pad_connect`, `pad_delete` |
| Replace + popup menu | `replace`, `popup-menu` | done | `src/canvas/replace_menu.rs`, `factory::replace_options`; `view_tests::pad_replace` |
| Replace preview | `replace-preview` | done | `ReplaceMenu::with_preview` row hover; `Canvas::replace_preview` paints the selection replaced on a document clone (`view_aids.rs`); `view_tests::aids::replace_menu_hover_previews`, `replace_menu_row_hover`. |
| Append-anything menu | `append-menu` (`AppendMenuProvider`) | done | `src/canvas/append_menu.rs`, `view_overlays.rs`: key `a` or pad `+` entry opens a filterable popup of every kind `factory::append` accepts; `view_tests::text::append_menu_key`, `append_menu_pad_entry`. No palette create-anything entry, no hover preview. |
| Label editing | `label-editing` | done | `src/canvas/text_input.rs` (multi-line `TextInput`: caret, selection, word/line motion, clipboard, IME), `view_overlays.rs`; `view_tests::label_edit`, `text::label_edit_*`. Text is not wrapped to the shape width. |
| Label move | diagram-js `move` on labels | done | `Command::MoveLabel`, `Canvas::label_at`; `parity::drag_label_moves_label_only`, `tests_arrange::move_label_alone` |
| Keyboard bindings | diagram-js `KeyboardBindings`, bpmn-js `BpmnKeyboardBindings` | done | Audited against both files: every 17.11.1 binding exists (`undo`, `redo`, `copy`, `paste`, `stepZoom` +/=/-, `zoom` 0, `removeSelection` Backspace/Delete, `selectElements`, `find`, `spaceTool` s, `lassoTool` l, `handTool` h, `globalConnectTool` c, `directEditing` e, `replaceElement` r); there is no moveToOrigin or create-anything key in core. `src/canvas/actions.rs`, `view_commands.rs::on_shortcut`; tool, edit and replace letters ignore modifiers like `hasModifier`. mangue adds cmd-y, `a` (append menu) and arrows. Tests: `parity::undo_redo_keys`, `delete_key_removes_selection`, `zoom_keys`, `zoom_plus_minus_keys`, `letter_keys_*`, `modified_letters_do_nothing`, `edit_and_replace_need_a_selection`, `append_key_needs_a_selection`, `select_all_then_delete`, `copy_paste_at_cursor`, `text::search_*`. |
| Keyboard move selection | diagram-js `keyboard-move-selection` | done | arrows 1px, shift 10px; `parity::arrows_move_selection` |
| Keyboard canvas move | diagram-js `navigation/keyboard-move` | done | cmd + arrows 50px, shift 200px; `parity::arrows_move_selection` |
| Editor actions | `editor-actions` | done | `src/canvas/actions.rs` actions + File/Edit/Arrange/View menus |
| Copy / paste | `copy-paste` | done | `src/modeling/clipboard.rs` (fresh ids, descendants, boundaries, inner connections, reparent at drop); `parity::copy_paste_at_cursor`, `tests_arrange::copy_paste_regenerates_ids`. Pools copy with their process, lanes and content and paste only into a collaboration (fresh process); a copied lane pastes as a new lane below the pool under the cursor (content centered in it), or only its content when no pool is there; `aids::copy_paste_pool`, `copy_paste_lane`. A collapsed sub-process copies its plane content (nested planes too) and pastes it onto the copy's new plane (`Command::CreatePlaneContent`); `tests_planes::copy_paste_keeps_plane_content`. In-app clipboard (see Decisions). |
| Align elements | `align-elements` | done | `arrange::align` (bpmn-js rule filters), Arrange menu; `tests_arrange::align_edges_and_middle`, `parity::align_top_action`. bpmn-js 17 also shows an align popup in the multi-selection pad (menu used instead). |
| Distribute elements | `distribute-elements` | done | `arrange::distribute`; `tests_arrange::distribute_equal_gaps` |
| Auto-place | `auto-place` | done | `factory::append_bounds`; `tests_factory` |
| Auto-resize | `auto-resize` | done | `modeling::auto_resize::with_auto_resize` (diagram-js padding 2/15, offset 60/100; participants via `lanes::resize`, propagates upward) on move, create, append and paste, same undo step; `aids::move_beyond_pool_auto_resizes`, `auto_resize::tests`. |
| Grid snapping | `grid-snapping` | done | Settings grid size/snap; move (center), create, paste, resize corner, bendpoint and segment drags snap (off with cmd); `parity::create_snaps_to_grid`, `view_tests::resize_handle_drag`, `segment_mid_drag_moves_segment`; the space tool delta snaps per axis (`Canvas::snapped_space`, `aids::space_tool_snaps_to_grid`). |
| Alignment snapping | `snapping` (`CreateMoveSnapping`, `ResizeSnapping`) | done | `modeling::snapping`: moves snap center/top-left/bottom-right to siblings' (7 screen px), resize corners to sibling edges, per axis before the grid; dashed snap lines in theme `selection`; `aids::move_snaps_to_sibling`, `snapping::tests`; the palette ghost and its drop snap the same way (`Canvas::snapped_create`, `aids::create_snaps_to_sibling`). No connect/docking snapping (`BpmnConnectSnapping`), see Decisions. |
| Move | diagram-js `move` | done | `Gesture::Move` in `src/canvas/view.rs`; `view_tests::move_undo_redo` |
| Resize | diagram-js `resize` | done | `src/canvas/handles.rs`, `lanes::resize`; `view_tests::resize_handle_drag`, `resize_respects_min_size` |
| Lasso tool | diagram-js `lasso-tool` | done | `view_tests::lasso_selects` |
| Hand tool | diagram-js `hand-tool` | done | `Tool::Hand`; `parity::letter_keys_arm_tools` |
| Space tool | diagram-js `space-tool` | done | `factory::make_space`; space guides in `view.rs` render |
| Global connect | diagram-js `global-connect` | done | `Tool::Connect`; `parity::letter_keys_arm_tools` |
| Connect + preview | diagram-js `connect`, `connection-preview` | done | `Canvas::connect_at`; `view_tests::pad_connect` |
| Bendpoints | diagram-js `bendpoints` | done | `src/modeling/waypoints.rs`; `view_tests::bendpoint_move_and_remove`, `segment_drag_inserts_bendpoint`, `segment_mid_drag_moves_segment` |
| Connection reconnect | diagram-js `bendpoints` (`BendpointMove` on ends) | done | `EdgeGrab::Endpoint`, `factory::reconnect`, `Command::Reconnect`; `parity::drag_end_reconnects`, `tests_arrange::reconnect_target`. The other end keeps its docking point (`preserveDocking`, `layout::Keep`); `tests_vertical::reconnect_keeps_the_other_docking`, `move_keeps_the_other_docking`. |
| Connection layout | bpmn-js `BpmnLayouter` | done | `src/modeling/layout.rs` ports the horizontal and vertical-pool preferred-layout tables (`Rule`, `preferred_ends_in`, `preferred_keeping`, `routed_in`, `is_direction_horizontal`): sequence flows `h:h`, out of a gateway `v:h`, into a gateway `h:v`, expanded sub-process ends `h:h`, message flows `v:v`, boundary event flows `s:t` from its attach orientation and the target orientation (`getBoundaryEventSourceLayout` / `TargetLayout`), boundary loops to the host, self-loops `t:r` / `r:b` / `b:l` / `l:t` by where the loop leaves; obstacle avoidance and the orthogonal-only invariant stay (associations are `h:h`, imports normalized at load, deviations in Decisions). Vertical pools (`isHorizontal="false"` on the pool, lane or the pool of the element's process) swap `h`/`v` (`v:v` flows, gateway `h:v` / `v:h`, message flows `h:h`, boundary corner variants, counter-clockwise loops `t:l` `r:t` `l:b` `b:r`); a compensation association (compensate boundary event to an `isForCompensation` activity) follows the boundary rule. Tests: `tests_vertical`, `tests_layout` (one per rule), `tests_orthogonal`, `tests_parity::manhattan_avoids_obstacles`. |
| Attach / boundary events | `modeling` `AttachEventBehavior` | done | `src/modeling/attach.rs`; `view_tests::boundary_attach_slide_detach` |
| Lanes | `modeling` lane behaviors | done | `src/modeling/lanes.rs`; `view_tests::pad_lane_tools` |
| Search | `search` (`BpmnSearchProvider`) | done | `src/canvas/search.rs`, `view_overlays.rs`: cmd-f overlay, case-insensitive contains over names/ids (max 12 hits), match in bold (`search::match_range`), arrows + Enter select and center; `view_tests::text::search_*`, `search::tests::match_range_ignores_case`. |
| Auto-scroll | diagram-js `auto-scroll` | done | `canvas::auto_scroll` (20px threshold, 10px step, 15ms repeat) for move, label, resize, bendpoint, lasso, space and palette drags (`view_aids.rs` timer); `aids::drag_near_edge_auto_scrolls`. Click-click connect does not scroll. |
| Selection | diagram-js `selection` | done | `Canvas::selection`; shift toggle in `on_down` |
| Outline | bpmn-js `outline` | done | `paint_overlay` selection outline |
| Hover | diagram-js `interaction-events` hover | done | `Canvas::hover`, theme `hover` |
| Interaction events | diagram-js `interaction-events` | done | `geometry::hit_shape`, `hit_edge`, `Canvas::label_at` |
| Overlays | diagram-js `overlays`, `tooltips` | done | `canvas::overlay::Overlays` (add / remove / get / on-element / prune, optional ttl with a 500 ms fade) behind `Canvas::add_overlay`, `remove_overlay`, `overlay`, `overlays_on`; anchors are an element corner or a diagram point, so the viewport carries them; drawn by `OverlayBox` in theme `overlay.badge` / `overlay.error`; overlays of deleted elements are pruned. Structured content (`Overlay`: optional `assets/icons` icon name, text, optional theme color key via `Theme::color_for`), `on_click` (`Click`, stops the press from reaching the canvas) and `show` zoom range (`ZoomRange`, diagram-js `minZoom` / `maxZoom`) through `Canvas::insert_overlay`. Still internal API, no HTML content. Tests: `overlay::tests`, `feedback::structured_overlay_clicks_and_follows_zoom_range`, `schema::tests::color_keys_resolve`, `feedback::overlay_api_follows_element_and_viewport`. |
| Zoom scroll | diagram-js `zoomscroll` | done | `Canvas::on_scroll`, `Viewport::step_zoom`; `geometry::tests::step_zoom_log_scale` |
| Move canvas | diagram-js `movecanvas` | done | `Gesture::Pan` (middle/right/space drag) |
| Fit viewport on open | `canvas.zoom('fit-viewport', 'auto')` | done | `Viewport::fit`, `Canvas::apply_fit`, View > Zoom to Fit (cmd-shift-0); `parity::open_fits_viewport`, `geometry::tests::fit_viewport` |
| Ordering | bpmn-js `ordering` | done | `modeling::ordering::paint_order`: graphical tree (participant / expanded sub-process / root, lanes skipped, boundary beside host), siblings by `BpmnOrderingProvider` level; painting and hit testing follow it, external labels painted last; `ordering::tests`. |
| DI ordering | bpmn-js `di-ordering` | done | Save re-sorts every plane with `ordering::order_planes` (shapes, then edges, each in paint order); `moddle_validate` checks the ordered export. |
| Sub-process drill-down | bpmn-js `drilldown` (collapsed sub-process planes) | done | Multiple `Diagram`s in `Definitions` (import/export all), drill-down button, breadcrumbs, per-plane viewport; collapse/expand via replace menu moves content between planes (`SubProcessPlaneBehavior`), undoable. Import gives legacy collapsed sub-processes drawn on the parent plane their own plane (`SubprocessCompatibility`, `compat_planes`); replacing one with a non-sub-process drops the plane and its content, undoable; copy/paste keeps plane content; `tests_planes::import_moves_legacy_content_to_plane`, `replace_with_task_drops_plane`, `copy_paste_keeps_plane_content`, `moddle_validate::pasted_and_legacy_planes_are_valid_for_bpmn_moddle`. |
| Text annotation | `modeling` | done | context-pad annotation entry; `TextAnnotation` paint and edit |
| Import / export | bpmn-js `importer`, `saveXML` | done | `src/io/`; `tests/io_roundtrip.rs`, `tests/moddle_validate.rs` |
| Modeling feedback | `modeling-feedback` | done | `view_feedback.rs`, `rules::{create_refusal, drop_refusal, connect_refusal}`: a refused palette drop, move or connect shows the not-allowed cursor plus a theme-colored marker while dragging or hovering, and on release an error tooltip 5px from the cursor ("flow elements must be children of pools/participants" as bpmn-js; mangue words the connect and attach refusals) that fades after 2 s. A flow node moved out of every pool is now refused instead of keeping its old parent. Tests: `feedback::*`, `tests_refusal`. |

## Decisions

- Layout ports the horizontal and the vertical-pool preferred-layout tables, the compensation-association boundary layout and `preserveDocking` on reconnect and move. Remaining cuts: `straight` options reduce to the default route because every connection is orthogonal; vertical pools and lanes are imported, painted (label band on top) and laid out, but the lane tools (add, split, resize) and the palette still create and edit horizontal ones only; `isForCompensation` is set when a compensation association is created but never cleared again (bpmn-js also clears it on delete and reconnect, and keeps a single compensation activity per boundary event); `preserveDocking` is dropped when the preferred layout cannot join the kept point or crosses an obstacle (the fallback re-docks), and applies to reconnect and to the end that did not move. Associations, data input and data output associations are routed with the manhattan layout like flows, a deliberate deviation from bpmn-js (straight); every connection is orthogonal-only and imported diagonal waypoints are rewritten at load. See [modeling](modeling.md).
- Copy/paste uses an in-app clipboard (bpmn-js also keeps its own clipboard, not the system one).
- No connect/docking snapping: bpmn-js `BpmnConnectSnapping` snaps the dragged docking point to the target's mid/center axes, but every connection here is re-routed orthogonally by the layout, which picks the docking itself; snapping the pointer would not change the result.
- Not built (cost over value, recorded): a palette create-anything entry (bpmn-js 17.11.1 core has no `CreateMenuProvider`; it lives in the `bpmn-js-create-append-anything` plugin, and the `a` append menu plus the palette cover creation); soft wrapping inside the label editor (painted labels already wrap to the shape; the editor keeps explicit lines because soft-wrapped caret motion needs a wrapped-line layout in `TextInput`); auto-scroll during click-click connect.
- Align/distribute are exposed in an Arrange menu instead of the bpmn-js multi-selection popup.
- Fit viewport runs on open of a file and on View > Zoom to Fit (cmd-shift-0 is a mangue choice; bpmn-js has no fit key).

## Files

- `src/canvas/view_commands.rs` — editor actions and keyboard shortcuts.
- `src/canvas/view_tests_parity.rs` — headless tests of the parity gestures.
- `src/modeling/arrange.rs` — align, distribute, grid snap.
- `src/modeling/clipboard.rs` — copy/paste.
- `src/modeling/auto_resize.rs` — container growth on move/create.
- `src/modeling/snapping.rs` — shape and grid snapping.
- `src/modeling/ordering.rs` — z-order and DI order.
- `src/canvas/auto_scroll.rs`, `src/canvas/view_aids.rs` — auto-scroll, snap lines, replace preview.
- `src/modeling/tests_arrange.rs` — unit tests of the parity commands.
