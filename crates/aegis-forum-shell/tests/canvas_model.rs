// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The canvas model: `camera_mut`, `Canvas::reveal` and `content_bounds`.
//!
//! The canvas looks at (100, 50) at scale 2 through an 800 by 600 viewport,
//! so the visible region is (-100, -100) to (300, 200) (half-widths 200 and
//! 150).
//!
//! Positive: the camera handle changes the camera the canvas culls with, a
//! node out of view is revealed, the content bounds are the hand-computed
//! union. Negative: an unknown key is refused and the camera stays, a node in
//! view is not moved to, an invalid zoom through the handle is ignored, an
//! empty canvas has no bounds. Boundary: a node exactly on the visible edge
//! stays and one unit past moves, zooming through the handle stops at the
//! scale limit, single, degenerate and full-range content.

mod common;

use aegis_forum_shell::canvas::camera::{Camera, MAX_SCALE};
use aegis_forum_shell::canvas::geometry::{MAX_COORDINATE, Viewport, WorldRect};
use aegis_forum_shell::canvas::registry::{
    ActionState, CanvasNode, NodeKey, NodeKind, RegistryError,
};
use aegis_forum_shell::canvas::{Canvas, CanvasError};

use common::{Fallible, close};

/// A rectangle from its corners.
fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Fallible<WorldRect> {
    Ok(WorldRect::new(x0, y0, x1, y1)?)
}

/// A plain node at `rect`.
fn node(key: u32, rect: WorldRect) -> CanvasNode {
    CanvasNode {
        key: NodeKey(key),
        label: format!("Model node {key}"),
        kind: NodeKind::Note,
        rect,
        actions: Vec::new(),
        levels: Vec::new(),
        state: ActionState::default(),
    }
}

/// A canvas with the camera above and the given nodes, keyed from 1.
fn canvas_with(rects: &[WorldRect]) -> Fallible<Canvas> {
    let camera = Camera::new((100.0, 50.0), 2.0, Viewport::new(800.0, 600.0)?)?;
    let mut canvas = Canvas::new(rect(-10_000.0, -10_000.0, 10_000.0, 10_000.0)?, camera);
    for (key, at) in (1_u32..).zip(rects) {
        canvas.push(node(key, *at))?;
    }
    Ok(canvas)
}

/// The edges of the content bounds, or `None`.
fn edges(canvas: &Canvas) -> Option<[f64; 4]> {
    canvas
        .content_bounds()
        .map(|b| [b.x0(), b.y0(), b.x1(), b.y1()])
}

/// Whether `actual` holds the four `expected` edges.
fn same_edges(actual: Option<[f64; 4]>, expected: [f64; 4]) -> bool {
    actual.is_some_and(|got| got.iter().zip(expected).all(|(a, e)| close(*a, e)))
}

// --- Positive -------------------------------------------------------------

/// Positive: zooming through `camera_mut` changes the canvas's own camera
/// (2 to 4) and the cull set with it: the node spans x 250 to 260, inside the
/// visible x range 100 +- 200 at scale 2 but outside 100 +- 100 at scale 4.
#[test]
fn the_camera_handle_changes_the_canvas_camera_and_its_cull_set() -> Fallible {
    let mut canvas = canvas_with(&[rect(250.0, 50.0, 260.0, 60.0)?])?;
    let mut painted = Vec::new();
    assert_eq!(canvas.cull_into(&mut painted)?, 1);
    canvas.camera_mut().zoom_by(2.0);
    assert!(close(canvas.camera().scale(), 4.0));
    painted.clear();
    assert_eq!(canvas.cull_into(&mut painted)?, 0);
    Ok(())
}

/// Positive: a node at (1000, 1000) to (1010, 1010) is outside the view;
/// revealing it reports a move, centres on (1005, 1005) and puts it fully in
/// view.
#[test]
fn revealing_a_node_out_of_view_centres_it() -> Fallible {
    let mut canvas = canvas_with(&[rect(1_000.0, 1_000.0, 1_010.0, 1_010.0)?])?;
    assert!(!canvas.fully_in_view(NodeKey(1))?);
    assert!(canvas.reveal(NodeKey(1))?);
    assert!(close(canvas.camera().center().0, 1_005.0));
    assert!(close(canvas.camera().center().1, 1_005.0));
    assert!(canvas.fully_in_view(NodeKey(1))?);
    Ok(())
}

/// Positive: the bounds of (0, 0, 10, 10), (-5, 20, 3, 30) and
/// (100, -7, 120, 2) are x from -5 (second) to 120 (third), y from -7 (third)
/// to 30 (second); the order the nodes were pushed in does not matter.
#[test]
fn the_content_bounds_are_the_union_of_the_nodes() -> Fallible {
    let rects = [
        rect(0.0, 0.0, 10.0, 10.0)?,
        rect(-5.0, 20.0, 3.0, 30.0)?,
        rect(100.0, -7.0, 120.0, 2.0)?,
    ];
    let expected = [-5.0, -7.0, 120.0, 30.0];
    assert!(same_edges(edges(&canvas_with(&rects)?), expected));
    let [a, b, c] = rects;
    assert!(same_edges(edges(&canvas_with(&[c, b, a])?), expected));
    assert!(same_edges(edges(&canvas_with(&[b, c, a])?), expected));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: revealing a key nobody registered is `Unknown` with that key and
/// the camera does not move.
#[test]
fn revealing_an_unknown_key_is_refused_and_the_camera_stays() -> Fallible {
    let mut canvas = canvas_with(&[rect(1_000.0, 1_000.0, 1_010.0, 1_010.0)?])?;
    let before = *canvas.camera();
    assert_eq!(
        canvas.reveal(NodeKey(9)),
        Err(CanvasError::Registry(RegistryError::Unknown { key: 9 }))
    );
    assert_eq!(*canvas.camera(), before);
    Ok(())
}

/// Negative: a node already fully in view reports no move and the camera is
/// unchanged.
#[test]
fn revealing_a_node_in_view_does_not_move_the_camera() -> Fallible {
    let mut canvas = canvas_with(&[rect(90.0, 40.0, 110.0, 60.0)?])?;
    let before = *canvas.camera();
    assert!(!canvas.reveal(NodeKey(1))?);
    assert_eq!(*canvas.camera(), before);
    Ok(())
}

/// Negative: a zero, negative or NaN zoom factor through the handle leaves the
/// scale at 2.
#[test]
fn an_invalid_zoom_through_the_handle_is_ignored() -> Fallible {
    let mut canvas = canvas_with(&[])?;
    for factor in [0.0, -1.0, f64::NAN] {
        canvas.camera_mut().zoom_by(factor);
        assert!(close(canvas.camera().scale(), 2.0));
    }
    Ok(())
}

/// Negative: an empty canvas has no content bounds.
#[test]
fn an_empty_canvas_has_no_content_bounds() -> Fallible {
    assert!(canvas_with(&[])?.content_bounds().is_none());
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a node touching the visible edges, (280, 180) to (300, 200), is
/// in view and does not move the camera; (281, 181) to (301, 201) is one unit
/// past and does, centring on (291, 191) at the unchanged scale 2 (20 by 20
/// fits at 40 and 30).
#[test]
fn a_node_on_the_visible_edge_stays_and_one_past_moves() -> Fallible {
    let mut canvas = canvas_with(&[
        rect(280.0, 180.0, 300.0, 200.0)?,
        rect(281.0, 181.0, 301.0, 201.0)?,
    ])?;
    assert!(!canvas.reveal(NodeKey(1))?);
    assert!(close(canvas.camera().center().0, 100.0));
    assert!(canvas.reveal(NodeKey(2))?);
    assert!(close(canvas.camera().center().0, 291.0));
    assert!(close(canvas.camera().center().1, 191.0));
    assert!(close(canvas.camera().scale(), 2.0));
    Ok(())
}

/// Boundary: a node 4000 by 3000 does not fit; revealing it zooms the canvas
/// camera out to `800 / 4000 = 0.2`.
#[test]
fn revealing_a_node_larger_than_the_view_zooms_out() -> Fallible {
    let mut canvas = canvas_with(&[rect(0.0, 0.0, 4_000.0, 3_000.0)?])?;
    assert!(canvas.reveal(NodeKey(1))?);
    assert!(close(canvas.camera().scale(), 0.2));
    assert!(close(canvas.camera().center().1, 1_500.0));
    Ok(())
}

/// Boundary: zooming through the handle stops at `MAX_SCALE`.
#[test]
fn zooming_through_the_handle_stops_at_the_maximum_scale() -> Fallible {
    let mut canvas = canvas_with(&[])?;
    canvas.camera_mut().zoom_by(1_000.0);
    assert!(close(canvas.camera().scale(), MAX_SCALE));
    Ok(())
}

/// Boundary: one node's bounds are its own rectangle, a zero-size node is a
/// point, and nodes at both coordinate limits give the full range.
#[test]
fn the_content_bounds_hold_for_one_node_a_point_and_the_full_range() -> Fallible {
    let one = canvas_with(&[rect(1.0, 2.0, 3.0, 4.0)?])?;
    assert!(same_edges(edges(&one), [1.0, 2.0, 3.0, 4.0]));
    let point = canvas_with(&[rect(5.0, 6.0, 5.0, 6.0)?])?;
    assert!(same_edges(edges(&point), [5.0, 6.0, 5.0, 6.0]));
    let m = MAX_COORDINATE;
    let full = canvas_with(&[rect(-m, -m, -m, -m)?, rect(m, m, m, m)?])?;
    assert!(same_edges(edges(&full), [-m, -m, m, m]));
    Ok(())
}
