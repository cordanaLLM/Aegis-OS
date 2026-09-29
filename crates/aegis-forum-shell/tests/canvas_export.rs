// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E16-3's canvas half and REQ-P05-11: culling changes only what is painted.
//!
//! Positive: 1,000 seeded nodes with 10 in view export all 1,000 in the
//! `accesskit::TreeUpdate`, in focus order, each with its role, name and
//! bounds. Negative: culling implemented by dropping nodes from the update,
//! by hiding them, or by a clipping canvas fails the check, and so does an
//! empty canvas exported without its labelled empty state. Boundary: a node
//! exactly on the cull boundary and a node straddling the viewport edge are
//! both exported and painted, one unit further out is exported and not
//! painted, and focusing a culled node moves the camera until it is in view.

mod common;

use accesskit::{NodeId, Role, TreeUpdate};
use aegis_forum_shell::a11y::{
    CANVAS_ID, EMPTY_STATE_ID, ExportError, TreeError, check_canvas_export, check_tree, export,
    node_id,
};
use aegis_forum_shell::canvas::geometry::{GeometryError, WorldRect};
use aegis_forum_shell::canvas::registry::{ActionState, CanvasNode, NodeKey, NodeKind};
use aegis_forum_shell::canvas::seed::{self, SEED_COLUMNS, SEED_ROWS};
use aegis_forum_shell::canvas::{Canvas, camera::Camera};
use aegis_forum_shell::focus::FocusModel;
use aegis_forum_shell::state::ShellState;

use common::Fallible;

/// The seeded canvas inside a shell.
fn seeded_shell() -> Fallible<ShellState> {
    Ok(ShellState::new(seed::seeded()?))
}

/// The keys the camera paints, sorted.
fn painted(canvas: &Canvas) -> Fallible<Vec<NodeKey>> {
    let mut out = Vec::new();
    canvas.cull_into(&mut out)?;
    out.sort_unstable();
    Ok(out)
}

/// The canvas node's children in an update.
fn canvas_children(update: &TreeUpdate) -> Vec<NodeId> {
    update
        .nodes
        .iter()
        .find(|(id, _)| *id == CANVAS_ID)
        .map(|(_, node)| node.children().to_vec())
        .unwrap_or_default()
}

/// The tree ids of every canvas node the shell holds.
fn all_nodes(state: &ShellState) -> Vec<NodeId> {
    state
        .canvas
        .registry()
        .nodes()
        .iter()
        .map(|node| node_id(node.key))
        .collect()
}

/// A plain node at `rect` with key `key`.
fn node(key: u32, rect: WorldRect) -> CanvasNode {
    CanvasNode {
        key: NodeKey(key),
        label: format!("Boundary node {key}"),
        kind: NodeKind::Note,
        rect,
        actions: Vec::new(),
        levels: Vec::new(),
        state: ActionState::default(),
    }
}

// --- Positive -------------------------------------------------------------

/// Positive: the seed is 1,000 nodes, 10 of them painted.
#[test]
fn the_seeded_canvas_has_one_thousand_nodes_and_ten_in_view() -> Fallible {
    let canvas = seed::seeded()?;
    assert_eq!(canvas.registry().len(), 1_000);
    assert_eq!(SEED_COLUMNS.saturating_mul(SEED_ROWS), 1_000);
    let keys = painted(&canvas)?;
    let expected: Vec<NodeKey> = [0, 1, 2, 3, 4, 50, 51, 52, 53, 54].map(NodeKey).to_vec();
    assert_eq!(keys, expected);
    for key in &keys {
        assert!(canvas.fully_in_view(*key)?);
    }
    Ok(())
}

/// Positive: all 1,000 nodes are exported, in focus order, each with a role,
/// a name and bounds, and the export check agrees.
#[test]
fn culling_leaves_all_one_thousand_nodes_in_the_tree_in_focus_order() -> Fallible {
    let state = seeded_shell()?;
    let update = export(&state);
    let children = canvas_children(&update);
    assert_eq!(children.len(), 1_000);
    let order: Vec<NodeId> = state
        .canvas
        .registry()
        .nodes()
        .iter()
        .map(|n| node_id(n.key))
        .collect();
    assert_eq!(children, order, "the canvas lists its nodes in focus order");
    for id in &children {
        let found = update
            .nodes
            .iter()
            .find(|(node, _)| node == id)
            .map(|(_, node)| node);
        let node = found.ok_or("a listed node is missing")?;
        assert!(!matches!(
            node.role(),
            Role::Unknown | Role::GenericContainer
        ));
        assert!(node.label().is_some_and(|name| !name.trim().is_empty()));
        assert!(node.bounds().is_some());
        assert!(!node.is_hidden());
    }
    check_canvas_export(&update, &state.canvas)?;
    assert_eq!(check_tree(&update)?.canvas_children, 1_000);
    Ok(())
}

/// Positive: a culled node's bounds lie outside the viewport, so an adapter
/// can tell it is off-screen, while a painted one's lie inside.
#[test]
fn a_culled_node_is_placed_off_screen_and_a_painted_one_on_it() -> Fallible {
    let state = seeded_shell()?;
    let update = export(&state);
    let bounds = |key: u32| {
        update
            .nodes
            .iter()
            .find(|(id, _)| *id == node_id(NodeKey(key)))
            .and_then(|(_, node)| node.bounds())
    };
    let inside = bounds(0).ok_or("node 0 has no bounds")?;
    let outside = bounds(999).ok_or("node 999 has no bounds")?;
    assert!(inside.x0 >= 0.0 && inside.x1 <= 1_000.0);
    assert!(
        outside.x0 > 1_000.0,
        "node 999 sits right of the viewport: {outside:?}"
    );
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: culling by dropping the culled nodes from the update leaves 10
/// and fails the 1,000-node check, naming the count.
#[test]
fn culling_by_dropping_nodes_fails_the_check() -> Fallible {
    let state = seeded_shell()?;
    let visible: Vec<NodeId> = painted(&state.canvas)?.into_iter().map(node_id).collect();
    let every = all_nodes(&state);
    let mut update = export(&state);
    update
        .nodes
        .retain(|(id, _)| !every.contains(id) || visible.contains(id));
    for (id, node) in &mut update.nodes {
        if *id == CANVAS_ID {
            node.set_children(visible.clone());
        }
    }
    assert!(
        check_tree(&update).is_ok(),
        "the dropped tree is still a well-formed tree"
    );
    assert_eq!(
        check_canvas_export(&update, &state.canvas),
        Err(ExportError::Count {
            expected: 1_000,
            found: 10
        })
    );
    Ok(())
}

/// Negative: culling by hiding the culled nodes fails both checks: an
/// adapter drops a hidden node from the tree it presents.
#[test]
fn culling_by_hiding_nodes_fails_the_check() -> Fallible {
    let state = seeded_shell()?;
    let visible: Vec<NodeId> = painted(&state.canvas)?.into_iter().map(node_id).collect();
    let every = all_nodes(&state);
    let mut update = export(&state);
    for (id, node) in &mut update.nodes {
        if every.contains(id) && !visible.contains(id) {
            node.set_hidden();
        }
    }
    let refused = check_tree(&update).err().ok_or("a hidden node passed")?;
    assert!(matches!(refused, TreeError::Hidden(_)), "{refused}");
    assert!(refused.to_string().contains("canvas node"), "{refused}");
    let export_check = check_canvas_export(&update, &state.canvas);
    assert_eq!(
        export_check,
        Err(ExportError::Placement(node_id(NodeKey(5)))),
        "the first culled node"
    );
    Ok(())
}

/// Negative: a canvas that clips its children is refused, because the
/// `AccessKit` consumer drops a clipping parent's off-screen children.
#[test]
fn a_clipping_canvas_is_refused() -> Fallible {
    let state = seeded_shell()?;
    let mut update = export(&state);
    for (id, node) in &mut update.nodes {
        if *id == CANVAS_ID {
            node.set_clips_children();
        }
    }
    assert_eq!(
        check_canvas_export(&update, &state.canvas),
        Err(ExportError::ClipsChildren)
    );
    Ok(())
}

/// Negative: a canvas whose children are out of focus order is refused.
#[test]
fn an_export_out_of_focus_order_is_refused() -> Fallible {
    let state = seeded_shell()?;
    let mut update = export(&state);
    for (id, node) in &mut update.nodes {
        if *id == CANVAS_ID {
            let mut children = node.children().to_vec();
            children.swap(0, 1);
            node.set_children(children);
        }
    }
    let refused = check_canvas_export(&update, &state.canvas);
    assert!(
        matches!(refused, Err(ExportError::Order { position: 0, .. })),
        "{refused:?}"
    );
    Ok(())
}

/// Negative: the export of an empty canvas passes only with the empty state
/// as the canvas's one child; with no child, another child in its place, or
/// the empty state listed twice, it is refused as missing.
#[test]
fn an_empty_canvas_without_its_empty_state_is_refused() -> Fallible {
    let state = ShellState::new(seed::empty()?);
    let update = export(&state);
    assert_eq!(canvas_children(&update), vec![EMPTY_STATE_ID]);
    check_canvas_export(&update, &state.canvas)?;
    let planted = [
        Vec::new(),
        vec![node_id(NodeKey(0))],
        vec![EMPTY_STATE_ID, EMPTY_STATE_ID],
    ];
    for children in planted {
        let mut stripped = update.clone();
        for (id, node) in &mut stripped.nodes {
            if *id == CANVAS_ID {
                node.set_children(children.clone());
            }
        }
        assert_eq!(
            check_canvas_export(&stripped, &state.canvas),
            Err(ExportError::EmptyStateMissing),
            "{children:?}"
        );
    }
    Ok(())
}

/// Negative: geometry outside the admitted range is refused.
#[test]
fn geometry_outside_the_admitted_range_is_refused() {
    assert_eq!(
        WorldRect::new(f64::NAN, 0.0, 1.0, 1.0),
        Err(GeometryError::NonFinite)
    );
    assert_eq!(
        WorldRect::new(2.0, 0.0, 1.0, 1.0),
        Err(GeometryError::Inverted)
    );
    assert_eq!(
        WorldRect::new(0.0, 0.0, 2.0e7, 1.0),
        Err(GeometryError::OutOfRange)
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the seeded view is `(-50, -50)` to `(950, 350)`. A node whose
/// left edge lies exactly on its right edge and a node straddling it are
/// both painted and exported; a node one unit further out is exported and
/// not painted.
#[test]
fn nodes_on_and_across_the_cull_boundary_are_exported() -> Fallible {
    let bounds = WorldRect::new(-1_000.0, -1_000.0, 3_000.0, 3_000.0)?;
    let mut canvas = Canvas::new(bounds, seed::seed_camera()?);
    canvas.push(node(
        1,
        WorldRect::from_origin_size(950.0, 0.0, 100.0, 100.0)?,
    ))?;
    canvas.push(node(
        2,
        WorldRect::from_origin_size(900.0, 200.0, 100.0, 100.0)?,
    ))?;
    canvas.push(node(
        3,
        WorldRect::from_origin_size(951.0, 0.0, 100.0, 100.0)?,
    ))?;
    assert_eq!(painted(&canvas)?, vec![NodeKey(1), NodeKey(2)]);
    let state = ShellState::new(canvas);
    let update = export(&state);
    assert_eq!(
        canvas_children(&update),
        [1, 2, 3].map(|key| node_id(NodeKey(key))).to_vec()
    );
    check_canvas_export(&update, &state.canvas)?;
    check_tree(&update)?;
    Ok(())
}

/// Boundary: focusing a culled node moves the camera until the node is in
/// view; focusing a node already in view leaves the camera where it is.
#[test]
fn focusing_a_culled_node_moves_the_camera_until_it_is_in_view() -> Fallible {
    let mut canvas = seed::seeded()?;
    let mut focus = FocusModel::new();
    let before: Camera = *canvas.camera();
    assert!(
        !focus.focus_node(&mut canvas, NodeKey(3))?,
        "node 3 is already in view"
    );
    assert_eq!(*canvas.camera(), before);
    let far = NodeKey(999);
    assert!(!canvas.fully_in_view(far)?);
    assert!(!painted(&canvas)?.contains(&far));
    assert!(focus.focus_node(&mut canvas, far)?, "the camera moved");
    assert!(canvas.fully_in_view(far)?);
    assert!(painted(&canvas)?.contains(&far));
    assert_eq!(
        canvas.registry().len(),
        1_000,
        "revealing culls nothing away"
    );
    Ok(())
}

/// Boundary: the cull index answers exactly what a scan of every node
/// answers, across views that cut through the grid's quadrants.
#[test]
fn the_cull_index_agrees_with_a_full_scan() -> Fallible {
    let canvas = seed::seeded()?;
    assert!(canvas.index().quadrant_count() > 1, "the index split");
    let views = [
        WorldRect::new(-50.0, -50.0, 950.0, 350.0)?,
        WorldRect::new(4_950.0, 1_950.0, 5_050.0, 2_050.0)?,
        WorldRect::new(0.0, 0.0, 10_000.0, 4_000.0)?,
        WorldRect::new(100.5, 100.5, 199.5, 199.5)?,
        WorldRect::new(-5_000.0, -5_000.0, -4_000.0, -4_000.0)?,
    ];
    let mut found = Vec::new();
    for view in views {
        canvas.index().query_into(&view, &mut found);
        found.sort_unstable();
        let mut scanned: Vec<NodeKey> = canvas
            .registry()
            .nodes()
            .iter()
            .filter(|node| node.rect.intersects(&view))
            .map(|node| node.key)
            .collect();
        scanned.sort_unstable();
        assert_eq!(found, scanned, "{view:?}");
    }
    Ok(())
}
