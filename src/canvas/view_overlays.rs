//! Text overlays of the `Canvas` view: the inline label editor (bpmn-js
//! `LabelEditingProvider`, on `TextInput`), cmd-f search (bpmn-js `search`)
//! and the append-anything menu (bpmn-js `AppendMenuProvider`), both on a
//! gpui-component `InputState`.

use gpui::{AppContext, Context, Entity, Focusable, Subscription, Window};
use gpui_component::input::{Escape, InputEvent, InputState, MoveDown, MoveUp};

use super::super::actions::Find;
use super::super::append_menu;
use super::super::search;
use super::super::text_input::{TextInput, TextInputEvent};
use super::Canvas;
use crate::model::{ElementId, ElementKind, Waypoint};
use crate::modeling::layout::center;
use crate::modeling::{Command, factory};

/// Inline label editor on element `id`.
pub(super) struct LabelEdit {
    pub id: ElementId,
    pub input: Entity<TextInput>,
    _events: Subscription,
}

/// Search or append popup: its kit query input (created on the next render,
/// which has the window it needs) and the highlighted row.
pub(super) struct Picker {
    pub input: Option<Entity<InputState>>,
    pub active: usize,
    placeholder: &'static str,
    on_event: OnEvent,
    _events: Option<Subscription>,
}

impl Picker {
    /// A picker whose input shows `placeholder` and reports to `on_event`.
    fn new(placeholder: &'static str, on_event: OnEvent) -> Self {
        Picker {
            input: None,
            active: 0,
            placeholder,
            on_event,
            _events: None,
        }
    }

    /// The typed query; empty until the input exists.
    pub fn query(&self, cx: &gpui::App) -> String {
        self.input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default()
    }
}

/// Handler of one overlay's input events.
type OnEvent = fn(&mut Canvas, &TextInputEvent, &mut Context<Canvas>);

impl Canvas {
    /// New input with `text`, routed to `on_event`, focused on next render.
    fn new_input(
        &mut self,
        text: &str,
        multiline: bool,
        placeholder: &'static str,
        on_event: OnEvent,
        cx: &mut Context<Self>,
    ) -> (Entity<TextInput>, Subscription) {
        let input = cx.new(|cx| TextInput::new(text, multiline, placeholder, cx));
        let events = cx.subscribe(&input, move |canvas, _, event, cx| {
            on_event(canvas, event, cx);
            cx.notify();
        });
        self.pending_focus = Some(input.read(cx).focus_handle(cx));

        (input, events)
    }

    /// Creates the kit input of an open search or append picker that has
    /// none yet, routing its change and Enter events to the picker handler
    /// and focusing it.
    pub(super) fn ensure_picker_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(picker) = self.search.as_mut().or(self.append_menu.as_mut()) else {
            return;
        };
        if picker.input.is_some() {
            return;
        }

        let placeholder = picker.placeholder;
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let on_event = picker.on_event;
        let events = cx.subscribe(&input, move |canvas, _, event: &InputEvent, cx| {
            let event = match event {
                InputEvent::Change => TextInputEvent::Changed,
                InputEvent::PressEnter { .. } => TextInputEvent::Confirm,
                _ => return,
            };
            on_event(canvas, &event, cx);
            cx.notify();
        });

        self.pending_focus = Some(input.read(cx).focus_handle(cx));
        picker.input = Some(input);
        picker._events = Some(events);
    }

    /// Up / Down / Escape inside an open picker's kit input move the
    /// highlight or close it, before the input handles them; otherwise the
    /// action propagates.
    pub(super) fn on_picker_key(&mut self, event: TextInputEvent, cx: &mut Context<Self>) {
        let Some(picker) = self.search.as_ref().or(self.append_menu.as_ref()) else {
            return;
        };

        let on_event = picker.on_event;
        cx.stop_propagation();
        on_event(self, &event, cx);
        cx.notify();
    }

    /// Capture-phase `MoveUp` handler for the picker input.
    pub(super) fn on_picker_up(&mut self, _: &MoveUp, _: &mut Window, cx: &mut Context<Self>) {
        self.on_picker_key(TextInputEvent::Navigate(-1), cx);
    }

    /// Capture-phase `MoveDown` handler for the picker input.
    pub(super) fn on_picker_down(&mut self, _: &MoveDown, _: &mut Window, cx: &mut Context<Self>) {
        self.on_picker_key(TextInputEvent::Navigate(1), cx);
    }

    /// Capture-phase `Escape` handler for the picker input.
    pub(super) fn on_picker_escape(&mut self, _: &Escape, _: &mut Window, cx: &mut Context<Self>) {
        self.on_picker_key(TextInputEvent::Cancel, cx);
    }

    /// Whether a label editor, search or append menu is open.
    pub(super) fn overlay_open(&self) -> bool {
        self.editing.is_some() || self.search.is_some() || self.append_menu.is_some()
    }

    /// Closes search and the append menu, refocusing the canvas.
    pub(super) fn close_pickers(&mut self) {
        if self.search.take().is_some() | self.append_menu.take().is_some() {
            self.pending_focus = Some(self.focus.clone());
        }
    }

    /// Opens the multi-line inline editor on `id` with its current label.
    pub(super) fn start_edit(&mut self, id: ElementId, cx: &mut Context<Self>) {
        let Some(element) = self.docs[self.active].definitions.get(&id) else {
            return;
        };

        let current = match &element.kind {
            ElementKind::TextAnnotation { text } => text.clone(),
            _ => element.name.clone(),
        };
        self.close_pickers();
        self.docs[self.active].selection = vec![id.clone()];

        let (input, events) = self.new_input(
            current.as_deref().unwrap_or(""),
            true,
            "",
            Self::on_label_event,
            cx,
        );
        self.editing = Some(LabelEdit {
            id,
            input,
            _events: events,
        });
        cx.notify();
    }

    /// Enter commits the label, Escape discards it.
    fn on_label_event(&mut self, event: &TextInputEvent, cx: &mut Context<Self>) {
        match event {
            TextInputEvent::Confirm => self.commit_edit(cx),
            TextInputEvent::Cancel => {
                self.editing = None;
                self.pending_focus = Some(self.focus.clone());
            }
            _ => {}
        }
    }

    /// Commits the inline editor (if open) as one `UpdateLabel` command.
    pub(super) fn commit_edit(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.editing.take() else {
            return;
        };

        let name = edit.input.read(cx).text().to_string();
        self.pending_focus = Some(self.focus.clone());
        self.execute(Command::UpdateLabel { id: edit.id, name }, cx);
    }

    /// cmd-f: opens the search overlay.
    pub(super) fn on_find(&mut self, _: &Find, _: &mut Window, cx: &mut Context<Self>) {
        self.commit_edit(cx);
        self.close_pickers();
        self.reset_tools();

        self.search = Some(Picker::new("Search in diagram", Self::on_search_event));
        cx.notify();
    }

    /// Current search results.
    pub(super) fn search_hits(&self, cx: &Context<Self>) -> Vec<search::Hit> {
        self.search.as_ref().map_or_else(Vec::new, |s| {
            search::find(&self.docs[self.active].definitions, &s.query(cx))
        })
    }

    /// Typing resets the highlight; arrows move it; Enter picks; Escape closes.
    fn on_search_event(&mut self, event: &TextInputEvent, cx: &mut Context<Self>) {
        let count = self.search_hits(cx).len();
        let Some(search) = &mut self.search else {
            return;
        };

        match event {
            TextInputEvent::Changed => search.active = 0,
            TextInputEvent::Navigate(d) => search.active = step(search.active, *d, count),
            TextInputEvent::Confirm => {
                let active = search.active;
                self.pick_hit(active, cx);
            }
            TextInputEvent::Cancel => self.close_pickers(),
        }
    }

    /// Selects search result `index`, centers the view on it and closes search.
    pub(super) fn pick_hit(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(hit) = self.search_hits(cx).into_iter().nth(index) else {
            return;
        };

        self.close_pickers();
        self.center_on(&hit.id);
        self.docs[self.active].selection = vec![hit.id];
        cx.notify();
    }

    /// Pans so the center of element `id` sits at the canvas center.
    fn center_on(&mut self, id: &ElementId) {
        let defs = &self.docs[self.active].definitions;
        let target = match defs.shape(id) {
            Some(shape) => center(&shape.bounds),
            None => match defs.edges().find(|e| &e.element_id == id) {
                Some(edge) if !edge.waypoints.is_empty() => {
                    edge.waypoints[edge.waypoints.len() / 2]
                }
                _ => return,
            },
        };

        let (w, h) = self.size.get();
        let scale = self.docs[self.active].viewport.scale;
        self.docs[self.active].viewport.offset = Waypoint {
            x: w / 2.0 - target.x * scale,
            y: h / 2.0 - target.y * scale,
        };
    }

    /// `a` or the pad entry: opens the append menu for the single selected
    /// shape when anything can be appended to it.
    pub(super) fn open_append_menu(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(id) = self.single_selected() else {
            return false;
        };
        if append_menu::options(&self.docs[self.active].definitions, &id, "").is_empty() {
            return false;
        }

        self.close_pickers();
        self.reset_tools();

        self.append_menu = Some(Picker::new("Search element", Self::on_append_event));
        cx.notify();
        true
    }

    /// Current append options for the selected shape.
    pub(super) fn append_options(&self, cx: &Context<Self>) -> Vec<ElementKind> {
        match (&self.append_menu, self.single_selected()) {
            (Some(menu), Some(id)) => {
                append_menu::options(&self.docs[self.active].definitions, &id, &menu.query(cx))
            }
            _ => Vec::new(),
        }
    }

    /// Typing resets the highlight; arrows move it; Enter appends; Escape closes.
    fn on_append_event(&mut self, event: &TextInputEvent, cx: &mut Context<Self>) {
        let options = self.append_options(cx);
        let Some(menu) = &mut self.append_menu else {
            return;
        };

        match event {
            TextInputEvent::Changed => menu.active = 0,
            TextInputEvent::Navigate(d) => menu.active = step(menu.active, *d, options.len()),
            TextInputEvent::Confirm => {
                if let Some(kind) = options.get(menu.active).cloned() {
                    self.append_kind(&kind, cx);
                }
            }
            TextInputEvent::Cancel => self.close_pickers(),
        }
    }

    /// Appends `kind` to the selected shape (`factory::append`) and closes
    /// the menu.
    pub(super) fn append_kind(&mut self, kind: &ElementKind, cx: &mut Context<Self>) {
        let source = self.single_selected();
        self.close_pickers();

        if let Some(command) = source
            .and_then(|id| factory::append(&self.docs[self.active].definitions, &id, kind.clone()))
        {
            self.execute_create(command, cx);
        }
        cx.notify();
    }
}

/// Row index `active` moved by `delta`, clamped to `count` rows.
fn step(active: usize, delta: isize, count: usize) -> usize {
    if count == 0 {
        return 0;
    }

    active.saturating_add_signed(delta).min(count - 1)
}
