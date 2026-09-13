// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The constraint screen, which is not a solver (REQ-P09-08).
//!
//! The developer guide maps this crate's directory to a subsystem that owns a
//! `Z3` solver, and the scaffold's `NeuroSymbolicSolver::verify_constraints`
//! stands in for it: it returns `is_valid` after checking that the token
//! string is non-empty and contains no exclamation mark, and fills the proof
//! hash with the byte `0x42`. That is a syntactic screen wearing a solver's
//! vocabulary, and this milestone admits no `Z3` foreign-function interface.
//!
//! So the type says what it is. A screen has two outcomes and neither of them
//! is "valid":
//!
//! * [`ScreenOutcome::RejectedByScreen`] -- the script carries the recorded
//!   negation byte, so it is refused here and never submitted;
//! * [`ScreenOutcome::NotRejected`] -- the screen found nothing, which is the
//!   absence of a rejection and **not** a verdict. Only P14 Hephaestus decides,
//!   and [`CadVerificationRequest`](crate::CadVerificationRequest) is how it is
//!   asked.
//!
//! There is no proof hash here either. A hash of a proof that was never
//! produced is a constant, and a constant that looks like evidence is worse
//! than an absent field.
//!
//! # Scope
//!
//! The screen tests one recorded byte. It is a check over an enumeration of
//! one, not a decision procedure over a constraint language, and a script it
//! does not reject may still be unsatisfiable, ill-typed or nonsense.

use crate::error::MinervaError;

/// Scalar upper bound, in bytes, on a constraint script.
pub const MAX_SCRIPT_BYTES: usize = 256;

/// The byte the recorded screen rejects on.
///
/// Kept from the scaffold, where a script containing `!` is refused. The rule
/// is recorded here rather than reinvented, and it is one byte rather than a
/// grammar.
pub const REJECTED_BYTE: u8 = b'!';

/// How many script bytes one constraint is counted as.
pub const BYTES_PER_CONSTRAINT: usize = 8;

/// A validated constraint script of at most [`MAX_SCRIPT_BYTES`] bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstraintScript {
    bytes: [u8; MAX_SCRIPT_BYTES],
    len: usize,
}

impl ConstraintScript {
    /// Parses `raw` into a constraint script.
    ///
    /// # Errors
    ///
    /// Returns [`MinervaError::ScriptOutOfRange`] when `raw` is empty or
    /// longer than [`MAX_SCRIPT_BYTES`], and [`MinervaError::ScriptCharset`]
    /// when it carries a byte outside printable ASCII. A script is never
    /// truncated: a long one is refused whole.
    pub fn parse(raw: &str) -> Result<Self, MinervaError> {
        let source = raw.as_bytes();
        if source.is_empty() || source.len() > MAX_SCRIPT_BYTES {
            return Err(MinervaError::ScriptOutOfRange {
                actual: source.len(),
                max: MAX_SCRIPT_BYTES,
            });
        }
        let mut bytes = [0u8; MAX_SCRIPT_BYTES];
        for (slot, byte) in bytes.iter_mut().zip(source.iter()) {
            if !byte.is_ascii_graphic() && *byte != b' ' {
                return Err(MinervaError::ScriptCharset);
            }
            *slot = *byte;
        }
        Ok(Self {
            bytes,
            len: source.len(),
        })
    }

    /// Returns the validated bytes, without the trailing padding.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..self.len).unwrap_or(&[])
    }

    /// Returns the script length in bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `false`; a script is never empty by construction.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns how many constraints the script is counted as carrying.
    ///
    /// The scaffold's arithmetic, kept: one constraint per
    /// [`BYTES_PER_CONSTRAINT`] bytes, and never fewer than one. It is a size
    /// estimate for a payload field, not a parse.
    #[must_use]
    pub fn constraint_count(&self) -> u32 {
        let counted = self.len.checked_div(BYTES_PER_CONSTRAINT).unwrap_or(0);
        u32::try_from(counted).unwrap_or(u32::MAX).max(1)
    }
}

/// What the screen found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ScreenOutcome {
    /// The script carries the recorded negation byte and is refused here.
    RejectedByScreen,
    /// The screen found nothing. This is not a verdict.
    NotRejected,
}

impl ScreenOutcome {
    /// Both outcomes, in declaration order.
    pub const BOTH: [Self; 2] = [Self::RejectedByScreen, Self::NotRejected];

    /// Returns the stable tag the wire form carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::RejectedByScreen => "rejected-by-screen",
            Self::NotRejected => "not-rejected",
        }
    }

    /// Returns `true` when the screen refused the script.
    ///
    /// There is deliberately no `is_valid`. The screen can refuse and it can
    /// fail to refuse; it cannot accept.
    #[must_use]
    pub const fn is_rejected(self) -> bool {
        matches!(self, Self::RejectedByScreen)
    }
}

/// The recorded screen over a constraint script.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConstraintScreen {
    screened: u64,
}

impl ConstraintScreen {
    /// Builds a screen that has seen nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self { screened: 0 }
    }

    /// Screens one script for the recorded negation byte.
    pub fn screen(&mut self, script: &ConstraintScript) -> ScreenOutcome {
        self.screened = self.screened.saturating_add(1);
        if script.as_bytes().contains(&REJECTED_BYTE) {
            return ScreenOutcome::RejectedByScreen;
        }
        ScreenOutcome::NotRejected
    }

    /// Returns how many scripts have been screened.
    #[must_use]
    pub const fn screened(&self) -> u64 {
        self.screened
    }
}
