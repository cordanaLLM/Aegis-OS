// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Canvas geometry: closed rectangles in canvas units and the camera's
//! viewport in screen pixels.
//!
//! Every rectangle is closed on all four edges, so two rectangles that only
//! touch intersect. That is the rule the cull boundary uses: a node whose edge
//! lies exactly on the edge of the visible region counts as in view (E16-3's
//! boundary), and a node one unit further out does not.

use thiserror::Error;

/// The largest absolute coordinate the canvas admits, in canvas units.
///
/// A bound rather than a claim about any display: it keeps every product of a
/// coordinate and a camera scale finite and far from `f64` precision loss.
pub const MAX_COORDINATE: f64 = 1.0e7;

/// The largest viewport edge the camera admits, in screen pixels.
pub const MAX_VIEWPORT_PIXELS: f64 = 65_536.0;

/// Why a rectangle or a viewport was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum GeometryError {
    /// A coordinate or an extent was NaN or infinite.
    #[error("a coordinate is not a finite number")]
    NonFinite,
    /// A minimum edge lies beyond its maximum edge.
    #[error("a rectangle's minimum edge lies beyond its maximum edge")]
    Inverted,
    /// A coordinate lies outside `MAX_COORDINATE`.
    #[error("a coordinate lies outside the admitted canvas range")]
    OutOfRange,
    /// A viewport edge is zero, negative or larger than `MAX_VIEWPORT_PIXELS`.
    #[error("a viewport edge is not a positive size within the admitted bound")]
    Viewport,
}

/// An axis-aligned rectangle in canvas units, closed on every edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldRect {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

/// Returns whether `value` is a finite coordinate inside the admitted range.
fn admitted(value: f64) -> Result<f64, GeometryError> {
    if !value.is_finite() {
        return Err(GeometryError::NonFinite);
    }
    if value.abs() > MAX_COORDINATE {
        return Err(GeometryError::OutOfRange);
    }
    Ok(value)
}

impl WorldRect {
    /// Builds a rectangle from its minimum and maximum corners.
    ///
    /// # Errors
    ///
    /// Returns [`GeometryError::NonFinite`] or [`GeometryError::OutOfRange`]
    /// for a coordinate outside the admitted range, and
    /// [`GeometryError::Inverted`] when a minimum lies beyond its maximum. A
    /// rectangle of zero width or height is admitted.
    pub fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Result<Self, GeometryError> {
        let (x0, y0, x1, y1) = (admitted(x0)?, admitted(y0)?, admitted(x1)?, admitted(y1)?);
        if x0 > x1 || y0 > y1 {
            return Err(GeometryError::Inverted);
        }
        Ok(Self { x0, y0, x1, y1 })
    }

    /// Builds a rectangle from its top-left corner and its size.
    ///
    /// # Errors
    ///
    /// As [`WorldRect::new`]; a negative size is [`GeometryError::Inverted`].
    pub fn from_origin_size(
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Result<Self, GeometryError> {
        Self::new(x, y, x + width, y + height)
    }

    /// The minimum x coordinate.
    #[must_use]
    pub const fn x0(&self) -> f64 {
        self.x0
    }

    /// The minimum y coordinate.
    #[must_use]
    pub const fn y0(&self) -> f64 {
        self.y0
    }

    /// The maximum x coordinate.
    #[must_use]
    pub const fn x1(&self) -> f64 {
        self.x1
    }

    /// The maximum y coordinate.
    #[must_use]
    pub const fn y1(&self) -> f64 {
        self.y1
    }

    /// The width, in canvas units.
    #[must_use]
    pub fn width(&self) -> f64 {
        self.x1 - self.x0
    }

    /// The height, in canvas units.
    #[must_use]
    pub fn height(&self) -> f64 {
        self.y1 - self.y0
    }

    /// The centre point.
    #[must_use]
    pub fn center(&self) -> (f64, f64) {
        (self.x0 + self.width() / 2.0, self.y0 + self.height() / 2.0)
    }

    /// Whether the two closed rectangles share at least one point.
    #[must_use]
    pub fn intersects(&self, other: &Self) -> bool {
        self.x0 <= other.x1 && other.x0 <= self.x1 && self.y0 <= other.y1 && other.y0 <= self.y1
    }

    /// Whether `other` lies entirely inside this rectangle, edges included.
    #[must_use]
    pub fn contains(&self, other: &Self) -> bool {
        self.x0 <= other.x0 && other.x1 <= self.x1 && self.y0 <= other.y0 && other.y1 <= self.y1
    }

    /// The four quadrants of this rectangle, in the order north-west,
    /// north-east, south-west, south-east.
    #[must_use]
    pub fn quadrants(&self) -> [Self; 4] {
        let (cx, cy) = self.center();
        [
            Self {
                x0: self.x0,
                y0: self.y0,
                x1: cx,
                y1: cy,
            },
            Self {
                x0: cx,
                y0: self.y0,
                x1: self.x1,
                y1: cy,
            },
            Self {
                x0: self.x0,
                y0: cy,
                x1: cx,
                y1: self.y1,
            },
            Self {
                x0: cx,
                y0: cy,
                x1: self.x1,
                y1: self.y1,
            },
        ]
    }
}

/// The camera's viewport: the screen area the canvas is painted into, in
/// pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    width: f64,
    height: f64,
}

impl Viewport {
    /// Builds a viewport.
    ///
    /// # Errors
    ///
    /// Returns [`GeometryError::Viewport`] unless both edges are finite, above
    /// zero and at most [`MAX_VIEWPORT_PIXELS`].
    pub fn new(width: f64, height: f64) -> Result<Self, GeometryError> {
        let fits = |edge: f64| edge.is_finite() && edge > 0.0 && edge <= MAX_VIEWPORT_PIXELS;
        if fits(width) && fits(height) {
            Ok(Self { width, height })
        } else {
            Err(GeometryError::Viewport)
        }
    }

    /// The width, in screen pixels.
    #[must_use]
    pub const fn width(&self) -> f64 {
        self.width
    }

    /// The height, in screen pixels.
    #[must_use]
    pub const fn height(&self) -> f64 {
        self.height
    }
}
