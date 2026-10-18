// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Unit tests for pure pixel measurement and contrast evaluation (M28 E28-4).
//!
//! Tests positive, negative, and boundary cases for synthetic image generation,
//! raw byte decoding, band width measurement, WCAG contrast calculation,
//! and focus ring acceptance judgement without a compositor or display.

use aegis_concordia::tokens::{COLOR_FOCUS_RING, COLOR_SURFACE, COLOR_SURFACE_RAISED};
use aegis_forum_shell::canvas::camera::ScreenRect;
use aegis_forum_shell_gpui::pixels::{
    Image, PixelError, Side, SideMeasure, contrast_ratio, from_raw, judge_ring, measure,
    measure_with_colors,
};
use aegis_forum_shell_gpui::ring::{RingGeometry, ring_offset_px};

type Fallible = Result<(), Box<dyn std::error::Error>>;

const RING_U8: [u8; 3] = [
    COLOR_FOCUS_RING.red,
    COLOR_FOCUS_RING.green,
    COLOR_FOCUS_RING.blue,
];
const SURFACE_U8: [u8; 3] = [COLOR_SURFACE.red, COLOR_SURFACE.green, COLOR_SURFACE.blue];
const RAISED_U8: [u8; 3] = [
    COLOR_SURFACE_RAISED.red,
    COLOR_SURFACE_RAISED.green,
    COLOR_SURFACE_RAISED.blue,
];

fn blend(ring: [u8; 3], surface: [u8; 3], frac: f64) -> [u8; 3] {
    let clamp_f = frac.clamp(0.0, 1.0);
    let r = (f64::from(ring[0]) * clamp_f + f64::from(surface[0]) * (1.0 - clamp_f)).round();
    let g = (f64::from(ring[1]) * clamp_f + f64::from(surface[1]) * (1.0 - clamp_f)).round();
    let b = (f64::from(ring[2]) * clamp_f + f64::from(surface[2]) * (1.0 - clamp_f)).round();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "channel bounds 0..255"
    )]
    [r as u8, g as u8, b as u8]
}

fn test_geometry(width: f64) -> RingGeometry {
    RingGeometry {
        node: ScreenRect {
            x0: 50.0,
            y0: 50.0,
            x1: 110.0,
            y1: 80.0,
        },
        width,
        offset: ring_offset_px(),
    }
}

fn pixel_index(width: usize, height: usize, x: i32, y: i32) -> Option<usize> {
    let ux = usize::try_from(x).ok()?;
    let uy = usize::try_from(y).ok()?;
    if ux >= width || uy >= height {
        return None;
    }
    uy.checked_mul(width)?.checked_add(ux)
}

fn set_px(pixels: &mut [[u8; 3]], width: usize, height: usize, x: i32, y: i32, col: [u8; 3]) {
    if let Some(slot) = pixel_index(width, height, x, y).and_then(|idx| pixels.get_mut(idx)) {
        *slot = col;
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "test synthetic image geometry"
)]
fn paint_left_right(
    pixels: &mut [[u8; 3]],
    inner: &ScreenRect,
    n_full: i32,
    frac: f64,
    colors: ([u8; 3], [u8; 3]),
    visible: (bool, bool),
) {
    let (ring_col, frac_col) = colors;
    if visible.0 {
        let x_inner = inner.x0.floor() as i32;
        let y_start = (inner.y0.floor() as i32).saturating_sub(5);
        let y_end = (inner.y1.ceil() as i32).saturating_add(5);
        for y in y_start..=y_end {
            for step in 1..=n_full {
                set_px(pixels, 160, 130, x_inner.saturating_sub(step), y, ring_col);
            }
            if frac > 0.001 {
                let x_frac = x_inner.saturating_sub(n_full).saturating_sub(1);
                set_px(pixels, 160, 130, x_frac, y, frac_col);
            }
        }
    }
    if visible.1 {
        let x_inner = inner.x1.ceil() as i32;
        let y_start = (inner.y0.floor() as i32).saturating_sub(5);
        let y_end = (inner.y1.ceil() as i32).saturating_add(5);
        for y in y_start..=y_end {
            for step in 0..n_full {
                set_px(pixels, 160, 130, x_inner.saturating_add(step), y, ring_col);
            }
            if frac > 0.001 {
                let x_frac = x_inner.saturating_add(n_full);
                set_px(pixels, 160, 130, x_frac, y, frac_col);
            }
        }
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "test synthetic image geometry"
)]
fn paint_top_bottom(
    pixels: &mut [[u8; 3]],
    inner: &ScreenRect,
    n_full: i32,
    frac: f64,
    colors: ([u8; 3], [u8; 3]),
    visible: (bool, bool),
) {
    let (ring_col, frac_col) = colors;
    if visible.0 {
        let y_inner = inner.y0.floor() as i32;
        let x_start = (inner.x0.floor() as i32).saturating_sub(5);
        let x_end = (inner.x1.ceil() as i32).saturating_add(5);
        for x in x_start..=x_end {
            for step in 1..=n_full {
                set_px(pixels, 160, 130, x, y_inner.saturating_sub(step), ring_col);
            }
            if frac > 0.001 {
                let y_frac = y_inner.saturating_sub(n_full).saturating_sub(1);
                set_px(pixels, 160, 130, x, y_frac, frac_col);
            }
        }
    }
    if visible.1 {
        let y_inner = inner.y1.ceil() as i32;
        let x_start = (inner.x0.floor() as i32).saturating_sub(5);
        let x_end = (inner.x1.ceil() as i32).saturating_add(5);
        for x in x_start..=x_end {
            for step in 0..n_full {
                set_px(pixels, 160, 130, x, y_inner.saturating_add(step), ring_col);
            }
            if frac > 0.001 {
                let y_frac = y_inner.saturating_add(n_full);
                set_px(pixels, 160, 130, x, y_frac, frac_col);
            }
        }
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "test geometry width conversion"
)]
fn synthesize_image(
    geom: &RingGeometry,
    ring_color: [u8; 3],
    surface_color: [u8; 3],
    visible_sides: [bool; 4], // [Left, Right, Top, Bottom]
) -> Image {
    let width = 160u32;
    let height = 130u32;
    let mut pixels = vec![surface_color; 160 * 130];

    let inner = geom.inner();
    let n_full = geom.width.floor() as i32;
    let frac = geom.width - geom.width.floor();
    let frac_col = blend(ring_color, surface_color, frac);

    paint_left_right(
        &mut pixels,
        &inner,
        n_full,
        frac,
        (ring_color, frac_col),
        (visible_sides[0], visible_sides[1]),
    );
    paint_top_bottom(
        &mut pixels,
        &inner,
        n_full,
        frac,
        (ring_color, frac_col),
        (visible_sides[2], visible_sides[3]),
    );

    Image {
        width,
        height,
        pixels,
    }
}

// --- Positive -------------------------------------------------------------

/// Positive: 3.0 px band on `COLOR_SURFACE` measures ~3.0 (within 0.02) and passes contrast.
#[test]
fn synthetic_3px_ring_passes_measurement_and_contrast() {
    let geom = test_geometry(3.0);
    let img = synthesize_image(&geom, RING_U8, SURFACE_U8, [true, true, true, true]);
    let origin = (0, 0);

    let measures = measure(&img, &geom, origin);
    assert_eq!(measures.len(), 4);

    let expected_contrast = contrast_ratio(RING_U8, SURFACE_U8);
    for m in &measures {
        assert!(
            (m.width - 3.0).abs() < 0.02,
            "{:?}: width {:.3} outside tolerance of 3.0",
            m.side,
            m.width
        );
        assert!(
            (m.contrast - expected_contrast).abs() < 0.1,
            "{:?}: contrast {:.2} outside token ratio {:.2}",
            m.side,
            m.contrast,
            expected_contrast
        );
    }

    let result = judge_ring(&measures, "ring/unit");
    assert!(result.passed, "judge_ring failed: {}", result.why);
    assert_eq!(result.why, "");
}

/// Positive: `from_raw` decodes format 16 (RGBX8888) and format 5 (ARGB32) to the same Image.
#[test]
fn from_raw_decodes_formats_16_and_5_identically() -> Fallible {
    let width = 2u32;
    let height = 2u32;
    let stride = 8u32; // 2 pixels * 4 bytes

    // Format 16: memory order [R, G, B, X]
    let bytes_fmt16: Vec<u8> = vec![
        10, 88, 202, 255, 255, 255, 255, 255, 243, 244, 246, 255, 0, 0, 0, 255,
    ];

    // Format 5: little-endian ARGB32, memory order [B, G, R, A]
    let bytes_fmt5: Vec<u8> = vec![
        202, 88, 10, 255, 255, 255, 255, 255, 246, 244, 243, 255, 0, 0, 0, 255,
    ];

    let img16 = from_raw(&bytes_fmt16, width, height, stride, 16)?;
    let img5 = from_raw(&bytes_fmt5, width, height, stride, 5)?;

    assert_eq!(img16.width, width);
    assert_eq!(img16.height, height);
    assert_eq!(img16.pixels, img5.pixels);
    assert_eq!(img16.pixels.first().copied(), Some([10, 88, 202]));
    assert_eq!(img16.pixels.get(1).copied(), Some([255, 255, 255]));
    assert_eq!(img16.pixels.get(2).copied(), Some([243, 244, 246]));
    assert_eq!(img16.pixels.get(3).copied(), Some([0, 0, 0]));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: 1.5 px band fails naming "1.5".
#[test]
fn synthetic_1p5px_ring_fails_naming_width() {
    let geom = test_geometry(1.5);
    let img = synthesize_image(&geom, RING_U8, SURFACE_U8, [true, true, true, true]);
    let measures = measure(&img, &geom, (0, 0));
    let result = judge_ring(&measures, "ring/scaled-layer");
    assert!(!result.passed);
    assert!(
        result.why.contains("want >= 2 px")
            && (result.why.contains("1.49") || result.why.contains("1.50")),
        "unexpected message: {}",
        result.why
    );
}

/// Negative: band fully covered by `COLOR_SURFACE_RAISED` fails with low width on all sides.
#[test]
fn ring_under_panel_fails_with_low_width() {
    let geom = test_geometry(3.0);
    // Draw whole image with COLOR_SURFACE_RAISED
    let img = synthesize_image(&geom, RAISED_U8, RAISED_U8, [false, false, false, false]);
    let measures = measure(&img, &geom, (0, 0));
    for m in &measures {
        assert!(m.width < 0.5, "{:?} width {} >= 0.5", m.side, m.width);
    }
    let result = judge_ring(&measures, "ring/under-panel");
    assert!(!result.passed);
    assert!(
        result.why.contains("ring not found") || result.why.contains("0."),
        "unexpected message: {}",
        result.why
    );
}

/// Negative: low-contrast ring colour (200, 200, 200 on white) at 3 px fails naming the ratio.
#[test]
fn low_contrast_ring_fails_naming_ratio() {
    let geom = test_geometry(3.0);
    let low_contrast = [200, 200, 200];
    let img = synthesize_image(&geom, low_contrast, SURFACE_U8, [true, true, true, true]);
    let measures = measure_with_colors(&img, &geom, (0, 0), low_contrast, SURFACE_U8);
    let result = judge_ring(&measures, "ring/low-contrast");
    assert!(!result.passed);
    assert!(
        result.why.contains(":1") && (result.why.contains("1.6") || result.why.contains("1.7")),
        "why text '{}' did not name the ratio",
        result.why
    );
}

/// Negative: format 3 is refused naming 3.
#[test]
fn format_3_refused_naming_number() {
    let bytes = vec![0u8; 16];
    let res = from_raw(&bytes, 2, 2, 8, 3);
    assert_eq!(res, Err(PixelError::Format(3)));
}

/// Negative: short buffer is refused naming expected and actual lengths.
#[test]
fn short_buffer_refused() {
    let bytes = vec![0u8; 10]; // expected 2 * 8 = 16
    let res = from_raw(&bytes, 2, 2, 8, 16);
    assert_eq!(
        res,
        Err(PixelError::ShortBuffer {
            expected: 16,
            actual: 10,
        })
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: 2.0 px ring passes and 1.9 px ring fails.
#[test]
fn boundary_2px_passes_and_1p9px_fails() {
    // 2.0 px
    let geom2 = test_geometry(2.0);
    let img2 = synthesize_image(&geom2, RING_U8, SURFACE_U8, [true, true, true, true]);
    let m2 = measure(&img2, &geom2, (0, 0));
    let r2 = judge_ring(&m2, "ring/width-2-0");
    assert!(r2.passed, "2.0 px should pass: {}", r2.why);

    // 1.9 px
    let geom19 = test_geometry(1.9);
    let img19 = synthesize_image(&geom19, RING_U8, SURFACE_U8, [true, true, true, true]);
    let m19 = measure(&img19, &geom19, (0, 0));
    let r19 = judge_ring(&m19, "ring/width-1-9");
    assert!(!r19.passed, "1.9 px should fail: {}", r19.why);
    assert!(
        r19.why.contains("want >= 2 px") && (r19.why.contains("1.89") || r19.why.contains("1.90")),
        "unexpected message: {}",
        r19.why
    );
}

/// Boundary: partly covered node (only right side visible) passes.
#[test]
fn partly_covered_node_with_only_right_side_visible_passes() {
    let geom = test_geometry(3.0);
    // Only right side visible: [false, true, false, false]
    let img = synthesize_image(&geom, RING_U8, SURFACE_U8, [false, true, false, false]);
    let measures = measure(&img, &geom, (0, 0));
    let result = judge_ring(&measures, "ring/partly-covered");
    assert!(
        result.passed,
        "partly covered node should pass on right side: {}",
        result.why
    );
}

/// Boundary: `contrast_ratio` on black/white is 21.0, same colour is 1.0; contrast exactly 3.0 passes.
#[test]
fn contrast_ratio_boundary_values() {
    let black = [0, 0, 0];
    let white = [255, 255, 255];
    assert!((contrast_ratio(black, white) - 21.0).abs() < 1e-4);
    assert!((contrast_ratio([100, 100, 100], [100, 100, 100]) - 1.0).abs() < 1e-4);

    // Color with contrast >= 3.0 against white: [148, 148, 148] gives ~ 3.02:1
    let gray_pass = [148, 148, 148];
    assert!(contrast_ratio(gray_pass, white) >= 3.0);

    // Color with contrast < 3.0 against white: [149, 149, 149] gives ~ 2.98:1
    let gray_fail = [149, 149, 149];
    assert!(contrast_ratio(gray_fail, white) < 3.0);

    // A measure with width 2.0 and contrast 3.0 passes
    let boundary_measure = vec![SideMeasure {
        side: Side::Right,
        width: 2.0,
        contrast: 3.0,
    }];
    let r = judge_ring(&boundary_measure, "ring/contrast-boundary");
    assert!(r.passed);
}
