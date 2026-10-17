// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The generated constants against D16, the reduced-motion and forced-colours
//! overrides, and the crate's toolkit-free closure (D78, D101).
//!
//! Whether the constants equal the token file is `tools/test_concordia_theme.py`'s
//! check, which regenerates them; these tests hold the properties the shell
//! relies on whatever the values are.

#[path = "../../dependency_closure.rs"]
mod dependency_closure;

use aegis_concordia::tokens::{
    COLOR_FOCUS_RING, FOCUS_RING_STYLE, FOCUS_RING_WIDTH, FORCED_COLORS_COLOR_BORDER,
    FORCED_COLORS_COLOR_FOCUS_RING, MOTION_DURATION, REDUCED_MOTION_MOTION_DURATION,
};
use aegis_concordia::{FOCUS_RING_FLOOR, LineStyle, Millis, Px, SystemColor};

/// This crate's package name, the start of its closure.
const PACKAGE: &str = "aegis-concordia";

/// Toolkit, accessibility and bus crates the token crate must not resolve:
/// the M16 model and every crate that reads the tokens stay toolkit-free.
const FORBIDDEN: [&str; 8] = [
    "gpui",
    "accesskit",
    "accesskit_unix",
    "wayland-client",
    "smithay-client-toolkit",
    "zbus",
    "atspi",
    "tokio",
];

// --- Positive -------------------------------------------------------------

/// Positive (D16): the focus ring is 3 px, solid, and at least the floor.
#[test]
fn the_focus_ring_is_three_pixels_and_solid() {
    assert_eq!(FOCUS_RING_WIDTH, Px(3));
    assert!(FOCUS_RING_WIDTH >= FOCUS_RING_FLOOR);
    assert_eq!(FOCUS_RING_STYLE, LineStyle::Solid);
}

/// Positive: reduced motion sets the motion duration to zero.
#[test]
fn reduced_motion_stops_motion() {
    assert!(MOTION_DURATION > Millis(0));
    assert_eq!(REDUCED_MOTION_MOTION_DURATION, Millis(0));
}

/// Positive: forced colours hand the ring and the border to system colours.
#[test]
fn forced_colours_name_system_colours() {
    assert_eq!(FORCED_COLORS_COLOR_FOCUS_RING, SystemColor::Highlight);
    assert_eq!(FORCED_COLORS_COLOR_BORDER, SystemColor::CanvasText);
}

// --- Negative -------------------------------------------------------------

/// Negative (D78, D101): the closure resolves no toolkit, accessibility or bus crate.
#[test]
fn the_closure_resolves_no_toolkit() -> Result<(), String> {
    let names = dependency_closure::own_closure(PACKAGE)?;
    let found = dependency_closure::hits(&names, &FORBIDDEN);
    assert!(
        found.is_empty(),
        "the closure of {PACKAGE} resolves {found:?}"
    );
    assert!(names.contains(PACKAGE));
    Ok(())
}

/// Negative: the sweep would notice gpui planted in this closure.
#[test]
fn the_sweep_detects_a_planted_toolkit() {
    let planted = dependency_closure::synthetic(PACKAGE, &["gpui"]);
    let names = dependency_closure::closure(&planted, PACKAGE);
    assert!(matches!(
        names.map(|names| dependency_closure::hits(&names, &FORBIDDEN)),
        Ok(found) if found == ["gpui"]
    ));
}

/// Negative: the focus-ring colour differs from the surface it is drawn on.
#[test]
fn the_focus_ring_colour_is_not_the_surface() {
    assert_ne!(COLOR_FOCUS_RING, aegis_concordia::tokens::COLOR_SURFACE);
}

// --- Boundary -------------------------------------------------------------

/// Boundary (D16): the floor is exactly 2 px, one below the token.
#[test]
fn the_floor_is_two_pixels() {
    assert_eq!(FOCUS_RING_FLOOR, Px(2));
    assert!(Px(2) >= FOCUS_RING_FLOOR);
    assert!(Px(1) < FOCUS_RING_FLOOR);
}
