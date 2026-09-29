// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The canvas camera (D74): a centre in canvas units, a scale and the
//! viewport it paints into.
//!
//! The camera decides what is painted and nothing else. The accessibility
//! export reads the camera only to place each node's screen bounds; it never
//! reads it to decide whether a node is exported (REQ-P05-11).

use super::geometry::{GeometryError, Viewport, WorldRect};

/// The smallest camera scale: one screen pixel shows eight canvas units.
pub const MIN_SCALE: f64 = 0.125;

/// The largest camera scale: one canvas unit fills eight screen pixels.
pub const MAX_SCALE: f64 = 8.0;

/// A rectangle in screen pixels, relative to the viewport's top-left corner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenRect {
    /// The left edge.
    pub x0: f64,
    /// The top edge.
    pub y0: f64,
    /// The right edge.
    pub x1: f64,
    /// The bottom edge.
    pub y1: f64,
}

/// Where the camera looks and how far it is zoomed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    center_x: f64,
    center_y: f64,
    scale: f64,
    viewport: Viewport,
}

/// Clamps a scale into the admitted range.
fn clamp_scale(scale: f64) -> f64 {
    scale.clamp(MIN_SCALE, MAX_SCALE)
}

impl Camera {
    /// Builds a camera looking at `center` at `scale`.
    ///
    /// # Errors
    ///
    /// Returns [`GeometryError::NonFinite`] for a centre or scale that is not
    /// finite, [`GeometryError::OutOfRange`] for a centre outside the canvas
    /// range or a scale outside [`MIN_SCALE`] to [`MAX_SCALE`].
    pub fn new(center: (f64, f64), scale: f64, viewport: Viewport) -> Result<Self, GeometryError> {
        let probe = WorldRect::new(center.0, center.1, center.0, center.1)?;
        if !scale.is_finite() {
            return Err(GeometryError::NonFinite);
        }
        if !(MIN_SCALE..=MAX_SCALE).contains(&scale) {
            return Err(GeometryError::OutOfRange);
        }
        let (center_x, center_y) = probe.center();
        Ok(Self {
            center_x,
            center_y,
            scale,
            viewport,
        })
    }

    /// The camera centre, in canvas units.
    #[must_use]
    pub const fn center(&self) -> (f64, f64) {
        (self.center_x, self.center_y)
    }

    /// The camera scale.
    #[must_use]
    pub const fn scale(&self) -> f64 {
        self.scale
    }

    /// The viewport the camera paints into.
    #[must_use]
    pub const fn viewport(&self) -> Viewport {
        self.viewport
    }

    /// The region of the canvas the viewport shows: the cull bounds.
    ///
    /// # Errors
    ///
    /// Returns [`GeometryError::OutOfRange`] when the visible region would
    /// reach past the admitted canvas range, which a camera built by
    /// [`Camera::new`] near the range's edge can produce.
    pub fn visible(&self) -> Result<WorldRect, GeometryError> {
        let half_width = self.viewport.width() / (2.0 * self.scale);
        let half_height = self.viewport.height() / (2.0 * self.scale);
        WorldRect::new(
            self.center_x - half_width,
            self.center_y - half_height,
            self.center_x + half_width,
            self.center_y + half_height,
        )
    }

    /// Maps a canvas rectangle to screen pixels, relative to the viewport.
    #[must_use]
    pub fn to_screen(&self, rect: &WorldRect) -> ScreenRect {
        let origin_x = self.viewport.width() / 2.0;
        let origin_y = self.viewport.height() / 2.0;
        ScreenRect {
            x0: (rect.x0() - self.center_x) * self.scale + origin_x,
            y0: (rect.y0() - self.center_y) * self.scale + origin_y,
            x1: (rect.x1() - self.center_x) * self.scale + origin_x,
            y1: (rect.y1() - self.center_y) * self.scale + origin_y,
        }
    }

    /// Moves the camera until `rect` is in view (REQ-P05-09).
    ///
    /// A rectangle already inside the visible region leaves the camera where
    /// it is. Otherwise the camera centres on it, and zooms out first when the
    /// rectangle is larger than the viewport shows at the current scale, down
    /// to [`MIN_SCALE`]. Returns whether the camera moved.
    ///
    /// # Errors
    ///
    /// Propagates [`Camera::visible`].
    pub fn reveal(&mut self, rect: &WorldRect) -> Result<bool, GeometryError> {
        if self.visible()?.contains(rect) {
            return Ok(false);
        }
        let fit_x = self.viewport.width() / rect.width().max(f64::MIN_POSITIVE);
        let fit_y = self.viewport.height() / rect.height().max(f64::MIN_POSITIVE);
        self.scale = clamp_scale(self.scale.min(fit_x).min(fit_y));
        let (center_x, center_y) = rect.center();
        self.center_x = center_x;
        self.center_y = center_y;
        Ok(true)
    }

    /// Multiplies the scale by `factor`, clamped to the admitted range.
    pub fn zoom_by(&mut self, factor: f64) {
        if factor.is_finite() && factor > 0.0 {
            self.scale = clamp_scale(self.scale * factor);
        }
    }

    /// Centres the camera on `rect` and scales it to fit, clamped.
    pub fn fit(&mut self, rect: &WorldRect) {
        let fit_x = self.viewport.width() / rect.width().max(f64::MIN_POSITIVE);
        let fit_y = self.viewport.height() / rect.height().max(f64::MIN_POSITIVE);
        self.scale = clamp_scale(fit_x.min(fit_y));
        let (center_x, center_y) = rect.center();
        self.center_x = center_x;
        self.center_y = center_y;
    }
}
