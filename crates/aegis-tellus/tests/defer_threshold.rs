// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E05-1, the spatiotemporal defer threshold: 300.0 is not a deferral and
//! 300.001 is.
//!
//! The comparison is the scaffold's, `self.current_grid_intensity > 300.0`,
//! and this file is where the two recorded boundary values are exercised
//! literally rather than paraphrased.

mod common;

use aegis_tellus::{
    DEFER_THRESHOLD_G_PER_KWH, EmbodiedCarbon, GridIntensity, MAX_GRID_INTENSITY_G_PER_KWH,
    SciEngine,
};

use common::{EPSILON, Fallible};

/// Builds an engine at `intensity` with the default embodied constant.
fn at(intensity: f64) -> Result<SciEngine, Box<dyn std::error::Error>> {
    Ok(SciEngine::with_inputs(
        GridIntensity::new(intensity)?,
        EmbodiedCarbon::DEFAULT,
    ))
}

// --- Positive -------------------------------------------------------------

/// Positive: a grid dirtier than the threshold defers background work.
#[test]
fn a_dirty_grid_defers_background_work() -> Fallible {
    assert!(at(400.0)?.should_defer());
    assert!(at(MAX_GRID_INTENSITY_G_PER_KWH)?.should_defer());
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a clean grid does not defer, and neither does the default.
///
/// The scaffold's default intensity is 220.0, which is below the threshold, so
/// the daemon it was extracted from reports "Deferral active = false" on its
/// own defaults. That is the behaviour kept here.
#[test]
fn a_clean_grid_does_not_defer() -> Fallible {
    assert!(!at(0.0)?.should_defer());
    assert!(!at(220.0)?.should_defer());
    assert!(!SciEngine::new().should_defer());
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: exactly 300.0 is not a deferral, and 300.001 is.
///
/// These are the two values the recorded acceptance names. The comparison is
/// strict, so the threshold itself sits on the non-deferring side.
#[test]
fn the_threshold_itself_is_not_a_deferral_and_one_thousandth_above_it_is() -> Fallible {
    assert!(
        (DEFER_THRESHOLD_G_PER_KWH - 300.0).abs() < EPSILON,
        "the recorded threshold must be 300.0"
    );
    assert!(
        !at(300.0)?.should_defer(),
        "exactly 300.0 must not defer: the comparison is strict"
    );
    assert!(
        at(300.001)?.should_defer(),
        "300.001 must defer: it is above the threshold"
    );
    assert!((GridIntensity::AT_THRESHOLD.get() - DEFER_THRESHOLD_G_PER_KWH).abs() < EPSILON);
    assert!(
        !SciEngine::with_inputs(GridIntensity::AT_THRESHOLD, EmbodiedCarbon::DEFAULT)
            .should_defer()
    );
    Ok(())
}

/// Boundary: the next representable `f64` above the threshold defers.
///
/// 300.001 is a decimal step; this is the smallest step there is, so the test
/// states that the turn is at the threshold and not somewhere in the gap
/// between it and 300.001.
#[test]
fn the_next_representable_value_above_the_threshold_defers() -> Fallible {
    let next = f64::from_bits(DEFER_THRESHOLD_G_PER_KWH.to_bits().saturating_add(1));
    assert!(next > DEFER_THRESHOLD_G_PER_KWH);
    assert!(
        at(next)?.should_defer(),
        "the smallest representable step above the threshold must defer"
    );

    let previous = f64::from_bits(DEFER_THRESHOLD_G_PER_KWH.to_bits().saturating_sub(1));
    assert!(previous < DEFER_THRESHOLD_G_PER_KWH);
    assert!(!at(previous)?.should_defer());
    Ok(())
}
