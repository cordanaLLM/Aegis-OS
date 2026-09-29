// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The seeded canvas E16-3 and E16-4 name: 1,000 nodes, 10 of them in view.
//!
//! Nodes are [`NODE_SIZE`] canvas units square on a [`STRIDE`] grid of
//! [`SEED_COLUMNS`] by [`SEED_ROWS`], in reading order, which is also their
//! focus order. The seeded camera looks at the top-left corner at scale 1.0
//! through a [`SEED_VIEWPORT`] viewport, whose visible region is
//! `(-50, -50)` to `(950, 350)`: five columns of two rows, ten nodes, entirely
//! inside it, and the sixth column and third row entirely outside. Every
//! hundredth node carries an instrument three levels deep -- an instrument, a
//! surrogate inside it and a transcluded fragment inside that -- so the
//! keyboard walk has nesting to climb out of.

use super::camera::Camera;
use super::geometry::{GeometryError, Viewport, WorldRect};
use super::registry::{
    ActionKind, ActionState, Binding, CanvasNode, InstrumentLevel, LevelKind, NodeAction, NodeKey,
    NodeKind, TabPolicy,
};
use super::{Canvas, CanvasError};

/// Columns of the seeded grid.
pub const SEED_COLUMNS: u32 = 50;

/// Rows of the seeded grid.
pub const SEED_ROWS: u32 = 20;

/// The edge of every seeded node, in canvas units.
pub const NODE_SIZE: f64 = 100.0;

/// The distance between two neighbouring nodes' origins, in canvas units.
pub const STRIDE: f64 = 200.0;

/// The seeded viewport, in screen pixels: width and height.
pub const SEED_VIEWPORT: (f64, f64) = (1_000.0, 400.0);

/// The seeded camera centre, in canvas units.
pub const SEED_CENTER: (f64, f64) = (450.0, 150.0);

/// Every how many nodes one carries a three-level instrument.
pub const INSTRUMENT_EVERY: u32 = 100;

/// The seeded camera: [`SEED_CENTER`] at scale 1.0 through [`SEED_VIEWPORT`].
///
/// # Errors
///
/// Propagates [`Viewport::new`] and [`Camera::new`]; the constants are inside
/// both bounds.
pub fn seed_camera() -> Result<Camera, GeometryError> {
    let viewport = Viewport::new(SEED_VIEWPORT.0, SEED_VIEWPORT.1)?;
    Camera::new(SEED_CENTER, 1.0, viewport)
}

/// The cull index bounds for a `columns` by `rows` grid, one stride of margin
/// on every side.
fn grid_bounds(columns: u32, rows: u32) -> Result<WorldRect, GeometryError> {
    WorldRect::new(
        -STRIDE,
        -STRIDE,
        f64::from(columns.saturating_add(1)) * STRIDE,
        f64::from(rows.saturating_add(1)) * STRIDE,
    )
}

/// The two actions every seeded node offers: select on Space, pin on
/// Control+P.
fn node_actions() -> Vec<NodeAction> {
    vec![
        NodeAction::both(ActionKind::Select, Binding::Space),
        NodeAction::both(ActionKind::Pin, Binding::Ctrl('p')),
    ]
}

/// One instrument level with an increment and a decrement action.
fn level(label: String, kind: LevelKind) -> InstrumentLevel {
    InstrumentLevel {
        label,
        kind,
        actions: vec![
            NodeAction::both(ActionKind::Increment, Binding::Ctrl('=')),
            NodeAction::both(ActionKind::Decrement, Binding::Ctrl('-')),
        ],
        tab: TabPolicy::Leaves,
        state: ActionState::default(),
    }
}

/// The three-level instrument a seeded node at `number` carries.
fn instrument(number: u32) -> Vec<InstrumentLevel> {
    vec![
        level(
            format!("Settings instrument for node {number}"),
            LevelKind::Instrument,
        ),
        level(
            format!("Surrogate inside node {number}'s instrument"),
            LevelKind::Surrogate,
        ),
        level(
            format!("Fragment transcluded into node {number}"),
            LevelKind::Fragment,
        ),
    ]
}

/// The node kind the seed gives the node at `index`.
const fn kind_for(index: u32) -> NodeKind {
    match index % 3 {
        0 => NodeKind::Application,
        1 => NodeKind::Document,
        _ => NodeKind::Note,
    }
}

/// The seeded node at `column`, `row`; `index` is its focus-order position.
fn seeded_node(index: u32, column: u32, row: u32) -> Result<CanvasNode, GeometryError> {
    let number = index.saturating_add(1);
    let rect = WorldRect::from_origin_size(
        f64::from(column) * STRIDE,
        f64::from(row) * STRIDE,
        NODE_SIZE,
        NODE_SIZE,
    )?;
    let levels = if number.is_multiple_of(INSTRUMENT_EVERY) {
        instrument(number)
    } else {
        Vec::new()
    };
    Ok(CanvasNode {
        key: NodeKey(index),
        label: format!(
            "Node {number}, row {}, column {}",
            row.saturating_add(1),
            column.saturating_add(1)
        ),
        kind: kind_for(index),
        rect,
        actions: node_actions(),
        levels,
        state: ActionState::default(),
    })
}

/// A `columns` by `rows` seeded canvas behind the seeded camera.
///
/// # Errors
///
/// Propagates the geometry and registry bounds; a grid past
/// [`super::registry::MAX_NODES`] nodes is refused as full.
pub fn grid(columns: u32, rows: u32) -> Result<Canvas, CanvasError> {
    let mut canvas = Canvas::new(grid_bounds(columns, rows)?, seed_camera()?);
    for row in 0..rows {
        for column in 0..columns {
            let index = row.saturating_mul(columns).saturating_add(column);
            canvas.push(seeded_node(index, column, row)?)?;
        }
    }
    Ok(canvas)
}

/// The 1,000-node seeded canvas: [`SEED_COLUMNS`] by [`SEED_ROWS`].
///
/// # Errors
///
/// As [`grid`]; the constants are inside every bound.
pub fn seeded() -> Result<Canvas, CanvasError> {
    grid(SEED_COLUMNS, SEED_ROWS)
}

/// An empty canvas behind the seeded camera.
///
/// # Errors
///
/// As [`grid`].
pub fn empty() -> Result<Canvas, CanvasError> {
    grid(0, 0)
}
