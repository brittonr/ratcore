//! Helix-style leader key menu: types, builder, and state machine.
//!
//! Generic over the action type `A`. Press a leader key → see available
//! actions → press another key to execute or open a submenu. Escape or
//! unrecognized keys dismiss the menu.
//!
//! Items are contributed dynamically via [`MenuContributor`] with
//! priority-based conflict resolution. Multiple sources (builtins,
//! plugins, user config) can register to the same menu, and higher
//! priority wins on key collisions.
//!
//! This module is rendering-agnostic. Both TUI and web frontends
//! consume the same state machine.
//!
//! # Example
//!
//! ```
//! use ratcore::leaderkey::*;
//!
//! #[derive(Debug, Clone, PartialEq, Eq)]
//! enum Act { Save, Quit }
//!
//! struct Builtins;
//! impl MenuContributor<Act> for Builtins {
//!     fn menu_items(&self) -> Vec<MenuContribution<Act>> {
//!         vec![
//!             MenuContribution {
//!                 key: 's',
//!                 label: "save".into(),
//!                 action: LeaderAction::Action(Act::Save),
//!                 placement: MenuPlacement::Root,
//!                 priority: PRIORITY_BUILTIN,
//!                 source: "builtin".into(),
//!             },
//!             MenuContribution {
//!                 key: 'q',
//!                 label: "quit".into(),
//!                 action: LeaderAction::Action(Act::Quit),
//!                 placement: MenuPlacement::Root,
//!                 priority: PRIORITY_BUILTIN,
//!                 source: "builtin".into(),
//!             },
//!         ]
//!     }
//! }
//!
//! let hidden = std::collections::HashSet::new();
//! let (mut menu, conflicts) = build(&[&Builtins], &hidden);
//! assert!(conflicts.is_empty());
//! menu.open();
//! assert_eq!(menu.handle_char('s'), Some(LeaderAction::Action(Act::Save)));
//! ```

use std::collections::{HashMap, HashSet};

// ═══════════════════════════════════════════════════════════════════
// Priority constants
// ═══════════════════════════════════════════════════════════════════

/// Priority for built-in (compile-time) registrations.
pub const PRIORITY_BUILTIN: u16 = 0;

/// Priority for plugin registrations (loaded at runtime).
pub const PRIORITY_PLUGIN: u16 = 100;

/// Priority for user config overrides (highest, always wins).
pub const PRIORITY_USER: u16 = 200;

// ═══════════════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════════════

/// An action that a leader menu item can trigger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaderAction<A> {
    /// Dispatch a user-defined action.
    Action(A),
    /// Execute a command string (e.g. "/compact", ":write").
    Command(String),
    /// Open a named submenu.
    Submenu(String),
}

/// A single entry in the leader key menu.
#[derive(Debug, Clone)]
pub struct LeaderMenuItem<A> {
    /// Key to press (single char).
    pub key: char,
    /// Display label.
    pub label: String,
    /// What happens when selected.
    pub action: LeaderAction<A>,
}

/// A named menu level (root or submenu).
#[derive(Debug, Clone)]
pub struct LeaderMenuDef<A> {
    pub label: String,
    pub items: Vec<LeaderMenuItem<A>>,
}

/// Where a menu item should appear.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MenuPlacement {
    /// Top-level root menu.
    Root,
    /// Inside a named submenu (created if it doesn't exist).
    Submenu(String),
}

/// A single contribution to the leader menu from any source.
#[derive(Debug, Clone)]
pub struct MenuContribution<A> {
    /// Key to press (single char).
    pub key: char,
    /// Display label.
    pub label: String,
    /// What happens when selected.
    pub action: LeaderAction<A>,
    /// Where this item appears.
    pub placement: MenuPlacement,
    /// Priority for conflict resolution (higher wins).
    pub priority: u16,
    /// Source identifier for diagnostics ("builtin", plugin name, "config").
    pub source: String,
}

/// Anything that contributes items to the leader menu.
pub trait MenuContributor<A> {
    fn menu_items(&self) -> Vec<MenuContribution<A>>;
}

/// Set of `(key, placement)` pairs to exclude from the built menu.
pub type HiddenSet = HashSet<(char, MenuPlacement)>;

// ═══════════════════════════════════════════════════════════════════
// Conflicts
// ═══════════════════════════════════════════════════════════════════

/// A conflict detected during menu build.
///
/// When two sources register the same key in the same scope, the
/// higher-priority source wins and a `Conflict` is reported.
#[derive(Debug, Clone)]
pub struct Conflict {
    /// Which registry detected the conflict.
    pub registry: &'static str,
    /// What conflicted (key char, placement, etc.).
    pub key: String,
    /// Source that won.
    pub winner: String,
    /// Source that lost.
    pub loser: String,
}

// ═══════════════════════════════════════════════════════════════════
// State machine
// ═══════════════════════════════════════════════════════════════════

/// Input event for the leader menu state machine.
///
/// Renderers convert platform-specific key events into this enum
/// before calling [`LeaderMenu::handle_input`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuInput {
    /// A printable character (with or without Shift).
    Char(char),
    /// Escape key — back one level or close.
    Escape,
    /// Any other key — dismisses the menu.
    Other,
}

/// Leader key menu state and navigation.
///
/// Generic over `A`, the application's action type.
pub struct LeaderMenu<A> {
    /// Whether the overlay is visible.
    pub visible: bool,
    /// Stack of menu levels (root at bottom, current at top).
    stack: Vec<LeaderMenuDef<A>>,
    /// Breadcrumb labels for the title bar.
    breadcrumb: Vec<String>,
    /// All submenu definitions.
    submenus: Vec<LeaderMenuDef<A>>,
    /// The root menu definition.
    root: LeaderMenuDef<A>,
}

impl<A> LeaderMenu<A> {
    /// Create from pre-built parts (used by builders).
    fn from_parts(root: LeaderMenuDef<A>, submenus: Vec<LeaderMenuDef<A>>) -> Self {
        Self {
            visible: false,
            stack: Vec::new(),
            breadcrumb: Vec::new(),
            submenus,
            root,
        }
    }

    /// The root menu definition.
    pub fn root_def(&self) -> &LeaderMenuDef<A> {
        &self.root
    }

    /// All submenu definitions.
    pub fn submenu_defs(&self) -> &[LeaderMenuDef<A>] {
        &self.submenus
    }

    /// The currently displayed menu level (None if menu is closed).
    pub fn current(&self) -> Option<&LeaderMenuDef<A>> {
        self.stack.last()
    }

    /// Breadcrumb trail for the title bar.
    pub fn breadcrumb(&self) -> &[String] {
        &self.breadcrumb
    }

    /// Current submenu depth (0 = root, 1 = first submenu, etc.).
    pub fn depth(&self) -> usize {
        self.stack.len().saturating_sub(1)
    }

    /// Create from pre-built parts (public for platform wrappers and tests).
    pub fn test_from_parts(root: LeaderMenuDef<A>, submenus: Vec<LeaderMenuDef<A>>) -> Self {
        Self::from_parts(root, submenus)
    }

    /// Close the menu entirely.
    pub fn close(&mut self) {
        self.visible = false;
        self.stack.clear();
        self.breadcrumb.clear();
    }
}

impl<A: Clone> LeaderMenu<A> {
    /// Open the menu (shows root level).
    pub fn open(&mut self) {
        self.visible = true;
        self.stack.clear();
        self.breadcrumb.clear();
        self.stack.push(self.root.clone());
    }

    /// Handle a platform-neutral input event.
    ///
    /// Returns `Some(action)` if an action should be dispatched,
    /// `None` if the key was consumed internally (submenu nav, close).
    pub fn handle_input(&mut self, input: MenuInput) -> Option<LeaderAction<A>> {
        if !self.visible {
            return None;
        }

        match input {
            MenuInput::Escape => {
                if self.stack.len() > 1 {
                    self.stack.pop();
                    self.breadcrumb.pop();
                } else {
                    self.close();
                }
                None
            }
            MenuInput::Char(ch) => self.handle_char_inner(ch),
            MenuInput::Other => {
                self.close();
                None
            }
        }
    }

    /// Convenience: handle a single character directly.
    ///
    /// Equivalent to `handle_input(MenuInput::Char(ch))`.
    pub fn handle_char(&mut self, ch: char) -> Option<LeaderAction<A>> {
        self.handle_input(MenuInput::Char(ch))
    }

    fn handle_char_inner(&mut self, ch: char) -> Option<LeaderAction<A>> {
        let current = match self.current() {
            Some(m) => m,
            None => {
                self.close();
                return None;
            }
        };

        if let Some(item) = current.items.iter().find(|i| i.key == ch) {
            match &item.action {
                LeaderAction::Submenu(name) => {
                    if let Some(sub) = self.submenus.iter().find(|s| s.label == *name) {
                        self.breadcrumb.push(item.label.clone());
                        self.stack.push(sub.clone());
                    } else {
                        self.close();
                    }
                    None
                }
                action => {
                    let result = action.clone();
                    self.close();
                    Some(result)
                }
            }
        } else {
            // Unknown key → dismiss.
            self.close();
            None
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Builder
// ═══════════════════════════════════════════════════════════════════

/// Result of building a leader menu: the menu and any conflicts.
pub type BuildResult<A> = (LeaderMenu<A>, Vec<Conflict>);

/// Build a leader menu from contributors.
///
/// Collects all [`MenuContribution`] items, deduplicates by `(key, placement)`
/// with highest priority winning, removes hidden entries, and assembles the
/// menu tree.
pub fn build<A: Clone>(
    contributors: &[&dyn MenuContributor<A>],
    hidden: &HiddenSet,
) -> BuildResult<A> {
    let items = contributors.iter().flat_map(|c| c.menu_items()).collect();
    build_from_items(items, hidden)
}

/// Build from a pre-collected list of contributions.
pub fn build_from_items<A: Clone>(
    mut all_items: Vec<MenuContribution<A>>,
    hidden: &HiddenSet,
) -> BuildResult<A> {
    let mut conflicts = Vec::new();

    // Sort by priority (lowest first so highest overwrites).
    all_items.sort_by_key(|i| i.priority);

    // Deduplicate by (key, placement) — last writer wins.
    let mut seen: HashMap<(char, MenuPlacement), MenuContribution<A>> = HashMap::new();
    for item in all_items {
        let slot = (item.key, item.placement.clone());
        if let Some(existing) = seen.get(&slot) {
            conflicts.push(Conflict {
                registry: "leader_menu",
                key: format!("'{}' in {:?}", item.key, item.placement),
                winner: item.source.clone(),
                loser: existing.source.clone(),
            });
        }
        seen.insert(slot, item);
    }

    // Remove hidden entries.
    for h in hidden {
        seen.remove(h);
    }

    // Group by placement.
    let mut root_items: Vec<MenuContribution<A>> = Vec::new();
    let mut submenu_items: HashMap<String, Vec<MenuContribution<A>>> = HashMap::new();

    for ((_, placement), item) in seen {
        match placement {
            MenuPlacement::Root => root_items.push(item),
            MenuPlacement::Submenu(ref name) => {
                submenu_items.entry(name.clone()).or_default().push(item);
            }
        }
    }

    // Build submenu defs (sorted by key for stable ordering).
    let mut submenus: Vec<LeaderMenuDef<A>> = Vec::new();
    for (name, mut items) in submenu_items {
        items.sort_by_key(|i| i.key);
        submenus.push(LeaderMenuDef {
            label: name,
            items: items
                .into_iter()
                .map(|c| LeaderMenuItem {
                    key: c.key,
                    label: c.label,
                    action: c.action,
                })
                .collect(),
        });
    }

    // Build root def (sorted by key).
    root_items.sort_by_key(|i| i.key);
    let root = LeaderMenuDef {
        label: "Leader".into(),
        items: root_items
            .into_iter()
            .map(|c| LeaderMenuItem {
                key: c.key,
                label: c.label,
                action: c.action,
            })
            .collect(),
    };

    let menu = LeaderMenu::from_parts(root, submenus);
    (menu, conflicts)
}

// ═══════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Act {
        Save,
        Open,
    }

    struct TestContributor {
        items: Vec<MenuContribution<Act>>,
    }

    impl MenuContributor<Act> for TestContributor {
        fn menu_items(&self) -> Vec<MenuContribution<Act>> {
            self.items.clone()
        }
    }

    fn contrib(key: char, label: &str, action: LeaderAction<Act>) -> MenuContribution<Act> {
        MenuContribution {
            key,
            label: label.into(),
            action,
            placement: MenuPlacement::Root,
            priority: PRIORITY_BUILTIN,
            source: "test".into(),
        }
    }

    fn contrib_in(
        key: char,
        label: &str,
        action: LeaderAction<Act>,
        placement: MenuPlacement,
        priority: u16,
        source: &str,
    ) -> MenuContribution<Act> {
        MenuContribution {
            key,
            label: label.into(),
            action,
            placement,
            priority,
            source: source.into(),
        }
    }

    fn empty_hidden() -> HiddenSet {
        HashSet::new()
    }

    // -- state machine tests --

    fn make_menu() -> LeaderMenu<Act> {
        let root = LeaderMenuDef {
            label: "Leader".into(),
            items: vec![
                LeaderMenuItem { key: 's', label: "save".into(), action: LeaderAction::Action(Act::Save) },
                LeaderMenuItem { key: 'o', label: "open".into(), action: LeaderAction::Action(Act::Open) },
                LeaderMenuItem { key: 'x', label: "extra".into(), action: LeaderAction::Submenu("extra".into()) },
            ],
        };
        let submenus = vec![LeaderMenuDef {
            label: "extra".into(),
            items: vec![LeaderMenuItem {
                key: 'a', label: "alpha".into(), action: LeaderAction::Command("/alpha".into()),
            }],
        }];
        LeaderMenu::from_parts(root, submenus)
    }

    #[test]
    fn opens_and_closes() {
        let mut m = make_menu();
        assert!(!m.visible);
        m.open();
        assert!(m.visible);
        m.close();
        assert!(!m.visible);
    }

    #[test]
    fn escape_closes_root() {
        let mut m = make_menu();
        m.open();
        let r = m.handle_input(MenuInput::Escape);
        assert!(r.is_none());
        assert!(!m.visible);
    }

    #[test]
    fn unknown_key_dismisses() {
        let mut m = make_menu();
        m.open();
        let r = m.handle_char('z');
        assert!(r.is_none());
        assert!(!m.visible);
    }

    #[test]
    fn other_input_dismisses() {
        let mut m = make_menu();
        m.open();
        let r = m.handle_input(MenuInput::Other);
        assert!(r.is_none());
        assert!(!m.visible);
    }

    #[test]
    fn direct_action_returns_and_closes() {
        let mut m = make_menu();
        m.open();
        let r = m.handle_char('s');
        assert_eq!(r, Some(LeaderAction::Action(Act::Save)));
        assert!(!m.visible);
    }

    #[test]
    fn submenu_navigation() {
        let mut m = make_menu();
        m.open();

        let r = m.handle_char('x');
        assert!(r.is_none());
        assert!(m.visible);
        assert_eq!(m.depth(), 1);

        let r = m.handle_char('a');
        assert_eq!(r, Some(LeaderAction::Command("/alpha".into())));
        assert!(!m.visible);
    }

    #[test]
    fn escape_goes_back_from_submenu() {
        let mut m = make_menu();
        m.open();
        m.handle_char('x');
        assert_eq!(m.depth(), 1);

        m.handle_input(MenuInput::Escape);
        assert!(m.visible);
        assert_eq!(m.depth(), 0);

        m.handle_input(MenuInput::Escape);
        assert!(!m.visible);
    }

    #[test]
    fn not_visible_returns_none() {
        let mut m = make_menu();
        let r = m.handle_char('s');
        assert!(r.is_none());
    }

    #[test]
    fn breadcrumb_tracks_navigation() {
        let mut m = make_menu();
        m.open();
        assert!(m.breadcrumb().is_empty());

        m.handle_char('x');
        assert_eq!(m.breadcrumb(), &["extra"]);

        m.handle_input(MenuInput::Escape);
        assert!(m.breadcrumb().is_empty());
    }

    // -- builder tests --

    #[test]
    fn single_contributor() {
        let c = TestContributor {
            items: vec![
                contrib('a', "alpha", LeaderAction::Command("/alpha".into())),
                contrib('b', "beta", LeaderAction::Command("/beta".into())),
            ],
        };
        let (menu, conflicts) = build(&[&c], &empty_hidden());
        assert!(conflicts.is_empty());
        assert_eq!(menu.root_def().items.len(), 2);
        assert_eq!(menu.root_def().items[0].key, 'a');
        assert_eq!(menu.root_def().items[1].key, 'b');
    }

    #[test]
    fn higher_priority_wins() {
        let lo = TestContributor {
            items: vec![contrib_in(
                'x', "lo", LeaderAction::Action(Act::Save),
                MenuPlacement::Root, PRIORITY_BUILTIN, "builtin",
            )],
        };
        let hi = TestContributor {
            items: vec![contrib_in(
                'x', "hi", LeaderAction::Action(Act::Open),
                MenuPlacement::Root, PRIORITY_PLUGIN, "plugin",
            )],
        };
        let (menu, conflicts) = build(&[&lo, &hi], &empty_hidden());
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].winner, "plugin");
        assert_eq!(menu.root_def().items[0].label, "hi");
    }

    #[test]
    fn hidden_excluded() {
        let c = TestContributor {
            items: vec![
                contrib('a', "keep", LeaderAction::Action(Act::Save)),
                contrib('b', "hide", LeaderAction::Action(Act::Open)),
            ],
        };
        let mut hidden = HashSet::new();
        hidden.insert(('b', MenuPlacement::Root));
        let (menu, _) = build(&[&c], &hidden);
        assert_eq!(menu.root_def().items.len(), 1);
        assert_eq!(menu.root_def().items[0].key, 'a');
    }

    #[test]
    fn submenu_auto_creation() {
        let c = TestContributor {
            items: vec![
                contrib_in(
                    'p', "plugins", LeaderAction::Submenu("plugins".into()),
                    MenuPlacement::Root, PRIORITY_BUILTIN, "test",
                ),
                contrib_in(
                    'c', "calendar", LeaderAction::Command("/cal".into()),
                    MenuPlacement::Submenu("plugins".into()), PRIORITY_PLUGIN, "calendar",
                ),
            ],
        };
        let (menu, _) = build(&[&c], &empty_hidden());
        assert_eq!(menu.root_def().items.len(), 1);
        let subs = menu.submenu_defs();
        let plugins = subs.iter().find(|s| s.label == "plugins").unwrap();
        assert_eq!(plugins.items.len(), 1);
        assert_eq!(plugins.items[0].key, 'c');
    }

    #[test]
    fn same_key_different_placement_no_conflict() {
        let c = TestContributor {
            items: vec![
                contrib_in(
                    'a', "root-a", LeaderAction::Action(Act::Save),
                    MenuPlacement::Root, PRIORITY_BUILTIN, "test",
                ),
                contrib_in(
                    'a', "sub-a", LeaderAction::Action(Act::Open),
                    MenuPlacement::Submenu("foo".into()), PRIORITY_BUILTIN, "test",
                ),
            ],
        };
        let (_, conflicts) = build(&[&c], &empty_hidden());
        assert!(conflicts.is_empty());
    }

    #[test]
    fn empty_build() {
        let (menu, conflicts) = build::<Act>(&[], &empty_hidden());
        assert!(conflicts.is_empty());
        assert!(menu.root_def().items.is_empty());
    }
}
