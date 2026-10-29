//! In-window menu bar for Windows and Linux, where `set_menus` has no global
//! menu bar: the kit `AppMenuBar` (fed by `actions::set_menus`) above the canvas.

use gpui::{
    Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription, Window, div, px,
};
use gpui_component::{ActiveTheme, menu::AppMenuBar, v_flex};

use super::Canvas;
use crate::settings::recent::RecentFiles;

/// A canvas with the app menu bar on top.
pub struct MenuShell {
    pub canvas: Entity<Canvas>,
    menu_bar: Entity<AppMenuBar>,
    _recent: Subscription,
}

impl MenuShell {
    /// Wraps `canvas`; the menu bar reloads when the recent files change.
    pub fn new(canvas: Entity<Canvas>, cx: &mut Context<Self>) -> Self {
        let menu_bar = AppMenuBar::new(cx);
        let recent = cx.observe_global::<RecentFiles>(|shell, cx| {
            shell.menu_bar.update(cx, |bar, cx| bar.reload(cx));
        });

        MenuShell {
            canvas,
            menu_bar,
            _recent: recent,
        }
    }
}

impl Render for MenuShell {
    /// The menu bar row above the canvas filling the rest of the window.
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(
                div()
                    .h(px(30.))
                    .px_1()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().title_bar)
                    .child(self.menu_bar.clone()),
            )
            .child(div().flex_1().min_h_0().child(self.canvas.clone()))
    }
}
