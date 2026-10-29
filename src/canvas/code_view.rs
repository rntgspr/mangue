//! Code mode editor: gpui-component's code editor showing a document's BPMN XML.

use gpui::{App, AppContext, Context, Entity, Window};
use gpui_component::input::EditorState;

/// Initializes gpui-component's globals (theme, input key bindings); call
/// once at app start and in test setup before the first code-mode render.
pub fn init_editor(cx: &mut App) {
    gpui_component::init(cx);
}

/// Creates a code editor state holding `xml`, highlighted as markup.
pub(super) fn new_editor<V: 'static>(
    xml: &str,
    window: &mut Window,
    cx: &mut Context<V>,
) -> Entity<EditorState> {
    let xml = xml.to_owned();

    cx.new(|cx| {
        EditorState::new(window, cx)
            .language("html")
            .default_value(xml)
    })
}
