---
human_revised: false
name: canvas
summary: GPUI canvas that paints a Definitions diagram with bpmn-js styling, plus GPUI-independent viewport geometry for pan, zoom and hit-testing.
depends-on: [model, modeling]
relates: []
targets: [platform]
---

# Canvas

## Overview

`mangue::canvas` displays a `Definitions` diagram. It is split in two layers:
pure geometry (`geometry.rs`: `Viewport` transform, hit-testing, arrowheads),
GPUI-independent and unit-tested; and the GPUI side (`view.rs` holds the
`Canvas` view entity, `paint.rs` paints shapes and edges immediately with
`PathBuilder` + `Window::paint_path` and labels with
`WindowTextSystem::shape_text`). Visual reference is bpmn-js
(https://github.com/bpmn-io/bpmn-js) `BpmnRenderer`.

## Requirements (EARS / RFC 2119)

- The geometry layer MUST NOT depend on GPUI.
- The viewport MUST map `screen = diagram * scale + offset`, with screen coordinates relative to the canvas origin.
- WHEN the user scrolls (wheel or trackpad) without modifiers THE SYSTEM SHALL pan by the scroll delta.
- WHEN the user scrolls while holding cmd or ctrl THE SYSTEM SHALL zoom keeping the diagram point under the cursor fixed.
- The zoom scale MUST stay within `0.2..=4.0`.
- WHEN the user drags with the middle or right button, or with space held, THE SYSTEM SHALL pan.
- WHEN the user clicks an element THE SYSTEM SHALL select it (edges hit first, 5px tolerance), shift-click toggles it in the selection.
- WHEN the user drags on empty canvas THE SYSTEM SHALL lasso-select elements fully inside the rectangle.
- WHEN the user drags a selected shape THE SYSTEM SHALL preview the move live and commit one `MoveShapes` on release.
- Selection MUST be outlined in `#4d90fe` (bpmn-js `djs-outline`).
- WHEN Delete or Backspace is pressed THE SYSTEM SHALL delete the selection; cmd-z undoes, cmd-shift-z redoes.
- Shortcuts are written with `cmd-` and MUST resolve to `ctrl-` off macOS from one binding table (`actions::bind`, also used by the label editor).
- Off macOS THE SYSTEM SHALL render the File/Edit/Arrange/View/Window menus in an in-window menu bar (`menu_shell.rs`, kit `AppMenuBar`) built from the same `menus()` definition as `cx.set_menus`.
- WHEN an element is double-clicked (or `e` is pressed) THE SYSTEM SHALL open a multi-line inline label editor (`TextInput`: caret, selection, word/line motion, cmd-a/c/x/v, IME); shift+Enter inserts a newline, Enter or a click outside commits `UpdateLabel`, Escape cancels.
- WHEN cmd-f is pressed THE SYSTEM SHALL open a search overlay listing displayed elements whose name or id contains the query (case-insensitive); arrows move the highlight, Enter or a click selects the hit and centers the viewport on it, Escape closes.
- WHEN `a` or the context pad `+` entry is used on a single shape THE SYSTEM SHALL open a filterable append menu of every kind `factory::append` accepts; picking one appends it.
- cmd-o opens, cmd-s saves (prompting for a path when new), cmd-shift-s saves as, cmd-shift-o opens a folder, cmd-n creates a new diagram, cmd-q quits; the window title shows the file name. File, Edit and View menus expose the same actions.
- WHEN the save dialog opens THE SYSTEM SHALL start it in the bound file's directory, else `~/Documents`, else the home directory (`dialog_start_dir`).
- WHEN Open picks a file whose extension is not `.bpmn` or `.xml` (case-insensitive) THE SYSTEM SHALL keep the current document and show the "Could not open <path>" alert (`check_openable`).
- WHEN Save As is used THE SYSTEM SHALL prompt for a new path prefilled with the current name, write there, rebind the window and clear the dirty marker; Save on an unbound document uses the same flow.
- WHEN a save write fails THE SYSTEM SHALL keep the document dirty and show a critical "Could not save <path>" alert.
- WHEN File "Open Recent" is used THE SYSTEM SHALL open the chosen recent file (see [settings](settings.md)) in a tab; a missing file is alerted and dropped; "Clear Menu" empties the list.
- WHEN Open Folder picks a directory THE SYSTEM SHALL show a 220px sidebar (theme colors) listing sub-directories first then `.bpmn` files, hidden entries skipped, directories expanded lazily, with a Refresh action and the active file highlighted (kit `Tree`; an open empty or unscanned directory shows a disabled `(empty)` row; the active file is marked by text color); View "Toggle Sidebar" toggles it (no key binding); clicking a file opens it in a tab.
- The folder scan (`files::tree`) MUST NOT depend on GPUI.
- WHILE the document differs from the last loaded/saved revision THE SYSTEM SHALL title the window `name * - mangue` and set the macOS edited marker (`Window::set_window_edited`); undoing back to the saved revision clears it.
- WHEN the startup file cannot be read or imported THE SYSTEM SHALL print the error to stderr, open the window with no document (empty state) and show a critical `Window::prompt` "Could not open <path>"; WHEN a file chosen with Open fails THE SYSTEM SHALL keep the current document and show the same alert (`io::load`).
- WHEN a document is opened (startup argument, Open, or a Finder/`open` event) THE SYSTEM SHALL present it clean: no dirty marker in the title and nothing to undo; load-time normalization (`orthogonalize_all`) MUST NOT go through the command stack (test: `freshly_loaded_document_is_clean`).
- WHILE a create entry is armed or dragged from the palette and the cursor is over the canvas THE SYSTEM SHALL paint a 50% opacity ghost of the shape to be created, centered at the cursor.
- WHILE the space tool is armed THE SYSTEM SHALL draw dashed crosshair guides at the cursor; while spacing, a guide across the start point perpendicular to the drag plus the drag line.
- Replace menu rows MUST show the BPMN glyph of the kind (palette glyph painter) before its label.
- Shapes and edges MUST be painted and hit-tested in bpmn-js z-order (`ordering::paint_order`: participants < lanes < flow nodes < sub-processes with their content < boundary events < flows), external labels last; only the active plane is painted and hit-tested.
- WHILE a drag (move, label, resize, bendpoint, lasso, space, palette) is within 20px of the canvas edge THE SYSTEM SHALL scroll 10px toward that edge every 15ms (`auto_scroll`), the drag following the pointer.
- WHILE moving THE SYSTEM SHALL snap the primary shape's center/top-left/bottom-right to its siblings' within 7 screen px, and a resized corner to sibling edges, drawing dashed snap lines (theme `selection`); unsnapped axes, bendpoints and segments snap to the settings grid; cmd disables snapping.
- WHILE the pointer hovers a replace-menu option THE SYSTEM SHALL paint the selection replaced by it, without changing the document.
- WHEN a collapsed sub-process owns a plane THE SYSTEM SHALL show a drill-down button at its bottom-right (8px/7px overhang, selection color; empty planes only while selected); clicking it opens that plane (bpmn-js `DrilldownOverlayBehavior`).
- WHILE a sub-process plane is open THE SYSTEM SHALL show a breadcrumbs bar right of the palette (process/sub-process ancestors with a plane, name or id); clicking a crumb opens its plane. Each plane keeps its own viewport; switching planes clears selection.
- WHEN a sub-process is selected THE replace menu MUST offer "Sub-Process (collapsed/expanded)" first, issuing `ToggleCollapse`.
- Rendering MUST follow bpmn-js defaults: white fill, `#22242a` stroke, 2px activity/gateway outlines, 4px end events, double 1.5px intermediate events, rounded (r=10) activities, filled-arrow sequence flows, dashed message flows with open circle start and open arrow end, dotted associations.
- Hit-testing MUST return the topmost (last painted) shape containing the point.
- A palette (left, two columns) MUST offer hand, lasso, space and global connect tools, a separator, then create entries for start, intermediate throw and end events, exclusive gateway, task, expanded sub-process, data object, data store, pool and group, drawn as miniature bpmn-js glyphs.
- WHEN a create entry is picked and the canvas clicked, or the entry is dragged and released over the canvas, THE SYSTEM SHALL create the element at the cursor. Tools are one-shot; picking the armed entry or Escape disarms it.
- WHILE exactly one shape is selected and no gesture or tool is active THE SYSTEM SHALL show a context pad 10px right of it: append end event, gateway, task, intermediate throw event, text annotation and append-anything (flow nodes other than end events; annotation only for end events, pools and data), then change type (when replace options exist), connect (not lanes/groups) and delete.
- WHEN connect is armed (pad or tool) THE SYSTEM SHALL draw a preview line to the cursor and connect to the next clicked shape with the connection the rules allow; clicking empty canvas cancels.
- WHEN change type is picked THE SYSTEM SHALL open a replace menu below the pad listing sibling kinds; picking one issues `ReplaceElement`.
- WHILE idle THE SYSTEM SHALL outline the hovered element in the theme `hover` color (see [theme](theme.md)); the cursor MUST reflect the tool (open/closed hand for hand and pan, crosshair for lasso/space/connect/create, resize while spacing, pointer over elements).
- WHILE exactly one resizable shape is selected (`rules::can_resize`) THE SYSTEM SHALL paint 8px corner handles (theme `selection` stroke on `canvas.background`); dragging one previews and commits `lanes::resize` (bpmn-js min sizes, lane neighbors follow).
- WHEN a single attachable event is dragged within 15px of an activity border THE SYSTEM SHALL preview it snapped onto the border and attach it on drop (`attach::attach`); a dragged boundary event slides along its host or detaches when dropped off it (`attach::move_boundary`). Other moves append `UpdateFlowNodeRefs` for every participant.
- Participants and lanes MUST show lane tools first in the context pad: add lane above (`arrow-up-to-line`), below (`arrow-down-to-line`), split in two (`rows-2`) or three (`rows-3`).
- WHILE exactly one connection is selected THE SYSTEM SHALL let the user drag an inner bendpoint (`bendpoint_move`), a segment middle (perpendicular `segment_move`) or any other segment point (inserts a bendpoint there, then moves it); double-clicking a bendpoint removes it. Grab tolerance is 6 screen px.
- WHILE the settings grid is visible THE SYSTEM SHALL paint a dot (1 screen px radius, theme `grid`) at every grid intersection under the shapes, only within the viewport; WHEN dots would be closer than 20 screen px THE SYSTEM SHALL paint only every 2nd/5th/10th... dot (`grid::dot_step`). The dots MUST come from one cached raster tile (`grid::raster`, keyed by on-screen spacing, color and scale factor) tiled across the viewport (`grid::tile_range`), a few images per frame instead of a quad per dot. Hit testing MUST reuse a paint order cached until the document changes, and drag previews MUST be rebuilt at most once per frame. Timing: the debug-build lib test run went from about 60 s to 0.25 s with the tile; no release build was measured (disk). Create, move and paste snap to the settings grid size unless snapping is off (see [settings](settings.md)).
- WHEN a palette drop, a move or a connect would be refused by the rules (`rules::Refusal`) THE SYSTEM SHALL show the not-allowed cursor and a theme `overlay.error` marker at the pointer while dragging or hovering; on release it MUST leave the document unchanged and show an error tooltip 5px from the cursor that fades over its last 500 ms and disappears after 2 s (bpmn-js `ModelingFeedback`); a new refusal replaces the previous tooltip.
- Canvas overlays (`Canvas::add_overlay`, `remove_overlay`, `overlay`, `overlays_on`) MUST be anchored to an element corner or a diagram point, follow the viewport, drop with their element and take theme `overlay.badge` / `overlay.error` colors (see [theme](theme.md)). `Canvas::insert_overlay` adds structured content (`Overlay`: optional `assets/icons` icon name, text, optional theme color key, see `Theme::color_for`); an overlay with `on_click` MUST run it on a left press without the press reaching the canvas, and one with a `show` zoom range MUST be hidden outside it (inclusive bounds, diagram-js `minZoom` / `maxZoom`). The overlay API stays internal.
- Participants and lanes whose DI has `isHorizontal="false"` MUST paint their name in a horizontal band along the top (30px, with the rule under it for a pool with a process) instead of the rotated left band.
- WHEN the window close button is pressed or Quit is used with dirty tabs THE SYSTEM SHALL refuse the close (`Window::on_window_should_close`) and prompt Save / Don't Save / Cancel (`Window::prompt`) for each dirty tab in turn, activating it, closing only after Save or Don't Save on all and aborting on Cancel.
- One window MUST host an ordered list of documents as tabs (`Canvas::docs`, `active`); each tab MUST keep its own definitions, path, undo/redo history, saved revision, selection, viewport and drill-down plane, and switching tabs MUST NOT mutate any of them (`Document`).
- WHEN a file is opened (cmd-o, Open Recent, sidebar pick, startup argument, Finder / `open` event) THE SYSTEM SHALL open it in a new tab of the frontmost window and activate it, without the discard prompt; a window is created only when none exists.
- WHEN the opened path (canonicalized) is already open in a tab THE SYSTEM SHALL activate that tab instead of opening a duplicate.
- WHEN cmd-n is used THE SYSTEM SHALL open a new "Untitled" tab.
- A 28px tab strip above the canvas MUST show each tab's file name (or "Untitled"), a `*` dirty marker and an `x` close button in the tab suffix (kit `TabBar`); the active tab MUST be visually distinct.
- WHEN a tab is clicked THE SYSTEM SHALL make it active, committing or cancelling transient UI state (label edit, gesture, tool, hover, search, append menu), and update the window title and the sidebar highlight.
- WHEN cmd-w, File > Close Tab or a tab's close button closes a dirty tab THE SYSTEM SHALL show the unsaved-changes prompt (Save / Don't Save / Cancel) for that tab only.
- WHEN the last tab closes THE SYSTEM SHALL keep the window and show the empty state.
- WHEN the app starts without a file argument THE SYSTEM SHALL open the window with no document and show the empty state.
- WHILE the window holds no document THE SYSTEM SHALL show a centered empty state with an "Open File" button and a Cmd+O hint, no tab strip and no canvas; the window title MUST be `mangue`.
- WHEN the "Open File" button is pressed THE SYSTEM SHALL run the same Open flow as cmd-o (`EmptyState` in `canvas/empty_state.rs`; test: `empty_state_button_dispatches_open`).
- WHILE the window holds no document, document actions (save, undo/redo, toggle mode, zoom, edit) MUST be no-ops and MUST NOT panic; cmd-o, cmd-n and Open Recent MUST still work and open the document in a new active tab.
- A 32px full-width document header (`DocHeader`) MUST sit below the tab strip and above the canvas, showing a breadcrumb placeholder on the left (`Breadcrumbs` with one crumb, the document name) and the mode button on the right (see [code-editor](code-editor.md)).
- Code mode (per-tab mode, XML editor, sync on leave, invalid-XML banner, separate histories) is specified in [code-editor](code-editor.md).
- WHEN cmd-shift-] / ctrl-tab or cmd-shift-[ / ctrl-shift-tab is used THE SYSTEM SHALL activate the next / previous tab, wrapping; cmd-1..cmd-8 SHALL activate tab N (out of range is a no-op) and cmd-9 the last tab; the Window menu MUST offer Next Tab / Previous Tab.
- WHEN cmd-a is pressed outside text editing THE SYSTEM SHALL select every shape and connection.
- WHEN an arrow key is pressed THE SYSTEM SHALL move the selection 1px (10px with shift); cmd + arrow SHALL scroll the canvas 50px (200px with shift).
- WHEN h, l, s or c is pressed outside label editing THE SYSTEM SHALL arm the hand, lasso, space or global connect tool; e SHALL edit the selected label; r SHALL open the replace menu; cmd-y MUST redo like cmd-shift-z.
- WHEN cmd-c then cmd-v are pressed THE SYSTEM SHALL paste a copy of the selection with fresh ids centered at the grid-snapped cursor and select it.
- WHEN cmd-= / cmd-+ or cmd-- is pressed THE SYSTEM SHALL zoom one log step (10 steps over 0.2..4) around the canvas center; cmd-0 SHALL reset to 1; cmd-shift-0 (View > Zoom to Fit) and opening a file SHALL fit the diagram centered, never above scale 1.
- The Arrange menu MUST offer align left/center/right/top/middle/bottom and distribute horizontally/vertically on the selection.
- WHEN the first or last waypoint of the selected connection is dragged onto a shape THE SYSTEM SHALL reconnect that end if the rules allow the same connection kind, re-routing it.
- WHEN the external label of a named element is dragged THE SYSTEM SHALL move only the label (`MoveLabel`).
- WHILE the palette create tool is armed THE SYSTEM SHALL snap the ghost (and the drop) center/edges to the would-be siblings' within 7 screen px, drawing snap lines, else to the grid.
- WHEN the space tool is released THE SYSTEM SHALL make space by the grid-snapped delta (each axis quantized to the grid size, off with snapping disabled).
- Every window MUST render inside `gpui_component::Root`.
- WHILE the replace menu is open THE SYSTEM SHALL move its highlight with plain Up/Down (instead of nudging the selection), painting the highlighted option's preview, and Enter SHALL apply it; the collapse/expand row stays mouse-only.
- WHILE search lists results THE SYSTEM SHALL show the first case-insensitive match of the query in bold in each name and id.

## Decisions

- Immediate-mode painting each frame from `Rc<Definitions>`; no retained scene graph until editing (T5) proves the need.
- Stroke widths, dash lengths and font size scale with zoom, like an SVG transform.
- GPUI 0.2.2 has no pinch/magnify event, so zoom is cmd/ctrl + scroll only.
- GPUI cannot rotate text: pool/lane names are SVGs generated on demand by `Assets` (`vlabel/{w}x{h}x{font}/{text}`, `<text transform="rotate(-90 ...)">`), rasterized by GPUI's resvg (system fonts) and painted with `Window::paint_svg` as tinted sprites, reading bottom-to-top like bpmn-js.
- Dirty state is a revision id stack mirrored next to the `CommandStack` in the view (modeling exposes no stack position); saved = revision id at load/save.
- Gestures are verified by headless interaction tests (`src/canvas/view_tests.rs`, gpui `test-support` dev-dependency, `#[gpui::test]` + `VisualTestContext::simulate_*` on the test platform), never by synthetic input to the desktop. The test platform does not report the window title back, so tests read the view's applied title.
- Task-type icons are placeholder letters (send/receive use envelopes); error/conditional/link/compensation event glyphs are letters.
- Until XML import is wired, `main.rs::load_definitions` returns `canvas::demo::demo_definitions()`.
- Palette and context pad are single painted canvases hit-tested by pure layout functions (unit-tested), instead of one GPUI element per entry; replace menu rows are kit `ListItem`s with a painted glyph canvas, the highlight index kept on `Canvas` (the kit `ListState` would take focus from the canvas).
- App chrome uses gpui-component 0.8.0: empty state (`Empty`), XML error (`Alert`), breadcrumbs (`Breadcrumb`), sidebar (`Tree`), tabs (`TabBar`), search and append menu (`Input` + `ListItem`; `searchable_list` has no renderable popup in 0.8.0), doc header mode `Button`. Clicks fire on mouse up; kit Up/Down/Escape in pickers are captured on the canvas surface with `capture_action`; headless tests rendering kit elements call `init_editor` first. Palette, context pad, overlays, the drill-down button (bpmn-js geometry) and the label editor stay custom.
- Generic tool icons (hand, lasso, space `move-horizontal`, global connect `spline`, pad connect `arrow-up-right`, change type `wrench`, delete `trash-2`, text annotation `message-square-text`) are Lucide SVGs (https://lucide.dev, ISC) vendored from `lucide-static@0.460.0` into `assets/icons/`, embedded by `src/assets.rs` (`AssetSource`, registered with `Application::with_assets`) and rendered with `svg()` tinted with the theme `palette.icon`.
- GPUI 0.2.2 `PathPromptOptions` has no extension filter, so file types are checked after the pick.
- No file watching (manual Refresh).
- BPMN element glyphs stay BPMN miniatures: the shape painter on a 40-unit box scaled to the cell (Lucide has no BPMN notation).

## Files

- `src/canvas/mod.rs` — module root; re-exports `Canvas`, `Viewport`.
- `src/canvas/geometry.rs` — `Viewport`, `contains`, `hit_shape`, `hit_edge`, `rect_from`, `encloses`, `arrowhead`, unit tests.
- `src/canvas/paint.rs` — bpmn-js styled painter for shapes, edges, labels, overlays (hover, selection, lasso, connect preview) and palette/pad glyphs.
- `src/canvas/view.rs` — `Canvas` GPUI view: selection, lasso, move, delete, label edit, undo/redo, pan/zoom, open/save, dirty state + discard prompts (menu actions and close button), create ghost, space guides, resize/attach/waypoint drags.
- `src/canvas/view_tests.rs` — headless GPUI interaction tests of `Canvas` (palette click/drag create, ghost, context pad append/connect/replace/delete, move, lasso, label edit, undo/redo, new/quit confirms, save, resize handles, attach/slide/detach, lane tools, lane membership, bendpoint/segment drags, external labels, close button).
- `src/canvas/actions.rs` — GPUI actions, platform-resolved key bindings (`keystroke`, `bind`) and the menu definition (`menus`, `set_menus`).
- `src/canvas/menu_shell.rs` — in-window menu bar wrapper used off macOS.
- `src/canvas/text_edit.rs` — GPUI-independent text/selection/IME state.
- `src/canvas/text_input.rs` — reusable `TextInput` view (`EntityInputHandler`, `TextInput` key context bindings).
- `src/canvas/text_element.rs` — element painting a `TextInput` (caret/selection use theme `selection`).
- `src/canvas/search.rs` — `SearchBox` overlay and `find`.
- `src/canvas/append_menu.rs` — `AppendMenu` popup and `options`.
- `src/canvas/overlay.rs` — `Overlays` registry, `Overlay`, `Anchor`, `Corner`, `Tone`, `Click`, `ZoomRange`.
- `src/canvas/overlay_box.rs` — `OverlayBox` badge / tooltip element.
- `src/canvas/view_feedback.rs` — overlay API, refusal detection, not-allowed cursor and error tooltip wiring of `Canvas`.
- `src/canvas/view_tests_feedback.rs` — headless overlay and feedback tests.
- `src/canvas/view_overlays.rs` — label editor, search and append menu wiring of `Canvas`.
- `src/canvas/view_tests_text.rs` — headless tests of the text overlays.
- `src/canvas/palette.rs` — `Tool`, palette entries, layout, hit-test and `Palette` element.
- `src/canvas/context_pad.rs` — `PadEntry`, pad entries per kind, layout, hit-test and `ContextPad` element.
- `src/canvas/replace_menu.rs` — `ReplaceMenu` popup element (with optional collapse/expand row).
- `src/canvas/drilldown.rs` — drill-down targets, button rect and breadcrumb path (`Crumb`).
- `src/canvas/drill_button.rs` — `DrillButton` overlay element.
- `src/canvas/breadcrumbs.rs` — `Breadcrumbs` bar element.
- `src/canvas/view_planes.rs` — plane switching, per-plane viewports and toggle wiring of `Canvas`.
- `src/canvas/view_tests_planes.rs` — headless drill-down, breadcrumb and toggle-undo tests.
- `src/canvas/grid.rs` — dot grid painter (cached raster tile), `dot_step`, `raster`, `tile_range`.
- `src/canvas/view_tests_settings.rs` — headless Settings window -> canvas grid/theme tests.
- `src/canvas/icon.rs` — `icon_at`, Lucide SVG cell helper.
- `src/canvas/handles.rs` — resize corner and connection grab geometry (`corner_at`, `resized`, `edge_grab`).
- `src/assets.rs` — embedded `Assets` source, `icon_path` (icon name to asset path) plus generated vertical label SVGs (`vertical_label_path`); `assets/icons/` — vendored Lucide SVGs + LICENSE.
- `src/canvas/demo.rs` — hardcoded demo collaboration.
- Colors, main stroke widths and font come from the `Theme` global; see [theme](theme.md).
- `src/canvas/view_commands.rs` — editor actions and keyboard shortcuts.
- `src/canvas/view_tests_parity.rs` — headless tests of the parity gestures.
- `src/canvas/sidebar.rs` — `Sidebar` folder tree element (`WIDTH`, rows, disclosure, refresh).
- `src/canvas/document.rs` — `Document`: per-tab definitions, stack, path, revisions, selection, viewport, planes.
- `src/canvas/doc_header.rs` — `DocHeader` element (`HEIGHT`, breadcrumb placeholder, mode button `label`).
- `src/canvas/tab_bar.rs` — `TabBar` strip element (`HEIGHT`, dirty marker, close button).
- `src/canvas/view_tabs.rs` — tab open/dedup (`tab_of`), activate, close, navigation handlers and transient reset, mode toggle, code sync on leave, revert / back to code.
- `src/canvas/code_view.rs`, `src/canvas/xml_error.rs` — code mode; see [code-editor](code-editor.md).
- `src/canvas/view_tests_tabs.rs` — headless tab tests.
- `src/canvas/empty_state.rs` — `EmptyState` element (title, "Open File" button dispatching `Open`, Cmd+O hint) shown when the canvas holds no document.
- `src/canvas/view_tests_empty.rs` — headless tests of the document-less canvas (startup, last-tab close, no-op actions, New / open from empty).
- `src/files/tree.rs` — GPUI-independent folder scan (`Node`, `scan`, `expand`), unit tests.
