//! `Breadcrumbs`: the drill-down path bar (bpmn-js `.bjs-breadcrumbs`),
//! root first; every crumb but the last opens its plane.

use std::rc::Rc;

use gpui::{App, MouseButton, Window, div, prelude::*, px};
use gpui_component::breadcrumb::{Breadcrumb, BreadcrumbItem};

use super::drilldown::Crumb;
use crate::model::{ElementId, Waypoint};
use crate::theme::Theme;

/// Bar of `crumbs` at canvas-local `origin`; picks go to `on_pick`.
#[derive(IntoElement)]
pub struct Breadcrumbs {
    origin: Waypoint,
    crumbs: Vec<Crumb>,
    on_pick: Rc<dyn Fn(Option<&ElementId>, &mut Window, &mut App)>,
}

impl Breadcrumbs {
    /// Builds the bar for `crumbs` at canvas-local `origin`.
    pub fn new(
        origin: Waypoint,
        crumbs: Vec<Crumb>,
        on_pick: impl Fn(Option<&ElementId>, &mut Window, &mut App) + 'static,
    ) -> Self {
        Breadcrumbs {
            origin,
            crumbs,
            on_pick: Rc::new(on_pick),
        }
    }
}

impl RenderOnce for Breadcrumbs {
    /// Renders the kit breadcrumb on the popup box colors; every crumb but
    /// the current plane is clickable.
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = Theme::current(cx).replace_menu;
        let last = self.crumbs.len().saturating_sub(1);

        let items = self.crumbs.into_iter().enumerate().map(|(i, crumb)| {
            let item = BreadcrumbItem::new(crumb.label)
                .max_w(px(200.))
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap();
            if i == last {
                return item;
            }

            let on_pick = self.on_pick.clone();
            let plane = crumb.plane;
            item.on_click(move |_, window, cx| {
                cx.stop_propagation();
                on_pick(plane.as_ref(), window, cx);
            })
        });

        div()
            .id("breadcrumbs")
            .debug_selector(|| "breadcrumbs".into())
            .absolute()
            .left(px(self.origin.x as f32))
            .top(px(self.origin.y as f32))
            .h(px(28.))
            .px_2()
            .flex()
            .items_center()
            .bg(colors.background.hsla())
            .border_1()
            .border_color(colors.border.hsla())
            .rounded_sm()
            .child(Breadcrumb::new().children(items))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
    }
}
