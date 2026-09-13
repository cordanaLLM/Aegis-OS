// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Decision D27, recorded: which way the code-CAD verification edge runs.
//!
//! The graph of record draws `VERIFY_CODE_CAD` from P09 to P14; the dataflow
//! document draws the same solver-backed verification from P14 to P09; a third
//! source has no such edge at all. The M01 register carries the collision as
//! dispute DSP-05 and poses it as D27, with a recommendation and **no
//! decision**.
//!
//! **D27 is recorded here, not settled.** This is the shape milestone M14 used
//! for D04: both readings stay representable in
//! [`CadVerificationDirection`], neither is authoritative, and the register
//! says which source draws which. What this
//! milestone does is narrower than settling the dispute and is the part the
//! roadmap asks for: the direction is *typed*, so a payload cannot be silent
//! about which reading it claims, and a submission carries the graph's reading
//! and refuses the other.
//!
//! # Why that is not a decision
//!
//! A request type is a producer's statement about its own message. P09
//! submitting to P14 does not make the other reading false, and closing D27
//! would need what the register asks for: evidence, or an owner's decision
//! about which side invokes the solver. The P14 slice is milestone M08, which
//! is where the other half of the evidence appears.
//!
//! # Scope
//!
//! Nothing here admits or refuses anything. The register names the two
//! readings and their sources; the contract module is where one of them is
//! carried.

use crate::contracts::graph::CadVerificationDirection;

/// Whether a recorded decision is settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DecisionState {
    /// The decision is settled and the register names what was chosen.
    Closed,
    /// The decision is recorded and still open; every reading stays
    /// representable.
    Unresolved,
}

impl DecisionState {
    /// Returns the stable name this state is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Unresolved => "unresolved",
        }
    }
}

/// A source, cited by export identifier and digest prefix only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Citation {
    /// The export identifier of the source.
    pub export: &'static str,
    /// The first twelve hexadecimal characters of that export's sha256.
    pub sha256_prefix: &'static str,
}

/// One recorded decision about an edge direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecisionRecord {
    /// The decision identifier used in `docs/roadmap/inventory.md`.
    pub id: &'static str,
    /// The question the decision answers, or leaves open.
    pub question: &'static str,
    /// Whether the decision is settled.
    pub state: DecisionState,
    /// The readings that stay representable while it is open.
    pub readings: [CadVerificationDirection; 2],
    /// The reading this milestone's payload carries, and why that is not a
    /// decision.
    pub carried: &'static str,
    /// What closing the decision would take.
    pub settled_by: &'static str,
    /// The milestone that records it.
    pub recorded_at: &'static str,
    /// The requirements the decision touches.
    pub touches: [&'static str; 2],
    /// The dispute it is carried as.
    pub dispute: &'static str,
    /// The sources that posed the question.
    pub sources: [Citation; 2],
}

impl DecisionRecord {
    /// Returns `true` only for a settled decision.
    #[must_use]
    pub const fn is_settled(&self) -> bool {
        matches!(self.state, DecisionState::Closed)
    }
}

/// D27: which direction the code-CAD verification edge runs. Unresolved.
pub const D27_CAD_VERIFICATION_DIRECTION: DecisionRecord = DecisionRecord {
    id: "D27",
    question: "Which direction does the code-CAD verification edge run: P09 calls P14, \
               or P14 calls P09?",
    state: DecisionState::Unresolved,
    readings: CadVerificationDirection::BOTH,
    carried: "a request is a submission, so it carries the graph of record's reading and \
              refuses the other; the opposite reading needs a request type in P14's crate, \
              not a change here",
    settled_by: "evidence about which side invokes the solver, or an owner's decision; the \
                 P14 slice is milestone M08",
    recorded_at: "M06",
    touches: ["REQ-P14-05", "REQ-GRAPH-05"],
    dispute: "DSP-05",
    sources: [
        Citation {
            export: "export-062",
            sha256_prefix: "1ce919ed54bb",
        },
        Citation {
            export: "export-002",
            sha256_prefix: "7e0c95f4ea05",
        },
    ],
};
