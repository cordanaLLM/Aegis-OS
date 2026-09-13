// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The candidate evaluation P10 Vesta hands to P16 Athena.
//!
//! # Where the fields come from
//!
//! | Field | Source |
//! | :-- | :-- |
//! | `edge`, transport | export-062 `1ce919ed54bb`, edge `SANDBOX_CANDIDATE_EVALUATION` (REQ-P10-07, REQ-P16-04) |
//! | `vm-id`, `vmm` | export-037 `ce490c88081f`, `MicroVmInstance`, and decision D58 |
//! | `candidate`, `verdict` | export-062 `1ce919ed54bb`: isolated execution and verification of A/B system candidates |
//! | `correlation-id`, `evaluated-at` | `docs/integration/stack.md` and the M14 field encodings |
//!
//! # There is no measurement field, on purpose
//!
//! An A/B candidate evaluation is where a boot time or a footprint would
//! eventually travel, and D58 requires every such figure to record which
//! monitor produced it. The monitor is already a field; the figure is not,
//! because nothing in this repository has measured one. Adding an unmeasured
//! number to a payload consumed by an audit ledger is exactly the mistake
//! REQ-P10-05 records, so the field arrives with the measurement, under a new
//! contract version.
//!
//! # What this module does not do
//!
//! No transport is implemented. Nothing here opens an `AF_VSOCK` socket, signs
//! a record or reaches P16. The verdict is a value a caller supplies.

use crate::contracts::graph::EdgeId;
use crate::contracts::{
    ContractError, Correlation, PayloadBuffer, SchemaId, decode_text, encode_into,
};
use crate::id::{CorrelationId, Label};
use crate::microvm::VmId;
use crate::vmm::VmmIdentity;

use aegis_justitia::UnixSeconds;

/// The contract versions of the candidate evaluation this build admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum CandidateEvaluationVersion {
    /// Version 1, tagged `aegis.p10-p16.candidate-evaluation.v1`.
    #[serde(rename = "aegis.p10-p16.candidate-evaluation.v1")]
    V1,
}

/// What an isolated evaluation concluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum EvaluationVerdict {
    /// The candidate satisfied every check the sandbox ran.
    Passed,
    /// The candidate failed at least one check.
    Failed,
    /// The sandbox could not decide, and says so rather than guessing.
    Inconclusive,
}

impl EvaluationVerdict {
    /// Every verdict, in declaration order.
    pub const ALL: [Self; 3] = [Self::Passed, Self::Failed, Self::Inconclusive];

    /// Returns the stable tag the wire form carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Inconclusive => "inconclusive",
        }
    }

    /// Returns `true` only for a verdict that lets a candidate proceed.
    ///
    /// [`Self::Inconclusive`] is not a pass: a candidate the sandbox could not
    /// decide about is refused by this method, so an undecided evaluation
    /// cannot be read as a positive one further down the chain.
    #[must_use]
    pub const fn admits_candidate(self) -> bool {
        matches!(self, Self::Passed)
    }
}

/// One A/B candidate evaluation P10 hands to P16.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CandidateEvaluation {
    /// The contract version this payload claims.
    pub schema: CandidateEvaluationVersion,
    /// The subsystem-graph edge the payload travels on.
    pub edge: EdgeId,
    /// The identifier threading this evaluation to the request that caused it.
    pub correlation_id: CorrelationId,
    /// The candidate that was evaluated.
    pub candidate: Label,
    /// The sandbox that evaluated it.
    pub vm_id: VmId,
    /// The monitor that ran the sandbox (decision D58).
    pub vmm: VmmIdentity,
    /// What the evaluation concluded.
    pub verdict: EvaluationVerdict,
    /// When the evaluation was recorded.
    pub evaluated_at: UnixSeconds,
}

impl CandidateEvaluation {
    /// The contract this type instantiates.
    pub const SCHEMA: SchemaId = SchemaId::CandidateEvaluation;

    /// The edge the evaluation travels on, kept from the graph of record.
    pub const EDGE: EdgeId = EdgeId::SandboxCandidateEvaluation;

    /// Returns what a refusal of this payload is about.
    #[must_use]
    pub const fn correlation(&self) -> Correlation {
        Correlation::new(Self::SCHEMA, Some(self.correlation_id))
    }

    /// Checks the invariants the field types cannot express.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::WrongEdge`] when the payload names the other
    /// edge this crate carries.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.edge != Self::EDGE {
            return Err(ContractError::WrongEdge {
                correlation: self.correlation(),
            });
        }
        Ok(())
    }

    /// Encodes a validated evaluation into `buffer`.
    ///
    /// # Errors
    ///
    /// Propagates [`Self::validate`], and returns
    /// [`ContractError::PayloadTooLong`] when the payload does not fit.
    pub fn encode_into<'b>(&self, buffer: &'b mut PayloadBuffer) -> Result<&'b str, ContractError> {
        self.validate()?;
        encode_into(self, self.correlation(), buffer)
    }

    /// Decodes and validates one candidate evaluation payload.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::UnknownVersion`] for a payload naming another
    /// contract version, [`ContractError::Malformed`] for anything else the
    /// schema refuses, [`ContractError::PayloadTooLong`] past the byte bound,
    /// and [`ContractError::WrongEdge`] for the other edge.
    pub fn decode(text: &str) -> Result<Self, ContractError> {
        let decoded: Self = decode_text(Self::SCHEMA, text)?;
        decoded.validate()?;
        Ok(decoded)
    }
}
