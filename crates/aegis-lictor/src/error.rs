// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! What the P07 tables and constructors refuse, and why.
//!
//! The imported scaffold (export-032 `30d67bc52290`) refuses nothing at all:
//! `register_process` pushes onto an unbounded `Vec` behind a mutex it unwraps,
//! so there is no capacity refusal to return and a poisoned lock aborts the
//! daemon. Each refusal is a variant here instead, and each carries the bound
//! or the value it refused, so a test asserts the bound rather than a sentence.

use crate::id::IdError;
use crate::probe::ProbeStage;

/// Reasons a P07 table or constructor refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum LictorError {
    /// The tracked-process table is full.
    #[error("the broker tracks its maximum of {max} processes")]
    ProcessTableFull {
        /// The scalar bound that was hit.
        max: usize,
    },
    /// The broker tracks no process with that identifier.
    #[error("the broker tracks no process {pid}")]
    UnknownPid {
        /// The identifier that was named.
        pid: u32,
    },
    /// A process identifier was outside the range a Linux system hands out.
    #[error("a process identifier of {pid} is outside 1..={max}")]
    PidOutOfRange {
        /// The identifier that was offered.
        pid: u32,
        /// The largest admissible identifier.
        max: u32,
    },
    /// A logical CPU index was outside the mask this build carries.
    #[error("a logical CPU index of {core} is outside 0..{width}")]
    CoreOutOfRange {
        /// The index that was offered.
        core: u32,
        /// The width of the mask.
        width: u32,
    },
    /// The fragility-probe lifecycle does not admit that step.
    #[error("a fragility probe may not step from {from:?} to {to:?}")]
    IllegalProbeTransition {
        /// The stage the probe is in.
        from: ProbeStage,
        /// The stage that was asked for.
        to: ProbeStage,
    },
    /// A probe was asked to promote from a non-authoritative path it never left.
    #[error("a fragility probe cannot be promoted before it is observed")]
    ProbeNotObserved,
    /// An identifier was refused by its validating constructor.
    #[error("an identifier was refused: {0}")]
    Identifier(#[from] IdError),
}
