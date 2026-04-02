//! Framework-agnostic inline view tree and reconciler.
//!
//! Provides the data types and pure-function reconciliation algorithm
//! used by both ratatui (rat-inline) and dioxus (met-inline) backends
//! for inline scrollback rendering.
//!
//! The reconciler matches nodes by key (stable identity across reorders)
//! then by position + type (fast path for static layouts). Matched nodes
//! preserve their opaque state blob across rebuilds.

use std::any::{Any, TypeId};
use std::collections::HashMap;

// ── Types ────────────────────────────────────────────────────────────────────

/// A string key for stable node identity across rebuilds.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeKey(pub String);

impl<S: Into<String>> From<S> for NodeKey {
    fn from(s: S) -> Self {
        Self(s.into())
    }
}

/// A single node in the view tree.
///
/// Carries an optional key for reconciliation, a `TypeId` for
/// type-based positional matching, and an opaque state slot that
/// backends use to preserve widget state across rebuilds.
pub struct ViewNode {
    /// Optional key for stable identity across rebuilds.
    pub key: Option<NodeKey>,
    /// Type discriminant — used for positional matching when no key is set.
    pub type_tag: TypeId,
    /// Opaque state preserved across reconciliation. `None` for new nodes.
    pub state: Option<Box<dyn Any>>,
}

impl std::fmt::Debug for ViewNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewNode")
            .field("key", &self.key)
            .field("type_tag", &self.type_tag)
            .field("has_state", &self.state.is_some())
            .finish()
    }
}

/// A flat list of view nodes representing the current inline content.
#[derive(Debug, Default)]
pub struct ViewTree {
    pub nodes: Vec<ViewNode>,
}

impl ViewTree {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn push(&mut self, node: ViewNode) {
        self.nodes.push(node);
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

// ── Reconciliation ───────────────────────────────────────────────────────────

/// Result of reconciling an old tree against a new tree.
pub struct ReconcileResult {
    /// The reconciled nodes — new nodes with state carried forward from
    /// matched old nodes.
    pub nodes: Vec<ViewNode>,
}

/// Reconcile a new view tree against an old one.
///
/// Two-pass O(N) algorithm:
/// 1. Match keyed new nodes against keyed old nodes by key.
/// 2. Match remaining unkeyed new nodes by position + type_tag.
///
/// Matched nodes get their `state` transferred from the old node.
/// Unmatched new nodes keep `state: None`.
/// Unmatched old nodes are dropped.
///
/// This is a pure function with no side effects.
pub fn reconcile(mut old: Vec<ViewNode>, new: Vec<ViewNode>) -> ReconcileResult {
    // Build index of keyed old nodes: key -> position in old vec.
    let mut old_by_key: HashMap<NodeKey, usize> = HashMap::new();
    for (i, node) in old.iter().enumerate() {
        if let Some(ref key) = node.key {
            old_by_key.insert(key.clone(), i);
        }
    }

    // Track which old nodes have been consumed (by index).
    let mut old_consumed = vec![false; old.len()];

    // Pass 1: match keyed nodes.
    // We collect which old index each new node matched, if any.
    let mut matched_old: Vec<Option<usize>> = Vec::with_capacity(new.len());
    for new_node in &new {
        if let Some(ref key) = new_node.key {
            if let Some(&old_idx) = old_by_key.get(key) {
                if !old_consumed[old_idx] {
                    old_consumed[old_idx] = true;
                    matched_old.push(Some(old_idx));
                    continue;
                }
            }
        }
        matched_old.push(None);
    }

    // Pass 2: match remaining unkeyed nodes by position + type.
    // Build a list of unconsumed old nodes grouped by type for positional matching.
    // We iterate unconsumed old nodes in order, tracking position per type.
    let mut positional_queues: HashMap<TypeId, Vec<usize>> = HashMap::new();
    for (i, node) in old.iter().enumerate() {
        if !old_consumed[i] && node.key.is_none() {
            positional_queues
                .entry(node.type_tag)
                .or_default()
                .push(i);
        }
    }

    // For each unmatched new node (no key match), try positional match.
    for (new_idx, new_node) in new.iter().enumerate() {
        if matched_old[new_idx].is_some() {
            continue; // already matched by key
        }
        if new_node.key.is_some() {
            continue; // keyed but no match found — stays unmatched
        }
        if let Some(queue) = positional_queues.get_mut(&new_node.type_tag) {
            if let Some(old_idx) = queue.first().copied() {
                queue.remove(0);
                old_consumed[old_idx] = true;
                matched_old[new_idx] = Some(old_idx);
            }
        }
    }

    // Build result: transfer state from matched old nodes.
    let mut result_nodes: Vec<ViewNode> = Vec::with_capacity(new.len());
    for (new_idx, mut new_node) in new.into_iter().enumerate() {
        if let Some(old_idx) = matched_old[new_idx] {
            new_node.state = old[old_idx].state.take();
        }
        result_nodes.push(new_node);
    }

    ReconcileResult {
        nodes: result_nodes,
    }
}

// ── Commit tracking ──────────────────────────────────────────────────────────

/// Compute which nodes have been fully scrolled above the viewport.
///
/// Given node heights and a viewport height, returns indices of nodes
/// whose rows are entirely above the visible region. Assumes nodes are
/// stacked top-to-bottom starting from `scroll_offset`.
///
/// This is a pure function — the backend supplies the actual viewport
/// height from the terminal.
pub fn compute_commits(
    node_heights: &[u16],
    viewport_height: u16,
    scroll_offset: u16,
) -> Vec<usize> {
    let total_height: u16 = node_heights.iter().copied().sum();
    if total_height <= viewport_height {
        return Vec::new();
    }

    // Nodes above the viewport are those whose bottom edge is at or
    // above the scroll offset. In scrollback mode, scroll_offset
    // represents how many rows have scrolled above the viewport.
    let mut committed = Vec::new();
    let mut y = 0u16;
    for (i, &h) in node_heights.iter().enumerate() {
        let node_bottom = y.saturating_add(h);
        if node_bottom <= scroll_offset {
            committed.push(i);
        } else {
            break; // nodes are stacked, so once we're in the viewport, stop
        }
        y = node_bottom;
    }
    committed
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a ViewNode with a given type tag and optional key.
    fn node<T: 'static>(key: Option<&str>) -> ViewNode {
        ViewNode {
            key: key.map(|k| NodeKey(k.to_string())),
            type_tag: TypeId::of::<T>(),
            state: None,
        }
    }

    /// Helper: create a ViewNode with state.
    fn node_with_state<T: 'static>(key: Option<&str>, state: u64) -> ViewNode {
        ViewNode {
            key: key.map(|k| NodeKey(k.to_string())),
            type_tag: TypeId::of::<T>(),
            state: Some(Box::new(state)),
        }
    }

    // Dummy types for type_tag discrimination.
    struct TypeA;
    struct TypeB;

    // ── Reconciler tests ─────────────────────────────────────────────────

    #[test]
    fn keyed_nodes_preserve_state_on_reorder() {
        let old = vec![
            node_with_state::<TypeA>(Some("a"), 42),
            node_with_state::<TypeA>(Some("b"), 99),
        ];
        let new = vec![
            node::<TypeA>(Some("b")),
            node::<TypeA>(Some("a")),
        ];

        let result = reconcile(old, new);
        assert_eq!(result.nodes.len(), 2);

        // "b" was second in old (state=99), now first
        let b_state = result.nodes[0].state.as_ref().unwrap();
        assert_eq!(*b_state.downcast_ref::<u64>().unwrap(), 99);

        // "a" was first in old (state=42), now second
        let a_state = result.nodes[1].state.as_ref().unwrap();
        assert_eq!(*a_state.downcast_ref::<u64>().unwrap(), 42);
    }

    #[test]
    fn keyed_node_removed() {
        let old = vec![
            node_with_state::<TypeA>(Some("a"), 1),
            node_with_state::<TypeA>(Some("b"), 2),
        ];
        let new = vec![
            node::<TypeA>(Some("a")),
        ];

        let result = reconcile(old, new);
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].key.as_ref().unwrap().0, "a");
        let state = result.nodes[0].state.as_ref().unwrap();
        assert_eq!(*state.downcast_ref::<u64>().unwrap(), 1);
    }

    #[test]
    fn positional_match_by_type() {
        let old = vec![
            node_with_state::<TypeA>(None, 10),
            node_with_state::<TypeB>(None, 20),
        ];
        let new = vec![
            node::<TypeA>(None),
            node::<TypeB>(None),
        ];

        let result = reconcile(old, new);
        assert_eq!(result.nodes.len(), 2);
        assert_eq!(
            *result.nodes[0].state.as_ref().unwrap().downcast_ref::<u64>().unwrap(),
            10
        );
        assert_eq!(
            *result.nodes[1].state.as_ref().unwrap().downcast_ref::<u64>().unwrap(),
            20
        );
    }

    #[test]
    fn type_mismatch_at_position_creates_new_node() {
        let old = vec![
            node_with_state::<TypeA>(None, 10),
        ];
        let new = vec![
            node::<TypeB>(None),
        ];

        let result = reconcile(old, new);
        assert_eq!(result.nodes.len(), 1);
        // No state transfer — type mismatch.
        assert!(result.nodes[0].state.is_none());
    }

    #[test]
    fn appended_node_gets_fresh_state() {
        let old = vec![
            node_with_state::<TypeA>(Some("a"), 42),
        ];
        let new = vec![
            node::<TypeA>(Some("a")),
            node::<TypeA>(Some("b")),
        ];

        let result = reconcile(old, new);
        assert_eq!(result.nodes.len(), 2);
        // "a" preserves state.
        assert_eq!(
            *result.nodes[0].state.as_ref().unwrap().downcast_ref::<u64>().unwrap(),
            42
        );
        // "b" is new — no state.
        assert!(result.nodes[1].state.is_none());
    }

    #[test]
    fn reconcile_is_deterministic() {
        let mk_old = || vec![
            node_with_state::<TypeA>(Some("x"), 7),
            node_with_state::<TypeB>(None, 8),
        ];
        let mk_new = || vec![
            node::<TypeB>(None),
            node::<TypeA>(Some("x")),
        ];

        let r1 = reconcile(mk_old(), mk_new());
        let r2 = reconcile(mk_old(), mk_new());

        assert_eq!(r1.nodes.len(), r2.nodes.len());
        for (a, b) in r1.nodes.iter().zip(r2.nodes.iter()) {
            assert_eq!(a.key, b.key);
            assert_eq!(a.type_tag, b.type_tag);
            assert_eq!(a.state.is_some(), b.state.is_some());
        }
    }

    #[test]
    fn empty_old_tree() {
        let old = vec![];
        let new = vec![
            node::<TypeA>(Some("a")),
        ];

        let result = reconcile(old, new);
        assert_eq!(result.nodes.len(), 1);
        assert!(result.nodes[0].state.is_none());
    }

    #[test]
    fn empty_new_tree() {
        let old = vec![
            node_with_state::<TypeA>(Some("a"), 1),
        ];
        let new = vec![];

        let result = reconcile(old, new);
        assert!(result.nodes.is_empty());
    }

    // ── Commit tracking tests ────────────────────────────────────────────

    #[test]
    fn no_commits_when_content_fits() {
        let heights = [5, 5, 5];
        let commits = compute_commits(&heights, 20, 0);
        assert!(commits.is_empty());
    }

    #[test]
    fn commits_nodes_above_scroll_offset() {
        // 4 nodes of 5 rows each = 20 total.
        // viewport = 10, scroll_offset = 10 means first 10 rows scrolled off.
        let heights = [5, 5, 5, 5];
        let commits = compute_commits(&heights, 10, 10);
        assert_eq!(commits, vec![0, 1]);
    }

    #[test]
    fn partial_node_not_committed() {
        // Node 0 is 5 rows, node 1 is 5 rows. scroll_offset = 7.
        // Node 0 bottom = 5 <= 7 → committed.
        // Node 1 bottom = 10 > 7 → not committed (partially visible).
        let heights = [5, 5, 5];
        let commits = compute_commits(&heights, 8, 7);
        assert_eq!(commits, vec![0]);
    }

    #[test]
    fn no_commits_at_zero_scroll() {
        let heights = [5, 5, 5, 5];
        let commits = compute_commits(&heights, 10, 0);
        assert!(commits.is_empty());
    }

    #[test]
    fn all_nodes_committed() {
        let heights = [3, 3, 3];
        // total = 9, viewport = 5, scroll_offset = 9 → all above viewport.
        let commits = compute_commits(&heights, 5, 9);
        assert_eq!(commits, vec![0, 1, 2]);
    }

    #[test]
    fn empty_heights() {
        let heights: [u16; 0] = [];
        let commits = compute_commits(&heights, 10, 0);
        assert!(commits.is_empty());
    }
}
