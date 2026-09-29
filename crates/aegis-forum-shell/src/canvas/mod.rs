// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The D74 canvas model: the node registry, the camera and the `QuadTree`
//! cull index, as tested shell state.
//!
//! | Part | Module | What it decides |
//! | :-- | :-- | :-- |
//! | Nodes, actions, instruments | [`registry`] | what exists, in focus order |
//! | Camera | [`camera`] | where the viewport looks, and how far zoomed |
//! | Cull index | [`quadtree`] | which nodes are painted |
//! | Seeded canvas | [`seed`] | the 1,000-node fixture E16-3 names |
//!
//! Culling changes only what is painted (REQ-P05-11): [`Canvas::cull_into`]
//! is the paint set, and nothing in `crate::a11y` calls it.

pub mod camera;
pub mod geometry;
pub mod quadtree;
pub mod registry;
pub mod seed;

use thiserror::Error;

use camera::Camera;
use geometry::{GeometryError, WorldRect};
use quadtree::QuadTree;
use registry::{ActionKind, ActionState, CanvasNode, NodeKey, Registry, RegistryError};

/// Why a canvas operation was refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CanvasError {
    /// The registry refused a node or does not know a key.
    #[error(transparent)]
    Registry(#[from] RegistryError),
    /// A rectangle or the camera left the admitted geometry.
    #[error(transparent)]
    Geometry(#[from] GeometryError),
    /// An instrument level the node does not have.
    #[error("canvas node {key} has no instrument level {depth}")]
    Level {
        /// The node concerned.
        key: u32,
        /// The level asked for.
        depth: usize,
    },
}

/// What an action or a focus target belongs to: a node, or one level of its
/// instrument (0 is the outermost).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The node itself.
    Node(NodeKey),
    /// One level of the node's instrument.
    Level(NodeKey, usize),
}

/// The canvas: nodes, cull index and camera.
#[derive(Debug, Clone)]
pub struct Canvas {
    registry: Registry,
    index: QuadTree,
    camera: Camera,
}

impl Canvas {
    /// An empty canvas whose cull index covers `bounds`.
    #[must_use]
    pub fn new(bounds: WorldRect, camera: Camera) -> Self {
        Self {
            registry: Registry::new(),
            index: QuadTree::new(bounds),
            camera,
        }
    }

    /// Adds a node at the end of the focus order and indexes its bounds.
    ///
    /// # Errors
    ///
    /// Propagates [`Registry::push`].
    pub fn push(&mut self, node: CanvasNode) -> Result<(), CanvasError> {
        let (key, rect) = (node.key, node.rect);
        self.registry.push(node)?;
        self.index.insert(key, rect);
        Ok(())
    }

    /// The nodes, in focus order.
    #[must_use]
    pub const fn registry(&self) -> &Registry {
        &self.registry
    }

    /// The camera.
    #[must_use]
    pub const fn camera(&self) -> &Camera {
        &self.camera
    }

    /// The camera, mutably.
    pub const fn camera_mut(&mut self) -> &mut Camera {
        &mut self.camera
    }

    /// The cull index.
    #[must_use]
    pub const fn index(&self) -> &QuadTree {
        &self.index
    }

    /// Writes the keys of every node the camera paints into `out`: the cull
    /// set, not the export set.
    ///
    /// # Errors
    ///
    /// Propagates [`Camera::visible`].
    pub fn cull_into(&self, out: &mut Vec<NodeKey>) -> Result<usize, CanvasError> {
        let view = self.camera.visible()?;
        Ok(self.index.query_into(&view, out))
    }

    /// Whether the node under `key` is inside the visible region entirely.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::Unknown`] for an unregistered key, and
    /// propagates [`Camera::visible`].
    pub fn fully_in_view(&self, key: NodeKey) -> Result<bool, CanvasError> {
        let node = self
            .registry
            .get(key)
            .ok_or(RegistryError::Unknown { key: key.0 })?;
        Ok(self.camera.visible()?.contains(&node.rect))
    }

    /// Moves the camera until the node under `key` is in view (REQ-P05-09),
    /// and returns whether it moved.
    ///
    /// # Errors
    ///
    /// As [`Canvas::fully_in_view`].
    pub fn reveal(&mut self, key: NodeKey) -> Result<bool, CanvasError> {
        let rect = self
            .registry
            .get(key)
            .ok_or(RegistryError::Unknown { key: key.0 })?
            .rect;
        Ok(self.camera.reveal(&rect)?)
    }

    /// The smallest rectangle holding every node, or `None` on an empty
    /// canvas.
    #[must_use]
    pub fn content_bounds(&self) -> Option<WorldRect> {
        let mut nodes = self.registry.nodes().iter();
        let first = nodes.next()?.rect;
        let (x0, y0, x1, y1) = nodes.fold(
            (first.x0(), first.y0(), first.x1(), first.y1()),
            |(x0, y0, x1, y1), node| {
                let rect = node.rect;
                (
                    x0.min(rect.x0()),
                    y0.min(rect.y0()),
                    x1.max(rect.x1()),
                    y1.max(rect.y1()),
                )
            },
        );
        WorldRect::new(x0, y0, x1, y1).ok()
    }

    /// Applies `kind` to `target`'s state.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::Unknown`] for an unregistered node and
    /// [`CanvasError::Level`] for a level the node does not have.
    pub fn apply(&mut self, target: Target, kind: ActionKind) -> Result<(), CanvasError> {
        let state = self.state_mut(target)?;
        state.apply(kind);
        Ok(())
    }

    /// The state `target`'s actions change, mutably.
    fn state_mut(&mut self, target: Target) -> Result<&mut ActionState, CanvasError> {
        match target {
            Target::Node(key) => Ok(&mut self.registry.get_mut(key)?.state),
            Target::Level(key, depth) => self
                .registry
                .get_mut(key)?
                .levels
                .get_mut(depth)
                .map(|level| &mut level.state)
                .ok_or(CanvasError::Level { key: key.0, depth }),
        }
    }

    /// Every action state on the canvas, node by node in focus order: the
    /// node's own, then each instrument level's. Two canvases that reached
    /// the same state compare equal here (E16-3).
    #[must_use]
    pub fn action_states(&self) -> Vec<(NodeKey, ActionState, Vec<ActionState>)> {
        self.registry
            .nodes()
            .iter()
            .map(|node| {
                let levels = node.levels.iter().map(|level| level.state).collect();
                (node.key, node.state, levels)
            })
            .collect()
    }
}
