//! `Canvas`: the GPUI view that displays and edits a diagram (select, lasso,
//! move, delete, label edit, undo/redo, pan/zoom, palette tools, context pad,
//! replace menu, hover, create preview) and opens/saves files, tracking
//! unsaved changes.

use std::cell::Cell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::{
    Context, CursorStyle, Div, FocusHandle, KeyDownEvent, KeyUpEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PathPromptOptions, Pixels, Point, PromptLevel, ScrollWheelEvent,
    Subscription, Window, WindowAppearance, canvas, div, point, prelude::*, px,
};

use super::actions::{
    CloseTab, Delete, New, Open, OpenFolder, OpenRecent, Quit, Redo, Save, SaveAs, ToggleMode,
    ToggleSidebar, Undo,
};
use super::append_menu::AppendMenu;
use super::context_pad::{self, ContextPad, PadEntry};
use super::document::Document;
use super::empty_state::EmptyState;
use super::geometry::contains;
use super::geometry::{encloses, hit_edge, hit_shape, rect_from};
use super::grid::paint_grid;
use super::handles::{EdgeGrab, GRAB_TOLERANCE, corner_at, edge_grab};
use super::overlay::Overlays;
use super::paint::{paint_definitions, paint_ghost, paint_overlay};
use super::palette::{self, Palette, Tool};
use super::replace_menu::ReplaceMenu;
use super::search::SearchBox;
use super::sidebar::{self, Sidebar};
use crate::files::tree::{self, Node};
use crate::files::{check_openable, dialog_start_dir, home_dir};
use crate::model::{Bounds, Definitions, ElementId, ElementKind, Waypoint};
use crate::modeling::clipboard::Clipboard;
use crate::modeling::label::{flow_label_bounds, has_external_label, shape_label_bounds};
use crate::modeling::layout::center;
use crate::modeling::ordering::{DiItem, paint_order};
use crate::modeling::rules::{can_resize, connect_refusal, create_refusal, drop_refusal};
use crate::modeling::{
    Command, arrange, attach, auto_resize, created_id, created_shape, factory, lanes, waypoints,
};
use crate::settings::{GridSettings, Settings};
use crate::theme::{self, Theme};
use gpui_component::tree::{TreeEvent, TreeState};
use overlays::{LabelEdit, Picker};

/// Zoom sensitivity per scrolled pixel (exponential, so zoom feels uniform).
const ZOOM_PER_PIXEL: f64 = 0.01;

/// Screen distance (px) within which a click hits an edge.
const EDGE_TOLERANCE: f64 = 5.0;

/// Screen distance (px) a press must travel before it becomes a move.
const DRAG_THRESHOLD: f64 = 3.0;

/// Half length (screen px) of the space tool guide lines, enough to cross
/// any window; the content mask clips them.
const GUIDE_SPAN: f64 = 10_000.0;

/// Buttons of the discard-changes prompt, in answer order.
const DISCARD_ANSWERS: [&str; 3] = ["Save", "Don't Save", "Cancel"];

/// Continuation run once unsaved changes were saved or discarded.
type AfterConfirm = Box<dyn FnOnce(&mut Canvas, &mut Window, &mut Context<Canvas>)>;

/// Pointer gesture in progress.
enum Gesture {
    Idle,
    Pan {
        last: Point<Pixels>,
    },
    Lasso {
        start: Waypoint,
        current: Waypoint,
        additive: bool,
    },
    Move {
        start: Waypoint,
        current: Waypoint,
        /// Grid snapping on (off while cmd is held, like diagram-js).
        snap: bool,
        preview: Option<Rc<Definitions>>,
    },
    /// Dragging the external label of `id` on its own.
    Label {
        id: ElementId,
        start: Waypoint,
        current: Waypoint,
        preview: Option<Rc<Definitions>>,
    },
    /// Dragging a corner handle of the single selected shape.
    Resize {
        id: ElementId,
        corner: (i8, i8),
        start: Waypoint,
        current: Waypoint,
        preview: Option<Rc<Definitions>>,
    },
    /// Dragging a bendpoint or segment of the single selected connection.
    Waypoints {
        id: ElementId,
        grab: EdgeGrab,
        start: Waypoint,
        current: Waypoint,
        preview: Option<Rc<Definitions>>,
    },
    Space {
        start: Waypoint,
        current: Waypoint,
    },
}

/// Diagram view: owns the `Definitions`, its command stack, the selection,
/// the viewport and the file it was loaded from.
pub struct Canvas {
    /// Open documents (tabs), in tab order.
    docs: Vec<Document>,
    /// Index in `docs` of the document being edited.
    active: usize,
    origin: Rc<Cell<Point<Pixels>>>,
    focus: FocusHandle,
    gesture: Gesture,
    space: bool,
    editing: Option<LabelEdit>,
    /// Open cmd-f search overlay.
    search: Option<Picker>,
    /// Open append-anything menu of the selected shape.
    append_menu: Option<Picker>,
    /// Focus to move to on the next render (overlay inputs, or back).
    pending_focus: Option<FocusHandle>,
    /// Tool armed from the palette (one-shot, like bpmn-js).
    tool: Option<Tool>,
    /// Source chosen for a pending connect (connect tool or context pad).
    connect_source: Option<ElementId>,
    /// Shape or edge under the idle cursor.
    hover: Option<ElementId>,
    /// Last cursor position in diagram coordinates.
    cursor: Waypoint,
    /// Whether the replace menu of the selected shape is open.
    replacing: bool,
    /// Keyboard-highlighted replace-menu option, previewed on the canvas.
    replace_active: Option<usize>,
    /// Whether the drag preview lags the pointer; rebuilt once per frame.
    preview_stale: bool,
    /// Window title last applied, to update it only on change.
    shown_title: String,
    /// Last copied elements (an in-app clipboard, like bpmn-js).
    clipboard: Option<Clipboard>,
    /// Canvas size in screen px, written on paint.
    size: Rc<Cell<(f64, f64)>>,
    /// Whether to fit the viewport to the diagram on the next render.
    fit_pending: bool,
    /// Keeps the theme following the window appearance.
    _appearance: Subscription,
    /// Grid settings, refreshed when the `Settings` global changes.
    grid: GridSettings,
    /// Auto-scroll, snapping and replace-preview state of the current drag.
    aids: aids::Aids,
    /// Badges and tooltips anchored to elements or points.
    overlay_store: Overlays,
    /// Whether the overlay clock task is running.
    overlay_clock: bool,
    /// Folder chosen with Open Folder, browsed by the sidebar.
    folder: Option<PathBuf>,
    /// Scanned tree of `folder`, expanded lazily.
    tree: Option<Node>,
    /// Directories opened in the sidebar.
    expanded: HashSet<PathBuf>,
    /// Whether the sidebar is shown.
    sidebar_visible: bool,
    /// Kit tree state of the sidebar, rebuilt from `tree` and `expanded`.
    sidebar_tree: gpui::Entity<TreeState>,
}

impl Canvas {
    /// Creates a canvas showing `definitions` (loaded from `path`, if any),
    /// focuses it and sets the window title.
    pub fn new(
        definitions: Definitions,
        path: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let fit_pending = path.is_some();

        Self::with_docs(
            vec![Document::new(definitions, path)],
            fit_pending,
            window,
            cx,
        )
    }

    /// Creates a canvas holding no document (the empty state), focused.
    pub fn empty(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_docs(Vec::new(), false, window, cx)
    }

    /// Builds the canvas around `docs` (the first one active), wiring focus,
    /// theme tracking and the window close handler.
    fn with_docs(
        docs: Vec<Document>,
        fit_pending: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);

        theme::apply(window.appearance(), cx);
        let appearance = cx.observe_window_appearance(window, |canvas, window, cx| {
            canvas.on_appearance(window.appearance(), cx);
        });
        cx.observe_global_in::<Settings>(window, |canvas, window, cx| {
            canvas.on_appearance(window.appearance(), cx);
        })
        .detach();

        let sidebar_tree = cx.new(|cx| TreeState::new(cx));
        cx.subscribe(&sidebar_tree, |canvas, _, event: &TreeEvent, cx| {
            canvas.on_sidebar_tree_event(event, cx)
        })
        .detach();

        let this = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            this.update(cx, |canvas, cx| canvas.should_close(window, cx))
                .unwrap_or(true)
        });

        Canvas {
            docs,
            active: 0,
            origin: Rc::new(Cell::new(point(px(0.), px(0.)))),
            focus,
            gesture: Gesture::Idle,
            space: false,
            editing: None,
            search: None,
            append_menu: None,
            pending_focus: None,
            tool: None,
            connect_source: None,
            hover: None,
            cursor: Waypoint { x: 0.0, y: 0.0 },
            replacing: false,
            replace_active: None,
            preview_stale: false,
            shown_title: String::new(),
            clipboard: None,
            size: Rc::new(Cell::new((0.0, 0.0))),
            fit_pending,
            _appearance: appearance,
            grid: Settings::current(cx).grid,
            aids: aids::Aids::default(),
            overlay_store: Overlays::default(),
            overlay_clock: false,
            folder: None,
            tree: None,
            expanded: HashSet::new(),
            sidebar_visible: false,
            sidebar_tree,
        }
    }

    /// The folder chosen with Open Folder, if any.
    pub fn folder(&self) -> Option<&PathBuf> {
        self.folder.as_ref()
    }

    /// Stores `path` as the browsed folder, scans it, shows the sidebar and
    /// repaints.
    pub fn set_folder(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.tree = Some(tree::scan(&path));
        self.folder = Some(path);
        self.expanded.clear();
        self.sidebar_visible = true;
        self.sync_sidebar_tree(cx);
        cx.notify();
    }

    /// Whether the sidebar is shown.
    pub fn sidebar_visible(&self) -> bool {
        self.sidebar_visible
    }

    /// Shows or hides the sidebar.
    fn on_toggle_sidebar(&mut self, _: &ToggleSidebar, _: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_visible = !self.sidebar_visible;
        cx.notify();
    }

    /// Flips the active document between preview and code mode.
    fn on_toggle_mode(&mut self, _: &ToggleMode, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_mode(cx);
    }

    /// Records a directory opened or closed in the sidebar tree, loading its
    /// children on first open.
    fn on_sidebar_tree_event(&mut self, event: &TreeEvent, cx: &mut Context<Self>) {
        let (id, open) = match event {
            TreeEvent::Expanded(id) => (id, true),
            TreeEvent::Collapsed(id) => (id, false),
        };
        let path = PathBuf::from(id.as_ref());

        if !open {
            self.expanded.remove(&path);
        } else {
            self.expanded.insert(path.clone());
            if let Some(node) = self.tree.as_mut().and_then(|t| tree::find_mut(t, &path))
                && node.children.is_none()
            {
                node.expand();
            }
        }

        self.sync_sidebar_tree(cx);
        cx.notify();
    }

    /// Rebuilds the sidebar's kit tree items from `tree` and `expanded`.
    fn sync_sidebar_tree(&mut self, cx: &mut Context<Self>) {
        let items = self
            .tree
            .as_ref()
            .map(|root| sidebar::items(root, &self.expanded))
            .unwrap_or_default();

        self.sidebar_tree
            .update(cx, |state, cx| state.set_items(items, cx));
    }

    /// Opens the sidebar file at `path` in a tab.
    fn on_sidebar_pick(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.load(path, window, cx);
    }

    /// Rescans the folder, keeping the open directories that still exist.
    fn on_sidebar_refresh(&mut self, cx: &mut Context<Self>) {
        let Some(folder) = self.folder.clone() else {
            return;
        };

        let mut root = tree::scan(&folder);
        let mut open: Vec<PathBuf> = self.expanded.iter().cloned().collect();
        open.sort();
        self.expanded.clear();
        for path in open {
            if let Some(node) = tree::find_mut(&mut root, &path)
                && node.is_dir
            {
                node.expand();
                self.expanded.insert(path);
            }
        }
        self.tree = Some(root);
        self.sync_sidebar_tree(cx);
        cx.notify();
    }

    /// Root element holding the window-level actions (file, tab and mode
    /// commands), shared by the document and the empty-state layouts.
    fn root(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .size_full()
            .on_action(cx.listener(Self::on_new))
            .on_action(cx.listener(Self::on_open))
            .on_action(cx.listener(Self::on_open_recent))
            .on_action(cx.listener(Self::on_open_folder))
            .on_action(cx.listener(Self::on_toggle_sidebar))
            .on_action(cx.listener(Self::on_toggle_mode))
            .on_action(cx.listener(Self::on_save))
            .on_action(cx.listener(Self::on_save_as))
            .on_action(cx.listener(Self::on_close_tab))
            .on_action(cx.listener(Self::on_next_tab))
            .on_action(cx.listener(Self::on_prev_tab))
            .on_action(cx.listener(Self::on_activate_tab))
            .on_action(cx.listener(Self::on_activate_last_tab))
            .on_action(cx.listener(Self::on_quit))
    }

    /// The folder sidebar when shown and scanned, marking the active file.
    fn sidebar_element(&self, cx: &mut Context<Self>) -> Option<Sidebar> {
        let active_path = self.docs.get(self.active).and_then(|d| d.path.clone());

        self.sidebar_visible
            .then_some(self.tree.as_ref())
            .flatten()
            .map(|root| {
                Sidebar::new(
                    root.name.clone(),
                    self.sidebar_tree.clone(),
                    active_path,
                    cx.listener(|c, path: &Path, window, cx| {
                        c.on_sidebar_pick(path.to_path_buf(), window, cx)
                    }),
                    cx.listener(|c, _: &(), _, cx| c.on_sidebar_refresh(cx)),
                )
            })
    }

    /// Switches to the theme of `appearance` and repaints.
    pub fn on_appearance(&mut self, appearance: WindowAppearance, cx: &mut Context<Self>) {
        theme::apply(appearance, cx);
        self.grid = Settings::current(cx).grid;
        cx.notify();
    }

    /// The document being edited.
    pub fn definitions(&self) -> &Definitions {
        &self.docs[self.active].definitions
    }

    /// Whether the document has changes since it was loaded or last saved.
    pub fn is_dirty(&self) -> bool {
        self.docs.get(self.active).is_some_and(Document::is_dirty)
    }

    /// Window title: file name (or "Untitled"), a `*` when dirty, then the
    /// app; just the app name when no document is open.
    pub fn title(&self) -> String {
        let Some(doc) = self.docs.get(self.active) else {
            return "mangue".to_string();
        };
        let name = doc.title_name();
        let marker = if self.is_dirty() { " *" } else { "" };

        format!("{name}{marker} - mangue")
    }

    /// Runs `then` when the document is clean; otherwise asks to Save, Don't
    /// Save or Cancel (bpmn-io/Camunda Modeler style) and runs it after a
    /// successful save or on Don't Save.
    fn confirm_discard(&mut self, then: AfterConfirm, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_edit(cx);
        if !self.is_dirty() {
            then(self, window, cx);
            return;
        }

        let message = format!(
            "Do you want to save the changes to {}?",
            self.docs[self.active].title_name()
        );
        let answer = window.prompt(
            PromptLevel::Warning,
            &message,
            Some("Your changes will be lost if you don't save them."),
            &DISCARD_ANSWERS,
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let Ok(answer) = answer.await else {
                return;
            };

            match answer {
                0 => {
                    let _ =
                        this.update_in(cx, |this, window, cx| this.save(Some(then), window, cx));
                }

                1 => {
                    let _ = this.update_in(cx, |this, window, cx| then(this, window, cx));
                }

                _ => {}
            }
        })
        .detach();
    }

    /// Window close button: closes right away when every tab is clean; otherwise
    /// prompts for each dirty tab in turn and closes after the last answer.
    fn should_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.commit_edit(cx);
        if !self.docs.iter().any(Document::is_dirty) {
            return true;
        }

        self.confirm_all_from(
            0,
            Box::new(|_, window, _| window.remove_window()),
            window,
            cx,
        );
        false
    }

    /// Executes a command on the document; returns whether it changed anything.
    fn execute(&mut self, command: Command, cx: &mut Context<Self>) -> bool {
        if self.docs.is_empty() {
            return false;
        }

        let doc = &mut self.docs[self.active];
        doc.order.take();
        let changed = doc
            .stack
            .execute(command, Rc::make_mut(&mut doc.definitions));
        if changed {
            doc.record_execute();
        }

        self.prune_selection();
        cx.notify();
        changed
    }

    /// Executes a creating command, growing the container it lands in
    /// (`auto_resize`), and selects the element it creates.
    fn execute_create(&mut self, command: Command, cx: &mut Context<Self>) {
        let created = created_id(&command);
        let command = auto_resize::with_auto_resize(&self.docs[self.active].definitions, command);

        if self.execute(command, cx) {
            self.docs[self.active].selection = created.into_iter().collect();
        }
    }

    /// Drops selected ids that no longer exist (after delete or undo).
    fn prune_selection(&mut self) {
        if self.docs.is_empty() {
            return;
        }

        self.sync_plane();

        let doc = &mut self.docs[self.active];
        let defs = &doc.definitions;
        doc.selection.retain(|id| defs.get(id).is_some());
        self.overlay_store.prune(defs);
        if self.hover.as_ref().is_some_and(|h| defs.get(h).is_none()) {
            self.hover = None;
        }
    }

    /// Disarms the palette tool, a pending connect and the replace menu.
    fn reset_tools(&mut self) {
        self.tool = None;
        self.connect_source = None;
        self.replacing = false;
    }

    /// Palette pick: arms the tool (picking the armed one disarms it).
    fn on_palette(&mut self, tool: &Tool, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        self.commit_edit(cx);

        let again = self.tool.as_ref() == Some(tool);
        self.reset_tools();
        if !again {
            self.tool = Some(tool.clone());
        }

        cx.notify();
    }

    /// Context-pad pick on the single selected shape.
    fn on_pad(&mut self, entry: &PadEntry, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        let Some(id) = self.single_selected() else {
            return;
        };

        match entry {
            PadEntry::Append(kind) => {
                self.reset_tools();
                if let Some(command) =
                    factory::append(&self.docs[self.active].definitions, &id, kind.clone())
                {
                    self.execute_create(command, cx);
                }
            }

            PadEntry::Connect => {
                self.reset_tools();
                self.connect_source = Some(id);
            }

            PadEntry::Replace => {
                self.replacing = !self.replacing;
                self.replace_active = None;
            }

            PadEntry::AppendAnything => {
                self.open_append_menu(cx);
            }

            PadEntry::AddLane(side) => {
                self.reset_tools();
                if let Some(command) =
                    lanes::add_lane(&self.docs[self.active].definitions, &id, *side)
                {
                    self.execute(command, cx);
                }
            }

            PadEntry::SplitLane(count) => {
                self.reset_tools();
                if let Some(command) =
                    lanes::split_lane(&self.docs[self.active].definitions, &id, *count)
                {
                    self.execute(command, cx);
                }
            }

            PadEntry::Delete => {
                self.reset_tools();
                self.docs[self.active].selection.clear();
                self.execute(Command::DeleteElements { ids: vec![id] }, cx);
            }
        }

        cx.notify();
    }

    /// Replace-menu pick: changes the type of the selected shape.
    fn on_replace(&mut self, kind: &ElementKind, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_replace(kind, cx);
    }

    /// Changes the type of the selected shape to `kind` and closes the menu.
    fn apply_replace(&mut self, kind: &ElementKind, cx: &mut Context<Self>) {
        self.replacing = false;
        self.aids.replace_preview = None;

        if let Some(id) = self.single_selected() {
            self.execute(
                Command::ReplaceElement {
                    id,
                    kind: kind.clone(),
                },
                cx,
            );
        }

        cx.notify();
    }

    /// Replace-menu keys: Up / Down move the highlight and preview it, Enter
    /// applies it. Returns whether `key` was consumed.
    fn on_replace_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Some(options) = self.single_selected().and_then(|id| {
            let element = self.docs[self.active].definitions.get(&id)?;
            Some(factory::replace_options(&element.kind))
        }) else {
            return false;
        };
        if options.is_empty() {
            return false;
        }

        let last = options.len() - 1;
        let active = match (key, self.replace_active) {
            ("down", None) => 0,
            ("down", Some(i)) => (i + 1).min(last),
            ("up", None) => last,
            ("up", Some(i)) => i.saturating_sub(1),
            ("enter", Some(i)) => {
                self.apply_replace(&options[i], cx);
                return true;
            }
            _ => return false,
        };

        self.replace_active = Some(active);
        self.preview_replace(Some(&options[active]), cx);
        true
    }

    /// The selected element when exactly one shape is selected.
    fn single_selected(&self) -> Option<ElementId> {
        match self.docs[self.active].selection.as_slice() {
            [id] if self.docs[self.active].definitions.shape(id).is_some() => Some(id.clone()),
            _ => None,
        }
    }

    /// Creates the armed palette kind centered at diagram point `p`.
    fn create_at(&mut self, p: Waypoint, cx: &mut Context<Self>) {
        let Some(Tool::Create(kind, expanded)) = self.tool.take() else {
            return;
        };

        let (p, _) = self.snapped_create(&kind, expanded, p);
        let refusal = create_refusal(&self.docs[self.active].definitions, &kind, p);
        if let Some(command) =
            factory::create_shape(&self.docs[self.active].definitions, kind, expanded, p)
        {
            self.execute_create(command, cx);
        } else if let Some(refusal) = refusal {
            self.cursor = p;
            self.refuse(refusal, cx);
        }
    }

    /// Completes a pending connect at diagram point `p`: picks the source
    /// first (connect tool), then connects to the shape under `p`.
    fn connect_at(&mut self, p: Waypoint, cx: &mut Context<Self>) {
        let target = hit_shape(self.docs[self.active].definitions.shapes(), p).cloned();

        let Some(source) = self.connect_source.clone() else {
            match target {
                Some(id) => self.connect_source = Some(id),
                None => self.reset_tools(),
            }
            cx.notify();
            return;
        };

        self.reset_tools();
        let refusal = target.as_ref().and_then(|target| {
            connect_refusal(&self.docs[self.active].definitions, &source, target)
        });
        if let Some(command) = target.and_then(|target| {
            factory::connect(&self.docs[self.active].definitions, &source, &target)
        }) {
            self.execute_create(command, cx);
        } else if let Some(refusal) = refusal {
            self.refuse(refusal, cx);
        }
        cx.notify();
    }

    /// Converts a window position to canvas-local screen coordinates.
    fn local(&self, p: Point<Pixels>) -> Waypoint {
        let origin = self.origin.get();

        Waypoint {
            x: f64::from(p.x - origin.x),
            y: f64::from(p.y - origin.y),
        }
    }

    /// Converts a window position to diagram coordinates.
    fn diagram_point(&self, p: Point<Pixels>) -> Waypoint {
        self.docs[self.active].viewport.to_diagram(self.local(p))
    }

    /// Topmost element under a diagram point: edges first (painted on top),
    /// then shapes in paint order (`ordering::paint_order`).
    fn hit(&self, p: Waypoint) -> Option<ElementId> {
        let defs = &self.docs[self.active].definitions;
        let tolerance = EDGE_TOLERANCE / self.docs[self.active].viewport.scale;
        let plane = defs.plane();
        let mut cache = self.docs[self.active].order.borrow_mut();
        let order = cache.get_or_insert_with(|| {
            paint_order(defs, plane)
                .into_iter()
                .filter_map(|item| match item {
                    DiItem::Shape(i) => Some(i),
                    DiItem::Edge(_) => None,
                })
                .collect()
        });
        let painted = order.iter().map(|&i| &plane.shapes[i]);

        hit_edge(defs.edges(), p, tolerance)
            .or_else(|| hit_shape(painted, p))
            .cloned()
    }

    /// Mutable access to the document; drops the cached paint order.
    fn defs_mut(&mut self) -> &mut Definitions {
        self.docs[self.active].order.take();

        Rc::make_mut(&mut self.docs[self.active].definitions)
    }

    /// Scroll pans; cmd/ctrl + scroll zooms around the cursor.
    fn on_scroll(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(window.line_height());
        let (dx, dy) = (f64::from(delta.x), f64::from(delta.y));

        if event.modifiers.platform || event.modifiers.control {
            let cursor = self.local(event.position);
            self.docs[self.active]
                .viewport
                .zoom_at(cursor, (dy * ZOOM_PER_PIXEL).exp());
        } else {
            self.docs[self.active].viewport.pan(dx, dy);
        }

        cx.notify();
    }

    /// Left press: pan (space held), label edit (double-click), select or
    /// start a move on a hit, otherwise start a lasso.
    fn on_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        self.commit_edit(cx);
        self.close_pickers();

        if self.space {
            self.gesture = Gesture::Pan {
                last: event.position,
            };
            return;
        }

        let p = self.diagram_point(event.position);
        self.replacing = false;

        match &self.tool {
            Some(Tool::Hand) => {
                self.gesture = Gesture::Pan {
                    last: event.position,
                };
                return;
            }

            Some(Tool::Lasso) => {
                self.docs[self.active].selection.clear();
                self.gesture = Gesture::Lasso {
                    start: p,
                    current: p,
                    additive: false,
                };
                return;
            }

            Some(Tool::Space) => {
                self.gesture = Gesture::Space {
                    start: p,
                    current: p,
                };
                return;
            }

            Some(Tool::Create(..)) => {
                self.create_at(p, cx);
                return;
            }

            Some(Tool::Connect) => {
                self.connect_at(p, cx);
                return;
            }

            None if self.connect_source.is_some() => {
                self.connect_at(p, cx);
                return;
            }

            None => {}
        }

        if self.grab_handle(p, event.click_count, cx) {
            cx.notify();
            return;
        }

        let shift = event.modifiers.shift;
        if let (Some(id), 1, false) = (self.label_at(p), event.click_count, shift) {
            self.docs[self.active].selection = vec![id.clone()];
            self.gesture = Gesture::Label {
                id,
                start: p,
                current: p,
                preview: None,
            };
            cx.notify();
            return;
        }

        let hit = self.hit(p).or_else(|| self.label_at(p));

        if let (Some(id), 2) = (&hit, event.click_count) {
            self.start_edit(id.clone(), cx);
            return;
        }

        match hit {
            Some(id) if shift => {
                if let Some(i) = self.docs[self.active]
                    .selection
                    .iter()
                    .position(|s| *s == id)
                {
                    self.docs[self.active].selection.remove(i);
                } else {
                    self.docs[self.active].selection.push(id);
                }
            }

            Some(id) => {
                if !self.docs[self.active].selection.contains(&id) {
                    self.docs[self.active].selection = vec![id];
                }

                self.gesture = Gesture::Move {
                    start: p,
                    current: p,
                    snap: !event.modifiers.platform,
                    preview: None,
                };
            }

            None => {
                if !shift {
                    self.docs[self.active].selection.clear();
                }

                self.gesture = Gesture::Lasso {
                    start: p,
                    current: p,
                    additive: shift,
                };
            }
        }

        cx.notify();
    }

    /// Press on an editing handle of the single selection: a resize corner
    /// starts a resize; on a connection, a bendpoint or segment starts a
    /// waypoint drag and a double-click on a bendpoint removes it. Returns
    /// whether the press was taken.
    fn grab_handle(&mut self, p: Waypoint, clicks: usize, cx: &mut Context<Self>) -> bool {
        let [id] = self.docs[self.active].selection.as_slice() else {
            return false;
        };
        let id = id.clone();
        let defs = &self.docs[self.active].definitions;
        let tolerance = GRAB_TOLERANCE / self.docs[self.active].viewport.scale;

        if let (Some(shape), Some(element)) = (defs.shape(&id), defs.get(&id)) {
            let resizable = can_resize(&element.kind, shape.is_expanded == Some(true));
            let Some(corner) = corner_at(&shape.bounds, p, tolerance).filter(|_| resizable) else {
                return false;
            };

            self.gesture = Gesture::Resize {
                id,
                corner,
                start: p,
                current: p,
                preview: None,
            };
            return true;
        }

        let Some(grab) = defs
            .edges()
            .find(|e| e.element_id == id)
            .and_then(|e| edge_grab(&e.waypoints, p, tolerance))
        else {
            return false;
        };

        if let (EdgeGrab::Bendpoint(index), 2) = (grab, clicks) {
            let command = attach::bendpoint_remove(defs, &id, index);
            if let Some(command) = command {
                self.execute(command, cx);
            }
            return true;
        }

        self.gesture = Gesture::Waypoints {
            id,
            grab,
            start: p,
            current: p,
            preview: None,
        };
        true
    }

    /// Element whose external label (DI bounds, else the default spot) is
    /// under `p`; only named elements show a label.
    fn label_at(&self, p: Waypoint) -> Option<ElementId> {
        let defs = &self.docs[self.active].definitions;
        let named = |id: &ElementId| {
            defs.get(id)
                .filter(|e| has_external_label(&e.kind) && e.name.is_some())
        };

        let edges = defs.edges().filter_map(|e| {
            named(&e.element_id)?;
            let label = e.label_bounds.or_else(|| flow_label_bounds(&e.waypoints))?;
            Some((e.element_id.clone(), label))
        });
        let shapes = defs.shapes().filter_map(|s| {
            let element = named(&s.element_id)?;
            let label = s
                .label_bounds
                .unwrap_or_else(|| shape_label_bounds(&element.kind, &s.bounds));
            Some((s.element_id.clone(), label))
        });

        edges
            .chain(shapes)
            .filter(|(_, label)| contains(label, p))
            .map(|(id, _)| id)
            .last()
    }

    /// Command a drag gesture commits at its current point: a move (boundary
    /// attach/detach aware, snapped to siblings or the grid, growing its
    /// container), a label move, a resize (corner snapped), a reconnect or
    /// a waypoint edit (grid snapped); `None` when the gesture edits nothing
    /// or the rules refuse it.
    fn drag_command(&self, gesture: &Gesture) -> Option<Command> {
        let defs = &self.docs[self.active].definitions;

        match gesture {
            Gesture::Move {
                start,
                current,
                snap: snapping,
                ..
            } => {
                let snapped = self.snapped_move(*start, *current, *snapping);
                let command = self.move_command(snapped.dx, snapped.dy)?;

                Some(auto_resize::with_auto_resize(defs, command))
            }

            Gesture::Label {
                id, start, current, ..
            } => Some(Command::MoveLabel {
                id: id.clone(),
                dx: current.x - start.x,
                dy: current.y - start.y,
            }),

            Gesture::Resize {
                id,
                corner,
                start,
                current,
                ..
            } => {
                let b = defs.shape(id)?.bounds;
                let (bounds, _) = self.snapped_resize(id, &b, *corner, *start, *current);

                lanes::resize(defs, id, bounds)
            }

            Gesture::Waypoints {
                id,
                grab,
                start,
                current,
                ..
            } => {
                let points = &defs.edges().find(|e| e.element_id == *id)?.waypoints;
                let edited = match *grab {
                    EdgeGrab::Endpoint(start) => {
                        let to = hit_shape(defs.shapes(), *current)?;
                        return factory::reconnect(defs, id, start, to);
                    }

                    EdgeGrab::Bendpoint(index) => {
                        let to = self.snapped_bendpoint(*current);
                        waypoints::move_bendpoint(points, index, to)?
                    }

                    EdgeGrab::Segment(segment) => {
                        let inserted = waypoints::insert_bendpoint(points, segment, *start)?;
                        let to = self.snapped_bendpoint(*current);
                        waypoints::move_bendpoint(&inserted, segment + 1, to)?
                    }

                    EdgeGrab::SegmentMid(segment) => {
                        let (a, b) = (points[segment], points[segment + 1]);
                        let delta = if (a.y - b.y).abs() < 1.0 {
                            self.snapped_segment(a.y, current.y - start.y)
                        } else {
                            self.snapped_segment(a.x, current.x - start.x)
                        };
                        waypoints::move_segment(points, segment, delta)?
                    }
                };

                Some(Command::UpdateWaypoints {
                    id: id.clone(),
                    waypoints: edited,
                })
            }

            _ => None,
        }
    }

    /// Command moving the selection by `(dx, dy)`. A single boundary event
    /// slides along its host or detaches (`attach::move_boundary`); a single
    /// attachable event dropped on an activity border attaches to it;
    /// otherwise the shapes move and lane membership is recomputed for every
    /// participant.
    fn move_command(&self, dx: f64, dy: f64) -> Option<Command> {
        let defs = &self.docs[self.active].definitions;
        let ids = self.moving_ids();

        if drop_refusal(defs, &ids, dx, dy).is_some() {
            return None;
        }

        if let [id] = ids.as_slice() {
            if let Some(command) = attach::move_boundary(defs, id, dx, dy) {
                return Some(command);
            }

            if let Some(command) = self.attach_command(id, dx, dy) {
                return Some(command);
            }
        }

        Some(arrange::move_selection(defs, ids, dx, dy))
    }

    /// Attach command for event `id` moved by `(dx, dy)` onto the border of
    /// the topmost activity within 15px of its new center (bpmn-js
    /// `AttachSupport`), when `rules::can_attach` allows it.
    fn attach_command(&self, id: &ElementId, dx: f64, dy: f64) -> Option<Command> {
        let defs = &self.docs[self.active].definitions;
        let c = center(&defs.shape(id)?.bounds);
        let point = Waypoint {
            x: c.x + dx,
            y: c.y + dy,
        };
        let host = attach::host_at(defs, id, point)?;

        attach::attach(defs, id, &host, point)
    }

    /// Middle/right press always pans.
    fn on_pan_down(&mut self, event: &MouseDownEvent, _: &mut Window, _: &mut Context<Self>) {
        self.gesture = Gesture::Pan {
            last: event.position,
        };
    }

    /// Tracks the cursor and hover; updates the current gesture: pans, grows
    /// the lasso or space line, or previews a move; drags near the canvas
    /// edge auto-scroll first.
    fn on_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let p = self.diagram_point(event.position);
        self.cursor = p;

        if event.pressed_button.is_none() {
            self.gesture = Gesture::Idle;

            let hover = if palette::covers(self.local(event.position)) {
                None
            } else {
                self.hit(p)
            };
            if hover != self.hover || self.connect_source.is_some() || self.tool.is_some() {
                self.hover = hover;
                cx.notify();
            }
            return;
        }

        self.aids.free = event.modifiers.platform;
        self.auto_scroll(self.local(event.position), cx);
        let p = self.diagram_point(event.position);
        self.cursor = p;

        if matches!(self.tool, Some(Tool::Create(..))) {
            cx.notify();
            return;
        }

        let scale = self.docs[self.active].viewport.scale;

        match &mut self.gesture {
            Gesture::Idle => return,

            Gesture::Pan { last } => {
                let (dx, dy) = (event.position.x - last.x, event.position.y - last.y);
                self.docs[self.active]
                    .viewport
                    .pan(f64::from(dx), f64::from(dy));
                *last = event.position;
            }

            Gesture::Lasso { current, .. } | Gesture::Space { current, .. } => *current = p,

            Gesture::Move {
                start,
                current,
                preview,
                snap,
            } => {
                *current = p;
                *snap = !event.modifiers.platform;
                if preview.is_none()
                    && (p.x - start.x).hypot(p.y - start.y) * scale < DRAG_THRESHOLD
                {
                    return;
                }

                self.refresh_preview();
            }

            Gesture::Label {
                start,
                current,
                preview,
                ..
            }
            | Gesture::Resize {
                start,
                current,
                preview,
                ..
            }
            | Gesture::Waypoints {
                start,
                current,
                preview,
                ..
            } => {
                *current = p;
                if preview.is_none()
                    && (p.x - start.x).hypot(p.y - start.y) * scale < DRAG_THRESHOLD
                {
                    return;
                }

                self.refresh_preview();
            }
        }

        cx.notify();
    }

    /// Asks for a fresh drag preview: built at once when the drag has none
    /// yet, else marked stale and rebuilt once per frame by `render`, so a
    /// burst of mouse moves costs one document clone.
    fn refresh_preview(&mut self) {
        let started = matches!(
            &self.gesture,
            Gesture::Move {
                preview: Some(_),
                ..
            } | Gesture::Label {
                preview: Some(_),
                ..
            } | Gesture::Resize {
                preview: Some(_),
                ..
            } | Gesture::Waypoints {
                preview: Some(_),
                ..
            }
        );

        if started {
            self.preview_stale = true;
        } else {
            self.rebuild_preview();
        }
    }

    /// Recomputes the preview of the drag gesture: its command executed on a
    /// clone of the document, plus its snap lines.
    fn rebuild_preview(&mut self) {
        self.preview_stale = false;
        self.aids.lines = self.drag_lines();

        let mut defs = (*self.docs[self.active].definitions).clone();
        if let Some(command) = self.drag_command(&self.gesture) {
            let _ = command.execute(&mut defs);
        }

        if let Gesture::Move { preview, .. }
        | Gesture::Label { preview, .. }
        | Gesture::Resize { preview, .. }
        | Gesture::Waypoints { preview, .. } = &mut self.gesture
        {
            *preview = Some(Rc::new(defs));
        }
    }

    /// Finishes the gesture: commits a move or space, applies the lasso
    /// selection, or drops a shape dragged from the palette.
    fn on_up(&mut self, event: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let gesture = std::mem::replace(&mut self.gesture, Gesture::Idle);
        self.preview_stale = false;
        self.end_aids();

        if matches!(self.tool, Some(Tool::Create(..)))
            && !palette::covers(self.local(event.position))
        {
            let p = self.diagram_point(event.position);
            self.create_at(p, cx);
            return;
        }

        if let Some(refusal) = self.gesture_refusal(&gesture) {
            self.refuse(refusal, cx);
        }

        let gestured = !matches!(gesture, Gesture::Idle);
        if gestured && matches!(self.tool, Some(Tool::Hand | Tool::Lasso | Tool::Space)) {
            self.tool = None;
        }

        match gesture {
            Gesture::Space { start, current } => {
                let (dx, dy) = self.snapped_space(start, current);
                if let Some(command) =
                    factory::make_space(&self.docs[self.active].definitions, start, dx, dy)
                {
                    self.execute(command, cx);
                }
            }

            Gesture::Move {
                preview: Some(_), ..
            }
            | Gesture::Label {
                preview: Some(_), ..
            }
            | Gesture::Resize {
                preview: Some(_), ..
            }
            | Gesture::Waypoints {
                preview: Some(_), ..
            } => {
                if let Some(command) = self.drag_command(&gesture) {
                    self.execute(command, cx);
                }
            }

            Gesture::Lasso {
                start,
                current,
                additive,
            } => {
                let rect = rect_from(start, current);
                let defs = &self.docs[self.active].definitions;
                let shapes = defs
                    .shapes()
                    .filter(|s| encloses(&rect, &s.bounds))
                    .map(|s| s.element_id.clone());
                let edges = defs
                    .edges()
                    .filter(|e| {
                        e.waypoints.iter().all(|w| {
                            encloses(
                                &rect,
                                &Bounds {
                                    x: w.x,
                                    y: w.y,
                                    w: 0.0,
                                    h: 0.0,
                                },
                            )
                        })
                    })
                    .map(|e| e.element_id.clone());
                let hits: Vec<ElementId> = shapes.chain(edges).collect();

                if !additive {
                    self.docs[self.active].selection.clear();
                }
                for id in hits {
                    if !self.docs[self.active].selection.contains(&id) {
                        self.docs[self.active].selection.push(id);
                    }
                }
            }

            _ => {}
        }

        cx.notify();
    }

    /// Selected ids that have a DI shape (edges follow their endpoints).
    fn moving_ids(&self) -> Vec<ElementId> {
        self.docs[self.active]
            .selection
            .iter()
            .filter(|id| self.docs[self.active].definitions.shape(id).is_some())
            .cloned()
            .collect()
    }

    /// Key presses outside text overlays (their input handles typing):
    /// shortcuts, space for pan and Escape to clear the selection.
    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();

        if self.overlay_open() {
            return;
        }

        let bare = !event.keystroke.modifiers.modified();
        if self.replacing && bare && self.on_replace_key(key, cx) {
            cx.notify();
            return;
        }

        if self.on_shortcut(event, cx) {
            cx.notify();
            return;
        }

        match key {
            "space" => self.space = true,
            "escape" if self.tool.is_some() || self.connect_source.is_some() || self.replacing => {
                self.reset_tools()
            }
            "escape" => self.docs[self.active].selection.clear(),
            _ => return,
        }

        cx.notify();
    }

    /// Releasing space ends space-panning.
    fn on_key_up(&mut self, event: &KeyUpEvent, _: &mut Window, _: &mut Context<Self>) {
        if event.keystroke.key == "space" {
            self.space = false;
        }
    }

    /// Delete/Backspace: deletes the selection.
    fn on_delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        if self.docs.is_empty() {
            return;
        }

        let ids = std::mem::take(&mut self.docs[self.active].selection);
        self.execute(Command::DeleteElements { ids }, cx);
    }

    /// Undoes the last command.
    fn on_undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if self.docs.is_empty() || self.overlay_open() {
            return;
        }

        let doc = &mut self.docs[self.active];
        doc.order.take();
        if doc.stack.undo(Rc::make_mut(&mut doc.definitions)) {
            doc.record_undo();
        }

        self.prune_selection();
        cx.notify();
    }

    /// Redoes the last undone command.
    fn on_redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if self.docs.is_empty() || self.overlay_open() {
            return;
        }

        let doc = &mut self.docs[self.active];
        doc.order.take();
        if doc.stack.redo(Rc::make_mut(&mut doc.definitions)) {
            doc.record_redo();
        }

        self.prune_selection();
        cx.notify();
    }

    /// Opens the bpmn-js initial diagram as a new Untitled tab.
    fn on_new(&mut self, _: &New, _: &mut Window, cx: &mut Context<Self>) {
        self.open_tab(crate::io::new_diagram(), None, cx);
    }

    /// Closes the active tab, after confirming its unsaved changes.
    fn on_close_tab(&mut self, _: &CloseTab, window: &mut Window, cx: &mut Context<Self>) {
        self.close_tab(self.active, window, cx);
    }

    /// Quits the app, after confirming unsaved changes of every tab.
    fn on_quit(&mut self, _: &Quit, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_all_from(0, Box::new(|_, _, cx| cx.quit()), window, cx);
    }

    /// Prompts for a BPMN file and opens it in a tab.
    fn on_open(&mut self, _: &Open, window: &mut Window, cx: &mut Context<Self>) {
        self.open_prompt(window, cx);
    }

    /// Prompts for a folder and stores it as the browsed root.
    fn on_open_folder(&mut self, _: &OpenFolder, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });

        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };

            let _ = this.update(cx, |this, cx| this.set_folder(path, cx));
        })
        .detach();
    }

    /// Opens the recent file at `action.index` in a tab.
    fn on_open_recent(&mut self, action: &OpenRecent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = crate::settings::recent::current(cx)
            .files()
            .get(action.index)
            .cloned()
        else {
            return;
        };

        self.load(path, window, cx);
    }

    /// Prompts for a BPMN file and loads it.
    fn open_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });

        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };

            let _ = this.update_in(cx, |this, window, cx| this.load(path, window, cx));
        })
        .detach();
    }

    /// Opens `path` in a new tab and activates it; when a tab already holds
    /// the same file (canonicalized) activates that one. On failure keeps the
    /// current tab and alerts the user.
    pub fn load(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(err) = check_openable(&path) {
            if !path.exists() {
                crate::settings::recent::forget(&path, cx);
            }
            self.alert_open_failure(&path, &err, window, cx);
            return;
        }

        if let Some(index) = self.tab_of(&path) {
            crate::settings::recent::record(&path, cx);
            self.activate_tab(index, cx);
            return;
        }

        match super::persist::open(&path) {
            Ok(defs) => {
                crate::settings::recent::record(&path, cx);
                self.open_tab(defs, Some(path), cx);
            }
            Err(err) => self.alert_open_failure(&path, &err, window, cx),
        }
    }

    /// Reports an open failure on stderr and in a native critical alert.
    pub fn alert_open_failure(
        &mut self,
        path: &Path,
        err: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        eprintln!("open {}: {err}", path.display());

        let message = format!("Could not open {}", path.display());
        let _ = window.prompt(PromptLevel::Critical, &message, Some(err), &["OK"], cx);
    }

    /// Saves to the current path, prompting for one when the diagram is new.
    fn on_save(&mut self, _: &Save, window: &mut Window, cx: &mut Context<Self>) {
        if self.docs.is_empty() {
            return;
        }

        self.save(None, window, cx);
    }

    /// Always prompts for a destination, then rebinds the document to it.
    fn on_save_as(&mut self, _: &SaveAs, window: &mut Window, cx: &mut Context<Self>) {
        if self.docs.is_empty() {
            return;
        }

        self.commit_edit(cx);
        self.prompt_save_as(None, window, cx);
    }

    /// Writes to the bound path, or runs the Save As flow when unbound; runs
    /// `then` after a successful save.
    fn save(&mut self, then: Option<AfterConfirm>, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_edit(cx);

        let Some(path) = self.docs[self.active].path.clone() else {
            self.prompt_save_as(then, window, cx);
            return;
        };

        match self.write(&path) {
            Ok(()) => {
                crate::settings::recent::record(&path, cx);
                self.docs[self.active].mark_saved();
                cx.notify();
                if let Some(then) = then {
                    then(self, window, cx);
                }
            }
            Err(err) => self.alert_save_failure(&path, &err, window, cx),
        }
    }

    /// Asks for a destination (named after the current file) and finishes the
    /// save there; runs `then` on success.
    fn prompt_save_as(
        &mut self,
        then: Option<AfterConfirm>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let dir = dialog_start_dir(
            self.docs[self.active].path.as_deref(),
            home_dir().as_deref(),
        );
        let name = self.docs[self.active]
            .path
            .as_deref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "diagram.bpmn".into());
        let chosen = cx.prompt_for_new_path(&dir, Some(&name));

        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(path))) = chosen.await else {
                return;
            };

            let _ = this.update_in(cx, |this, window, cx| {
                if this.finish_save_as(path, window, cx)
                    && let Some(then) = then
                {
                    then(this, window, cx);
                }
            });
        })
        .detach();
    }

    /// Writes to `path`; on success rebinds the document to it and marks it
    /// clean, on failure alerts and leaves it dirty. Returns whether it saved.
    fn finish_save_as(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match self.write(&path) {
            Ok(()) => {
                crate::settings::recent::record(&path, cx);
                self.docs[self.active].path = Some(path);
                self.docs[self.active].mark_saved();
                cx.notify();
                true
            }
            Err(err) => {
                self.alert_save_failure(&path, &err, window, cx);
                false
            }
        }
    }

    /// Writes the active document to `path` and returns any error message.
    fn write(&self, path: &Path) -> Result<(), String> {
        super::persist::save(path, &self.docs[self.active].definitions)
    }

    /// Reports a save failure on stderr and in a native critical alert; the
    /// document stays dirty.
    fn alert_save_failure(
        &mut self,
        path: &Path,
        err: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        eprintln!("save {}: {err}", path.display());

        let message = format!("Could not save {}", path.display());
        let _ = window.prompt(PromptLevel::Critical, &message, Some(err), &["OK"], cx);
    }

    /// Screen rectangle (canvas-local) of the inline editor for `id`.
    fn edit_bounds(&self, id: &ElementId) -> Option<Bounds> {
        let defs = &self.docs[self.active].definitions;

        let diagram = if let Some(shape) = defs.shape(id) {
            let b = shape.bounds;
            if b.w >= 80.0 {
                b
            } else {
                Bounds {
                    x: b.x + b.w / 2.0 - 50.0,
                    y: b.y + b.h + 5.0,
                    w: 100.0,
                    h: 30.0,
                }
            }
        } else {
            let edge = defs.edges().find(|e| &e.element_id == id)?;
            let mid = edge.waypoints.get(edge.waypoints.len() / 2)?;
            Bounds {
                x: mid.x - 50.0,
                y: mid.y - 15.0,
                w: 100.0,
                h: 30.0,
            }
        };

        Some(self.docs[self.active].viewport.bounds_to_screen(diagram))
    }
}

impl Render for Canvas {
    /// Renders the painted diagram, selection overlay and inline label editor.
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.apply_fit(window);

        if self.preview_stale {
            self.rebuild_preview();
        }

        self.ensure_picker_input(window, cx);
        if let Some(focus) = self.pending_focus.take() {
            window.focus(&focus, cx);
        }

        let title = self.title();
        if title != self.shown_title {
            window.set_window_title(&title);
            window.set_window_edited(self.is_dirty());
            self.shown_title = title;
        }

        if self.docs.is_empty() {
            return self
                .root(cx)
                .track_focus(&self.focus)
                .children(self.sidebar_element(cx))
                .child(EmptyState)
                .into_any_element();
        }

        let definitions = match &self.gesture {
            Gesture::Move {
                preview: Some(p), ..
            }
            | Gesture::Label {
                preview: Some(p), ..
            }
            | Gesture::Resize {
                preview: Some(p), ..
            }
            | Gesture::Waypoints {
                preview: Some(p), ..
            } => p.clone(),
            _ => self
                .replace_preview()
                .unwrap_or_else(|| self.docs[self.active].definitions.clone()),
        };
        let lasso = match &self.gesture {
            Gesture::Lasso { start, current, .. } => Some(rect_from(*start, *current)),
            _ => None,
        };
        let selection = self.docs[self.active].selection.clone();
        let viewport = self.docs[self.active].viewport;
        let origin = self.origin.clone();
        let size = self.size.clone();
        let hover = self.hover.clone();
        let connect = self.connect_source.as_ref().and_then(|s| {
            let from = center(&self.docs[self.active].definitions.shape(s)?.bounds);
            Some((from, self.cursor))
        });

        let over_palette = palette::covers(viewport.to_screen(self.cursor));
        let mut create_lines = Vec::new();
        let ghost = match &self.tool {
            Some(Tool::Create(kind, expanded)) if !over_palette => {
                let (p, lines) = self.snapped_create(kind, *expanded, self.cursor);
                create_lines = lines;
                factory::create_shape(
                    &self.docs[self.active].definitions,
                    kind.clone(),
                    *expanded,
                    p,
                )
                .as_ref()
                .and_then(created_shape)
            }
            _ => None,
        };

        let span = GUIDE_SPAN / viewport.scale;
        let mut guides = match (&self.gesture, &self.tool) {
            (Gesture::Space { start, current }, _) => {
                let horizontal = (current.x - start.x).abs() >= (current.y - start.y).abs();
                let across = if horizontal {
                    (
                        Waypoint {
                            x: start.x,
                            y: start.y - span,
                        },
                        Waypoint {
                            x: start.x,
                            y: start.y + span,
                        },
                    )
                } else {
                    (
                        Waypoint {
                            x: start.x - span,
                            y: start.y,
                        },
                        Waypoint {
                            x: start.x + span,
                            y: start.y,
                        },
                    )
                };
                let (dx, dy) = self.snapped_space(*start, *current);
                let end = Waypoint {
                    x: start.x + dx,
                    y: start.y + dy,
                };
                vec![across, (*start, end)]
            }
            (_, Some(Tool::Space)) if !over_palette => {
                let c = self.cursor;
                vec![
                    (
                        Waypoint {
                            x: c.x,
                            y: c.y - span,
                        },
                        Waypoint {
                            x: c.x,
                            y: c.y + span,
                        },
                    ),
                    (
                        Waypoint {
                            x: c.x - span,
                            y: c.y,
                        },
                        Waypoint {
                            x: c.x + span,
                            y: c.y,
                        },
                    ),
                ]
            }
            _ => Vec::new(),
        };
        guides.extend(self.aids.lines.iter().copied());
        guides.extend(create_lines);

        let refused = self.current_refusal().is_some();
        let overlay_boxes = self.overlay_boxes(refused);

        let cursor = match (&self.gesture, &self.tool) {
            (Gesture::Pan { .. }, _) => CursorStyle::ClosedHand,
            _ if refused => CursorStyle::OperationNotAllowed,
            (_, Some(Tool::Hand)) => CursorStyle::OpenHand,
            (Gesture::Space { start, current }, _) => {
                if (current.x - start.x).abs() >= (current.y - start.y).abs() {
                    CursorStyle::ResizeLeftRight
                } else {
                    CursorStyle::ResizeUpDown
                }
            }
            (_, Some(_)) => CursorStyle::Crosshair,
            _ if self.connect_source.is_some() => CursorStyle::Crosshair,
            _ if self.space => CursorStyle::OpenHand,
            _ if self.hover.is_some() => CursorStyle::PointingHand,
            _ => CursorStyle::Arrow,
        };

        let palette = Palette::new(self.tool.clone(), cx.listener(Self::on_palette));

        let idle = matches!(self.gesture, Gesture::Idle) && self.editing.is_none();
        let drill = self.drill_buttons(cx);
        let crumbs = self.breadcrumbs(cx);

        let pad_target = self
            .single_selected()
            .filter(|_| idle && self.tool.is_none() && self.connect_source.is_none());
        let pad_at = pad_target.as_ref().and_then(|id| {
            let element = self.docs[self.active].definitions.get(id)?;
            let screen = self.docs[self.active]
                .viewport
                .bounds_to_screen(self.docs[self.active].definitions.shape(id)?.bounds);
            let options = factory::replace_options(&element.kind);
            let groups = context_pad::entries(&element.kind, !options.is_empty());
            Some((context_pad::origin_for(&screen), groups, options))
        });
        let (pad, menu, append) = match pad_at {
            Some((at, groups, options)) => {
                let (_, h) = context_pad::pad_size(&groups);
                let below = Waypoint {
                    x: at.x,
                    y: at.y + h + 4.0,
                };
                let menu = self.replacing.then(|| {
                    let menu = ReplaceMenu::new(below, options, cx.listener(Self::on_replace))
                        .with_active(self.replace_active)
                        .with_preview(cx.listener(|c, kind: &Option<ElementKind>, _, cx| {
                            c.preview_replace(kind.as_ref(), cx)
                        }));
                    match self.collapse_state() {
                        Some(expanded) => {
                            let toggle =
                                cx.listener(|c, _: &(), w, cx| c.on_toggle_collapse(w, cx));
                            menu.with_toggle(expanded, move |w, cx| toggle(&(), w, cx))
                        }
                        None => menu,
                    }
                });
                let append = self.append_menu.as_ref().and_then(|picker| {
                    let input = picker.input.clone()?;

                    Some(AppendMenu::new(
                        below,
                        input,
                        self.append_options(cx),
                        picker.active,
                        cx.listener(|canvas, kind: &ElementKind, _, cx| {
                            canvas.append_kind(kind, cx)
                        }),
                    ))
                });
                (
                    Some(ContextPad::new(at, groups, cx.listener(Self::on_pad))),
                    menu,
                    append,
                )
            }
            None => (None, None, None),
        };

        let search = self.search.as_ref().and_then(|picker| {
            Some(SearchBox::new(
                picker.input.clone()?,
                picker.query(cx),
                self.search_hits(cx),
                picker.active,
                cx.listener(|canvas, index: &usize, _, cx| canvas.pick_hit(*index, cx)),
            ))
        });

        let colors = Theme::current(cx);
        let editor = self.editing.as_ref().and_then(|edit| {
            let b = self.edit_bounds(&edit.id)?;
            Some(
                div()
                    .absolute()
                    .left(px(b.x as f32))
                    .top(px(b.y as f32))
                    .w(px(b.w as f32))
                    .min_h(px(b.h as f32))
                    .p_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(colors.label_editor.background.hsla())
                    .border_1()
                    .border_color(colors.label_editor.border.hsla())
                    .text_sm()
                    .text_color(colors.label_editor.text.hsla())
                    .child(edit.input.clone())
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()),
            )
        });

        let (grid, grid_color) = (self.grid.clone(), colors.grid.hsla());

        let sidebar = self.sidebar_element(cx);

        let surface = div()
            .relative()
            .flex_1()
            .h_full()
            .min_w_0()
            .bg(colors.canvas.background.hsla())
            .track_focus(&self.focus)
            .key_context("Canvas")
            .on_action(cx.listener(Self::on_delete))
            .on_action(cx.listener(Self::on_undo))
            .on_action(cx.listener(Self::on_redo))
            .on_action(cx.listener(Self::on_select_all))
            .on_action(cx.listener(Self::on_copy))
            .on_action(cx.listener(Self::on_paste))
            .on_action(cx.listener(Self::on_zoom_in))
            .on_action(cx.listener(Self::on_zoom_out))
            .on_action(cx.listener(Self::on_zoom_reset))
            .on_action(cx.listener(Self::on_zoom_fit))
            .on_action(cx.listener(Self::on_align_left))
            .on_action(cx.listener(Self::on_align_center))
            .on_action(cx.listener(Self::on_align_right))
            .on_action(cx.listener(Self::on_align_top))
            .on_action(cx.listener(Self::on_align_middle))
            .on_action(cx.listener(Self::on_align_bottom))
            .on_action(cx.listener(Self::on_distribute_horizontally))
            .on_action(cx.listener(Self::on_distribute_vertically))
            .on_action(cx.listener(Self::on_find))
            .capture_action(cx.listener(Self::on_picker_up))
            .capture_action(cx.listener(Self::on_picker_down))
            .capture_action(cx.listener(Self::on_picker_escape))
            .on_key_down(cx.listener(Self::on_key_down))
            .on_key_up(cx.listener(Self::on_key_up))
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, cx| {
                        origin.set(bounds.origin);
                        size.set((f64::from(bounds.size.width), f64::from(bounds.size.height)));
                        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
                            if grid.visible {
                                paint_grid(
                                    grid.size,
                                    grid_color,
                                    viewport,
                                    bounds.origin,
                                    size.get(),
                                    window,
                                );
                            }
                            paint_definitions(&definitions, viewport, bounds.origin, window, cx);
                            paint_overlay(
                                &definitions,
                                &selection,
                                hover.as_ref(),
                                lasso,
                                connect,
                                &guides,
                                viewport,
                                bounds.origin,
                                window,
                                cx,
                            );
                        });
                    },
                )
                .size_full(),
            )
            .children(ghost.map(|(kind, shape)| {
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, cx| {
                        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
                            paint_ghost(&kind, &shape, viewport, bounds.origin, window, cx);
                        });
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .opacity(0.5)
            }))
            .children(drill)
            .children(crumbs)
            .children(editor)
            .children(pad)
            .children(menu)
            .children(append)
            .children(search)
            .children(overlay_boxes)
            .child(palette)
            .cursor(cursor)
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_pan_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_pan_down))
            .on_mouse_move(cx.listener(Self::on_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::on_up));

        let tab_bar = self.tab_bar(cx);
        let doc_header = self.doc_header(cx);
        let body = self.body(surface, window, cx);

        self.root(cx)
            .children(sidebar)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(tab_bar)
                    .child(doc_header)
                    .child(body),
            )
            .into_any_element()
    }
}

#[path = "view_aids.rs"]
mod aids;

#[path = "view_commands.rs"]
mod commands;

#[path = "view_feedback.rs"]
mod feedback;

#[path = "view_overlays.rs"]
mod overlays;

#[path = "view_planes.rs"]
mod planes;

#[path = "view_tabs.rs"]
mod tabs;

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
