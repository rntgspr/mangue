//! `ReplaceMenu`: the change-type popup of the context pad (bpmn-js
//! `PopupMenu` with `ReplaceMenuProvider`), listing sibling element kinds,
//! each with its BPMN glyph and label.

use std::rc::Rc;

use gpui::{App, MouseButton, SharedString, Window, canvas, div, prelude::*, px};
use gpui_component::list::ListItem;

use super::paint::{Glyph, paint_glyph};
use crate::model::{ElementKind, Waypoint};
use crate::modeling::factory::kind_label;
use crate::theme::Theme;

/// Popup listing `options` at a canvas-local `origin` (option `active`
/// highlighted); picks go to `on_pick`.
#[derive(IntoElement)]
pub struct ReplaceMenu {
    origin: Waypoint,
    options: Vec<ElementKind>,
    active: Option<usize>,
    on_pick: Rc<dyn Fn(&ElementKind, &mut Window, &mut App)>,
    /// Collapse/expand row of a sub-process: whether it is expanded now,
    /// and the handler toggling it.
    toggle: Option<(bool, Rc<dyn Fn(&mut Window, &mut App)>)>,
    /// Hover handler: the option under the pointer, `None` when it leaves.
    on_preview: Option<Rc<dyn Fn(&Option<ElementKind>, &mut Window, &mut App)>>,
}

impl ReplaceMenu {
    /// Builds the popup for `options` at canvas-local `origin`.
    pub fn new(
        origin: Waypoint,
        options: Vec<ElementKind>,
        on_pick: impl Fn(&ElementKind, &mut Window, &mut App) + 'static,
    ) -> Self {
        ReplaceMenu {
            origin,
            options,
            active: None,
            on_pick: Rc::new(on_pick),
            toggle: None,
            on_preview: None,
        }
    }

    /// Adds the bpmn-js "Sub-process (collapsed/expanded)" row first, for a
    /// sub-process currently `expanded` or not; a press goes to `on_toggle`.
    pub fn with_toggle(
        mut self,
        expanded: bool,
        on_toggle: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.toggle = Some((expanded, Rc::new(on_toggle)));
        self
    }

    /// Highlights option `active` (keyboard navigation), if any.
    pub fn with_active(mut self, active: Option<usize>) -> Self {
        self.active = active;
        self
    }

    /// Reports the option hovered (bpmn-js replace preview) to `on_preview`,
    /// with `None` when the pointer leaves it.
    pub fn with_preview(
        mut self,
        on_preview: impl Fn(&Option<ElementKind>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_preview = Some(Rc::new(on_preview));
        self
    }
}

impl RenderOnce for ReplaceMenu {
    /// Renders one kit list row (glyph + label) per option; presses never
    /// reach the canvas.
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = Theme::current(cx).replace_menu;

        let sub_process = ElementKind::SubProcess {
            triggered_by_event: false,
        };
        let toggle = self.toggle.map(|(expanded, on_toggle)| {
            let label = if expanded {
                "Sub-Process (collapsed)"
            } else {
                "Sub-Process (expanded)"
            };
            let glyph = Glyph::Element(sub_process, !expanded);
            Self::row("replace-toggle".into(), glyph, label.into()).on_click(move |_, window, cx| {
                cx.stop_propagation();
                on_toggle(window, cx);
            })
        });

        let rows = self.options.into_iter().enumerate().map(|(i, kind)| {
            let on_pick = self.on_pick.clone();
            let on_preview = self.on_preview.clone();
            let hovered = kind.clone();
            let glyph = Glyph::Element(kind.clone(), false);
            Self::row(format!("replace-option-{i}").into(), glyph, kind_label(&kind))
                .selected(self.active == Some(i))
                .on_hover(move |inside, window, cx| {
                    if let Some(on_preview) = &on_preview {
                        on_preview(&inside.then(|| hovered.clone()), window, cx);
                    }
                })
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    on_pick(&kind, window, cx);
                })
        });

        div()
            .absolute()
            .left(px(self.origin.x as f32))
            .top(px(self.origin.y as f32))
            .min_w(px(180.))
            .py_1()
            .bg(colors.background.hsla())
            .border_1()
            .border_color(colors.border.hsla())
            .rounded_sm()
            .shadow_md()
            .flex()
            .flex_col()
            .children(toggle)
            .children(rows)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
    }
}

impl ReplaceMenu {
    /// One kit list row `id` (also its debug selector): `glyph` icon and
    /// `label`.
    fn row(id: SharedString, glyph: Glyph, label: String) -> ListItem {
        let selector = id.to_string();
        let icon = canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| paint_glyph(&glyph, bounds, false, window, cx),
        )
        .size(px(20.));

        ListItem::new(id)
            .debug_selector(move || selector)
            .px_2()
            .text_sm()
            .cursor_pointer()
            .child(div().flex().items_center().gap_2().child(icon).child(label))
    }
}
