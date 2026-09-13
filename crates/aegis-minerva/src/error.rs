// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! What the P09 logic refuses, and why.
//!
//! The imported scaffold returns `&'static str` from the router and the replay
//! buffer, so the two capacity refusals are indistinguishable to a program and
//! a caller can only match on prose. Each refusal is a variant here instead,
//! carrying the bound it hit.

use crate::id::IdError;

/// Reasons the P09 logic refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MinervaError {
    /// The expert table is full.
    #[error("the expert table holds its maximum of {max} experts")]
    ExpertTableFull {
        /// The scalar bound that was hit.
        max: usize,
    },
    /// The trajectory buffer is full.
    #[error("the trajectory buffer holds its maximum of {max} steps")]
    TrajectoryBufferFull {
        /// The scalar bound that was hit.
        max: usize,
    },
    /// A power envelope was outside the admissible range.
    #[error("a power envelope of {milliwatts} mW is outside {min}..={max}")]
    PowerEnvelopeOutOfRange {
        /// The envelope that was offered.
        milliwatts: u32,
        /// The smallest admissible envelope.
        min: u32,
        /// The biomimetic power cap.
        max: u32,
    },
    /// A constraint script was empty or past its bound.
    #[error("a constraint script of {actual} bytes is outside 1..={max}")]
    ScriptOutOfRange {
        /// The length that was offered.
        actual: usize,
        /// The largest admissible script.
        max: usize,
    },
    /// A constraint script carried a byte outside the permitted set.
    #[error("a constraint script carries a byte outside the permitted character set")]
    ScriptCharset,
    /// A label was refused by its validating constructor.
    #[error("a label was refused: {0}")]
    Label(#[from] IdError),
}
