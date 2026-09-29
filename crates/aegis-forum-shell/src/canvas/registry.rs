// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The canvas node registry (D74): every node, its actions and its
//! instruments, in focus order.
//!
//! The registry's order **is** the focus order. The keyboard walk moves
//! through it, and the accessibility export lists the canvas's children in it,
//! so the two cannot disagree (REQ-P05-09, REQ-P05-11). A node is admitted only
//! with a non-empty name: the model refuses the defect the tree check
//! (E16-4) exists to catch, and the check still runs over the exported tree,
//! because an exporter can drop what the model holds.

use thiserror::Error;

use super::geometry::WorldRect;

/// The most nodes the canvas holds.
pub const MAX_NODES: usize = 4_096;

/// The most actions one node or one instrument level carries.
pub const MAX_ACTIONS: usize = 8;

/// The deepest instrument nesting a node may carry: an instrument, a
/// surrogate inside it, a transcluded fragment inside that, and one more.
pub const MAX_INSTRUMENT_DEPTH: usize = 4;

/// The longest node or instrument name, in bytes.
pub const MAX_LABEL_BYTES: usize = 256;

/// The value bound an instrument's increment and decrement actions stay in.
pub const MAX_VALUE: i8 = 8;

/// The key a node is registered under; stable for the node's lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeKey(pub u32);

/// What a canvas node stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// An application surface placed on the canvas.
    Application,
    /// A document placed on the canvas.
    Document,
    /// A free-standing note.
    Note,
}

/// What an action does to its target's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ActionKind {
    /// Toggles the target's selection.
    Select,
    /// Toggles whether the target is pinned in place.
    Pin,
    /// Raises the target's value by one, up to [`MAX_VALUE`].
    Increment,
    /// Lowers the target's value by one, down to `-MAX_VALUE`.
    Decrement,
}

/// A keyboard binding for an action. A single-letter binding is not one of
/// them: single-character shortcuts are global commands that can be turned
/// off (REQ-P05-09), so an action must never depend on one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    /// The space bar.
    Space,
    /// Control and a character.
    Ctrl(char),
}

/// One action a node or an instrument level offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeAction {
    /// What the action does.
    pub kind: ActionKind,
    /// The keyboard binding, or `None` for a pointer-only action, which the
    /// keyboard walk refuses (E16-3's negative).
    pub keyboard: Option<Binding>,
    /// Whether a pointer can invoke the action.
    pub pointer: bool,
}

impl NodeAction {
    /// An action reachable from the keyboard and the pointer alike.
    #[must_use]
    pub const fn both(kind: ActionKind, keyboard: Binding) -> Self {
        Self {
            kind,
            keyboard: Some(keyboard),
            pointer: true,
        }
    }

    /// An action only a pointer can reach: a defect the walk refuses.
    #[must_use]
    pub const fn pointer_only(kind: ActionKind) -> Self {
        Self {
            kind,
            keyboard: None,
            pointer: true,
        }
    }
}

/// What an instrument level is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelKind {
    /// An instrument that replaces a modal dialog.
    Instrument,
    /// A surrogate nested inside an instrument.
    Surrogate,
    /// A fragment transcluded from another node.
    Fragment,
}

/// What Tab and Shift+Tab do while an instrument level has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabPolicy {
    /// They leave the canvas for the next or previous shell region.
    Leaves,
    /// The level consumes them: a focus trap, which the walk refuses
    /// (REQ-P05-10's negative).
    Swallows,
}

/// The state an action changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ActionState {
    /// Whether the target is selected.
    pub selected: bool,
    /// Whether the target is pinned.
    pub pinned: bool,
    /// The target's value, within `-MAX_VALUE..=MAX_VALUE`.
    pub value: i8,
}

impl ActionState {
    /// Applies one action.
    pub fn apply(&mut self, kind: ActionKind) {
        match kind {
            ActionKind::Select => self.selected = !self.selected,
            ActionKind::Pin => self.pinned = !self.pinned,
            ActionKind::Increment => self.value = self.value.saturating_add(1).min(MAX_VALUE),
            ActionKind::Decrement => {
                self.value = self.value.saturating_sub(1).max(MAX_VALUE.saturating_neg());
            }
        }
    }
}

/// One nesting level of a node's instrument.
#[derive(Debug, Clone, PartialEq)]
pub struct InstrumentLevel {
    /// The accessible name.
    pub label: String,
    /// What the level is.
    pub kind: LevelKind,
    /// The actions the level offers.
    pub actions: Vec<NodeAction>,
    /// What Tab does while the level has focus.
    pub tab: TabPolicy,
    /// The state the level's actions change.
    pub state: ActionState,
}

/// One node of the canvas.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasNode {
    /// The key the node is registered under.
    pub key: NodeKey,
    /// The accessible name.
    pub label: String,
    /// What the node stands for.
    pub kind: NodeKind,
    /// Where the node sits, in canvas units.
    pub rect: WorldRect,
    /// The actions the node offers.
    pub actions: Vec<NodeAction>,
    /// The node's instrument, outermost level first; empty for none.
    pub levels: Vec<InstrumentLevel>,
    /// The state the node's own actions change.
    pub state: ActionState,
}

/// Why a node or an instrument level was refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RegistryError {
    /// The canvas already holds [`MAX_NODES`] nodes.
    #[error("the canvas already holds {MAX_NODES} nodes")]
    Full,
    /// A name is empty, blank, or longer than [`MAX_LABEL_BYTES`].
    #[error("canvas node {key}: a name must be non-blank and at most {MAX_LABEL_BYTES} bytes")]
    Label {
        /// The node the name was for.
        key: u32,
    },
    /// A node or a level carries more than [`MAX_ACTIONS`] actions.
    #[error("canvas node {key}: more than {MAX_ACTIONS} actions")]
    Actions {
        /// The node concerned.
        key: u32,
    },
    /// An instrument nests deeper than [`MAX_INSTRUMENT_DEPTH`].
    #[error("canvas node {key}: an instrument nests deeper than {MAX_INSTRUMENT_DEPTH} levels")]
    Depth {
        /// The node concerned.
        key: u32,
    },
    /// A node is already registered under the key.
    #[error("canvas node {key} is already registered")]
    Duplicate {
        /// The key concerned.
        key: u32,
    },
    /// No node is registered under the key.
    #[error("no canvas node {key}")]
    Unknown {
        /// The key asked for.
        key: u32,
    },
}

/// Returns whether `label` is an admissible accessible name.
fn admissible(label: &str) -> bool {
    !label.trim().is_empty() && label.len() <= MAX_LABEL_BYTES
}

/// Checks a node's name, actions and instrument before it is admitted.
fn check_node(node: &CanvasNode) -> Result<(), RegistryError> {
    let key = node.key.0;
    if !admissible(&node.label) || node.levels.iter().any(|level| !admissible(&level.label)) {
        return Err(RegistryError::Label { key });
    }
    let too_many = node.actions.len() > MAX_ACTIONS
        || node
            .levels
            .iter()
            .any(|level| level.actions.len() > MAX_ACTIONS);
    if too_many {
        return Err(RegistryError::Actions { key });
    }
    if node.levels.len() > MAX_INSTRUMENT_DEPTH {
        return Err(RegistryError::Depth { key });
    }
    Ok(())
}

/// Every canvas node, in focus order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Registry {
    nodes: Vec<CanvasNode>,
}

impl Registry {
    /// An empty registry.
    #[must_use]
    pub const fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Appends a node at the end of the focus order.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::Full`] past [`MAX_NODES`],
    /// [`RegistryError::Duplicate`] for a key already registered, and
    /// [`RegistryError::Label`], [`RegistryError::Actions`] or
    /// [`RegistryError::Depth`] for a node that breaks a bound.
    pub fn push(&mut self, node: CanvasNode) -> Result<(), RegistryError> {
        if self.nodes.len() >= MAX_NODES {
            return Err(RegistryError::Full);
        }
        if self.get(node.key).is_some() {
            return Err(RegistryError::Duplicate { key: node.key.0 });
        }
        check_node(&node)?;
        self.nodes.push(node);
        Ok(())
    }

    /// The nodes, in focus order.
    #[must_use]
    pub fn nodes(&self) -> &[CanvasNode] {
        &self.nodes
    }

    /// The number of nodes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the canvas is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// The position of `key` in the focus order.
    #[must_use]
    pub fn position(&self, key: NodeKey) -> Option<usize> {
        self.nodes.iter().position(|node| node.key == key)
    }

    /// The node registered under `key`.
    #[must_use]
    pub fn get(&self, key: NodeKey) -> Option<&CanvasNode> {
        self.nodes.iter().find(|node| node.key == key)
    }

    /// The node registered under `key`, mutably.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::Unknown`] for a key no node carries.
    pub fn get_mut(&mut self, key: NodeKey) -> Result<&mut CanvasNode, RegistryError> {
        self.nodes
            .iter_mut()
            .find(|node| node.key == key)
            .ok_or(RegistryError::Unknown { key: key.0 })
    }

    /// The node at `position` in the focus order.
    #[must_use]
    pub fn at(&self, position: usize) -> Option<&CanvasNode> {
        self.nodes.get(position)
    }
}
