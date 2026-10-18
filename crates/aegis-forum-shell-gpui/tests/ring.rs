// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Unit tests for pure focus-ring geometry (M28 E28-4).
//!
//! Tests positive, negative, and boundary cases for screen-space versus
//! layer-space ring geometry, ring line formatting and parsing, and
//! capture rectangle calculation without a display or bus.

use std::str::FromStr;

use aegis_concordia::tokens;
use aegis_forum_shell::canvas::camera::{Camera, MAX_SCALE, MIN_SCALE};
use aegis_forum_shell::canvas::geometry::{Viewport, WorldRect};
use aegis_forum_shell_gpui::ring::{
    capture_rect, layer_space_ring, parse_ring_line, ring_line, ring_offset_px, ring_width_px,
    screen_space_ring,
};
use aegis_forum_shell_gpui::ring_fixture::{
    RING_FIXTURE_HEIGHT, RingVariant, SURFACE_PROBE, geometry, panel, scale, stroke, stroke_color,
};

type Fallible = Result<(), Box<dyn std::error::Error>>;

fn test_camera(s: f64) -> Result<Camera, Box<dyn std::error::Error>> {
    let vp = Viewport::new(1280.0, f64::from(RING_FIXTURE_HEIGHT))?;
    let cam = Camera::new((0.0, 0.0), s, vp)?;
    Ok(cam)
}

// --- Positive -------------------------------------------------------------

/// Positive: `screen_space_ring` maintains width 3.0 and offset 2.0 at MIN, 1.0, and MAX scale.
#[test]
fn screen_space_ring_constant_across_camera_scales() -> Fallible {
    let node = WorldRect::new(-6.0, -2.0, 6.0, 2.0)?;
    for s in [MIN_SCALE, 1.0, MAX_SCALE] {
        let cam = test_camera(s)?;
        let ring = screen_space_ring(&cam, &node);
        assert_eq!(ring.width, 3.0);
        assert_eq!(ring.offset, 2.0);
        assert_eq!(ring.width, ring_width_px());
        assert_eq!(ring.offset, ring_offset_px());
    }
    Ok(())
}

/// Positive: `ring_line` round-trips via `parse_ring_line`.
#[test]
fn ring_line_round_trips_formatted_line() -> Fallible {
    let cam = test_camera(1.0)?;
    let node = WorldRect::new(-6.0, -2.0, 6.0, 2.0)?;
    let ring = screen_space_ring(&cam, &node);
    let line = ring_line(&ring);
    assert!(line.starts_with("RING "));
    let parsed = parse_ring_line(&line)?;
    assert!((parsed.node.x0 - ring.node.x0).abs() < 1e-3);
    assert!((parsed.node.y0 - ring.node.y0).abs() < 1e-3);
    assert!((parsed.node.x1 - ring.node.x1).abs() < 1e-3);
    assert!((parsed.node.y1 - ring.node.y1).abs() < 1e-3);
    assert!((parsed.width - ring.width).abs() < 1e-3);
    assert!((parsed.offset - ring.offset).abs() < 1e-3);
    Ok(())
}

/// Positive: every [`RingVariant`] kebab name round-trips via `FromStr` and `as_str`.
#[test]
fn ring_variant_round_trips_all_names() -> Fallible {
    let variants = [
        RingVariant::Min,
        RingVariant::Unit,
        RingVariant::Max,
        RingVariant::ScaledLayer,
        RingVariant::UnderPanel,
        RingVariant::PartlyCovered,
        RingVariant::Width2,
        RingVariant::Width19,
    ];
    for v in variants {
        let name = v.as_str();
        let parsed = RingVariant::from_str(name)?;
        assert_eq!(parsed, v);
    }
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: `layer_space_ring` at scale 0.5 scales width to 1.5.
#[test]
fn layer_space_ring_scales_width_with_camera() -> Fallible {
    let cam = test_camera(0.5)?;
    let node = WorldRect::new(-6.0, -2.0, 6.0, 2.0)?;
    let ring = layer_space_ring(&cam, &node);
    assert_eq!(ring.width, 1.5);
    assert_eq!(ring.offset, 1.0);
    Ok(())
}

/// Negative: unknown variant string is refused and error lists all allowed names.
#[test]
fn unknown_ring_variant_refused_with_allowed_list() -> Fallible {
    let res = RingVariant::from_str("nonexistent-variant");
    assert!(res.is_err());
    let msg = match res {
        Err(e) => e,
        Ok(v) => return Err(format!("expected Err, got {v:?}").into()),
    };
    let expected = [
        "nonexistent-variant",
        "min",
        "unit",
        "max",
        "scaled-layer",
        "under-panel",
        "partly-covered",
        "width-2-0",
        "width-1-9",
    ];
    for token in expected {
        assert!(msg.contains(token), "missing token: {token}");
    }
    Ok(())
}

/// Negative: `parse_ring_line` refuses lines with missing fields, NaN, inverted rect, or zero width.
#[test]
fn parse_ring_line_refuses_invalid_inputs() {
    // 5 fields (missing offset)
    assert!(parse_ring_line("RING 10.0 20.0 30.0 40.0 3.0").is_err());
    // Not starting with RING
    assert!(parse_ring_line("LINE 10.0 20.0 30.0 40.0 3.0 2.0").is_err());
    // NaN coordinate
    assert!(parse_ring_line("RING NaN 20.0 30.0 40.0 3.0 2.0").is_err());
    // x1 <= x0 (inverted)
    assert!(parse_ring_line("RING 30.0 20.0 10.0 40.0 3.0 2.0").is_err());
    // x1 == x0
    assert!(parse_ring_line("RING 10.0 20.0 10.0 40.0 3.0 2.0").is_err());
    // width <= 0
    assert!(parse_ring_line("RING 10.0 20.0 30.0 40.0 0.0 2.0").is_err());
    assert!(parse_ring_line("RING 10.0 20.0 30.0 40.0 -1.0 2.0").is_err());
}

// --- Boundary -------------------------------------------------------------

/// Boundary: `Width19` geometry width is exactly 1.9, `Width2` is exactly 2.0.
#[test]
fn width_variants_have_exact_boundary_widths() {
    let geom19 = geometry(RingVariant::Width19);
    assert_eq!(geom19.width, 1.9);

    let geom2 = geometry(RingVariant::Width2);
    assert_eq!(geom2.width, 2.0);
}

/// Boundary: `outer()` of every variant lies inside the viewport with >= 4 px margin.
#[test]
fn all_variants_outer_rect_within_viewport_with_margin() {
    let variants = [
        RingVariant::Min,
        RingVariant::Unit,
        RingVariant::Max,
        RingVariant::ScaledLayer,
        RingVariant::UnderPanel,
        RingVariant::PartlyCovered,
        RingVariant::Width2,
        RingVariant::Width19,
    ];
    let vp_width = 1280.0;
    let vp_height = f64::from(RING_FIXTURE_HEIGHT);
    let margin = 4.0;

    for v in variants {
        let geom = geometry(v);
        let outer = geom.outer();
        assert!(
            outer.x0 >= margin,
            "{}: x0 {:.3} < margin {margin}",
            v.as_str(),
            outer.x0
        );
        assert!(
            outer.y0 >= margin,
            "{}: y0 {:.3} < margin {margin}",
            v.as_str(),
            outer.y0
        );
        assert!(
            outer.x1 <= vp_width - margin,
            "{}: x1 {:.3} > max {}",
            v.as_str(),
            outer.x1,
            vp_width - margin
        );
        assert!(
            outer.y1 <= vp_height - margin,
            "{}: y1 {:.3} > max {}",
            v.as_str(),
            outer.y1,
            vp_height - margin
        );
    }
}

/// Boundary: `capture_rect` clamps negative outer bounds at zero.
#[test]
fn capture_rect_clamps_at_zero() -> Fallible {
    let cam = test_camera(1.0)?;
    // Place node at negative screen coords
    let node = WorldRect::new(-1000.0, -1000.0, -900.0, -900.0)?;
    let ring = screen_space_ring(&cam, &node);
    let (x, y, _w, _h) = capture_rect(&ring, 4);
    assert_eq!(x, 0);
    assert_eq!(y, 0);
    Ok(())
}

/// Boundary: scale function returns exact constants for all variants.
#[test]
fn variant_scales_match_constants() {
    assert_eq!(scale(RingVariant::Min), MIN_SCALE);
    assert_eq!(scale(RingVariant::Unit), 1.0);
    assert_eq!(scale(RingVariant::Max), MAX_SCALE);
    assert_eq!(scale(RingVariant::ScaledLayer), 0.5);
    assert_eq!(scale(RingVariant::UnderPanel), 1.0);
    assert_eq!(scale(RingVariant::PartlyCovered), 1.0);
    assert_eq!(scale(RingVariant::Width2), 1.0);
    assert_eq!(scale(RingVariant::Width19), 1.0);
}

#[test]
fn stroke_paints_whole_pixels_with_exact_total_coverage() -> Fallible {
    for (width, whole, coverage) in [
        (3.0, 3.0, 1.0),
        (2.0, 2.0, 1.0),
        (1.9, 2.0, 0.95),
        (1.5, 2.0, 0.75),
    ] {
        let (got_whole, got_coverage) = stroke(width).ok_or("stroke refused a positive width")?;
        assert!((got_whole - whole).abs() < 1e-9, "{width}: {got_whole} px");
        assert!(
            (got_coverage - coverage).abs() < 1e-9,
            "{width}: coverage {got_coverage}"
        );
        assert!(
            (got_whole * got_coverage - width).abs() < 1e-9,
            "{width}: total"
        );
    }
    Ok(())
}

#[test]
fn stroke_refuses_non_positive_and_non_finite_widths() {
    for width in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(stroke(width).is_none(), "{width} accepted");
    }
}

#[test]
fn stroke_color_spans_surface_to_ring_token() {
    assert_eq!(stroke_color(1.0), tokens::COLOR_FOCUS_RING);
    assert_eq!(stroke_color(0.0), tokens::COLOR_SURFACE);
    assert_eq!(stroke_color(7.0), tokens::COLOR_FOCUS_RING);
    assert_eq!(stroke_color(-1.0), tokens::COLOR_SURFACE);
    let mid = stroke_color(0.95);
    assert_ne!(mid, tokens::COLOR_FOCUS_RING);
    assert!(mid.red > tokens::COLOR_FOCUS_RING.red && mid.red < tokens::COLOR_SURFACE.red);
}

#[test]
fn surface_probe_clear_of_every_ring_and_panel() {
    let (px, py) = (f64::from(SURFACE_PROBE.0), f64::from(SURFACE_PROBE.1));
    for v in RingVariant::ALL {
        let outer = geometry(v).outer();
        let mut rects = vec![outer];
        rects.extend(panel(v));
        for r in rects {
            let inside = px >= r.x0 && px < r.x1 && py >= r.y0 && py < r.y1;
            assert!(!inside, "{v:?}: probe {SURFACE_PROBE:?} inside {r:?}");
        }
    }
    assert!(py < f64::from(RING_FIXTURE_HEIGHT));
}
