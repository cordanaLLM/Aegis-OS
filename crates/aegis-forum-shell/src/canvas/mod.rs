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
    /// A node is pinned and cannot be moved or resized.
    #[error("canvas node {key} is pinned")]
    Pinned {
        /// The node concerned.
        key: u32,
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

/// Policy governing whether small targets may be treated as pointer-inert (REQ-P05-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InertPolicy {
    /// Small targets below the size floor are allowed to be pointer-inert
    /// when an equivalent keyboard control is available.
    #[default]
    Allowed,
    /// Pointer-inert status is forbidden; any target below the size floor
    /// is treated as an accessibility defect.
    Forbidden,
}

/// The canvas: nodes, cull index and camera.
#[derive(Debug, Clone)]
pub struct Canvas {
    registry: Registry,
    index: QuadTree,
    camera: Camera,
    inert_policy: InertPolicy,
}

impl Canvas {
    /// An empty canvas whose cull index covers `bounds`.
    #[must_use]
    pub fn new(bounds: WorldRect, camera: Camera) -> Self {
        Self {
            registry: Registry::new(),
            index: QuadTree::new(bounds),
            camera,
            inert_policy: InertPolicy::Allowed,
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

    /// Moves the node under `key` by `(dx, dy)` in world units, updating the cull index.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::Unknown`] if `key` is unregistered,
    /// [`CanvasError::Pinned`] if the node is pinned, or propagates
    /// [`GeometryError`].
    pub fn move_node(&mut self, key: NodeKey, dx: f64, dy: f64) -> Result<(), CanvasError> {
        let node = self.registry.get_mut(key)?;
        if node.state.pinned {
            return Err(CanvasError::Pinned { key: key.0 });
        }
        let old_rect = node.rect;
        let new_rect = WorldRect::new(
            old_rect.x0() + dx,
            old_rect.y0() + dy,
            old_rect.x1() + dx,
            old_rect.y1() + dy,
        )?;
        node.rect = new_rect;
        self.index.remove(key);
        self.index.insert(key, new_rect);
        Ok(())
    }

    /// Resizes the node under `key` by `(dw, dh)` in world units, updating the cull index.
    ///
    /// Extents are clamped to at least [`crate::pointer::MIN_NODE_SIZE`].
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::Unknown`] if `key` is unregistered,
    /// [`CanvasError::Pinned`] if the node is pinned, or propagates
    /// [`GeometryError`].
    pub fn resize_node(&mut self, key: NodeKey, dw: f64, dh: f64) -> Result<(), CanvasError> {
        let node = self.registry.get_mut(key)?;
        if node.state.pinned {
            return Err(CanvasError::Pinned { key: key.0 });
        }
        let old_rect = node.rect;
        let new_w = (old_rect.width() + dw).max(crate::pointer::MIN_NODE_SIZE);
        let new_h = (old_rect.height() + dh).max(crate::pointer::MIN_NODE_SIZE);
        let new_rect = WorldRect::from_origin_size(old_rect.x0(), old_rect.y0(), new_w, new_h)?;
        node.rect = new_rect;
        self.index.remove(key);
        self.index.insert(key, new_rect);
        Ok(())
    }

    /// Rotates the instrument handle at `target` by `delta_degrees` in whole steps.
    ///
    /// # Errors
    ///
    /// Propagates [`CanvasError`] if target node or level is unknown.
    pub fn rotate_handle(&mut self, target: Target, delta_degrees: i16) -> Result<(), CanvasError> {
        let state = self.state_mut(target)?;
        state.rotate(delta_degrees);
        Ok(())
    }

    /// Moves the camera centre by `(dx, dy)` in canvas units.
    pub fn pan(&mut self, dx: f64, dy: f64) {
        self.camera.pan_by(dx, dy);
    }

    /// Multiplies the camera scale by `factor`, clamped to admitted range.
    pub fn zoom(&mut self, factor: f64) {
        self.camera.zoom_by(factor);
    }

    /// Returns the target hit by the screen coordinates `(x, y)`, skipping inert nodes.
    #[must_use]
    pub fn hit_test(&self, pos: (f64, f64)) -> Option<Target> {
        for node in self.registry.nodes() {
            if self.is_inert(node.key) {
                continue;
            }
            let s = self.camera.to_screen(&node.rect);
            if pos.0 >= s.x0 && pos.0 <= s.x1 && pos.1 >= s.y0 && pos.1 <= s.y1 {
                return Some(Target::Node(node.key));
            }
        }
        None
    }

    /// Returns the current policy on pointer-inert targets.
    #[must_use]
    pub const fn inert_policy(&self) -> InertPolicy {
        self.inert_policy
    }

    /// Sets the policy on pointer-inert targets.
    pub fn set_inert_policy(&mut self, policy: InertPolicy) {
        self.inert_policy = policy;
    }

    /// Whether `key` is currently pointer-inert (screen bounds below
    /// [`crate::pointer::MIN_TARGET_CSS_PX`] with inert permitted).
    #[must_use]
    pub fn is_inert(&self, key: NodeKey) -> bool {
        if self.inert_policy == InertPolicy::Forbidden {
            return false;
        }
        let Some(node) = self.registry.get(key) else {
            return false;
        };
        let s = self.camera.to_screen(&node.rect);
        let w = s.x1 - s.x0;
        let h = s.y1 - s.y0;
        w < crate::pointer::MIN_TARGET_CSS_PX - crate::pointer::FLOAT_TOLERANCE
            || h < crate::pointer::MIN_TARGET_CSS_PX - crate::pointer::FLOAT_TOLERANCE
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
