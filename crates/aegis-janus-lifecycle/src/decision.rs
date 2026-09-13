// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Decision D13, recorded against the dm-verity requirement.
//!
//! REQ-P02-05 and REQ-P02-01 pull in different directions, which is what made
//! D13 a decision rather than a detail. REQ-P02-05 asks for *reversible
//! structural consolidation*: a mature security structure stays reopenable for
//! bounded maintenance, so the system cannot lock itself into an uncorrectable
//! "false maturity" (the same failure REQ-P16-05 names for P16 Athena's
//! maturity gates). REQ-P02-01 asks that the read-only root be bit-for-bit
//! identical to the signed release before it is trusted. Reopening a
//! consolidated slot is exactly the operation that could break that.
//!
//! **The decision, recorded 2026-09-13: reversible structural consolidation,
//! reconciled with dm-verity root integrity.** The reconciliation is one rule:
//! reopening a blessed slot discards its dm-verity measurement, so the slot can
//! only be blessed again by presenting a fresh measurement that equals the
//! signed release's root hash. Reversibility costs a re-verification; it never
//! costs the integrity check.
//!
//! # What this module is, and what holds the rule
//!
//! This module is a register: a static record of what was decided, which
//! requirements it reconciles, which dispute it closes and which sources posed
//! it. It admits and refuses nothing. The rule itself lives in
//! [`Machine`](crate::Machine): `Effect::Reopen` clears the held measurement,
//! and a bless with no measurement is [`LifecycleError::VerityUnmeasured`],
//! while a bless with the wrong one is
//! [`LifecycleError::VerityMismatch`]. `tests/consolidation_decision.rs` ties
//! the two together, so the register cannot drift away from the behaviour.
//!
//! [`LifecycleError::VerityUnmeasured`]: crate::LifecycleError::VerityUnmeasured
//! [`LifecycleError::VerityMismatch`]: crate::LifecycleError::VerityMismatch
//!
//! # Scope
//!
//! Recording D13 is not evidence that a real reopened slot re-verifies. No
//! device is opened and no hash is computed anywhere in this crate; the model
//! says when a match is required, and M24 is where a real transfer is compared
//! against it.

/// Whether a recorded decision is settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DecisionState {
    /// The decision is settled and the register names what was chosen.
    Closed,
    /// The decision is recorded but not settled.
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

/// The two security models D13 poses against each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ConsolidationModel {
    /// A mature structure stays reopenable for bounded maintenance.
    ReversibleConsolidation,
    /// A mature structure is locked indefinitely.
    PermanentHardening,
}

impl ConsolidationModel {
    /// Both models, in declaration order.
    ///
    /// The rejected option stays representable so that the register states a
    /// choice between two readings rather than asserting the only one it can
    /// express.
    pub const ALL: [Self; 2] = [Self::ReversibleConsolidation, Self::PermanentHardening];

    /// Returns the stable name this model is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ReversibleConsolidation => "reversible-structural-consolidation",
            Self::PermanentHardening => "permanent-hardening",
        }
    }

    /// Returns `true` when the model lets a consolidated slot be reopened.
    #[must_use]
    pub const fn permits_reopening(self) -> bool {
        matches!(self, Self::ReversibleConsolidation)
    }
}

/// A private source, cited by export identifier and digest prefix only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Citation {
    /// The export identifier of the private source.
    pub export: &'static str,
    /// The first twelve hexadecimal characters of that export's sha256.
    pub sha256_prefix: &'static str,
}

/// One recorded decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecisionRecord {
    /// The decision identifier used in `docs/roadmap/README.md`.
    pub id: &'static str,
    /// The question the decision answers.
    pub question: &'static str,
    /// Whether the decision is settled.
    pub state: DecisionState,
    /// The model that was chosen.
    pub chosen: ConsolidationModel,
    /// The model that was not chosen.
    pub rejected: ConsolidationModel,
    /// The rule that reconciles the two requirements.
    pub reconciliation: &'static str,
    /// The requirements the decision reconciles.
    pub reconciles: [&'static str; 3],
    /// The dispute this decision closes.
    pub dispute: &'static str,
    /// The private sources that posed the question.
    pub sources: [Citation; 2],
}

/// D13: reversible structural consolidation against permanent hardening.
pub const D13_CONSOLIDATION: DecisionRecord = DecisionRecord {
    id: "D13",
    question: "Reversible structural consolidation versus permanent hardening for P02?",
    state: DecisionState::Closed,
    chosen: ConsolidationModel::ReversibleConsolidation,
    rejected: ConsolidationModel::PermanentHardening,
    reconciliation: "reopening a blessed slot discards its dm-verity measurement, so \
                     re-blessing requires a fresh measurement equal to the signed \
                     release root hash",
    reconciles: ["REQ-P02-05", "REQ-P02-01", "REQ-P16-05"],
    dispute: "DSP-27",
    sources: [
        Citation {
            export: "export-004",
            sha256_prefix: "15831276a058",
        },
        Citation {
            export: "export-011",
            sha256_prefix: "528da609dab7",
        },
    ],
};
