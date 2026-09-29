// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The accessibility tree, exported as an `accesskit::TreeUpdate`, and the
//! checks run over it (D74, D101; E16-3, E16-4).
//!
//! [`export`] builds the whole tree from the [`ShellState`] with no toolkit
//! and no platform adapter: a window holding the status bar, the canvas and
//! the pending decisions, in Tab order. **Every** canvas node is exported, in
//! focus order, with its role, name and screen bounds, whatever the camera
//! culls (REQ-P05-11); the export never calls the cull index. A culled node's
//! bounds lie outside the viewport, which is how an adapter tells it is
//! off-screen.
//!
//! The canvas node deliberately does not set `clips_children`. In
//! `accesskit_consumer` 0.39.1, the release that pairs with accesskit 0.25,
//! `common_filter` (`src/filters.rs`) excludes the subtree of a child whose
//! bounds do not intersect a clipping parent's, unless a neighbouring sibling
//! is inside them; a clipping canvas would therefore hide most culled nodes
//! from AT-SPI, which is what REQ-P05-11 forbids. [`check_canvas_export`]
//! refuses a clipping canvas for that reason, and the same filter's exclusion
//! of hidden nodes and of `GenericContainer` is why [`check_tree`] refuses
//! both.
//!
//! [`check_tree`] is E16-4's tree check: every node reachable from the root
//! exactly once, every node with a role and a non-blank name, none hidden,
//! focus on a node of the tree, and an empty canvas exporting a labelled
//! empty state. It is narrower than the axe-core scan it replaces (ADR-0004):
//! it checks a role and a name, not the WCAG 2.2 AA rule set.

use std::collections::{BTreeMap, BTreeSet};

use accesskit::{Action, Node, NodeId, Rect, Role, TreeId, TreeInfo, TreeUpdate};
use serde::Serialize;
use thiserror::Error;

use aegis_justitia::DecisionRequest;
use aegis_tellus::CarbonTelemetry;

use crate::canvas::camera::ScreenRect;
use crate::canvas::registry::{
    ActionKind, CanvasNode, InstrumentLevel, LevelKind, NodeKey, NodeKind,
};
use crate::canvas::{Canvas, Target};
use crate::focus::{Focus, Region};
use crate::state::ShellState;

/// The window: the tree's root.
pub const ROOT_ID: NodeId = NodeId(1);
/// The status bar region.
pub const STATUS_BAR_ID: NodeId = NodeId(2);
/// The carbon telemetry readout inside the status bar.
pub const TELEMETRY_ID: NodeId = NodeId(3);
/// The canvas region.
pub const CANVAS_ID: NodeId = NodeId(4);
/// The empty-state label an empty canvas exports.
pub const EMPTY_STATE_ID: NodeId = NodeId(5);
/// The pending decisions region.
pub const DECISIONS_ID: NodeId = NodeId(6);

/// The name the empty canvas's empty-state label carries.
pub const EMPTY_STATE_NAME: &str = "The canvas is empty";

/// The status bar's height in screen pixels, above the canvas.
pub const STATUS_BAR_HEIGHT: f64 = 32.0;

/// The decisions region's height in screen pixels, below the canvas.
pub const DECISIONS_HEIGHT: f64 = 160.0;

/// The high bits that separate the id ranges of canvas nodes, instrument
/// levels and decision requests.
const NODE_TAG: u64 = 1 << 40;
const LEVEL_TAG: u64 = 2 << 40;
const DECISION_TAG: u64 = 3 << 40;
const TAG_SHIFT: u32 = 40;
const LEVEL_SLOTS: u64 = 8;

/// The tree id of canvas node `key`.
#[must_use]
pub fn node_id(key: NodeKey) -> NodeId {
    NodeId(NODE_TAG | u64::from(key.0))
}

/// The tree id of instrument level `depth` of canvas node `key`.
#[must_use]
pub fn level_id(key: NodeKey, depth: usize) -> NodeId {
    let slot = u64::try_from(depth).unwrap_or(u64::MAX) % LEVEL_SLOTS;
    NodeId(LEVEL_TAG | u64::from(key.0).saturating_mul(LEVEL_SLOTS) | slot)
}

/// The tree id of pending decision request `index`.
#[must_use]
pub fn decision_id(index: usize) -> NodeId {
    NodeId(DECISION_TAG | u64::try_from(index).unwrap_or(0))
}

/// Names a tree node for a failure message: what it stands for, not only its
/// number.
#[must_use]
pub fn describe(id: &NodeId) -> String {
    let low = id.0 & (NODE_TAG - 1);
    match id.0.checked_shr(TAG_SHIFT) {
        Some(1) => format!("canvas node {low} (#{})", id.0),
        Some(2) => format!(
            "instrument level {} of canvas node {} (#{})",
            low % LEVEL_SLOTS,
            low / LEVEL_SLOTS,
            id.0
        ),
        Some(3) => format!("pending decision {low} (#{})", id.0),
        _ => format!("shell node #{}", id.0),
    }
}

/// Converts a screen rectangle into the tree's bounds, shifted by the
/// region's vertical offset.
fn bounds(rect: &ScreenRect, top: f64) -> Rect {
    Rect {
        x0: rect.x0,
        y0: rect.y0 + top,
        x1: rect.x1,
        y1: rect.y1 + top,
    }
}

/// A node with a role, a name and bounds.
fn labelled(role: Role, name: &str, rect: Rect) -> Node {
    let mut node = Node::new(role);
    node.set_label(name);
    node.set_bounds(rect);
    node
}

/// The role a canvas node's kind maps to.
const fn role_for(kind: NodeKind) -> Role {
    match kind {
        NodeKind::Application => Role::Group,
        NodeKind::Document => Role::Document,
        NodeKind::Note => Role::Note,
    }
}

/// Adds the tree actions a list of model actions offers.
fn add_actions(node: &mut Node, actions: &[crate::canvas::registry::NodeAction]) {
    node.add_action(Action::Focus);
    for action in actions {
        match action.kind {
            ActionKind::Select => node.add_action(Action::Click),
            ActionKind::Increment => node.add_action(Action::Increment),
            ActionKind::Decrement => node.add_action(Action::Decrement),
            ActionKind::Pin => {}
        }
    }
}

/// The role description of an instrument level.
const fn level_description(kind: LevelKind) -> &'static str {
    match kind {
        LevelKind::Instrument => "instrument",
        LevelKind::Surrogate => "surrogate",
        LevelKind::Fragment => "transcluded fragment",
    }
}

/// How many levels of `node`'s instrument are open: those at or above the
/// focused one.
fn open_levels(focus: Focus, key: NodeKey) -> usize {
    match focus {
        Focus::Target(Target::Level(owner, depth)) if owner == key => depth.saturating_add(1),
        _ => 0,
    }
}

/// Exports one canvas node and its open instrument levels.
fn export_node(out: &mut Vec<(NodeId, Node)>, canvas: &Canvas, found: &CanvasNode, open: usize) {
    let rect = bounds(&canvas.camera().to_screen(&found.rect), STATUS_BAR_HEIGHT);
    let mut node = labelled(role_for(found.kind), &found.label, rect);
    add_actions(&mut node, &found.actions);
    if !found.levels.is_empty() {
        node.add_action(Action::Expand);
    }
    if open > 0 {
        node.set_children(vec![level_id(found.key, 0)]);
    }
    out.push((node_id(found.key), node));
    let levels = found.levels.iter().take(open).enumerate();
    for (depth, level) in levels {
        let next =
            (depth.saturating_add(1) < open).then(|| level_id(found.key, depth.saturating_add(1)));
        out.push((level_id(found.key, depth), level_node(level, rect, next)));
    }
}

/// One open instrument level.
fn level_node(level: &InstrumentLevel, rect: Rect, child: Option<NodeId>) -> Node {
    let mut node = labelled(Role::Group, &level.label, rect);
    node.set_role_description(level_description(level.kind));
    add_actions(&mut node, &level.actions);
    if let Some(child) = child {
        node.set_children(vec![child]);
    }
    node
}

/// Exports the canvas region: every node in focus order, or the labelled
/// empty state.
fn export_canvas(out: &mut Vec<(NodeId, Node)>, state: &ShellState, width: f64, height: f64) {
    let canvas = &state.canvas;
    let area = Rect {
        x0: 0.0,
        y0: STATUS_BAR_HEIGHT,
        x1: width,
        y1: STATUS_BAR_HEIGHT + height,
    };
    let mut region = labelled(Role::Canvas, "Forum canvas", area);
    let nodes = canvas.registry().nodes();
    if nodes.is_empty() {
        region.set_children(vec![EMPTY_STATE_ID]);
        out.push((CANVAS_ID, region));
        out.push((
            EMPTY_STATE_ID,
            labelled(Role::Label, EMPTY_STATE_NAME, area),
        ));
        return;
    }
    region.set_children(
        nodes
            .iter()
            .map(|node| node_id(node.key))
            .collect::<Vec<_>>(),
    );
    out.push((CANVAS_ID, region));
    for node in nodes {
        export_node(
            out,
            canvas,
            node,
            open_levels(state.focus.focus(), node.key),
        );
    }
}

/// The wire name serde gives a contract enum, for a label.
fn wire_name<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|found| found.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// The accessible name of one pending decision request.
fn decision_name(request: &DecisionRequest) -> String {
    format!(
        "Decision request {}: {} proposes {} on {}, {}, {} required, due at {}",
        request.request_id,
        request.agent_id,
        wire_name(&request.proposed_action),
        request.target,
        wire_name(&request.risk_tier),
        wire_name(&request.required_approval),
        request.due_at.get(),
    )
}

/// The accessible name of the telemetry readout.
fn telemetry_name(telemetry: Option<&CarbonTelemetry>) -> String {
    telemetry.map_or_else(
        || "No carbon telemetry received yet".to_owned(),
        |update| {
            format!(
                "Power draw {} W ({}), SCI rate {} gCO2eq per unit, grid intensity {} gCO2eq/kWh",
                update.power_watts.get(),
                update.provenance,
                update.sci_rate.get(),
                update.grid_intensity.get(),
            )
        },
    )
}

/// Exports the status bar and the decisions region.
fn export_chrome(out: &mut Vec<(NodeId, Node)>, state: &ShellState, width: f64, height: f64) {
    let bar = Rect {
        x0: 0.0,
        y0: 0.0,
        x1: width,
        y1: STATUS_BAR_HEIGHT,
    };
    let mut status = labelled(Role::Toolbar, "Status bar", bar);
    status.set_children(vec![TELEMETRY_ID]);
    out.push((STATUS_BAR_ID, status));
    let readout = labelled(Role::Status, &telemetry_name(state.telemetry()), bar);
    out.push((TELEMETRY_ID, readout));
    let top = STATUS_BAR_HEIGHT + height;
    let area = Rect {
        x0: 0.0,
        y0: top,
        x1: width,
        y1: top + DECISIONS_HEIGHT,
    };
    let mut list = labelled(Role::List, "Pending decisions", area);
    let pending = state.decisions();
    list.set_children((0..pending.len()).map(decision_id).collect::<Vec<_>>());
    out.push((DECISIONS_ID, list));
    for (index, request) in pending.iter().enumerate() {
        out.push((
            decision_id(index),
            labelled(Role::ListItem, &decision_name(request), area),
        ));
    }
}

/// The tree id focus is on.
fn focus_id(focus: Focus) -> NodeId {
    match focus {
        Focus::Region(Region::StatusBar) => STATUS_BAR_ID,
        Focus::Region(Region::Canvas) => CANVAS_ID,
        Focus::Region(Region::Decisions) => DECISIONS_ID,
        Focus::Target(Target::Node(key)) => node_id(key),
        Focus::Target(Target::Level(key, depth)) => level_id(key, depth),
    }
}

/// Exports the whole shell state as one complete `TreeUpdate`.
#[must_use]
pub fn export(state: &ShellState) -> TreeUpdate {
    let viewport = state.canvas.camera().viewport();
    let (width, height) = (viewport.width(), viewport.height());
    let total = STATUS_BAR_HEIGHT + height + DECISIONS_HEIGHT;
    let mut root = labelled(
        Role::Window,
        "Aegis Forum shell",
        Rect {
            x0: 0.0,
            y0: 0.0,
            x1: width,
            y1: total,
        },
    );
    root.set_children(vec![STATUS_BAR_ID, CANVAS_ID, DECISIONS_ID]);
    let mut nodes = Vec::with_capacity(state.canvas.registry().len().saturating_add(16));
    nodes.push((ROOT_ID, root));
    export_chrome(&mut nodes, state, width, height);
    export_canvas(&mut nodes, state, width, height);
    TreeUpdate {
        nodes,
        tree: Some(TreeInfo::new(ROOT_ID)),
        tree_id: TreeId::ROOT,
        focus: focus_id(state.focus.focus()),
    }
}

/// Why the tree check failed; every variant names the node concerned.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum TreeError {
    /// The update carries no tree information, so it names no root.
    #[error("the update carries no tree information and names no root")]
    NoTreeInfo,
    /// Two entries share an id.
    #[error("{} appears twice in the update", describe(.0))]
    Duplicate(NodeId),
    /// The root is not among the nodes.
    #[error("the root {} is not in the update", describe(.0))]
    MissingRoot(NodeId),
    /// A node lists a child the update does not carry.
    #[error("{} lists {}, which the update does not carry", describe(.parent), describe(.child))]
    Dangling {
        /// The parent.
        parent: NodeId,
        /// The missing child.
        child: NodeId,
    },
    /// A node is reached twice: two parents or a cycle.
    #[error("{} is reached twice: two parents or a cycle", describe(.0))]
    Reached(NodeId),
    /// A node is not reachable from the root.
    #[error("{} is not reachable from the root", describe(.0))]
    Orphan(NodeId),
    /// A node has no role an adapter keeps.
    #[error("{} has no role: {role:?} is dropped by the adapter", describe(.id))]
    NoRole {
        /// The node.
        id: NodeId,
        /// The role it carries.
        role: Role,
    },
    /// A node has no accessible name.
    #[error("{} ({role:?}) has no accessible name", describe(.id))]
    Unlabelled {
        /// The node.
        id: NodeId,
        /// Its role.
        role: Role,
    },
    /// A node is hidden, and so dropped from the tree AT-SPI presents.
    #[error("{} is hidden from assistive technology", describe(.0))]
    Hidden(NodeId),
    /// Focus is on a node the tree does not carry.
    #[error("focus is on {}, which the tree does not carry", describe(.0))]
    FocusAbsent(NodeId),
    /// The canvas has no child at all: no nodes and no empty state.
    #[error("the canvas exports neither nodes nor a labelled empty state")]
    EmptyCanvasUnlabelled,
}

/// What the tree check saw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeReport {
    /// Nodes reachable from the root, each checked.
    pub nodes: usize,
    /// Children of the canvas.
    pub canvas_children: usize,
}

/// Indexes the update's nodes by id, refusing a duplicate.
fn index(update: &TreeUpdate) -> Result<BTreeMap<NodeId, &Node>, TreeError> {
    let mut map = BTreeMap::new();
    for (id, node) in &update.nodes {
        if map.insert(*id, node).is_some() {
            return Err(TreeError::Duplicate(*id));
        }
    }
    Ok(map)
}

/// Checks one node's role, name and visibility.
fn check_node(id: NodeId, node: &Node) -> Result<(), TreeError> {
    let role = node.role();
    if matches!(role, Role::Unknown | Role::GenericContainer) {
        return Err(TreeError::NoRole { id, role });
    }
    if node.label().is_none_or(|name| name.trim().is_empty()) {
        return Err(TreeError::Unlabelled { id, role });
    }
    if node.is_hidden() {
        return Err(TreeError::Hidden(id));
    }
    Ok(())
}

/// Walks the tree from `root` with an explicit stack, checking every node
/// once; returns the ids reached.
fn walk(map: &BTreeMap<NodeId, &Node>, root: NodeId) -> Result<BTreeSet<NodeId>, TreeError> {
    let mut reached = BTreeSet::new();
    let mut stack = vec![root];
    for _ in 0..=map.len() {
        let Some(id) = stack.pop() else {
            return Ok(reached);
        };
        let node = map.get(&id).ok_or(TreeError::MissingRoot(id))?;
        if !reached.insert(id) {
            return Err(TreeError::Reached(id));
        }
        check_node(id, node)?;
        for child in node.children().iter().rev() {
            if !map.contains_key(child) {
                return Err(TreeError::Dangling {
                    parent: id,
                    child: *child,
                });
            }
            stack.push(*child);
        }
    }
    stack
        .pop()
        .map_or(Ok(reached), |id| Err(TreeError::Reached(id)))
}

/// E16-4's tree check over an exported update; see the module documentation.
///
/// # Errors
///
/// Returns the first [`TreeError`], which names the node concerned.
pub fn check_tree(update: &TreeUpdate) -> Result<TreeReport, TreeError> {
    let root = update.tree.as_ref().ok_or(TreeError::NoTreeInfo)?.root;
    let map = index(update)?;
    if !map.contains_key(&root) {
        return Err(TreeError::MissingRoot(root));
    }
    let reached = walk(&map, root)?;
    if let Some(orphan) = map.keys().find(|id| !reached.contains(id)) {
        return Err(TreeError::Orphan(*orphan));
    }
    if !reached.contains(&update.focus) {
        return Err(TreeError::FocusAbsent(update.focus));
    }
    let canvas_children = map
        .get(&CANVAS_ID)
        .map_or(0, |canvas| canvas.children().len());
    if map.contains_key(&CANVAS_ID) && canvas_children == 0 {
        return Err(TreeError::EmptyCanvasUnlabelled);
    }
    Ok(TreeReport {
        nodes: reached.len(),
        canvas_children,
    })
}

/// Why an update does not carry the canvas the model holds (REQ-P05-11).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExportError {
    /// The update carries no canvas node.
    #[error("the update carries no canvas")]
    CanvasAbsent,
    /// The canvas clips its children, so an adapter drops off-screen nodes.
    #[error("the canvas clips its children, so an adapter would drop culled nodes")]
    ClipsChildren,
    /// The canvas lists another number of children than the model holds.
    #[error("the canvas lists {found} nodes, the model holds {expected}")]
    Count {
        /// Nodes in the model.
        expected: usize,
        /// Children of the canvas.
        found: usize,
    },
    /// A child is out of focus order.
    #[error("position {position} holds {}, focus order puts {} there", describe(.found), describe(.expected))]
    Order {
        /// The position in focus order.
        position: usize,
        /// The node the model puts there.
        expected: NodeId,
        /// The node the update lists there.
        found: NodeId,
    },
    /// A canvas node carries no bounds or is hidden.
    #[error("{} carries no bounds or is hidden", describe(.0))]
    Placement(NodeId),
    /// An empty canvas does not export its empty state.
    #[error("the empty canvas does not export its labelled empty state")]
    EmptyStateMissing,
}

/// Checks that `update` exports every node of `canvas`, in focus order, each
/// placed and none hidden, from a canvas that does not clip.
///
/// # Errors
///
/// Returns the first [`ExportError`].
pub fn check_canvas_export(update: &TreeUpdate, canvas: &Canvas) -> Result<(), ExportError> {
    let map: BTreeMap<NodeId, &Node> = update.nodes.iter().map(|(id, node)| (*id, node)).collect();
    let region = map.get(&CANVAS_ID).ok_or(ExportError::CanvasAbsent)?;
    if region.clips_children() {
        return Err(ExportError::ClipsChildren);
    }
    let children = region.children();
    if canvas.registry().is_empty() {
        return if children == [EMPTY_STATE_ID] {
            Ok(())
        } else {
            Err(ExportError::EmptyStateMissing)
        };
    }
    let expected = canvas.registry().len();
    if children.len() != expected {
        return Err(ExportError::Count {
            expected,
            found: children.len(),
        });
    }
    for (position, (node, found)) in canvas.registry().nodes().iter().zip(children).enumerate() {
        let wanted = node_id(node.key);
        if *found != wanted {
            return Err(ExportError::Order {
                position,
                expected: wanted,
                found: *found,
            });
        }
        let placed = map
            .get(found)
            .is_some_and(|child| child.bounds().is_some() && !child.is_hidden());
        if !placed {
            return Err(ExportError::Placement(*found));
        }
    }
    Ok(())
}
