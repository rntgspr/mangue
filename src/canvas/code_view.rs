//! Code mode editor: gpui-component's code editor showing a document's BPMN XML.

use gpui::{App, AppContext, Context, Entity, Window};
use gpui_component::input::{
    EditorState, RangeDecoration, RangeDecorationCollection, RangeDecorationStyle, RopeExt,
};
use quick_xml::{Reader, XmlVersion, events::Event};

use crate::model::ElementId;

/// Finds the XML range associated with an element ID.
pub(super) fn element_range(xml: &str, id: &ElementId) -> Option<std::ops::Range<usize>> {
    let mut reader = Reader::from_str(xml);

    loop {
        let start = reader.buffer_position() as usize;
        let event = reader.read_event().ok()?;
        let end = reader.buffer_position() as usize;

        match event {
            Event::Start(ref element) | Event::Empty(ref element) => {
                let found = element.attributes().flatten().any(|attr| {
                    attr.key.as_ref() == "id"
                        && attr
                            .normalized_value(XmlVersion::Implicit1_0)
                            .is_ok_and(|value| value == id.0)
                });

                if found {
                    let offset = xml[start..end].find('<')? + start;
                    if matches!(event, Event::Empty(_)) {
                        break Some(offset..end);
                    }

                    let mut depth = 1usize;
                    while depth > 0 {
                        match reader.read_event().ok()? {
                            Event::Start(_) => depth += 1,
                            Event::End(_) => depth -= 1,
                            Event::Eof => return None,
                            _ => {}
                        }
                    }
                    break Some(offset..reader.buffer_position() as usize);
                }
            }
            Event::Eof => break None,
            _ => {}
        }
    }
}

/// Initializes gpui-component's globals (theme, input key bindings); call
/// once at app start and in test setup before the first code-mode render.
pub fn init_editor(cx: &mut App) {
    gpui_component::init(cx);
}

/// Creates a code editor state holding `xml`, highlighted as markup.
pub(super) fn new_editor<V: 'static>(
    xml: &str,
    selected: Option<&ElementId>,
    window: &mut Window,
    cx: &mut Context<V>,
) -> (Entity<EditorState>, RangeDecorationCollection) {
    let xml = xml.to_owned();
    let range = selected.and_then(|id| element_range(&xml, id));

    let editor = cx.new(|cx| {
        let mut editor = EditorState::new(window, cx)
            .language("html")
            .default_value(xml);

        if let Some(range) = &range {
            let position = editor.text().offset_to_position(range.start);
            editor.set_cursor_position(position, window, cx);
        }

        editor
    });

    let highlight = editor.update(cx, |editor, cx| {
        editor.create_range_decorations_collection(
            range
                .into_iter()
                .map(|range| RangeDecoration::new(range).with_style(RangeDecorationStyle::Fill))
                .collect(),
            cx,
        )
    });
    (editor, highlight)
}
