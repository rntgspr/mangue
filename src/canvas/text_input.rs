//! `TextInput`: reusable single- or multi-line text input view (caret,
//! selection, word/line motion, clipboard, IME through GPUI
//! `EntityInputHandler`), after Zed's gpui `examples/input.rs`. Used by the
//! label editor, search and the append menu; the owner reacts to its
//! `TextInputEvent`s.

use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, EntityInputHandler, EventEmitter,
    FocusHandle, Focusable, KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, Point, ShapedLine, SharedString, UTF16Selection, Window, actions, div, point,
    prelude::*,
};

use super::text_edit::TextEdit;
use super::text_element::TextElement;

actions!(
    text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        WordLeft,
        WordRight,
        SelectWordLeft,
        SelectWordRight,
        Home,
        End,
        SelectHome,
        SelectEnd,
        SelectAll,
        Copy,
        Cut,
        Paste,
        Newline,
        Confirm,
        Cancel,
    ]
);

/// Key context of the input; its bindings win over the canvas ones.
const CONTEXT: &str = "TextInput";

/// Key bindings of the input (macOS conventions).
pub fn bindings() -> Vec<KeyBinding> {
    let c = Some(CONTEXT);

    vec![
        KeyBinding::new("backspace", Backspace, c),
        KeyBinding::new("delete", Delete, c),
        KeyBinding::new("left", Left, c),
        KeyBinding::new("right", Right, c),
        KeyBinding::new("up", Up, c),
        KeyBinding::new("down", Down, c),
        KeyBinding::new("shift-left", SelectLeft, c),
        KeyBinding::new("shift-right", SelectRight, c),
        KeyBinding::new("shift-up", SelectUp, c),
        KeyBinding::new("shift-down", SelectDown, c),
        KeyBinding::new("alt-left", WordLeft, c),
        KeyBinding::new("alt-right", WordRight, c),
        KeyBinding::new("alt-shift-left", SelectWordLeft, c),
        KeyBinding::new("alt-shift-right", SelectWordRight, c),
        KeyBinding::new("home", Home, c),
        KeyBinding::new("end", End, c),
        super::actions::bind("cmd-left", Home, c),
        super::actions::bind("cmd-right", End, c),
        KeyBinding::new("shift-home", SelectHome, c),
        KeyBinding::new("shift-end", SelectEnd, c),
        super::actions::bind("cmd-shift-left", SelectHome, c),
        super::actions::bind("cmd-shift-right", SelectEnd, c),
        super::actions::bind("cmd-a", SelectAll, c),
        super::actions::bind("cmd-c", Copy, c),
        super::actions::bind("cmd-x", Cut, c),
        super::actions::bind("cmd-v", Paste, c),
        KeyBinding::new("shift-enter", Newline, c),
        KeyBinding::new("enter", Confirm, c),
        KeyBinding::new("escape", Cancel, c),
    ]
}

/// What the input reports to its owner.
#[derive(Debug, Clone, PartialEq)]
pub enum TextInputEvent {
    /// The text changed.
    Changed,
    /// Enter was pressed.
    Confirm,
    /// Escape was pressed.
    Cancel,
    /// Up (-1) or down (+1) in a single-line input (list navigation).
    Navigate(isize),
}

/// Text input view; the text color and font come from the parent element.
pub struct TextInput {
    pub(super) edit: TextEdit,
    pub(super) focus: FocusHandle,
    pub(super) placeholder: SharedString,
    /// Shaped lines (with their start offset) and bounds of the last paint.
    layout: Vec<(usize, ShapedLine)>,
    bounds: Option<Bounds<Pixels>>,
    line_height: Pixels,
    selecting: bool,
}

impl EventEmitter<TextInputEvent> for TextInput {}

impl Focusable for TextInput {
    /// The input's focus handle (focus it to type).
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TextInput {
    /// Creates an input holding `text` (caret at the end), showing
    /// `placeholder` while empty.
    pub fn new(
        text: &str,
        multiline: bool,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> Self {
        TextInput {
            edit: TextEdit::new(text, multiline),
            focus: cx.focus_handle(),
            placeholder: placeholder.into(),
            layout: Vec::new(),
            bounds: None,
            line_height: Pixels::ZERO,
            selecting: false,
        }
    }

    /// Current text.
    pub fn text(&self) -> &str {
        &self.edit.text
    }

    /// Records the layout from the most recent paint for hit-testing.
    pub(super) fn record_layout(
        &mut self,
        layout: Vec<(usize, ShapedLine)>,
        bounds: Bounds<Pixels>,
        line_height: Pixels,
    ) {
        self.layout = layout;
        self.bounds = Some(bounds);
        self.line_height = line_height;
    }

    /// Moves the caret (or extends the selection when `select`) to `offset`.
    fn go(&mut self, offset: usize, select: bool, cx: &mut Context<Self>) {
        if select {
            self.edit.select_to(offset);
        } else {
            self.edit.move_to(offset);
        }

        cx.notify();
    }

    /// Replaces the selection with `text` and reports the change.
    fn insert(&mut self, text: &str, cx: &mut Context<Self>) {
        self.edit.replace(None, text);
        cx.emit(TextInputEvent::Changed);
        cx.notify();
    }

    /// Deletes the selection, or the character before the caret.
    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        if self.edit.selected.is_empty() {
            let to = self.edit.prev_char(self.edit.cursor());
            self.edit.select_to(to);
        }

        self.insert("", cx);
    }

    /// Deletes the selection, or the character after the caret.
    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        if self.edit.selected.is_empty() {
            let to = self.edit.next_char(self.edit.cursor());
            self.edit.select_to(to);
        }

        self.insert("", cx);
    }

    /// Caret one character left (or to the selection start).
    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        let e = &self.edit;
        let to = if e.selected.is_empty() {
            e.prev_char(e.cursor())
        } else {
            e.selected.start
        };

        self.go(to, false, cx);
    }

    /// Caret one character right (or to the selection end).
    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        let e = &self.edit;
        let to = if e.selected.is_empty() {
            e.next_char(e.cursor())
        } else {
            e.selected.end
        };

        self.go(to, false, cx);
    }

    /// Caret one line up, or list navigation in a single-line input.
    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        if !self.edit.multiline {
            cx.emit(TextInputEvent::Navigate(-1));
            return;
        }

        let to = self.edit.line_up(self.edit.cursor());
        self.go(to, false, cx);
    }

    /// Caret one line down, or list navigation in a single-line input.
    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        if !self.edit.multiline {
            cx.emit(TextInputEvent::Navigate(1));
            return;
        }

        let to = self.edit.line_down(self.edit.cursor());
        self.go(to, false, cx);
    }

    /// Extends the selection one character left.
    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.edit.prev_char(self.edit.cursor());
        self.go(to, true, cx);
    }

    /// Extends the selection one character right.
    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.edit.next_char(self.edit.cursor());
        self.go(to, true, cx);
    }

    /// Extends the selection one line up.
    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.edit.line_up(self.edit.cursor());
        self.go(to, true, cx);
    }

    /// Extends the selection one line down.
    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.edit.line_down(self.edit.cursor());
        self.go(to, true, cx);
    }

    /// Caret to the previous word start.
    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.edit.prev_word(self.edit.cursor());
        self.go(to, false, cx);
    }

    /// Caret to the next word end.
    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.edit.next_word(self.edit.cursor());
        self.go(to, false, cx);
    }

    /// Extends the selection to the previous word start.
    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.edit.prev_word(self.edit.cursor());
        self.go(to, true, cx);
    }

    /// Extends the selection to the next word end.
    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.edit.next_word(self.edit.cursor());
        self.go(to, true, cx);
    }

    /// Caret to the line start.
    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.edit.line_start(self.edit.cursor());
        self.go(to, false, cx);
    }

    /// Caret to the line end.
    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.edit.line_end(self.edit.cursor());
        self.go(to, false, cx);
    }

    /// Extends the selection to the line start.
    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.edit.line_start(self.edit.cursor());
        self.go(to, true, cx);
    }

    /// Extends the selection to the line end.
    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.edit.line_end(self.edit.cursor());
        self.go(to, true, cx);
    }

    /// Selects the whole text.
    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.edit.select_all();
        cx.notify();
    }

    /// Copies the selection to the system clipboard.
    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.edit.selected.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.edit.selected_text().to_string(),
            ));
        }
    }

    /// Copies the selection to the system clipboard and deletes it.
    fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if !self.edit.selected.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.edit.selected_text().to_string(),
            ));
            self.insert("", cx);
        }
    }

    /// Inserts the clipboard text over the selection.
    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.insert(&text, cx);
        }
    }

    /// Shift+Enter: a newline in a multi-line input (ignored otherwise).
    fn newline(&mut self, _: &Newline, _: &mut Window, cx: &mut Context<Self>) {
        if self.edit.multiline {
            self.insert("\n", cx);
        }
    }

    /// Enter: reports `Confirm`.
    fn confirm(&mut self, _: &Confirm, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextInputEvent::Confirm);
    }

    /// Escape: reports `Cancel`.
    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextInputEvent::Cancel);
    }

    /// Press: places the caret (shift extends the selection) and starts a
    /// drag selection; never reaches the views below.
    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        window.focus(&self.focus, cx);
        self.selecting = true;

        let offset = self.index_at(event.position);
        self.go(offset, event.modifiers.shift, cx);
    }

    /// Drag: extends the selection to the pointer.
    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.selecting {
            let offset = self.index_at(event.position);
            self.go(offset, true, cx);
        }
    }

    /// Release: ends the drag selection.
    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }

    /// Text offset closest to window point `p` (from the last paint).
    fn index_at(&self, p: Point<Pixels>) -> usize {
        let Some(bounds) = self.bounds else {
            return self.edit.text.len();
        };
        if self.layout.is_empty() || self.line_height <= Pixels::ZERO {
            return self.edit.text.len();
        }

        let row = ((p.y - bounds.top()) / self.line_height).floor().max(0.0) as usize;
        let (start, line) = &self.layout[row.min(self.layout.len() - 1)];
        start + line.closest_index_for_x(p.x - bounds.left())
    }

    /// Line index and shaped line containing byte `offset`.
    pub(super) fn line_of(&self, offset: usize) -> Option<(usize, &(usize, ShapedLine))> {
        self.layout
            .iter()
            .enumerate()
            .rev()
            .find(|(_, (start, _))| *start <= offset)
    }

    /// Window rectangle covering `range` on the line where it starts.
    fn range_bounds(&self, range: Range<usize>, bounds: Bounds<Pixels>) -> Option<Bounds<Pixels>> {
        let (row, (start, line)) = self.line_of(range.start)?;
        let top = bounds.top() + self.line_height * row as f32;
        let end = range.end.min(start + line.len());

        Some(Bounds::from_corners(
            point(bounds.left() + line.x_for_index(range.start - start), top),
            point(
                bounds.left() + line.x_for_index(end.max(range.start) - start),
                top + self.line_height,
            ),
        ))
    }
}

impl EntityInputHandler for TextInput {
    /// Text of a UTF-16 range.
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.edit.range_from_utf16(&range);
        actual.replace(self.edit.range_to_utf16(&range));
        Some(self.edit.text[range].to_string())
    }

    /// Selection in UTF-16 offsets.
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.edit.range_to_utf16(&self.edit.selected),
            reversed: self.edit.reversed,
        })
    }

    /// IME composition in UTF-16 offsets.
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.edit
            .marked
            .as_ref()
            .map(|r| self.edit.range_to_utf16(r))
    }

    /// Ends the IME composition, keeping its text.
    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.edit.marked = None;
    }

    /// Typed or committed text replaces a range (else composition/selection).
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range.map(|r| self.edit.range_from_utf16(&r));
        self.edit.replace(range, text);
        cx.emit(TextInputEvent::Changed);
        cx.notify();
    }

    /// IME composition update.
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range.map(|r| self.edit.range_from_utf16(&r));
        let inserted = TextEdit::new(text, true);
        let selected = selected.map(|s| inserted.range_from_utf16(&s));
        self.edit.replace_and_mark(range, text, selected);
        cx.emit(TextInputEvent::Changed);
        cx.notify();
    }

    /// Window rectangle of a UTF-16 range (IME candidate window placement).
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.edit.range_from_utf16(&range);
        self.range_bounds(range, bounds)
    }

    /// UTF-16 offset under window point `p`.
    fn character_index_for_point(
        &mut self,
        p: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        self.bounds?;
        Some(self.edit.offset_to_utf16(self.index_at(p)))
    }
}

impl Render for TextInput {
    /// Renders the text element inside a focusable, I-beam region.
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::cancel))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .child(TextElement::new(cx.entity()))
    }
}
