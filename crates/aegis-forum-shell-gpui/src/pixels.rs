// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Pure focus-ring measurement and contrast evaluation on rendered pixels (M28 E28-4).
//!
//! Provides image representation from raw compositor capture buffers,
//! pixel coverage projection, perpendicular scanline band-width measurement,
//! WCAG 2.x relative luminance contrast ratio calculation, and acceptance rules.
//! This module has no toolkit or display dependencies and is tested purely.

use aegis_concordia::tokens::{COLOR_FOCUS_RING, COLOR_SURFACE};
use thiserror::Error;

use crate::judge::CaseResult;
use crate::ring::{CONTRAST_FLOOR, RING_FLOOR_PX, RingGeometry, WIDTH_TOLERANCE_PX};

fn default_ring_color() -> [u8; 3] {
    [
        COLOR_FOCUS_RING.red,
        COLOR_FOCUS_RING.green,
        COLOR_FOCUS_RING.blue,
    ]
}

fn default_surface_color() -> [u8; 3] {
    [COLOR_SURFACE.red, COLOR_SURFACE.green, COLOR_SURFACE.blue]
}

/// A decoded RGB image buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// Width of the image in pixels.
    pub width: u32,
    /// Height of the image in pixels.
    pub height: u32,
    /// Raw RGB pixels in row-major order, each pixel `[R, G, B]`.
    pub pixels: Vec<[u8; 3]>,
}

impl Image {
    /// Retrieves the pixel at integer coordinate `(x, y)` if within bounds.
    #[must_use]
    pub fn pixel(&self, x: i32, y: i32) -> Option<[u8; 3]> {
        if x < 0 || y < 0 {
            return None;
        }
        let ux = usize::try_from(x).ok()?;
        let uy = usize::try_from(y).ok()?;
        let w = usize::try_from(self.width).ok()?;
        let h = usize::try_from(self.height).ok()?;
        if ux >= w || uy >= h {
            return None;
        }
        let idx = uy.checked_mul(w)?.checked_add(ux)?;
        self.pixels.get(idx).copied()
    }
}

/// Why raw pixel buffer decoding failed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PixelError {
    /// The `QImage` format integer is not supported.
    #[error("unsupported QImage format: {0}")]
    Format(u32),
    /// The row stride is smaller than `width * 4`.
    #[error("stride {stride} is less than minimum width stride {min_stride}")]
    StrideTooSmall {
        /// Actual stride.
        stride: u32,
        /// Minimum required stride.
        min_stride: u32,
    },
    /// The raw byte buffer does not hold `height * stride` bytes.
    #[error("raw buffer too short: expected {expected} bytes, got {actual}")]
    ShortBuffer {
        /// Expected minimum buffer length.
        expected: usize,
        /// Actual buffer length.
        actual: usize,
    },
    /// Coordinate or length calculation overflowed integer bounds.
    #[error("arithmetic overflow during pixel conversion")]
    Overflow,
}

fn validate_buffer(
    bytes: &[u8],
    width: u32,
    height: u32,
    stride: u32,
    format: u32,
) -> Result<bool, PixelError> {
    let is_bgr = matches!(format, 4..=6);
    let is_rgb = matches!(format, 16..=18);
    if !is_bgr && !is_rgb {
        return Err(PixelError::Format(format));
    }

    let min_stride = width.checked_mul(4).ok_or(PixelError::Overflow)?;
    if stride < min_stride {
        return Err(PixelError::StrideTooSmall { stride, min_stride });
    }

    let total_bytes = usize::try_from(height)
        .map_err(|_| PixelError::Overflow)?
        .checked_mul(usize::try_from(stride).map_err(|_| PixelError::Overflow)?)
        .ok_or(PixelError::Overflow)?;

    if bytes.len() < total_bytes {
        return Err(PixelError::ShortBuffer {
            expected: total_bytes,
            actual: bytes.len(),
        });
    }

    Ok(is_bgr)
}

/// Decodes raw pixel bytes from `KWin` `ScreenShot2` into an [`Image`].
///
/// Admitted `QImage` formats:
/// - 4 (RGB32), 5 (ARGB32), 6 (`ARGB32_Premultiplied`): little-endian `[B, G, R, A]`
/// - 16 (RGBX8888), 17 (RGBA8888), 18 (`RGBA8888_Premultiplied`): `[R, G, B, A]`
///
/// Alpha channels are ignored because the shell surface is opaque.
///
/// # Errors
///
/// Returns [`PixelError::Format`] for other format numbers, [`PixelError::StrideTooSmall`]
/// if `stride < width * 4`, and [`PixelError::ShortBuffer`] if buffer is truncated.
pub fn from_raw(
    bytes: &[u8],
    width: u32,
    height: u32,
    stride: u32,
    format: u32,
) -> Result<Image, PixelError> {
    let is_bgr = validate_buffer(bytes, width, height, stride, format)?;

    let total_bytes = usize::try_from(height)
        .map_err(|_| PixelError::Overflow)?
        .checked_mul(usize::try_from(stride).map_err(|_| PixelError::Overflow)?)
        .ok_or(PixelError::Overflow)?;

    if bytes.len() < total_bytes {
        return Err(PixelError::ShortBuffer {
            expected: total_bytes,
            actual: bytes.len(),
        });
    }

    let pixel_count = usize::try_from(width)
        .map_err(|_| PixelError::Overflow)?
        .checked_mul(usize::try_from(height).map_err(|_| PixelError::Overflow)?)
        .ok_or(PixelError::Overflow)?;

    let mut pixels = Vec::with_capacity(pixel_count);
    let stride_usize = usize::try_from(stride).map_err(|_| PixelError::Overflow)?;

    for y in 0..height {
        let row_start = (usize::try_from(y).map_err(|_| PixelError::Overflow)?)
            .checked_mul(stride_usize)
            .ok_or(PixelError::Overflow)?;
        for x in 0..width {
            let col_offset = (usize::try_from(x).map_err(|_| PixelError::Overflow)?)
                .checked_mul(4)
                .ok_or(PixelError::Overflow)?;
            let offset = row_start
                .checked_add(col_offset)
                .ok_or(PixelError::Overflow)?;

            pixels.push(read_pixel(bytes, offset, is_bgr));
        }
    }

    Ok(Image {
        width,
        height,
        pixels,
    })
}

/// Reads the RGB triple at `offset`, swapping byte order for BGR formats.
///
/// Bytes past the end read as zero; [`from_raw`] has checked the length already.
fn read_pixel(bytes: &[u8], offset: usize, is_bgr: bool) -> [u8; 3] {
    let at = |i: usize| {
        bytes
            .get(offset.saturating_add(i))
            .copied()
            .unwrap_or_default()
    };
    if is_bgr {
        [at(2), at(1), at(0)]
    } else {
        [at(0), at(1), at(2)]
    }
}

/// Computes the fractional coverage of `ring` colour in `pixel` relative to `surface`.
///
/// Projection of `(pixel - surface)` onto `(ring - surface)`, clamped to `0.0..=1.0`.
#[must_use]
pub fn coverage(pixel: [u8; 3], ring: [u8; 3], surface: [u8; 3]) -> f64 {
    let vr = f64::from(pixel[0]) - f64::from(surface[0]);
    let vg = f64::from(pixel[1]) - f64::from(surface[1]);
    let vb = f64::from(pixel[2]) - f64::from(surface[2]);

    let ur = f64::from(ring[0]) - f64::from(surface[0]);
    let ug = f64::from(ring[1]) - f64::from(surface[1]);
    let ub = f64::from(ring[2]) - f64::from(surface[2]);

    let dot = vr * ur + vg * ug + vb * ub;
    let norm_sq = ur * ur + ug * ug + ub * ub;

    if norm_sq <= 0.0 {
        return 0.0;
    }
    (dot / norm_sq).clamp(0.0, 1.0)
}

fn channel_luminance(c: u8) -> f64 {
    let s = f64::from(c) / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// Computes the WCAG 2.x relative luminance of an sRGB colour `[R, G, B]`.
fn relative_luminance(c: [u8; 3]) -> f64 {
    let r = channel_luminance(c[0]);
    let g = channel_luminance(c[1]);
    let b = channel_luminance(c[2]);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Computes the WCAG 2.x relative luminance contrast ratio between two colours.
///
/// Returns `(L1 + 0.05) / (L2 + 0.05)` with `L1 >= L2`.
#[must_use]
pub fn contrast_ratio(a: [u8; 3], b: [u8; 3]) -> f64 {
    let l1 = relative_luminance(a);
    let l2 = relative_luminance(b);
    let l_max = l1.max(l2);
    let l_min = l1.min(l2);
    (l_max + 0.05) / (l_min + 0.05)
}

/// One of the four sides of a rectangular node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Left edge.
    Left,
    /// Right edge.
    Right,
    /// Top edge.
    Top,
    /// Bottom edge.
    Bottom,
}

impl Side {
    /// Returns the lowercase name of the side.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Top => "top",
            Self::Bottom => "bottom",
        }
    }
}

/// Measurement result for a single side of the focus ring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SideMeasure {
    /// The side that was measured.
    pub side: Side,
    /// Measured stroke width in screen pixels.
    pub width: f64,
    /// Measured WCAG contrast ratio against the background surface.
    pub contrast: f64,
}

type ScanlineCoords = ((i32, i32), (i32, i32), (i32, i32));

#[expect(
    clippy::cast_possible_truncation,
    reason = "coordinate bounds within screen boundaries"
)]
fn horizontal_scanline_coords(
    geometry: &RingGeometry,
    side: Side,
    origin: (i32, i32),
) -> Option<ScanlineCoords> {
    let inner = geometry.inner();
    let outer = geometry.outer();
    let mid_y = f64::midpoint(geometry.node.y0, geometry.node.y1);
    let y = (mid_y.floor() as i32).checked_sub(origin.1)?;

    if side == Side::Left {
        let x_min = (outer.x0.floor() as i32).checked_sub(1)?;
        let x_max = (inner.x0.ceil() as i32).checked_add(1)?;
        let img_x_min = x_min.checked_sub(origin.0)?;
        let img_x_max = x_max.checked_sub(origin.0)?;
        Some(((img_x_min, y), (img_x_max, y), (img_x_min, y)))
    } else {
        let x_min = (inner.x1.floor() as i32).checked_sub(1)?;
        let x_max = (outer.x1.ceil() as i32).checked_add(1)?;
        let img_x_min = x_min.checked_sub(origin.0)?;
        let img_x_max = x_max.checked_sub(origin.0)?;
        Some(((img_x_min, y), (img_x_max, y), (img_x_max, y)))
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "coordinate bounds within screen boundaries"
)]
fn vertical_scanline_coords(
    geometry: &RingGeometry,
    side: Side,
    origin: (i32, i32),
) -> Option<ScanlineCoords> {
    let inner = geometry.inner();
    let outer = geometry.outer();
    let mid_x = f64::midpoint(geometry.node.x0, geometry.node.x1);
    let x = (mid_x.floor() as i32).checked_sub(origin.0)?;

    if side == Side::Top {
        let y_min = (outer.y0.floor() as i32).checked_sub(1)?;
        let y_max = (inner.y0.ceil() as i32).checked_add(1)?;
        let img_y_min = y_min.checked_sub(origin.1)?;
        let img_y_max = y_max.checked_sub(origin.1)?;
        Some(((x, img_y_min), (x, img_y_max), (x, img_y_min)))
    } else {
        let y_min = (inner.y1.floor() as i32).checked_sub(1)?;
        let y_max = (outer.y1.ceil() as i32).checked_add(1)?;
        let img_y_min = y_min.checked_sub(origin.1)?;
        let img_y_max = y_max.checked_sub(origin.1)?;
        Some(((x, img_y_min), (x, img_y_max), (x, img_y_max)))
    }
}

fn scanline_coords(
    geometry: &RingGeometry,
    side: Side,
    origin: (i32, i32),
) -> Option<ScanlineCoords> {
    match side {
        Side::Left | Side::Right => horizontal_scanline_coords(geometry, side, origin),
        Side::Top | Side::Bottom => vertical_scanline_coords(geometry, side, origin),
    }
}

/// Measures the focus ring band width on a perpendicular scanline through `side`'s midpoint.
///
/// Sums fractional coverage from outer edge - 1 to inner edge + 1. Returns `None`
/// if any scanline pixel leaves the image.
#[must_use]
pub fn band_width(
    img: &Image,
    geometry: &RingGeometry,
    side: Side,
    origin: (i32, i32),
) -> Option<f64> {
    band_width_with_colors(
        img,
        geometry,
        side,
        origin,
        default_ring_color(),
        default_surface_color(),
    )
}

/// Measures band width with custom ring and surface colour tokens.
#[must_use]
pub fn band_width_with_colors(
    img: &Image,
    geometry: &RingGeometry,
    side: Side,
    origin: (i32, i32),
    ring: [u8; 3],
    surface: [u8; 3],
) -> Option<f64> {
    let (start, end, _) = scanline_coords(geometry, side, origin)?;
    let mut sum = 0.0;

    if start.1 == end.1 {
        // Horizontal scanline
        let y = start.1;
        let x_start = start.0.min(end.0);
        let x_end = start.0.max(end.0);
        for x in x_start..=x_end {
            let px = img.pixel(x, y)?;
            sum += coverage(px, ring, surface);
        }
    } else {
        // Vertical scanline
        let x = start.0;
        let y_start = start.1.min(end.1);
        let y_end = start.1.max(end.1);
        for y in y_start..=y_end {
            let px = img.pixel(x, y)?;
            sum += coverage(px, ring, surface);
        }
    }
    Some(sum)
}

/// Measures band width and contrast for each of the four sides of `geometry`.
#[must_use]
pub fn measure(img: &Image, geometry: &RingGeometry, origin: (i32, i32)) -> Vec<SideMeasure> {
    measure_with_colors(
        img,
        geometry,
        origin,
        default_ring_color(),
        default_surface_color(),
    )
}

fn darkest_band_pixel(
    img: &Image,
    start: (i32, i32),
    end: (i32, i32),
    ring: [u8; 3],
    surface: [u8; 3],
    default_px: [u8; 3],
) -> [u8; 3] {
    let mut max_cov = 0.0;
    let mut darkest_px = default_px;
    let is_horizontal = start.1 == end.1;
    let range = if is_horizontal {
        start.0.min(end.0)..=start.0.max(end.0)
    } else {
        start.1.min(end.1)..=start.1.max(end.1)
    };

    for coord in range {
        let (x, y) = if is_horizontal {
            (coord, start.1)
        } else {
            (start.0, coord)
        };
        if let Some(px) = img.pixel(x, y) {
            let cov = coverage(px, ring, surface);
            if cov >= max_cov {
                max_cov = cov;
                darkest_px = px;
            }
        }
    }

    darkest_px
}

/// Measures band width and contrast with custom ring and surface colour tokens.
#[must_use]
pub fn measure_with_colors(
    img: &Image,
    geometry: &RingGeometry,
    origin: (i32, i32),
    ring: [u8; 3],
    surface: [u8; 3],
) -> Vec<SideMeasure> {
    let sides = [Side::Left, Side::Right, Side::Top, Side::Bottom];
    let mut measures = Vec::with_capacity(4);

    for side in sides {
        let Some(width) = band_width_with_colors(img, geometry, side, origin, ring, surface) else {
            continue;
        };

        let Some((start, end, surface_coord)) = scanline_coords(geometry, side, origin) else {
            continue;
        };

        let surface_px = img
            .pixel(surface_coord.0, surface_coord.1)
            .unwrap_or(surface);

        let darkest_px = darkest_band_pixel(img, start, end, ring, surface, surface_px);

        let contrast = contrast_ratio(darkest_px, surface_px);
        measures.push(SideMeasure {
            side,
            width,
            contrast,
        });
    }

    measures
}

/// Evaluates side measures against acceptance criteria.
///
/// Passes when at least one side satisfies `width >= 2.0 - 0.05` and `contrast >= 3.0`.
/// A failing verdict identifies the closest side or indicates that no ring was found.
#[must_use]
pub fn judge_ring(measures: &[SideMeasure], case: &'static str) -> CaseResult {
    let min_width = RING_FLOOR_PX - WIDTH_TOLERANCE_PX;
    if measures
        .iter()
        .any(|m| m.width >= min_width && m.contrast >= CONTRAST_FLOOR)
    {
        return CaseResult {
            name: case,
            passed: true,
            why: String::new(),
        };
    }

    if measures.is_empty() {
        return CaseResult {
            name: case,
            passed: false,
            why: "ring not found on any side".to_owned(),
        };
    }

    let best = measures.iter().max_by(|a, b| {
        a.width
            .total_cmp(&b.width)
            .then_with(|| a.contrast.total_cmp(&b.contrast))
    });

    match best {
        Some(b) if b.width > 0.05 => CaseResult {
            name: case,
            passed: false,
            why: format!(
                "best side {} {:.3} px, {:.2}:1; want >= 2 px and >= 3:1",
                b.side.as_str(),
                b.width,
                b.contrast
            ),
        },
        _ => CaseResult {
            name: case,
            passed: false,
            why: "ring not found on any side".to_owned(),
        },
    }
}
