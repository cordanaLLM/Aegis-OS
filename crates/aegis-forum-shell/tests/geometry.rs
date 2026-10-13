// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Canvas geometry: `WorldRect::y1` and `WorldRect::center`.
//!
//! Positive: the bottom edge and the centre of a rectangle are the
//! hand-computed values, and the quadrants split at the centre. Negative: a
//! rectangle that has no valid edges (inverted, NaN, past the range) is
//! refused, so neither accessor can report on it. Boundary: a zero-size
//! rectangle, a unit square (centre on a half), negative coordinates and the
//! full admitted range.

mod common;

use aegis_forum_shell::canvas::geometry::{GeometryError, MAX_COORDINATE, WorldRect};

use common::{Fallible, close};

/// Whether `rect` has the centre `(x, y)`.
fn centred_on(rect: &WorldRect, x: f64, y: f64) -> bool {
    close(rect.center().0, x) && close(rect.center().1, y)
}

// --- Positive -------------------------------------------------------------

/// Positive: for (10, 20) to (30, 60) the bottom edge is 60, distinct from
/// the top edge 20, the right edge 30 and the height 40.
#[test]
fn the_bottom_edge_is_the_maximum_y() -> Fallible {
    let rect = WorldRect::new(10.0, 20.0, 30.0, 60.0)?;
    assert!(close(rect.y1(), 60.0));
    assert!(close(rect.x1(), 30.0));
    assert!(close(rect.y0(), 20.0));
    let sized = WorldRect::from_origin_size(0.0, 0.0, 5.0, 7.0)?;
    assert!(close(sized.y1(), 7.0));
    Ok(())
}

/// Positive: the centre of (10, 20) to (30, 60) is
/// (10 + 20 / 2, 20 + 40 / 2) = (20, 40).
#[test]
fn the_center_is_the_midpoint() -> Fallible {
    let rect = WorldRect::new(10.0, 20.0, 30.0, 60.0)?;
    assert!(centred_on(&rect, 20.0, 40.0), "{:?}", rect.center());
    Ok(())
}

/// Positive: the quadrants meet at the centre: for (0, 0) to (10, 20) the
/// north-west one is (0, 0) to (5, 10) and the south-east one (5, 10) to
/// (10, 20).
#[test]
fn the_quadrants_split_at_the_center() -> Fallible {
    let quadrants = WorldRect::new(0.0, 0.0, 10.0, 20.0)?.quadrants();
    let expected = [
        (0.0, 0.0, 5.0, 10.0),
        (5.0, 0.0, 10.0, 10.0),
        (0.0, 10.0, 5.0, 20.0),
        (5.0, 10.0, 10.0, 20.0),
    ];
    for (quadrant, (x0, y0, x1, y1)) in quadrants.iter().zip(expected) {
        assert!(close(quadrant.x0(), x0) && close(quadrant.y0(), y0));
        assert!(close(quadrant.x1(), x1) && close(quadrant.y1(), y1));
    }
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: no rectangle exists to ask for `y1` or `center` when a minimum
/// edge lies beyond its maximum, a coordinate is NaN or infinite, or one is
/// past `MAX_COORDINATE`.
#[test]
fn a_rectangle_without_valid_edges_does_not_exist() {
    assert_eq!(
        WorldRect::new(30.0, 0.0, 10.0, 5.0),
        Err(GeometryError::Inverted)
    );
    assert_eq!(
        WorldRect::new(0.0, 60.0, 5.0, 20.0),
        Err(GeometryError::Inverted)
    );
    assert_eq!(
        WorldRect::new(0.0, 0.0, 5.0, f64::NAN),
        Err(GeometryError::NonFinite)
    );
    assert_eq!(
        WorldRect::new(0.0, 0.0, 5.0, MAX_COORDINATE + 1.0),
        Err(GeometryError::OutOfRange)
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a zero-size rectangle has its point as centre and as bottom
/// edge; a unit square centres on 0.5 (a half, not a whole).
#[test]
fn a_degenerate_and_a_unit_rectangle_centre_correctly() -> Fallible {
    let point = WorldRect::new(5.0, 7.0, 5.0, 7.0)?;
    assert!(centred_on(&point, 5.0, 7.0));
    assert!(close(point.y1(), 7.0));
    let unit = WorldRect::new(0.0, 0.0, 1.0, 1.0)?;
    assert!(centred_on(&unit, 0.5, 0.5));
    Ok(())
}

/// Boundary: negative coordinates centre by the same rule: (-50, -10) to
/// (-10, 30) has the centre (-30, 10) and the bottom edge 30.
#[test]
fn a_rectangle_across_the_origin_centres_by_the_same_rule() -> Fallible {
    let rect = WorldRect::new(-50.0, -10.0, -10.0, 30.0)?;
    assert!(centred_on(&rect, -30.0, 10.0), "{:?}", rect.center());
    assert!(close(rect.y1(), 30.0));
    let below = WorldRect::new(-8.0, -6.0, -2.0, -4.0)?;
    assert!(close(below.y1(), -4.0));
    assert!(centred_on(&below, -5.0, -5.0));
    Ok(())
}

/// Boundary: the full admitted range centres on the origin, and a rectangle
/// at the positive limit on the limit; `y1` reaches `MAX_COORDINATE`.
#[test]
fn the_full_admitted_range_centres_without_loss() -> Fallible {
    let full = WorldRect::new(
        -MAX_COORDINATE,
        -MAX_COORDINATE,
        MAX_COORDINATE,
        MAX_COORDINATE,
    )?;
    assert!(centred_on(&full, 0.0, 0.0));
    assert!(close(full.y1(), MAX_COORDINATE));
    let corner = WorldRect::new(
        MAX_COORDINATE,
        MAX_COORDINATE,
        MAX_COORDINATE,
        MAX_COORDINATE,
    )?;
    assert!(centred_on(&corner, MAX_COORDINATE, MAX_COORDINATE));
    Ok(())
}
