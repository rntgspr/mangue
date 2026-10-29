//! `AppendMenu`: the "append anything" popup (bpmn-js `AppendMenuProvider`
//! in a filterable popup menu): every kind that `factory::append` can attach
//! to the selected shape, filtered by the typed text.

use std::rc::Rc;

use gpui::{App, Entity, MouseButton, SharedString, Window, canvas, div, prelude::*, px};
use gpui_component::input::{Input, InputState};
use gpui_component::list::ListItem;
use gpui_component::{ActiveTheme, Sizable};

use super::paint::{Glyph, paint_glyph};
use crate::model::{Definitions, ElementId, ElementKind, EventDefinition, Waypoint};
use crate::modeling::factory::{self, kind_label};

/// Height of the scrolling option list in px.
const LIST_HEIGHT: f32 = 320.;

/// Candidate kinds in menu order (bpmn-js append menu groups: events,
/// gateways, activities, data, artifacts; no start events).
fn candidates() -> Vec<ElementKind> {
    use ElementKind::*;
    use EventDefinition as D;

    let mut kinds = vec![IntermediateThrowEvent(D::None), EndEvent(D::None)];

    kinds.extend(
        [D::Message, D::Timer, D::Conditional, D::Link, D::Signal].map(IntermediateCatchEvent),
    );
    kinds.extend(
        [
            D::Message,
            D::Escalation,
            D::Link,
            D::Compensation,
            D::Signal,
        ]
        .map(IntermediateThrowEvent),
    );
    kinds.extend(
        [
            D::Message,
            D::Escalation,
            D::Error,
            D::Compensation,
            D::Signal,
            D::Terminate,
        ]
        .map(EndEvent),
    );
    kinds.extend([
        ExclusiveGateway,
        ParallelGateway,
        InclusiveGateway,
        ComplexGateway,
        EventBasedGateway,
        Task,
        UserTask,
        ServiceTask,
        ScriptTask,
        SendTask,
        ReceiveTask,
        ManualTask,
        BusinessRuleTask,
        CallActivity,
        SubProcess {
            triggered_by_event: false,
        },
        DataObjectReference,
        DataStoreReference,
        TextAnnotation { text: None },
    ]);

    kinds
}

/// Kinds appendable to `source` whose label contains `query` (ignoring case).
pub fn options(defs: &Definitions, source: &ElementId, query: &str) -> Vec<ElementKind> {
    let query = query.trim().to_lowercase();

    candidates()
        .into_iter()
        .filter(|kind| kind_label(kind).to_lowercase().contains(&query))
        .filter(|kind| factory::append(defs, source, kind.clone()).is_some())
        .collect()
}

/// Popup at a canvas-local `origin` with the filter `input` over `options`
/// (row `active` highlighted); picks go to `on_pick`.
#[derive(IntoElement)]
pub struct AppendMenu {
    origin: Waypoint,
    input: Entity<InputState>,
    options: Vec<ElementKind>,
    active: usize,
    on_pick: Rc<dyn Fn(&ElementKind, &mut Window, &mut App)>,
}

impl AppendMenu {
    /// Builds the popup.
    pub fn new(
        origin: Waypoint,
        input: Entity<InputState>,
        options: Vec<ElementKind>,
        active: usize,
        on_pick: impl Fn(&ElementKind, &mut Window, &mut App) + 'static,
    ) -> Self {
        AppendMenu {
            origin,
            input,
            options,
            active,
            on_pick: Rc::new(on_pick),
        }
    }
}

impl RenderOnce for AppendMenu {
    /// Renders the kit filter input above one kit list row (painted glyph +
    /// label) per option; presses never reach the canvas.
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let rows = self.options.into_iter().enumerate().map(|(i, kind)| {
            let on_pick = self.on_pick.clone();
            let label = kind_label(&kind);
            let glyph = Glyph::Element(kind.clone(), false);
            let icon = canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| paint_glyph(&glyph, bounds, false, window, cx),
            )
            .size(px(20.));
            let selector = format!("append-option-{i}");

            ListItem::new(SharedString::from(selector.clone()))
                .debug_selector(move || selector)
                .selected(i == self.active)
                .text_sm()
                .child(div().flex().items_center().gap_2().child(icon).child(label))
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    on_pick(&kind, window, cx);
                })
        });

        div()
            .absolute()
            .left(px(self.origin.x as f32))
            .top(px(self.origin.y as f32))
            .w(px(220.))
            .p_1()
            .bg(cx.theme().popover)
            .text_color(cx.theme().popover_foreground)
            .border_1()
            .border_color(cx.theme().border)
            .rounded(cx.theme().radius)
            .shadow_md()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .debug_selector(|| "append-input".into())
                    .child(Input::new(&self.input).small()),
            )
            .child(
                div()
                    .id("append-menu-options")
                    .max_h(px(LIST_HEIGHT))
                    .overflow_y_scroll()
                    .children(rows),
            )
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A start event can append tasks, gateways and events but not a start
    /// event; the query filters by label.
    #[test]
    fn options_filter() {
        let defs = crate::io::new_diagram();
        let start = ElementId::from("StartEvent_1");

        let all = options(&defs, &start, "");
        assert!(all.contains(&ElementKind::Task));
        assert!(all.contains(&ElementKind::ExclusiveGateway));
        assert!(!all.iter().any(|k| matches!(k, ElementKind::StartEvent(_))));

        let users = options(&defs, &start, "user");
        assert_eq!(users, vec![ElementKind::UserTask]);
    }
}
