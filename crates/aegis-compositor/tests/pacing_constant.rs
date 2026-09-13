// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E07-1, the boundary case: the pacing constant, pinned.
//!
//! REQ-P04-08 records three figures that cannot all describe the same loop and
//! asks for the constant to be pinned. This file is the pin.
//!
//! Positive: the pinned value is the refresh-rate reading, and it is exactly
//! the period that reading implies. Negative: it is **not** either of the
//! other two recorded figures, and it is not a measurement. Boundary: the
//! period arithmetic is exact at both ends of the admissible rate range, and
//! zero has no period at all.
//!
//! Nothing here runs a loop, sleeps or reads a clock. Whether an
//! implementation can hold this period is a question for real hardware at
//! milestone M12 and for the latency fixtures at M23.

mod common;

use aegis_compositor::{
    CompositorError, DeclaredPeriodUs, MAX_REFRESH_HZ, MIN_REFRESH_HZ, PACING_COMMENT_REFRESH_HZ,
    PINNED_FRAME_PERIOD_US, PacingSource, RENDER_IPC_BUDGET_US, SCAFFOLD_EVENT_LOOP_CYCLES,
    SCAFFOLD_LOOP_PERIOD_US, budget_fits_in_period, frame_period_us,
};

// --- Positive -------------------------------------------------------------

/// Positive: the pinned constant is the period the recorded refresh rate
/// implies, and says which reading it came from.
#[test]
fn the_pinned_constant_is_the_refresh_rate_period() -> Result<(), CompositorError> {
    assert_eq!(PINNED_FRAME_PERIOD_US.micros(), 6_944);
    assert_eq!(
        PINNED_FRAME_PERIOD_US.micros(),
        frame_period_us(PACING_COMMENT_REFRESH_HZ)?
    );
    assert_eq!(
        PINNED_FRAME_PERIOD_US.source(),
        PacingSource::RefreshRateComment
    );
    assert_eq!(PACING_COMMENT_REFRESH_HZ, 144);
    Ok(())
}

/// Positive: the three recorded figures are each recorded, and each is
/// distinct.
#[test]
fn the_three_recorded_figures_are_distinct() {
    assert_eq!(SCAFFOLD_LOOP_PERIOD_US, 100);
    assert_eq!(RENDER_IPC_BUDGET_US, 1_500);
    assert_eq!(SCAFFOLD_EVENT_LOOP_CYCLES, 200);
    let figures = [
        SCAFFOLD_LOOP_PERIOD_US,
        RENDER_IPC_BUDGET_US,
        PINNED_FRAME_PERIOD_US.micros(),
    ];
    let mut sorted = figures.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 3);
    assert_eq!(PacingSource::ALL.len(), 3);
}

/// Positive: the per-frame work budget fits inside the pinned period, which is
/// the consistency check the pinned value has to satisfy.
#[test]
fn the_work_budget_fits_inside_the_pinned_period() {
    assert!(budget_fits_in_period(PINNED_FRAME_PERIOD_US));
    assert!(RENDER_IPC_BUDGET_US < PINNED_FRAME_PERIOD_US.micros());
}

/// Positive: each reading carries a distinct name, and only two of the three
/// are periods at all.
#[test]
fn only_two_of_the_three_readings_are_periods() {
    assert!(PacingSource::ScaffoldLiteral.is_a_period());
    assert!(PacingSource::RefreshRateComment.is_a_period());
    assert!(!PacingSource::RenderIpcBudget.is_a_period());
    let mut names: Vec<&str> = PacingSource::ALL.iter().map(|s| s.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 3);
    assert_eq!(PacingSource::RenderIpcBudget.name(), "render-ipc-budget");
}

// --- Negative -------------------------------------------------------------

/// Negative: the pinned constant is neither of the other two figures.
///
/// The scaffold's own literal is a 10 kHz period, about sixty-nine times the
/// rate its own comment names; the budget is work inside a frame rather than
/// the interval between frames.
#[test]
fn the_pinned_constant_is_not_the_other_two_figures() {
    assert_ne!(PINNED_FRAME_PERIOD_US.micros(), SCAFFOLD_LOOP_PERIOD_US);
    assert_ne!(PINNED_FRAME_PERIOD_US.micros(), RENDER_IPC_BUDGET_US);
    assert!(PINNED_FRAME_PERIOD_US.micros() > SCAFFOLD_LOOP_PERIOD_US);
    // The scaffold's literal would wake the loop sixty-nine times per frame.
    let wakeups = PINNED_FRAME_PERIOD_US
        .micros()
        .checked_div(SCAFFOLD_LOOP_PERIOD_US)
        .unwrap_or(0);
    assert_eq!(wakeups, 69);
}

/// Negative: the pinned value says it is unmeasured when it is rendered, so a
/// figure copied out of a log carries its own provenance.
#[test]
fn the_pinned_value_says_it_is_unmeasured() {
    let rendered = PINNED_FRAME_PERIOD_US.to_string();
    assert!(rendered.contains("unmeasured"));
    assert!(rendered.contains("declared by refresh-rate-comment"));
    assert!(rendered.starts_with("6944 us"));
}

/// Negative: a budget that did not fit inside a period would be reported as
/// not fitting, so the consistency check is not vacuous.
#[test]
fn the_consistency_check_can_fail() {
    let too_short = DeclaredPeriodUs::new(RENDER_IPC_BUDGET_US, PacingSource::RenderIpcBudget);
    assert!(!budget_fits_in_period(too_short));
    let shorter = DeclaredPeriodUs::new(SCAFFOLD_LOOP_PERIOD_US, PacingSource::ScaffoldLiteral);
    assert!(!budget_fits_in_period(shorter));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the period arithmetic is exact at both ends of the rate range,
/// and a rate of zero has no period.
#[test]
fn the_period_arithmetic_is_exact_at_both_ends() -> Result<(), CompositorError> {
    assert_eq!(frame_period_us(MIN_REFRESH_HZ)?, 1_000_000);
    assert_eq!(frame_period_us(MAX_REFRESH_HZ)?, 1);
    assert_eq!(frame_period_us(60)?, 16_666);
    assert_eq!(
        frame_period_us(0),
        Err(CompositorError::RefreshRateOutOfRange {
            hz: 0,
            min: MIN_REFRESH_HZ,
            max: MAX_REFRESH_HZ,
        })
    );
    assert_eq!(
        frame_period_us(MAX_REFRESH_HZ.saturating_add(1)),
        Err(CompositorError::RefreshRateOutOfRange {
            hz: MAX_REFRESH_HZ.saturating_add(1),
            min: MIN_REFRESH_HZ,
            max: MAX_REFRESH_HZ,
        })
    );
    assert_eq!((MIN_REFRESH_HZ, MAX_REFRESH_HZ), (1, 1_000_000));
    Ok(())
}

/// Boundary: the truncation is where it is expected, at the one rate that does
/// not divide the second evenly.
///
/// 1000000 / 144 is 6944.44..., so the pinned constant is the floor. Stating
/// which way it rounds is what stops a later edit turning it into 6945 on the
/// grounds that the true value is nearer that.
#[test]
fn the_pinned_period_is_the_floor_of_an_inexact_division() -> Result<(), CompositorError> {
    let period = frame_period_us(PACING_COMMENT_REFRESH_HZ)?;
    assert_eq!(period, 6_944);
    assert_eq!(
        period.saturating_mul(PACING_COMMENT_REFRESH_HZ),
        999_936,
        "the floor times the rate is just under one second, which is what a floor means"
    );
    assert!(period.saturating_mul(PACING_COMMENT_REFRESH_HZ) < MAX_REFRESH_HZ);
    assert!(
        period
            .saturating_add(1)
            .saturating_mul(PACING_COMMENT_REFRESH_HZ)
            > MAX_REFRESH_HZ
    );
    Ok(())
}
