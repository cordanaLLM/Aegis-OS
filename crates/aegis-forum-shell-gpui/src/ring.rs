// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Pure focus-ring geometry in screen space and layer space (M28 E28-4).
//!
//! Provides the geometry of the focus ring around a canvas node, calculations
//! for screen space (scale-invariant) versus layer space (scaled defect),
//! wire line serialization and deserialization, and bounding capture rectangles.
//! This module has no toolkit or display dependencies and is tested purely.

use aegis_concordia::tokens::{FOCUS_RING_OFFSET, FOCUS_RING_WIDTH};
use aegis_forum_shell::canvas::camera::{Camera, ScreenRect};
use aegis_forum_shell::canvas::geometry::WorldRect;
use thiserror::Error;

/// The narrowest focus ring admitted on the painted surface, in pixels.
pub const RING_FLOOR_PX: f64 = 2.0;

/// Measurement tolerance for anti-aliasing quantisation on 8-bit channels.
pub const WIDTH_TOLERANCE_PX: f64 = 0.05;

/// Minimum WCAG 2.x relative luminance contrast ratio for the focus ring.
pub const CONTRAST_FLOOR: f64 = 3.0;

/// The theme focus ring width in pixels: `--concordia-focus-ring-width`.
#[must_use]
pub fn ring_width_px() -> f64 {
    f64::from(FOCUS_RING_WIDTH.0)
}

/// The theme focus ring offset in pixels: `--concordia-focus-ring-offset`.
#[must_use]
pub fn ring_offset_px() -> f64 {
    f64::from(FOCUS_RING_OFFSET.0)
}

/// The screen-pixel geometry of a focused node and its surrounding ring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RingGeometry {
    /// The node's bounding rectangle in screen pixels.
    pub node: ScreenRect,
    /// The stroke width of the focus ring in screen pixels.
    pub width: f64,
    /// The offset between the node boundary and the inner ring edge in screen pixels.
    pub offset: f64,
}

impl RingGeometry {
    /// Returns the outer bounding rectangle of the focus ring.
    #[must_use]
    pub fn outer(&self) -> ScreenRect {
        let delta = self.offset + self.width;
        ScreenRect {
            x0: self.node.x0 - delta,
            y0: self.node.y0 - delta,
            x1: self.node.x1 + delta,
            y1: self.node.y1 + delta,
        }
    }

    /// Returns the inner bounding rectangle of the focus ring (node grown by offset).
    #[must_use]
    pub fn inner(&self) -> ScreenRect {
        ScreenRect {
            x0: self.node.x0 - self.offset,
            y0: self.node.y0 - self.offset,
            x1: self.node.x1 + self.offset,
            y1: self.node.y1 + self.offset,
        }
    }
}

/// Computes the screen-space focus ring for `node` under `camera`.
///
/// The focus ring width and offset are fixed in screen pixels and do not scale
/// with the camera zoom level.
#[must_use]
pub fn screen_space_ring(camera: &Camera, node: &WorldRect) -> RingGeometry {
    RingGeometry {
        node: camera.to_screen(node),
        width: ring_width_px(),
        offset: ring_offset_px(),
    }
}

/// Computes the layer-space focus ring for `node` under `camera`.
///
/// Multiplies the ring width and offset by the camera scale factor, simulating
/// the planted defect where the focus ring scales with canvas zoom.
#[must_use]
pub fn layer_space_ring(camera: &Camera, node: &WorldRect) -> RingGeometry {
    let scale = camera.scale();
    RingGeometry {
        node: camera.to_screen(node),
        width: ring_width_px() * scale,
        offset: ring_offset_px() * scale,
    }
}

/// Formats a [`RingGeometry`] as a standard `RING` line.
#[must_use]
pub fn ring_line(geom: &RingGeometry) -> String {
    format!(
        "RING {:.3} {:.3} {:.3} {:.3} {:.3} {:.3}",
        geom.node.x0, geom.node.y0, geom.node.x1, geom.node.y1, geom.width, geom.offset
    )
}

/// Why a `RING` line could not be parsed into a [`RingGeometry`].
#[derive(Debug, Clone, PartialEq, Error)]
pub enum RingLineError {
    /// The line does not begin with the `"RING "` keyword.
    #[error("missing 'RING' prefix")]
    MissingPrefix,
    /// The line has an unexpected number of whitespace-delimited fields.
    #[error("expected 6 fields after 'RING', got {0}")]
    FieldCount(usize),
    /// A coordinate or dimension is not a finite number.
    #[error("non-finite number in ring line")]
    NonFinite,
    /// A float value could not be parsed.
    #[error("invalid float: {0}")]
    ParseFloat(#[from] std::num::ParseFloatError),
    /// The rectangle right edge is not strictly greater than its left edge.
    #[error("invalid rectangle: x1 <= x0 ({x1} <= {x0})")]
    Inverted {
        /// Left coordinate.
        x0: f64,
        /// Right coordinate.
        x1: f64,
    },
    /// The ring width is zero or negative.
    #[error("invalid ring width: {0} <= 0")]
    NonPositiveWidth(f64),
}

fn parse_finite_f64(s: &str) -> Result<f64, RingLineError> {
    let v: f64 = s.parse()?;
    if !v.is_finite() {
        return Err(RingLineError::NonFinite);
    }
    Ok(v)
}

/// Parses a `RING <x0> <y0> <x1> <y1> <width> <offset>` line.
///
/// # Errors
///
/// Returns [`RingLineError`] if fields are missing, malformed, non-finite,
/// inverted (`x1 <= x0`), or if `width <= 0`.
pub fn parse_ring_line(s: &str) -> Result<RingGeometry, RingLineError> {
    let fields: Vec<&str> = s.split_whitespace().collect();
    let prefix = fields.first().copied();
    if prefix != Some("RING") {
        return Err(RingLineError::MissingPrefix);
    }
    if fields.len() != 7 {
        let field_count = fields.len().saturating_sub(1);
        return Err(RingLineError::FieldCount(field_count));
    }

    let x0 = parse_finite_f64(fields.get(1).copied().unwrap_or_default())?;
    let y0 = parse_finite_f64(fields.get(2).copied().unwrap_or_default())?;
    let x1 = parse_finite_f64(fields.get(3).copied().unwrap_or_default())?;
    let y1 = parse_finite_f64(fields.get(4).copied().unwrap_or_default())?;
    let width = parse_finite_f64(fields.get(5).copied().unwrap_or_default())?;
    let offset = parse_finite_f64(fields.get(6).copied().unwrap_or_default())?;

    if x1 <= x0 {
        return Err(RingLineError::Inverted { x0, x1 });
    }
    if width <= 0.0 {
        return Err(RingLineError::NonPositiveWidth(width));
    }

    Ok(RingGeometry {
        node: ScreenRect { x0, y0, x1, y1 },
        width,
        offset,
    })
}

/// Computes a screen capture bounding box `(x, y, width, height)` enclosing `geom`.
///
/// Floored/ceiled outward plus `margin`, clamped to non-negative bounds.
#[must_use]
pub fn capture_rect(geom: &RingGeometry, margin: i32) -> (i32, i32, i32, i32) {
    let outer = geom.outer();
    let margin_f = f64::from(margin);

    let min_x = (outer.x0.floor() - margin_f).max(0.0);
    let min_y = (outer.y0.floor() - margin_f).max(0.0);
    let max_x = (outer.x1.ceil() + margin_f).max(min_x);
    let max_y = (outer.y1.ceil() + margin_f).max(min_y);

    #[expect(
        clippy::cast_possible_truncation,
        reason = "coordinates clamped >= 0 and within display bounds"
    )]
    let x = min_x as i32;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "coordinates clamped >= 0 and within display bounds"
    )]
    let y = min_y as i32;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "dimensions positive and within display bounds"
    )]
    let w = (max_x - min_x) as i32;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "dimensions positive and within display bounds"
    )]
    let h = (max_y - min_y) as i32;

    (x, y, w, h)
}
