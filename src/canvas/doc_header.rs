//! `DocHeader`: the bar under the tab strip with a breadcrumb placeholder on
//! the left and the Code / Preview mode button on the right.

use std::rc::Rc;

use gpui::{App, Window, div, prelude::*, px};
use gpui_component::Sizable;
use gpui_component::button::Button;

use super::breadcrumbs::Breadcrumbs;
use super::drilldown::Crumb;
use crate::model::Waypoint;
use crate::theme::Theme;

/// Callback run when the mode button is clicked.
type ToggleHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Height of the header in pixels.
pub const HEIGHT: f32 = 32.0;

/// Label of the mode button: the mode a click switches to.
pub fn label(code: bool) -> &'static str {
    if code { "Preview" } else { "Code" }
}

/// Header showing `title` as a placeholder crumb and the mode button for the
/// `code` flag.
#[derive(IntoElement)]
pub struct DocHeader {
    title: String,
    code: bool,
    on_toggle: ToggleHandler,
}

impl DocHeader {
    /// Builds the header; a click on the mode button runs `on_toggle`.
    pub fn new(
        title: String,
        code: bool,
        on_toggle: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        DocHeader {
            title,
            code,
            on_toggle: Rc::new(on_toggle),
        }
    }
}

impl RenderOnce for DocHeader {
    /// Renders the placeholder `Breadcrumbs` on the left and the kit mode
    /// button on the right.
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        let colors = theme.replace_menu;
        let on_toggle = self.on_toggle;

        let crumb = Crumb {
            label: self.title,
            plane: None,
        };
        let crumbs = Breadcrumbs::new(Waypoint { x: 8.0, y: 2.0 }, vec![crumb], |_, _, _| {});

        let button = Button::new("doc-header-mode")
            .debug_selector(|| "doc-header-mode".to_string())
            .outline()
            .small()
            .label(label(self.code))
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                on_toggle(window, cx);
            });

        div()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_end()
            .h(px(HEIGHT))
            .w_full()
            .pr_2()
            .bg(colors.background.hsla())
            .border_b_1()
            .border_color(colors.border.hsla())
            .child(crumbs)
            .child(button)
    }
}
