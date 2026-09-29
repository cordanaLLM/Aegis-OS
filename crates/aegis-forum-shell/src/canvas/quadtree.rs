// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The cull index (D74): a region `QuadTree` over the canvas nodes' bounds.
//!
//! The tree answers one question, which nodes intersect the visible region,
//! and the answer decides what is painted and nothing else. It is built and
//! queried without recursion (HISS-01): quadrants live in one arena, insertion
//! descends with a loop bounded by [`MAX_DEPTH`], and a query walks an
//! explicit stack held in a fixed array, so a query allocates nothing beyond
//! the caller's output buffer (HISS-03).
//!
//! A rectangle that does not fit entirely inside one child quadrant stays in
//! the quadrant above, and a rectangle outside the tree's bounds stays in the
//! root, which every query visits whatever its bounds; so no node is ever
//! lost to the index, only placed less precisely.

use super::geometry::WorldRect;
use super::registry::NodeKey;

/// The deepest a quadrant may be split.
pub const MAX_DEPTH: u8 = 8;

/// How many rectangles a leaf holds before it splits.
pub const LEAF_CAPACITY: usize = 16;

/// The most quadrants the arena holds; past it, leaves stop splitting.
pub const MAX_QUADS: usize = 4_097;

/// The explicit stack a query walks: a depth-first walk over a tree of depth
/// [`MAX_DEPTH`] holds at most three siblings per level plus the current
/// quadrant, so this bound is never reached.
const QUERY_STACK: usize = 4 * (MAX_DEPTH as usize) + 1;

/// One quadrant of the arena.
#[derive(Debug, Clone)]
struct Quad {
    bounds: WorldRect,
    depth: u8,
    first_child: Option<usize>,
    items: Vec<(NodeKey, WorldRect)>,
}

impl Quad {
    /// A quadrant with no items and no children.
    fn leaf(bounds: WorldRect, depth: u8) -> Self {
        Self {
            bounds,
            depth,
            first_child: None,
            items: Vec::new(),
        }
    }
}

/// The cull index over every node's bounds.
#[derive(Debug, Clone)]
pub struct QuadTree {
    quads: Vec<Quad>,
    len: usize,
}

impl QuadTree {
    /// An empty index covering `bounds`.
    #[must_use]
    pub fn new(bounds: WorldRect) -> Self {
        Self {
            quads: vec![Quad::leaf(bounds, 0)],
            len: 0,
        }
    }

    /// The number of rectangles indexed.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the index holds no rectangle.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The number of quadrants the arena holds.
    #[must_use]
    pub fn quadrant_count(&self) -> usize {
        self.quads.len()
    }

    /// Returns the child of `index` that contains `rect` entirely, if any.
    fn child_containing(&self, index: usize, rect: &WorldRect) -> Option<usize> {
        let first = self.quads.get(index)?.first_child?;
        (first..first.saturating_add(4)).find(|child| {
            self.quads
                .get(*child)
                .is_some_and(|quad| quad.bounds.contains(rect))
        })
    }

    /// Indexes `key` at `rect`.
    pub fn insert(&mut self, key: NodeKey, rect: WorldRect) {
        let mut index = 0;
        for _ in 0..=MAX_DEPTH {
            match self.child_containing(index, &rect) {
                Some(child) => index = child,
                None => break,
            }
        }
        if let Some(quad) = self.quads.get_mut(index) {
            quad.items.push((key, rect));
            self.len = self.len.saturating_add(1);
        }
        self.split_if_full(index);
    }

    /// Splits a full leaf into four children and moves down every rectangle
    /// that fits entirely inside one of them.
    fn split_if_full(&mut self, index: usize) {
        let Some(quad) = self.quads.get(index) else {
            return;
        };
        let full = quad.items.len() > LEAF_CAPACITY;
        if !full || quad.first_child.is_some() || quad.depth >= MAX_DEPTH {
            return;
        }
        if self.quads.len().saturating_add(4) > MAX_QUADS {
            return;
        }
        let (bounds, depth) = (quad.bounds, quad.depth.saturating_add(1));
        let first = self.quads.len();
        self.quads
            .extend(bounds.quadrants().map(|child| Quad::leaf(child, depth)));
        let items = self.quads.get_mut(index).map(|quad| {
            quad.first_child = Some(first);
            std::mem::take(&mut quad.items)
        });
        for (key, rect) in items.unwrap_or_default() {
            let target = self.child_containing(index, &rect).unwrap_or(index);
            if let Some(quad) = self.quads.get_mut(target) {
                quad.items.push((key, rect));
            }
        }
    }

    /// Writes into `out` every key whose rectangle intersects `view`, and
    /// returns how many were written.
    ///
    /// `out` is cleared first and reused, so a caller that keeps its buffer
    /// between frames allocates nothing here once the buffer has grown. The
    /// order is the arena's, not the focus order.
    pub fn query_into(&self, view: &WorldRect, out: &mut Vec<NodeKey>) -> usize {
        out.clear();
        let mut stack = [0_usize; QUERY_STACK];
        let mut depth = 1_usize;
        for _ in 0..self.quads.len() {
            let Some(index) = depth.checked_sub(1).and_then(|top| stack.get(top).copied()) else {
                break;
            };
            depth = depth.saturating_sub(1);
            let Some(quad) = self.quads.get(index) else {
                continue;
            };
            if index != 0 && !quad.bounds.intersects(view) {
                continue;
            }
            out.extend(
                quad.items
                    .iter()
                    .filter(|(_, rect)| rect.intersects(view))
                    .map(|(key, _)| *key),
            );
            depth = push_children(&mut stack, depth, quad.first_child);
        }
        out.len()
    }
}

/// Pushes the four children starting at `first`, if any, onto the stack and
/// returns its new depth. A child that would overflow the stack is dropped,
/// which the stack bound makes unreachable.
fn push_children(stack: &mut [usize; QUERY_STACK], depth: usize, first: Option<usize>) -> usize {
    let Some(first) = first else {
        return depth;
    };
    let mut depth = depth;
    for child in first..first.saturating_add(4) {
        if let Some(slot) = stack.get_mut(depth) {
            *slot = child;
            depth = depth.saturating_add(1);
        }
    }
    depth
}
