// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The frame-pacing constant, pinned (REQ-P04-08).
//!
//! REQ-P04-08 records that the scaffold's event loop throttles to a 100
//! microsecond frame time, that its own comment on the same line says the loop
//! is targeting 144 Hz pacing, and that the subsystem graph gives the
//! compositor node a `<1.5ms (Render/IPC)` latency budget. Those three figures
//! cannot all describe the same loop, and the requirement asks for the
//! constant to be pinned.
//!
//! # The three candidates, and which one is pinned
//!
//! | Candidate | Value | Where it comes from |
//! | :-- | :-- | :-- |
//! | [`PacingSource::ScaffoldLiteral`] | 100 microseconds | `Duration::from_micros(100)` in export-027 `f19640d7a7da` |
//! | [`PacingSource::RefreshRateComment`] | 144 Hz, so 6944 microseconds | the comment on the line above that call, same export |
//! | [`PacingSource::RenderIpcBudget`] | 1500 microseconds | the compositor node's `latency_budget` attribute in export-062 `1ce919ed54bb` |
//!
//! **[`PINNED_FRAME_PERIOD_US`] is the refresh-rate reading: 6944
//! microseconds.** The reasoning is structural rather than a preference:
//!
//! * the constant governs a *pacing period*, and only one of the three
//!   candidates is one. The 100 microsecond literal is a 10 kHz period, about
//!   sixty-nine times the rate its own comment names, and a loop that woke at
//!   that rate would spend the budget on wakeups;
//! * the 1.5 millisecond figure is a *work budget per frame* -- the graph
//!   labels it Render and IPC -- not an interval between frames. A budget and
//!   a period are different quantities, and the consistency check between them
//!   is that the budget fits inside the period, which
//!   [`budget_fits_in_period`] states and the boundary test asserts.
//!
//! # This pins a constant; it measures nothing
//!
//! [`DeclaredPeriodUs`] exists so the pinned value cannot be read back as an
//! observation. Nothing in this crate runs a loop, sleeps, or reads a clock,
//! and the reference kernel is `PREEMPT_DYNAMIC` rather than `PREEMPT_RT`:
//! **no latency, jitter or frame-time figure is produced by this milestone.**
//! Whether a compositor can hold a 6944 microsecond period is a question for
//! real hardware at milestone M12 and for the latency fixtures at M23.

use core::fmt;

use crate::error::CompositorError;

/// The lowest refresh rate [`frame_period_us`] takes a period of, in hertz.
pub const MIN_REFRESH_HZ: u32 = 1;

/// The highest refresh rate [`frame_period_us`] takes a period of, in hertz.
pub const MAX_REFRESH_HZ: u32 = 1_000_000;

/// The loop period the scaffold actually sleeps to, in microseconds.
pub const SCAFFOLD_LOOP_PERIOD_US: u32 = 100;

/// The refresh rate the scaffold's own comment names, in hertz.
pub const PACING_COMMENT_REFRESH_HZ: u32 = 144;

/// The render-and-IPC budget the graph of record gives the P04 node, in
/// microseconds.
pub const RENDER_IPC_BUDGET_US: u32 = 1_500;

/// The loop-cycle bound the scaffold declares.
///
/// Recorded from export-027 `f19640d7a7da`, where `MAX_EVENT_LOOP_CYCLES` is
/// 200. Recorded, not used: this crate runs no loop to bound.
pub const SCAFFOLD_EVENT_LOOP_CYCLES: u32 = 200;

/// Which reading a pacing figure comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PacingSource {
    /// The literal the scaffold's event loop sleeps to.
    ScaffoldLiteral,
    /// The refresh rate the comment on the same line names.
    RefreshRateComment,
    /// The render-and-IPC budget the subsystem graph records for the node.
    RenderIpcBudget,
}

impl PacingSource {
    /// All three readings, in the order the requirement lists them.
    pub const ALL: [Self; 3] = [
        Self::ScaffoldLiteral,
        Self::RefreshRateComment,
        Self::RenderIpcBudget,
    ];

    /// Returns the stable name this reading is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ScaffoldLiteral => "scaffold-literal",
            Self::RefreshRateComment => "refresh-rate-comment",
            Self::RenderIpcBudget => "render-ipc-budget",
        }
    }

    /// Returns `true` when the reading is an interval between frames rather
    /// than an amount of work inside one.
    ///
    /// The budget is not: that is the whole reason it is not the pinned value.
    #[must_use]
    pub const fn is_a_period(self) -> bool {
        !matches!(self, Self::RenderIpcBudget)
    }
}

/// A frame period recorded from a source, which nothing here measured.
///
/// The wrapper is the point: a bare `u32` of 6944 is indistinguishable from a
/// measured frame time, and `tests/declared_literals.rs` is what stops one
/// becoming the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeclaredPeriodUs {
    micros: u32,
    source: PacingSource,
}

impl DeclaredPeriodUs {
    /// Records `micros` as declared by `source`, never as observed.
    #[must_use]
    pub const fn new(micros: u32, source: PacingSource) -> Self {
        Self { micros, source }
    }

    /// Returns the declared period in microseconds.
    #[must_use]
    pub const fn micros(self) -> u32 {
        self.micros
    }

    /// Returns which reading the period comes from.
    #[must_use]
    pub const fn source(self) -> PacingSource {
        self.source
    }
}

impl fmt::Display for DeclaredPeriodUs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} us (declared by {}, unmeasured)",
            self.micros,
            self.source.name()
        )
    }
}

/// Returns the frame period of `hz`, in microseconds, truncated.
///
/// # Errors
///
/// Returns [`CompositorError::RefreshRateOutOfRange`] outside
/// `MIN_REFRESH_HZ..=MAX_REFRESH_HZ`, so a rate of zero -- which has no period
/// -- never produces one.
pub const fn frame_period_us(hz: u32) -> Result<u32, CompositorError> {
    if hz < MIN_REFRESH_HZ || hz > MAX_REFRESH_HZ {
        return Err(CompositorError::RefreshRateOutOfRange {
            hz,
            min: MIN_REFRESH_HZ,
            max: MAX_REFRESH_HZ,
        });
    }
    match MAX_REFRESH_HZ.checked_div(hz) {
        Some(period) => Ok(period),
        None => Err(CompositorError::RefreshRateOutOfRange {
            hz,
            min: MIN_REFRESH_HZ,
            max: MAX_REFRESH_HZ,
        }),
    }
}

/// The pinned frame-pacing constant: one 144 Hz frame, in microseconds.
///
/// 6944, which is `1_000_000 / 144` truncated. See the module documentation
/// for why this reading rather than the other two.
pub const PINNED_FRAME_PERIOD_US: DeclaredPeriodUs =
    DeclaredPeriodUs::new(6_944, PacingSource::RefreshRateComment);

/// Returns `true` when the render-and-IPC budget fits inside `period`.
///
/// The consistency check the pinned value has to satisfy: a per-frame work
/// budget that did not fit inside the interval between frames would describe a
/// loop that can never keep up. It is arithmetic on two recorded constants and
/// is not evidence that any implementation meets either.
#[must_use]
pub const fn budget_fits_in_period(period: DeclaredPeriodUs) -> bool {
    RENDER_IPC_BUDGET_US < period.micros()
}
