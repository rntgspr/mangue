//! `Sidebar`: the folder browser on the left of the canvas, a gpui-component
//! `Tree` of directories and `.bpmn` files with a refresh button.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::{App, Entity, MouseButton, SharedString, Window, div, prelude::*, px, svg};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::list::ListItem;
use gpui_component::tree::{Tree, TreeEntry, TreeItem, TreeState};
use gpui_component::{ActiveTheme, Sizable};

use crate::files::tree::Node;

/// Callback taking the clicked path.
type PathHandler = Rc<dyn Fn(&Path, &mut Window, &mut App)>;

/// Callback for the refresh button.
type RefreshHandler = Rc<dyn Fn(&(), &mut Window, &mut App)>;

/// Width of the sidebar in pixels.
pub const WIDTH: f32 = 220.0;

/// Label of the stand-in child of a directory whose children are not loaded
/// yet or that holds nothing; the kit only treats items with children as
/// folders.
const PLACEHOLDER: &str = "(empty)";

/// Kit tree items for the children of `root`: directories carry their
/// loaded children (or a disabled placeholder) and are expanded when listed
/// in `expanded`; item ids are the node paths.
pub fn items(root: &Node, expanded: &HashSet<PathBuf>) -> Vec<TreeItem> {
    root.children
        .iter()
        .flatten()
        .map(|node| item(node, expanded))
        .collect()
}

/// Kit tree item for `node` and its loaded subtree.
fn item(node: &Node, expanded: &HashSet<PathBuf>) -> TreeItem {
    let item = TreeItem::new(path_id(&node.path), node.name.clone());
    if !node.is_dir {
        return item;
    }

    let mut children = items(node, expanded);
    if children.is_empty() {
        let id = format!("{}/{PLACEHOLDER}", path_id(&node.path));
        children.push(TreeItem::new(id, PLACEHOLDER).disabled(true));
    }

    item.children(children)
        .expanded(expanded.contains(&node.path))
}

/// Tree item id of `path`.
fn path_id(path: &Path) -> SharedString {
    path.to_string_lossy().into_owned().into()
}

/// Folder header and the kit tree held by `state`, with `active` marked.
#[derive(IntoElement)]
pub struct Sidebar {
    name: String,
    state: Entity<TreeState>,
    active: Option<PathBuf>,
    on_pick: PathHandler,
    on_refresh: RefreshHandler,
}

impl Sidebar {
    /// Builds the sidebar titled `name` over the tree `state`; file clicks go
    /// to `on_pick` and the refresh button to `on_refresh`. Directory
    /// expansion is reported by `state` as `TreeEvent`s.
    pub fn new(
        name: String,
        state: Entity<TreeState>,
        active: Option<PathBuf>,
        on_pick: impl Fn(&Path, &mut Window, &mut App) + 'static,
        on_refresh: impl Fn(&(), &mut Window, &mut App) + 'static,
    ) -> Self {
        Sidebar {
            name,
            state,
            active,
            on_pick: Rc::new(on_pick),
            on_refresh: Rc::new(on_refresh),
        }
    }
}

/// One tree row: disclosure chevron for directories, indentation by depth,
/// the active file in the primary color, file rows opening on click.
fn row(entry: &TreeEntry, active: Option<&Path>, on_pick: &PathHandler, cx: &App) -> ListItem {
    let item = entry.item();
    let path = PathBuf::from(item.id.as_ref());
    let selector = format!("sidebar-row-{}", item.label);

    let chevron = entry.is_folder().then(|| {
        svg()
            .path(if entry.is_expanded() {
                "icons/chevron-down.svg"
            } else {
                "icons/chevron-right.svg"
            })
            .size(px(14.))
            .flex_none()
            .text_color(cx.theme().foreground)
    });
    let indent = 14. * entry.depth() as f32 + if entry.is_folder() { 0. } else { 18. };

    let row = ListItem::new(SharedString::from(selector.clone()))
        .debug_selector(move || selector)
        .pl(px(8. + indent))
        .text_sm()
        .when(active == Some(path.as_path()), |r| {
            r.text_color(cx.theme().primary)
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .children(chevron)
                .child(item.label.clone()),
        );
    if entry.is_folder() || entry.is_disabled() {
        return row;
    }

    let on_pick = on_pick.clone();
    row.on_click(move |_, window, cx| on_pick(&path, window, cx))
}

impl RenderOnce for Sidebar {
    /// Renders the header (folder name and refresh) above the kit tree.
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let on_refresh = self.on_refresh.clone();
        let refresh = Button::new("sidebar-refresh")
            .debug_selector(|| "sidebar-refresh".into())
            .ghost()
            .xsmall()
            .label("Refresh")
            .on_click(move |_, window, cx| on_refresh(&(), window, cx));

        let active = self.active;
        let on_pick = self.on_pick;
        let tree = Tree::new(&self.state, move |_, entry, _, _, cx| {
            row(entry, active.as_deref(), &on_pick, cx)
        });

        div()
            .flex_none()
            .w(px(WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .bg(cx.theme().sidebar)
            .text_color(cx.theme().sidebar_foreground)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(28.))
                    .px_2()
                    .text_sm()
                    .border_b_1()
                    .border_color(cx.theme().sidebar_border)
                    .child(div().overflow_hidden().child(self.name))
                    .child(refresh),
            )
            .child(div().id("sidebar-rows").flex_1().min_h_0().child(tree))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
    }
}
