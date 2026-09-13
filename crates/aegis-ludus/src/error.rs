// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! What the P11 validators refuse, and why.
//!
//! The imported scaffold (export-033 `531cbdf98eb5`) returns
//! `Box<dyn Error>` built from a string literal for every refusal, so a caller
//! can only match on prose and the argument-count refusal is indistinguishable
//! from any other. Each refusal is a variant here instead, and each carries the
//! bound it hit, so a test asserts the bound rather than a sentence. That is
//! HISS-07 applied to this crate: the scaffold's assertions and string errors
//! become `Result` values with typed reasons.
//!
//! There is deliberately no identifier variant here. The correlation and
//! transaction identifiers on a receipt are
//! [`Identity`](aegis_justitia::Identity) values the caller has already
//! parsed, so this crate never refuses one and a variant for that refusal would
//! be public surface nothing can reach.

/// Reasons a P11 validator refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum LudusError {
    /// The command line carried more arguments than the recorded bound.
    #[error("a launch command line of at least {at_least} arguments exceeds the bound of {max}")]
    TooManyLaunchArguments {
        /// The scalar bound that was hit.
        max: usize,
        /// How many arguments the bounded count reached before it stopped.
        ///
        /// The counter stops one past the bound, so this is a lower bound on
        /// the real total and never a claim about it.
        at_least: usize,
    },
    /// One launch argument was empty or longer than the recorded bound.
    #[error("a launch argument of {actual} bytes is outside 1..={max}")]
    LaunchArgumentOutOfRange {
        /// The bound it passed.
        max: usize,
        /// The length observed.
        actual: usize,
    },
    /// One launch argument carried a byte outside printable ASCII.
    #[error("a launch argument carries a byte outside printable ASCII")]
    LaunchArgumentCharset,
    /// A platform configuration register index was outside the admissible range.
    #[error("a platform configuration register index of {index} is outside 0..={max}")]
    PcrIndexOutOfRange {
        /// The index that was offered.
        index: u8,
        /// The largest admissible index.
        max: u8,
    },
    /// A receipt amount was outside the admissible range.
    #[error("a receipt amount of {cents} cents is outside 0..={max}")]
    AmountOutOfRange {
        /// The amount that was offered.
        cents: u64,
        /// The largest admissible amount.
        max: u64,
    },
}
