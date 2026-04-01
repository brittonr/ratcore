//! Generic tree data model.
//!
//! Defines the `TreeData` trait for top-down tree traversal and a `SimpleTree`
//! adapter for parent-pointer data. Visible row computation flattens the tree
//! into a list based on expand/collapse state.

use std::collections::{BTreeMap, BTreeSet};

/// Trait for tree data accessed top-down.
///
/// Consumers implement this for their data structure. The widget calls these
/// methods during visible-row computation and rendering.
pub trait TreeData {
    /// Number of root-level nodes.
    fn root_count(&self) -> usize;

    /// Node id of the root at the given index (0-based).
    fn root(&self, index: usize) -> usize;

    /// Number of direct children of `node`.
    fn child_count(&self, node: usize) -> usize;

    /// Node id of the child at `index` under `node`.
    fn child(&self, node: usize, index: usize) -> usize;

    /// Display label for the node.
    fn node_label(&self, node: usize) -> &str;

    /// Optional icon string rendered before the label.
    fn node_icon(&self, _node: usize) -> Option<&str> {
        None
    }

    /// Return the parent node id, if any. Returns `None` for root nodes.
    fn parent(&self, node: usize) -> Option<usize>;
}

/// A single row in the flattened visible-row list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleRow {
    /// Node id in the tree data.
    pub node_id: usize,
    /// Depth in the tree (0 for roots).
    pub depth: usize,
    /// Whether this node has children.
    pub has_children: bool,
    /// Whether this node is currently expanded.
    pub is_expanded: bool,
    /// Whether this node is the last sibling at its level.
    pub is_last_sibling: bool,
    /// Ancestor "last sibling" flags for drawing guide lines.
    /// `ancestors_last[i]` is true when the ancestor at depth `i` is the last
    /// sibling at its level (so the guide pipe should be blank, not │).
    pub ancestors_last: Vec<bool>,
}

/// Compute the flat list of visible rows by walking the tree top-down.
///
/// Nodes whose parent is collapsed are excluded. The walk is depth-first.
pub fn compute_visible_rows(data: &dyn TreeData, expanded: &BTreeSet<usize>) -> Vec<VisibleRow> {
    let mut rows = Vec::new();
    let root_count = data.root_count();
    for i in 0..root_count {
        let node = data.root(i);
        let is_last = i == root_count - 1;
        walk_node(data, expanded, node, 0, is_last, &[], &mut rows);
    }
    rows
}

fn walk_node(
    data: &dyn TreeData,
    expanded: &BTreeSet<usize>,
    node: usize,
    depth: usize,
    is_last_sibling: bool,
    ancestors_last: &[bool],
    rows: &mut Vec<VisibleRow>,
) {
    let child_count = data.child_count(node);
    let has_children = child_count > 0;
    let is_expanded = has_children && expanded.contains(&node);

    rows.push(VisibleRow {
        node_id: node,
        depth,
        has_children,
        is_expanded,
        is_last_sibling,
        ancestors_last: ancestors_last.to_vec(),
    });

    if is_expanded {
        let mut child_ancestors: Vec<bool> = ancestors_last.to_vec();
        child_ancestors.push(is_last_sibling);
        for ci in 0..child_count {
            let child = data.child(node, ci);
            let child_is_last = ci == child_count - 1;
            walk_node(data, expanded, child, depth + 1, child_is_last, &child_ancestors, rows);
        }
    }
}

// ── SimpleTree adapter ──────────────────────────────────────────────────────

/// A simple tree built from flat `(id, parent_id, label)` data.
///
/// Stores children per node in a `BTreeMap` for deterministic order.
pub struct SimpleTree {
    roots: Vec<usize>,
    children: BTreeMap<usize, Vec<usize>>,
    parents: BTreeMap<usize, usize>,
    labels: BTreeMap<usize, String>,
}

impl SimpleTree {
    /// Build a tree from `(id, parent_id, label)` tuples.
    ///
    /// Entries with `parent_id = None` become roots. Children are ordered by
    /// their position in the input vec.
    pub fn new(entries: Vec<(usize, Option<usize>, String)>) -> Self {
        let mut roots = Vec::new();
        let mut children: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        let mut parents = BTreeMap::new();
        let mut labels = BTreeMap::new();

        for (id, parent_id, label) in entries {
            labels.insert(id, label);
            match parent_id {
                Some(pid) => {
                    children.entry(pid).or_default().push(id);
                    parents.insert(id, pid);
                }
                None => {
                    roots.push(id);
                }
            }
        }

        Self { roots, children, parents, labels }
    }
}

impl TreeData for SimpleTree {
    fn root_count(&self) -> usize {
        self.roots.len()
    }

    fn root(&self, index: usize) -> usize {
        self.roots[index]
    }

    fn child_count(&self, node: usize) -> usize {
        self.children.get(&node).map(|v| v.len()).unwrap_or(0)
    }

    fn child(&self, node: usize, index: usize) -> usize {
        self.children[&node][index]
    }

    fn node_label(&self, node: usize) -> &str {
        &self.labels[&node]
    }

    fn parent(&self, node: usize) -> Option<usize> {
        self.parents.get(&node).copied()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tree() -> SimpleTree {
        // root(0)
        //   ├─ a(1)
        //   │  ├─ a1(3)
        //   │  └─ a2(4)
        //   └─ b(2)
        //      └─ b1(5)
        SimpleTree::new(vec![
            (0, None, "root".into()),
            (1, Some(0), "a".into()),
            (2, Some(0), "b".into()),
            (3, Some(1), "a1".into()),
            (4, Some(1), "a2".into()),
            (5, Some(2), "b1".into()),
        ])
    }

    #[test]
    fn simple_tree_roots() {
        let tree = sample_tree();
        assert_eq!(tree.root_count(), 1);
        assert_eq!(tree.root(0), 0);
    }

    #[test]
    fn simple_tree_children() {
        let tree = sample_tree();
        assert_eq!(tree.child_count(0), 2);
        assert_eq!(tree.child(0, 0), 1);
        assert_eq!(tree.child(0, 1), 2);
        assert_eq!(tree.child_count(1), 2);
        assert_eq!(tree.child_count(5), 0); // leaf
    }

    #[test]
    fn simple_tree_labels() {
        let tree = sample_tree();
        assert_eq!(tree.node_label(0), "root");
        assert_eq!(tree.node_label(3), "a1");
    }

    #[test]
    fn simple_tree_parent() {
        let tree = sample_tree();
        assert_eq!(tree.parent(0), None);
        assert_eq!(tree.parent(1), Some(0));
        assert_eq!(tree.parent(3), Some(1));
    }

    #[test]
    fn simple_tree_icon_default() {
        let tree = sample_tree();
        assert_eq!(tree.node_icon(0), None);
    }

    #[test]
    fn simple_tree_multiple_roots() {
        let tree = SimpleTree::new(vec![
            (0, None, "root-a".into()),
            (1, None, "root-b".into()),
            (2, Some(0), "child".into()),
        ]);
        assert_eq!(tree.root_count(), 2);
        assert_eq!(tree.root(0), 0);
        assert_eq!(tree.root(1), 1);
        assert_eq!(tree.node_label(0), "root-a");
        assert_eq!(tree.node_label(1), "root-b");
    }

    #[test]
    fn visible_rows_all_collapsed() {
        let tree = sample_tree();
        let expanded = BTreeSet::new();
        let rows = compute_visible_rows(&tree, &expanded);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].node_id, 0);
        assert_eq!(rows[0].depth, 0);
        assert!(rows[0].has_children);
        assert!(!rows[0].is_expanded);
        assert!(rows[0].is_last_sibling);
    }

    #[test]
    fn visible_rows_expand_root() {
        let tree = sample_tree();
        let expanded = BTreeSet::from([0]);
        let rows = compute_visible_rows(&tree, &expanded);

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].node_id, 0);
        assert!(rows[0].is_expanded);
        assert_eq!(rows[1].node_id, 1);
        assert_eq!(rows[1].depth, 1);
        assert!(!rows[1].is_last_sibling);
        assert!(rows[1].has_children);
        assert_eq!(rows[2].node_id, 2);
        assert_eq!(rows[2].depth, 1);
        assert!(rows[2].is_last_sibling);
    }

    #[test]
    fn visible_rows_nested_expand() {
        let tree = sample_tree();
        let expanded = BTreeSet::from([0, 1]);
        let rows = compute_visible_rows(&tree, &expanded);

        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0].node_id, 0);
        assert_eq!(rows[1].node_id, 1);
        assert!(rows[1].is_expanded);
        assert_eq!(rows[2].node_id, 3); // a1
        assert_eq!(rows[2].depth, 2);
        assert!(!rows[2].is_last_sibling);
        assert_eq!(rows[3].node_id, 4); // a2
        assert_eq!(rows[3].depth, 2);
        assert!(rows[3].is_last_sibling);
        assert_eq!(rows[4].node_id, 2); // b
        assert_eq!(rows[4].depth, 1);
    }

    #[test]
    fn visible_rows_collapse_hides_descendants() {
        let tree = sample_tree();

        let expanded_all = BTreeSet::from([0, 1, 2]);
        let rows_all = compute_visible_rows(&tree, &expanded_all);
        assert_eq!(rows_all.len(), 6);

        let expanded_partial = BTreeSet::from([0, 2]);
        let rows_partial = compute_visible_rows(&tree, &expanded_partial);
        assert_eq!(rows_partial.len(), 4);
        let ids: Vec<usize> = rows_partial.iter().map(|r| r.node_id).collect();
        assert_eq!(ids, vec![0, 1, 2, 5]);
    }

    #[test]
    fn visible_rows_multiple_roots() {
        let tree = SimpleTree::new(vec![
            (0, None, "r1".into()),
            (1, None, "r2".into()),
        ]);
        let rows = compute_visible_rows(&tree, &BTreeSet::new());
        assert_eq!(rows.len(), 2);
        assert!(!rows[0].is_last_sibling);
        assert!(rows[1].is_last_sibling);
    }

    #[test]
    fn visible_rows_empty_tree() {
        let tree = SimpleTree::new(vec![]);
        let rows = compute_visible_rows(&tree, &BTreeSet::new());
        assert!(rows.is_empty());
    }

    #[test]
    fn visible_rows_expand_leaf_is_noop() {
        let tree = sample_tree();
        let expanded = BTreeSet::from([0, 1, 3]);
        let rows = compute_visible_rows(&tree, &expanded);
        let expected = BTreeSet::from([0, 1]);
        let rows_expected = compute_visible_rows(&tree, &expected);
        assert_eq!(rows.len(), rows_expected.len());
    }

    #[test]
    fn ancestors_last_tracking() {
        let tree = sample_tree();
        let expanded = BTreeSet::from([0, 1]);
        let rows = compute_visible_rows(&tree, &expanded);

        assert!(rows[0].ancestors_last.is_empty());
        assert_eq!(rows[1].ancestors_last, vec![true]);
        assert_eq!(rows[2].ancestors_last, vec![true, false]);
        assert_eq!(rows[3].ancestors_last, vec![true, false]);
        assert_eq!(rows[4].ancestors_last, vec![true]);
    }
}
