// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The P09 side of the code-CAD verification edge, consumed (REQ-P14-05).
//!
//! Milestone M06 typed `VERIFY_CODE_CAD` in the producer's crate, because P14
//! had no crate to consume it: `aegis-minerva`'s module documentation says so
//! in as many words -- "nothing on the other side parses it, because the P14
//! crate does not exist". This module is the other side, and it **consumes the
//! producer's type rather than redefining it**, which is what keeps the shape
//! M06 pinned the only shape.
//!
//! # Direction
//!
//! The M01 register (dispute DSP-05, open decision D27) records the graph of
//! record drawing the edge from P09 Minerva to P14 Hephaestus, and one
//! architecture document drawing the same verification the other way.
//! [`CadVerificationRequest`] is a
//! submission, so it carries the graph of record's reading and its own decoder
//! refuses the other with `WrongDirection`. This crate consumes what the
//! producer sends and therefore reads the same direction; **it does not settle
//! D27**, and `tests/verification_intake.rs` asserts that the record
//! `aegis-minerva` publishes is still `Unresolved`.
//!
//! Settling it the other way would mean a request type in this crate and the
//! producer's one retired. Nothing here has to be un-decided first, which is
//! the property M06 was after.
//!
//! # There is no verdict, because there is no solver
//!
//! REQ-P14-04 makes symbolic solver verification a hard gate before promotion.
//! No solver foreign-function interface is admitted at this milestone, in
//! either crate, so [`VerificationOutcome`] has exactly **one** variant and it
//! is not "verified": a request that decodes is admitted and waits. There is no
//! variant meaning valid and no proof field, for the reason `aegis-minerva`'s
//! screen gives -- a constant that looks like evidence is worse than an absent
//! field. `register` records REQ-P14-04 as a deferred dependency.

use aegis_minerva::{CadVerificationDirection, CadVerificationRequest, ContractError};

/// What an admitted verification request is waiting for.
///
/// One variant, on purpose: no solver is linked, so no outcome of this type can
/// say a script was verified. A later milestone that admits one adds the
/// variants beside this and every `match` on it has to be revisited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum VerificationOutcome {
    /// The request is well formed and waits for a solver this build has none of.
    AdmittedPendingSolver,
}

impl VerificationOutcome {
    /// The only outcome this build can reach.
    pub const ADMITTED: Self = Self::AdmittedPendingSolver;

    /// Returns the stable name this outcome is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::AdmittedPendingSolver => "admitted-pending-solver",
        }
    }

    /// Returns `false`; no variant of this type means a script was verified.
    ///
    /// Written as an exhaustive match rather than as a `false` literal, so a
    /// later variant that did mean it would fail to compile until someone
    /// decided what this returns for it.
    #[must_use]
    pub const fn is_verified(self) -> bool {
        match self {
            Self::AdmittedPendingSolver => false,
        }
    }
}

/// One request P14 took in, with what it is waiting for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmittedVerification {
    /// The producer's request, decoded through the producer's own type.
    pub request: CadVerificationRequest,
    /// What the request is waiting for. Never a verdict.
    pub outcome: VerificationOutcome,
}

/// The recorded intake over verification requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VerificationIntake {
    admitted: u64,
    refused: u64,
}

impl VerificationIntake {
    /// The direction the requests this intake admits carry.
    ///
    /// Read from the producer's own constant rather than restated, so the two
    /// sides of the edge cannot drift apart. Decision D27 stays open: this is
    /// the reading a submission carries, not a decision about the edge.
    pub const DIRECTION: CadVerificationDirection = CadVerificationRequest::DIRECTION;

    /// Builds an intake that has seen nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            admitted: 0,
            refused: 0,
        }
    }

    /// Decodes one payload through the producer's type and admits it.
    ///
    /// # Errors
    ///
    /// Propagates [`CadVerificationRequest::decode`] unchanged, so every
    /// refusal a consumer sees is the producer's refusal: an unknown contract
    /// version, a malformed payload, one past the byte bound, the other reading
    /// of D27, or a script the producer's own screen already rejected. Nothing
    /// is re-implemented here, so the two sides cannot disagree about what a
    /// well-formed request is.
    pub fn accept(&mut self, text: &str) -> Result<AdmittedVerification, ContractError> {
        match CadVerificationRequest::decode(text) {
            Ok(request) => {
                self.admitted = self.admitted.saturating_add(1);
                Ok(AdmittedVerification {
                    request,
                    outcome: VerificationOutcome::ADMITTED,
                })
            }
            Err(refusal) => {
                self.refused = self.refused.saturating_add(1);
                Err(refusal)
            }
        }
    }

    /// Returns how many requests were admitted.
    #[must_use]
    pub const fn admitted(&self) -> u64 {
        self.admitted
    }

    /// Returns how many payloads were refused.
    #[must_use]
    pub const fn refused(&self) -> u64 {
        self.refused
    }
}
