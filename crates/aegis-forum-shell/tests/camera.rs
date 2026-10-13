// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The D74 canvas camera: `center`, `to_screen`, `reveal` and `zoom_by`.
//!
//! Every camera below looks at (100, 50) at scale 2 through an 800 by 600
//! viewport. It therefore shows half-widths of `800 / (2 * 2) = 200` and
//! `600 / (2 * 2) = 150` canvas units: the visible region is (-100, -100) to
//! (300, 200), and the screen origin of the viewport centre is (400, 300).
//!
//! Positive: the centre reads back, a rectangle maps to hand-computed pixels,
//! a rectangle out of view is revealed, a zoom factor multiplies the scale.
//! Negative: a rectangle already in view does not move the camera, a camera
//! at the range edge refuses to reveal, a zero, negative or non-finite zoom
//! factor is ignored. Boundary: a rectangle exactly on the visible edge stays
//! put and one unit past moves, a rectangle larger than the view zooms out to
//! `MIN_SCALE` and no further, zooming stops at both scale limits.

mod common;

use aegis_forum_shell::canvas::camera::{Camera, MAX_SCALE, MIN_SCALE, ScreenRect};
use aegis_forum_shell::canvas::geometry::{GeometryError, Viewport, WorldRect};

use common::{Fallible, close};

/// The camera every test starts from.
fn camera() -> Fallible<Camera> {
    Ok(Camera::new(
        (100.0, 50.0),
        2.0,
        Viewport::new(800.0, 600.0)?,
    )?)
}

/// A rectangle from its corners.
fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Fallible<WorldRect> {
    Ok(WorldRect::new(x0, y0, x1, y1)?)
}

/// Whether a screen rectangle has the four given edges.
fn screen_is(actual: ScreenRect, edges: [f64; 4]) -> bool {
    close(actual.x0, edges[0])
        && close(actual.y0, edges[1])
        && close(actual.x1, edges[2])
        && close(actual.y1, edges[3])
}

// --- Positive -------------------------------------------------------------

/// Positive: the centre is the point the camera was built on, as (x, y), and
/// it follows a reveal.
#[test]
fn the_center_is_where_the_camera_looks() -> Fallible {
    let mut camera = camera()?;
    assert!(close(camera.center().0, 100.0));
    assert!(close(camera.center().1, 50.0));
    camera.reveal(&rect(1_000.0, 2_000.0, 1_010.0, 2_020.0)?)?;
    assert!(close(camera.center().0, 1_005.0));
    assert!(close(camera.center().1, 2_010.0));
    Ok(())
}

/// Positive: (90, 40) to (110, 60) maps to
/// x = (90 - 100) * 2 + 400 = 380, y = (40 - 50) * 2 + 300 = 280, and
/// x = (110 - 100) * 2 + 400 = 420, y = (60 - 50) * 2 + 300 = 320.
#[test]
fn a_rectangle_maps_to_hand_computed_pixels() -> Fallible {
    let screen = camera()?.to_screen(&rect(90.0, 40.0, 110.0, 60.0)?);
    assert!(
        screen_is(screen, [380.0, 280.0, 420.0, 320.0]),
        "{screen:?}"
    );
    Ok(())
}

/// Positive: a rectangle out of view moves the camera onto its centre and
/// leaves the scale, which already fits it: (1000, 1000) to (1010, 1010) is
/// 10 by 10, so the fit scales are 80 and 60, both above 2.
#[test]
fn a_rectangle_out_of_view_is_centred() -> Fallible {
    let mut camera = camera()?;
    assert!(camera.reveal(&rect(1_000.0, 1_000.0, 1_010.0, 1_010.0)?)?);
    assert!(close(camera.center().0, 1_005.0));
    assert!(close(camera.center().1, 1_005.0));
    assert!(close(camera.scale(), 2.0));
    Ok(())
}

/// Positive: a rectangle wider than the view zooms the camera out to fit the
/// binding axis: 4000 by 10 gives `800 / 4000 = 0.2` against `600 / 10 = 60`.
#[test]
fn a_wide_rectangle_zooms_out_on_the_width() -> Fallible {
    let mut camera = camera()?;
    assert!(camera.reveal(&rect(0.0, 0.0, 4_000.0, 10.0)?)?);
    assert!(close(camera.scale(), 0.2));
    assert!(close(camera.center().0, 2_000.0));
    assert!(close(camera.center().1, 5.0));
    Ok(())
}

/// Positive: a tall rectangle zooms out on the height: 100 by 3000 gives
/// `800 / 100 = 8` against `600 / 3000 = 0.2`.
#[test]
fn a_tall_rectangle_zooms_out_on_the_height() -> Fallible {
    let mut camera = camera()?;
    assert!(camera.reveal(&rect(0.0, 0.0, 100.0, 3_000.0)?)?);
    assert!(close(camera.scale(), 0.2));
    assert!(close(camera.center().0, 50.0));
    assert!(close(camera.center().1, 1_500.0));
    Ok(())
}

/// Positive: factor 2 doubles the scale (2 to 4) and factor 0.5 halves it
/// (4 to 2, then 2 to 1); factor 1 leaves it.
#[test]
fn a_zoom_factor_multiplies_the_scale() -> Fallible {
    let mut camera = camera()?;
    camera.zoom_by(2.0);
    assert!(close(camera.scale(), 4.0));
    camera.zoom_by(0.5);
    camera.zoom_by(0.5);
    assert!(close(camera.scale(), 1.0));
    camera.zoom_by(1.0);
    assert!(close(camera.scale(), 1.0));
    camera.zoom_by(1.5);
    assert!(close(camera.scale(), 1.5));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a rectangle already inside the visible region reports no move and
/// changes neither centre nor scale.
#[test]
fn a_rectangle_in_view_does_not_move_the_camera() -> Fallible {
    let mut camera = camera()?;
    let before = camera;
    assert!(!camera.reveal(&rect(90.0, 40.0, 110.0, 60.0)?)?);
    assert_eq!(camera, before);
    Ok(())
}

/// Negative: a camera whose visible region would pass the canvas range
/// (centre 1e7, half-width 50 at scale 1 through 100 pixels) refuses with
/// `OutOfRange` and stays where it was.
#[test]
fn a_camera_at_the_range_edge_refuses_to_reveal() -> Fallible {
    let mut camera = Camera::new((1.0e7, 0.0), 1.0, Viewport::new(100.0, 100.0)?)?;
    let before = camera;
    let result = camera.reveal(&rect(0.0, 0.0, 1.0, 1.0)?);
    assert_eq!(result, Err(GeometryError::OutOfRange));
    assert_eq!(camera, before);
    Ok(())
}

/// Negative: a factor of zero, a negative factor, NaN and both infinities are
/// ignored, and the scale stays 2.
#[test]
fn an_unusable_zoom_factor_is_ignored() -> Fallible {
    let mut camera = camera()?;
    for factor in [0.0, -2.0, -0.5, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        camera.zoom_by(factor);
        assert!(close(camera.scale(), 2.0), "factor {factor}");
    }
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a rectangle that is exactly the visible region, (-100, -100) to
/// (300, 200), counts as in view; one unit wider on the left does not, and
/// the camera then centres on (99.5, 50) at `800 / 401` because the rectangle
/// is 401 wide.
#[test]
fn a_rectangle_on_the_visible_edge_stays_and_one_past_moves() -> Fallible {
    let mut camera = camera()?;
    assert!(!camera.reveal(&rect(-100.0, -100.0, 300.0, 200.0)?)?);
    assert!(camera.reveal(&rect(-101.0, -100.0, 300.0, 200.0)?)?);
    assert!(close(camera.center().0, 99.5));
    assert!(close(camera.center().1, 50.0));
    assert!(close(camera.scale(), 800.0 / 401.0));
    Ok(())
}

/// Boundary: a rectangle of a million units would fit at 0.0008; the scale
/// stops at `MIN_SCALE` (0.125), and the camera still centres on it.
#[test]
fn a_huge_rectangle_stops_at_the_minimum_scale() -> Fallible {
    let mut camera = camera()?;
    assert!(camera.reveal(&rect(0.0, 0.0, 1.0e6, 1.0e6)?)?);
    assert!(close(camera.scale(), MIN_SCALE));
    assert!(close(camera.center().0, 5.0e5));
    assert!(close(camera.center().1, 5.0e5));
    Ok(())
}

/// Boundary: a zero-size rectangle out of view has no extent to fit, so it
/// centres without changing the scale.
#[test]
fn a_zero_size_rectangle_is_centred_at_the_current_scale() -> Fallible {
    let mut camera = camera()?;
    assert!(camera.reveal(&rect(1_000.0, 1_000.0, 1_000.0, 1_000.0)?)?);
    assert!(close(camera.scale(), 2.0));
    assert!(close(camera.center().0, 1_000.0));
    Ok(())
}

/// Boundary: a degenerate rectangle at the camera centre maps to the screen
/// centre (400, 300); at the scale limits a 1-unit step spans 8 and 0.125
/// pixels.
#[test]
fn the_screen_mapping_holds_at_the_centre_and_the_scale_limits() -> Fallible {
    let at_center = camera()?.to_screen(&rect(100.0, 50.0, 100.0, 50.0)?);
    assert!(screen_is(at_center, [400.0, 300.0, 400.0, 300.0]));
    let viewport = Viewport::new(800.0, 600.0)?;
    let unit = rect(100.0, 50.0, 101.0, 51.0)?;
    let near = Camera::new((100.0, 50.0), MAX_SCALE, viewport)?.to_screen(&unit);
    assert!(screen_is(near, [400.0, 300.0, 408.0, 308.0]), "{near:?}");
    let far = Camera::new((100.0, 50.0), MIN_SCALE, viewport)?.to_screen(&unit);
    assert!(screen_is(far, [400.0, 300.0, 400.125, 300.125]), "{far:?}");
    let off = camera()?.to_screen(&rect(-1_000.0, -900.0, -990.0, -880.0)?);
    assert!(
        screen_is(off, [-1_800.0, -1_600.0, -1_780.0, -1_560.0]),
        "{off:?}"
    );
    Ok(())
}

/// Boundary: zooming clamps at both limits: 2 times 4 is exactly the maximum
/// 8, times 100 stays 8; 2 times 0.0625 is exactly the minimum 0.125, times
/// `f64::MIN_POSITIVE` stays 0.125.
#[test]
fn zooming_stops_at_both_scale_limits() -> Fallible {
    let mut camera = camera()?;
    camera.zoom_by(4.0);
    assert!(close(camera.scale(), MAX_SCALE));
    camera.zoom_by(100.0);
    assert!(close(camera.scale(), MAX_SCALE));
    camera.zoom_by(0.015_625);
    assert!(close(camera.scale(), MIN_SCALE));
    camera.zoom_by(f64::MIN_POSITIVE);
    assert!(close(camera.scale(), MIN_SCALE));
    Ok(())
}
