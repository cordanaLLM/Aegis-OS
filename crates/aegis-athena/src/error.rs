// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Why a P16 value or a P16 transition was refused.
//!
//! Every refusal is a value, never a panic. The imported P16 scaffold
//! (export-025 `6b23723ddb76`) states two of its rules as `assert!` calls --
//! a non-empty candidate identifier and a positive latency -- so both aborted
//! the process and could only be covered by a test expecting a panic. Here the
//! first is a refusal from [`aegis_tellus::CandidateId::parse`] and the second
//! from [`crate::metrics::LatencyMs::new`], which is what HISS-07 asks for and
//! what makes the negative cases ordinary tests.

use aegis_tellus::TellusError;

use crate::ledger::LedgerError;
use crate::metrics::{
    MAX_LATENCY_MS, MAX_MEMORY_MB, MAX_SCI_CARBON_RATE, MIN_NULL_MODEL_RETENTION,
};
use crate::stage::{EventKind, Stage};

/// Reasons a P16 value or transition is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum AthenaError {
    /// An identifier was refused by the shared P13/P16 identifier type.
    ///
    /// The empty candidate identifier the scaffold guards with `assert!` lands
    /// here: it does not become a value at all.
    #[error("identifier: {0}")]
    Identifier(#[from] TellusError),
    /// A latency is non-finite, non-positive, or past the recorded bound.
    #[error("a latency of {reason} is not admissible (bound {MAX_LATENCY_MS} ms)")]
    Latency {
        /// Why the quantity was refused.
        reason: &'static str,
    },
    /// A memory figure is non-finite, negative, or past the recorded bound.
    #[error("a memory figure of {reason} is not admissible (bound {MAX_MEMORY_MB} MB)")]
    Memory {
        /// Why the quantity was refused.
        reason: &'static str,
    },
    /// A carbon rate is non-finite, negative, or past the recorded bound.
    #[error("a carbon rate of {reason} is not admissible (bound {MAX_SCI_CARBON_RATE})")]
    CarbonRate {
        /// Why the quantity was refused.
        reason: &'static str,
    },
    /// A retention figure is non-finite or outside `0.0..=1.0`.
    #[error(
        "a null-model retention of {reason} is not a proportion \
         (the gate's floor is {MIN_NULL_MODEL_RETENTION})"
    )]
    Retention {
        /// Why the quantity was refused.
        reason: &'static str,
    },
    /// The stage accepts events, but not this one.
    #[error("stage {stage} does not accept the event {event}")]
    Unexpected {
        /// The stage the machine was in.
        stage: Stage,
        /// The event that was offered.
        event: EventKind,
    },
    /// The stage is terminal and accepts nothing.
    #[error("stage {stage} is terminal and accepts no further event, including {event}")]
    Terminal {
        /// The terminal stage the machine was in.
        stage: Stage,
        /// The event that was offered.
        event: EventKind,
    },
    /// The check took longer than the recorded latency budget.
    ///
    /// REQ-P16-10 assigns Athena a 500 ms candidate-check budget. A check that
    /// overruns it is refused rather than allowed to publish late, because a
    /// budget that is only reported is not a budget.
    #[error("the candidate check took {elapsed_ms}ms, past the {budget_ms}ms budget")]
    CheckBudgetExceeded {
        /// The budget in milliseconds.
        budget_ms: u32,
        /// The elapsed time the caller reported, in milliseconds.
        elapsed_ms: u32,
    },
    /// A maturity gate was recorded twice, or the gate set is over its bound.
    #[error("{reason}")]
    MaturityGates {
        /// Why the gate result could not be recorded.
        reason: &'static str,
    },
    /// The checkpoint ledger refused the record.
    #[error("ledger: {0}")]
    Ledger(#[from] LedgerError),
}
