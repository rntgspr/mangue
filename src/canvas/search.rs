//! `SearchBox`: the cmd-f search overlay (bpmn-js `search` with
//! `BpmnSearchProvider`): a query input over a list of diagram elements whose
//! name or id contains the query, case-insensitively.

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, Entity, FontWeight, HighlightStyle, MouseButton, SharedString, StyledText, Window, div,
    prelude::*, px,
};
use gpui_component::input::{Input, InputState};
use gpui_component::list::ListItem;
use gpui_component::{ActiveTheme, Sizable};

use crate::model::{Definitions, ElementId, ElementKind};

/// Most results listed.
pub const MAX_RESULTS: usize = 12;

/// Width of the overlay in px.
const WIDTH: f32 = 340.;

/// Distance of the overlay from the canvas top in px.
const TOP: f32 = 16.;

/// One search result: a displayed element and its label text.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub id: ElementId,
    pub name: Option<String>,
}

/// Displayed shapes then connections (DI order) whose name (annotation
/// text) or id contains `query`, ignoring case; none for a blank query.
pub fn find(defs: &Definitions, query: &str) -> Vec<Hit> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }

    let ids = defs
        .shapes()
        .map(|s| &s.element_id)
        .chain(defs.edges().map(|e| &e.element_id));

    ids.filter_map(|id| {
        let element = defs.get(id)?;
        let name = match &element.kind {
            ElementKind::TextAnnotation { text } => text.clone(),
            _ => element.name.clone(),
        };

        let matches = id.as_str().to_lowercase().contains(&query)
            || name
                .as_deref()
                .is_some_and(|n| n.to_lowercase().contains(&query));
        matches.then(|| Hit {
            id: id.clone(),
            name,
        })
    })
    .take(MAX_RESULTS)
    .collect()
}

/// Byte range of the first case-insensitive occurrence of `query` (trimmed)
/// in `text`, for bolding the match (bpmn-js search highlights it); `None`
/// when absent, blank, or lowercasing changes `text`'s byte length.
pub fn match_range(text: &str, query: &str) -> Option<Range<usize>> {
    let query = query.trim().to_lowercase();
    let lower = text.to_lowercase();
    if query.is_empty() || lower.len() != text.len() {
        return None;
    }

    let start = lower.find(&query)?;
    Some(start..start + query.len())
}

/// `text` with the match of `query` in bold.
fn highlighted(text: String, query: &str) -> StyledText {
    let bold = HighlightStyle {
        font_weight: Some(FontWeight::BOLD),
        ..HighlightStyle::default()
    };
    let range = match_range(&text, query);

    StyledText::new(text).with_highlights(range.map(|r| (r, bold)))
}

/// Overlay with the query `input` and `hits` (row `active` highlighted);
/// row clicks go to `on_pick` with the row index.
#[derive(IntoElement)]
pub struct SearchBox {
    input: Entity<InputState>,
    query: String,
    hits: Vec<Hit>,
    active: usize,
    on_pick: Rc<dyn Fn(&usize, &mut Window, &mut App)>,
}

impl SearchBox {
    /// Builds the overlay.
    pub fn new(
        input: Entity<InputState>,
        query: String,
        hits: Vec<Hit>,
        active: usize,
        on_pick: impl Fn(&usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        SearchBox {
            input,
            query,
            hits,
            active,
            on_pick: Rc::new(on_pick),
        }
    }
}

impl RenderOnce for SearchBox {
    /// Renders the kit input above one kit list row (name, then id, matches
    /// in bold) per hit, centered at the canvas top; presses never reach the
    /// canvas.
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;

        let rows = self.hits.into_iter().enumerate().map(|(i, hit)| {
            let on_pick = self.on_pick.clone();
            let label = hit.name.clone().unwrap_or_default();
            let selector = format!("search-hit-{i}");

            ListItem::new(SharedString::from(selector.clone()))
                .debug_selector(move || selector)
                .selected(i == self.active)
                .text_sm()
                .child(
                    div()
                        .flex()
                        .w_full()
                        .justify_between()
                        .gap_2()
                        .child(highlighted(label, &self.query))
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(highlighted(hit.id.as_str().to_string(), &self.query)),
                        ),
                )
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    on_pick(&i, window, cx);
                })
        });

        let panel = div()
            .w(px(WIDTH))
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
                    .debug_selector(|| "search-input".into())
                    .child(Input::new(&self.input).small()),
            )
            .children(rows)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());

        div()
            .absolute()
            .top(px(TOP))
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .child(panel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Matches names and ids case-insensitively; blank queries find nothing.
    #[test]
    fn find_by_name_or_id() {
        let mut defs = crate::io::new_diagram();
        let start = ElementId::from("StartEvent_1");
        defs.elements.get_mut(&start).unwrap().name = Some("Order received".into());

        assert_eq!(find(&defs, "ORDER")[0].id, start);
        assert_eq!(find(&defs, "startevent")[0].id, start);
        assert!(find(&defs, "  ").is_empty());
        assert!(find(&defs, "nothing like it").is_empty());
    }

    /// The match range ignores case and query padding; blank or absent
    /// queries give none.
    #[test]
    fn match_range_ignores_case() {
        assert_eq!(match_range("Order received", " RECEIVED "), Some(6..14));
        assert_eq!(match_range("StartEvent_1", "event"), Some(5..10));
        assert_eq!(match_range("Order", ""), None);
        assert_eq!(match_range("Order", "ship"), None);
    }
}
