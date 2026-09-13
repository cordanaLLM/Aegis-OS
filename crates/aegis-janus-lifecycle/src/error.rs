// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Why a transition was refused.
//!
//! Every refusal is a value, never a panic: the workspace denies `unwrap`,
//! `expect`, `panic!` and `todo!`, so a lifecycle that cannot advance says so
//! and leaves the machine exactly where it was. A refused step writes nothing
//! to the trace either, which is what lets a trace be compared byte for byte:
//! it records what happened, not what was attempted.

use crate::clock::{ClockError, WatchdogError};
use crate::state::{EventKind, State};
use crate::verity::RootHash;

/// Why the machine refused to make a transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum LifecycleError {
    /// The clock could not be read, or moved backwards.
    #[error("clock: {0}")]
    Clock(#[from] ClockError),
    /// The watchdog could not be armed.
    #[error("watchdog: {0}")]
    Watchdog(#[from] WatchdogError),
    /// The state accepts events, but not this one.
    #[error("state {state} does not accept the event {event}")]
    Unexpected {
        /// The state the machine was in.
        state: State,
        /// The event that was offered.
        event: EventKind,
    },
    /// The state is terminal and accepts nothing.
    #[error("state {state} is terminal and accepts no further event, including {event}")]
    Terminal {
        /// The terminal state the machine was in.
        state: State,
        /// The event that was offered.
        event: EventKind,
    },
    /// A bless was asked for with no measurement to compare.
    ///
    /// This is the refusal a reopened slot meets first: reopening clears the
    /// measurement, so a re-bless without a fresh one has nothing to verify
    /// against and fails closed (D13 against REQ-P02-01).
    #[error("no dm-verity measurement is held; the slot cannot be blessed unverified")]
    VerityUnmeasured,
    /// The measurement did not match the signed release.
    #[error("dm-verity mismatch: signed release declares {expected}, slot measured {measured}")]
    VerityMismatch {
        /// The root hash the signed release declares.
        expected: RootHash,
        /// The root hash measured from the slot.
        measured: RootHash,
    },
    /// A stage that needs the candidate ran without one.
    #[error("no candidate is declared")]
    NoCandidate,
    /// The trace is full, so the transition could not be recorded.
    ///
    /// The transition is refused rather than made unrecorded: a trace with a
    /// hole in it is worse than a lifecycle that stops, because M24 would diff
    /// against it and read the hole as agreement.
    #[error("the transition trace is full at its bound of {bound} records")]
    TraceFull {
        /// The scalar bound the trace holds.
        bound: usize,
    },
}
