//! Folder tree of `.bpmn` files: directories first, then names, hidden
//! entries skipped; children of a directory load on demand.

use std::path::{Path, PathBuf};

/// One directory or `.bpmn` file in the browsed tree.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    /// Loaded children; `None` until [`Node::expand`] runs (always `None`
    /// for files).
    pub children: Option<Vec<Node>>,
}

impl Node {
    /// Loads this directory's direct children (no-op for files).
    pub fn expand(&mut self) {
        if self.is_dir {
            self.children = Some(list(&self.path));
        }
    }
}

/// Finds the node at `path` inside `node`, mutably.
pub fn find_mut<'a>(node: &'a mut Node, path: &Path) -> Option<&'a mut Node> {
    if node.path == path {
        return Some(node);
    }

    node.children
        .iter_mut()
        .flatten()
        .find_map(|child| find_mut(child, path))
}

/// Returns the node for `root` with its direct children loaded.
pub fn scan(root: &Path) -> Node {
    let mut node = Node {
        name: root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.display().to_string()),
        path: root.to_path_buf(),
        is_dir: true,
        children: None,
    };
    node.expand();

    node
}

/// Lists the visible subdirectories and `.bpmn` files of `dir`, directories
/// first then by name; an unreadable directory yields no children.
fn list(dir: &Path) -> Vec<Node> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut nodes: Vec<Node> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                return None;
            }

            let path = entry.path();
            let is_dir = path.is_dir();
            let is_bpmn = path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("bpmn"));
            (is_dir || is_bpmn).then_some(Node {
                name,
                path,
                is_dir,
                children: None,
            })
        })
        .collect();
    nodes.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));

    nodes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a fresh unique fixture directory.
    fn fixture(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mangue-tree-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Names of the loaded children of `node`.
    fn names(node: &Node) -> Vec<&str> {
        node.children
            .as_ref()
            .unwrap()
            .iter()
            .map(|n| n.name.as_str())
            .collect()
    }

    /// Directories come first, then files, each in name order.
    #[test]
    fn scan_orders_dirs_first_then_by_name() {
        let dir = fixture("order");
        std::fs::create_dir(dir.join("zeta")).unwrap();
        std::fs::create_dir(dir.join("alpha")).unwrap();
        std::fs::write(dir.join("b.bpmn"), "").unwrap();
        std::fs::write(dir.join("a.bpmn"), "").unwrap();

        let root = scan(&dir);

        assert_eq!(names(&root), ["alpha", "zeta", "a.bpmn", "b.bpmn"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Only `.bpmn` files (any case) are listed; hidden entries are skipped.
    #[test]
    fn scan_filters_extension_and_hidden() {
        let dir = fixture("filter");
        std::fs::write(dir.join("a.bpmn"), "").unwrap();
        std::fs::write(dir.join("B.BPMN"), "").unwrap();
        std::fs::write(dir.join("c.txt"), "").unwrap();
        std::fs::write(dir.join(".hidden.bpmn"), "").unwrap();
        std::fs::create_dir(dir.join(".git")).unwrap();

        let root = scan(&dir);

        assert_eq!(names(&root), ["B.BPMN", "a.bpmn"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Subdirectory children stay unloaded until expanded.
    #[test]
    fn children_load_lazily_on_expand() {
        let dir = fixture("lazy");
        std::fs::create_dir(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub").join("x.bpmn"), "").unwrap();

        let mut root = scan(&dir);
        let sub = &mut root.children.as_mut().unwrap()[0];
        assert!(sub.is_dir && sub.children.is_none());

        sub.expand();

        assert_eq!(names(sub), ["x.bpmn"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
