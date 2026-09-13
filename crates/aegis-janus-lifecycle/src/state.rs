// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The lifecycle vocabulary: states, events and the signature verdict.
//!
//! The six stages milestone M15 names -- candidate, signature check, delta
//! acquisition, slot swap, watchdog, bless or rollback -- appear here as
//! [`State`] values, plus the two D13 states a reopened slot passes through.
//! Every state name is stable: it is serialised into the transition trace that
//! M24 diffs against, so renaming one is a schema change.
//!
//! [`Event`] carries payloads (a candidate, a measured root hash, a timeout);
//! [`EventKind`] is the same set without them. Only the kind reaches the trace,
//! which is what keeps a trace record fixed-width.

use crate::candidate::Candidate;
use crate::clock::Timeout;
use crate::verity::RootHash;

/// Where one candidate stands in its A/B lifecycle.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum State {
    /// No candidate has been declared.
    Idle,
    /// A candidate release is declared and awaits its signature check.
    CandidateDeclared,
    /// The signature over the candidate verified.
    SignatureVerified,
    /// The delta landed in the target slot and was measured.
    DeltaAcquired,
    /// The boot order now tries the target slot.
    SlotSwapped,
    /// The candidate is on trial with the boot watchdog armed.
    WatchdogArmed,
    /// The candidate was blessed: consolidated, and bootable by default.
    Blessed,
    /// A blessed slot was reopened for bounded maintenance (D13).
    Reopened,
    /// The watchdog expired, or a maintenance window did; the fallback boots.
    RolledBack,
    /// The candidate was refused before it ever reached the boot order.
    Discarded,
}

impl State {
    /// Every state, in declaration order.
    pub const ALL: [Self; 10] = [
        Self::Idle,
        Self::CandidateDeclared,
        Self::SignatureVerified,
        Self::DeltaAcquired,
        Self::SlotSwapped,
        Self::WatchdogArmed,
        Self::Blessed,
        Self::Reopened,
        Self::RolledBack,
        Self::Discarded,
    ];

    /// Returns the stable name this state is traced under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::CandidateDeclared => "candidate-declared",
            Self::SignatureVerified => "signature-verified",
            Self::DeltaAcquired => "delta-acquired",
            Self::SlotSwapped => "slot-swapped",
            Self::WatchdogArmed => "watchdog-armed",
            Self::Blessed => "blessed",
            Self::Reopened => "reopened",
            Self::RolledBack => "rolled-back",
            Self::Discarded => "discarded",
        }
    }

    /// Returns `true` when no event is accepted from this state.
    ///
    /// [`Self::Blessed`] is deliberately not terminal: D13 chose reversible
    /// consolidation, so a blessed slot can still be reopened.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::RolledBack | Self::Discarded)
    }

    /// Returns `true` when the candidate is on trial under a deadline.
    #[must_use]
    pub const fn is_on_trial(self) -> bool {
        matches!(self, Self::WatchdogArmed | Self::Reopened)
    }
}

impl core::fmt::Display for State {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.name())
    }
}

/// What a signature check said about a candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum SignatureVerdict {
    /// The signature over the candidate verified.
    Valid,
    /// The signature did not verify, or there was none.
    Invalid,
}

impl SignatureVerdict {
    /// Both verdicts, in declaration order.
    pub const ALL: [Self; 2] = [Self::Valid, Self::Invalid];

    /// Returns the stable name this verdict is reported under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Invalid => "invalid",
        }
    }
}

/// An event offered to the machine, with its payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Event {
    /// Declare the candidate release the lifecycle is about.
    Declare(Candidate),
    /// Report the verdict of the signature check.
    CheckSignature(SignatureVerdict),
    /// Report the dm-verity root hash measured from what landed in the slot.
    AcquireDelta(RootHash),
    /// Report that the delta could not be acquired at all.
    FailDelta,
    /// Report that the boot order now tries the target slot.
    SwapSlot,
    /// Arm the boot watchdog for `Timeout` ticks.
    ArmWatchdog(Timeout),
    /// Sample the clock and re-evaluate any armed deadline.
    Tick,
    /// Ask for the candidate to be blessed.
    Bless,
    /// Reopen a blessed slot for a bounded maintenance window (D13).
    Reopen(Timeout),
    /// Report a fresh measurement taken after maintenance.
    Remeasure(RootHash),
}

impl Event {
    /// Returns the payload-free kind of this event.
    #[must_use]
    pub const fn kind(self) -> EventKind {
        match self {
            Self::Declare(_) => EventKind::Declare,
            Self::CheckSignature(_) => EventKind::CheckSignature,
            Self::AcquireDelta(_) => EventKind::AcquireDelta,
            Self::FailDelta => EventKind::FailDelta,
            Self::SwapSlot => EventKind::SwapSlot,
            Self::ArmWatchdog(_) => EventKind::ArmWatchdog,
            Self::Tick => EventKind::Tick,
            Self::Bless => EventKind::Bless,
            Self::Reopen(_) => EventKind::Reopen,
            Self::Remeasure(_) => EventKind::Remeasure,
        }
    }
}

/// An event without its payload: what a trace record carries.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum EventKind {
    /// A candidate was declared.
    Declare,
    /// A signature verdict was reported.
    CheckSignature,
    /// A measured delta was reported.
    AcquireDelta,
    /// A delta acquisition failure was reported.
    FailDelta,
    /// A slot swap was reported.
    SwapSlot,
    /// The watchdog was armed.
    ArmWatchdog,
    /// The clock was sampled.
    Tick,
    /// A bless was asked for.
    Bless,
    /// A blessed slot was reopened.
    Reopen,
    /// A fresh measurement was reported.
    Remeasure,
}

impl EventKind {
    /// Every event kind, in declaration order.
    pub const ALL: [Self; 10] = [
        Self::Declare,
        Self::CheckSignature,
        Self::AcquireDelta,
        Self::FailDelta,
        Self::SwapSlot,
        Self::ArmWatchdog,
        Self::Tick,
        Self::Bless,
        Self::Reopen,
        Self::Remeasure,
    ];

    /// Returns the stable name this kind is traced under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Declare => "declare",
            Self::CheckSignature => "check-signature",
            Self::AcquireDelta => "acquire-delta",
            Self::FailDelta => "fail-delta",
            Self::SwapSlot => "swap-slot",
            Self::ArmWatchdog => "arm-watchdog",
            Self::Tick => "tick",
            Self::Bless => "bless",
            Self::Reopen => "reopen",
            Self::Remeasure => "remeasure",
        }
    }
}

impl core::fmt::Display for EventKind {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.name())
    }
}
