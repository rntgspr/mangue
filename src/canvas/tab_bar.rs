//! `TabBar`: the strip above the canvas listing the open documents, a
//! gpui-component tab bar with one tab per document, a dirty marker and a
//! close button; the active tab is selected.

use std::rc::Rc;

use gpui::{App, SharedString, Window, div, prelude::*, px};
use gpui_component::Sizable;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::tab::{Tab, TabBar as GpuiTabBar};

/// Callback taking the clicked tab index.
type PickHandler = Rc<dyn Fn(&usize, &mut Window, &mut App)>;

/// Height of the tab strip in pixels.
pub const HEIGHT: f32 = 28.0;

/// Tabs as (name, dirty) with the `active` one highlighted.
#[derive(IntoElement)]
pub struct TabBar {
    tabs: Vec<(String, bool)>,
    active: usize,
    on_pick: PickHandler,
    on_close: PickHandler,
}

impl TabBar {
    /// Builds the strip; a click on a tab goes to `on_pick` with its index and
    /// one on its close button to `on_close`.
    pub fn new(
        tabs: Vec<(String, bool)>,
        active: usize,
        on_pick: impl Fn(&usize, &mut Window, &mut App) + 'static,
        on_close: impl Fn(&usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        TabBar {
            tabs,
            active,
            on_pick: Rc::new(on_pick),
            on_close: Rc::new(on_close),
        }
    }
}

impl RenderOnce for TabBar {
    /// Renders the kit tab bar: one `Tab` per document with a dirty marker
    /// and a close button as suffix, the active one selected.
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let tabs = self
            .tabs
            .into_iter()
            .enumerate()
            .map(|(index, (name, dirty))| {
                let on_close = self.on_close.clone();
                let tab_selector = format!("tab-{index}");
                let dirty_selector = format!("tab-dirty-{index}");
                let close_selector = format!("tab-close-{index}");

                let close = Button::new(SharedString::from(close_selector.clone()))
                    .debug_selector(move || close_selector)
                    .ghost()
                    .xsmall()
                    .label("x")
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        on_close(&index, window, cx);
                    });
                let marker = dirty.then(|| div().debug_selector(move || dirty_selector).child("*"));

                Tab::new()
                    .debug_selector(move || tab_selector)
                    .label(name)
                    .suffix(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .children(marker)
                            .child(close),
                    )
            });

        let on_pick = self.on_pick.clone();
        GpuiTabBar::new("tab-bar")
            .h(px(HEIGHT))
            .w_full()
            .flex_none()
            .selected_index(self.active)
            .children(tabs)
            .on_click(move |index, window, cx| on_pick(index, window, cx))
    }
}
