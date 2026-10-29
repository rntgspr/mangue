//! `XmlError`: the banner over the canvas telling that the edited XML was
//! rejected on leaving code mode, with Revert and Back to code buttons.

use std::rc::Rc;

use gpui::{App, SharedString, Window, div, prelude::*, px};
use gpui_component::Sizable;
use gpui_component::alert::Alert;
use gpui_component::button::Button;

/// Callback run when a banner button is clicked.
type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Banner showing `message` (the import error) and its two actions.
#[derive(IntoElement)]
pub struct XmlError {
    message: String,
    on_revert: Handler,
    on_back: Handler,
}

impl XmlError {
    /// Builds the banner; the buttons run `on_revert` and `on_back`.
    pub fn new(
        message: String,
        on_revert: impl Fn(&mut Window, &mut App) + 'static,
        on_back: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        XmlError {
            message,
            on_revert: Rc::new(on_revert),
            on_back: Rc::new(on_back),
        }
    }
}

/// One kit button of the banner, tagged with its `id` for lookup.
fn button(id: &'static str, label: &'static str, handler: Handler) -> Button {
    Button::new(id)
        .debug_selector(move || id.into())
        .small()
        .label(label)
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            handler(window, cx);
        })
}

impl RenderOnce for XmlError {
    /// Renders the kit error banner and its buttons at the top of the canvas.
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let text = SharedString::from(format!("Invalid XML: {}", self.message));

        div()
            .absolute()
            .top(px(8.))
            .left(px(8.))
            .right(px(8.))
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .debug_selector(|| "xml-error-alert".into())
                    .child(Alert::error("xml-error", text).banner()),
            )
            .child(button("xml-error-revert", "Revert", self.on_revert))
            .child(button("xml-error-back", "Back to code", self.on_back))
    }
}
