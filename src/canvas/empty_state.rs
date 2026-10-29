//! `EmptyState`: the centered call-to-action shown when the window holds no
//! document, rendered with gpui-component's `Empty`.

use gpui::{Action, Window, div, prelude::*};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::empty::{Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyTitle};

use super::actions::Open;
use crate::theme::Theme;

/// Title, hint line and a primary "Open File" button that runs the Open action.
#[derive(IntoElement)]
pub struct EmptyState;

impl RenderOnce for EmptyState {
    /// Renders the kit empty state on the theme's canvas background.
    fn render(self, _: &mut Window, cx: &mut gpui::App) -> impl IntoElement {
        let theme = Theme::current(cx);

        let header = EmptyHeader::new()
            .title(
                EmptyTitle::new().child(
                    div()
                        .debug_selector(|| "empty-state-title".into())
                        .child("No diagram open"),
                ),
            )
            .description(
                EmptyDescription::new().child(
                    div()
                        .debug_selector(|| "empty-state-hint".into())
                        .child("or press Cmd+O"),
                ),
            );

        let open = Button::new("empty-state-open")
            .debug_selector(|| "empty-state-open".into())
            .primary()
            .label("Open File")
            .on_click(|_, window, cx| {
                cx.stop_propagation();
                window.dispatch_action(Open.boxed_clone(), cx);
            });

        div()
            .id("empty-state")
            .debug_selector(|| "empty-state".into())
            .flex_1()
            .flex()
            .bg(theme.canvas.background.hsla())
            .child(
                Empty::new()
                    .border_0()
                    .header(header)
                    .content(EmptyContent::new().child(open)),
            )
    }
}
