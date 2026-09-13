// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! What the P14 validators refuse, and why.
//!
//! The imported scaffold (export-029 `427996186520`) returns
//! `Box<dyn Error>` built from a string literal for every refusal -- "STEP file
//! does not exist", "Mesh element count exceeds P10 security threshold",
//! "Unknown solver target" -- so a caller can only match on prose and cannot
//! read the bound that was hit. Each refusal is a variant here instead, and
//! each carries its bound, so a test asserts the bound rather than a sentence.
//! That is HISS-07 applied to this crate: the scaffold's assertions and string
//! errors become `Result` values with typed reasons.

use crate::id::IdError;

/// Reasons a P14 validator refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum HephaestusError {
    /// The request named no `STEP` source at all.
    #[error("a geometry evaluation was asked for without a STEP source path")]
    StepSourceMissing,
    /// A mesh was asked for more elements than the recorded bound.
    #[error("a mesh of {elements} elements exceeds the bound of {max}")]
    MeshElementsOutOfRange {
        /// The count that was asked for.
        elements: usize,
        /// The scalar bound that was hit.
        max: usize,
    },
    /// A mesh element size range was empty or inverted.
    #[error("a mesh element size range of {min}..={max} micrometres is empty or inverted")]
    ElementSizeRangeInvalid {
        /// The smallest element size that was offered.
        min: u32,
        /// The largest element size that was offered.
        max: u32,
    },
    /// A geometry tolerance was outside the admissible range.
    #[error("a tolerance of {micrometres} micrometres is outside {min}..={max}")]
    ToleranceOutOfRange {
        /// The tolerance that was offered.
        micrometres: u32,
        /// The smallest admissible tolerance.
        min: u32,
        /// The largest admissible tolerance.
        max: u32,
    },
    /// A solver tag named no solver the recorded set admits.
    ///
    /// The offered tag is deliberately not carried: it is caller-supplied text
    /// of unbounded length, and a refusal that echoes its input either owns
    /// heap or truncates. The caller knows what it offered.
    #[error("no admitted solver is named by the offered tag")]
    UnknownSolver,
    /// An identifier was refused by its validating constructor.
    #[error("an identifier was refused: {0}")]
    Identifier(#[from] IdError),
}
