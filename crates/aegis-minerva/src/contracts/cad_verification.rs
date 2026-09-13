// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The code-CAD verification request P09 Minerva submits to P14 Hephaestus.
//!
//! # Where the fields come from
//!
//! | Field | Source |
//! | :-- | :-- |
//! | `edge`, `direction` | export-062 `1ce919ed54bb`, edge `VERIFY_CODE_CAD`, against export-002 `7e0c95f4ea05` (REQ-GRAPH-05, REQ-P14-05) |
//! | `expert`, `constraint-count`, `screened` | export-034 `213a95fc0d02`, `NeuroSymbolicSolver::verify_constraints` |
//! | `script-digest` | the caller's digest of the script that was screened |
//! | `correlation-id`, `submitted-at` | `docs/integration/stack.md` and the M14 field encodings |
//!
//! # The script does not travel
//!
//! A digest does. The parametric script is bounded at
//! [`MAX_SCRIPT_BYTES`](crate::MAX_SCRIPT_BYTES) locally, and a payload that
//! carried the text would put the larger of the two bounds on the wire for no
//! gain this milestone can use: nothing on the other side parses it, because
//! the P14 crate does not exist. The digest names the script the screen saw,
//! which is what a verdict has to be tied back to.
//!
//! # What this module does not do
//!
//! No transport is implemented and no solver is invoked. Nothing here produces
//! a proof, and there is no proof field: the scaffold fills one with a constant
//! byte, and a constant that looks like evidence is worse than an absent field.

use aegis_justitia::{Digest32, Identity, UnixSeconds};

use crate::contracts::graph::{CadVerificationDirection, EdgeId};
use crate::contracts::{
    ContractError, Correlation, PayloadBuffer, SchemaId, decode_text, encode_into,
};
use crate::expert::ExpertId;
use crate::screen::ScreenOutcome;

/// The contract versions of the verification request this build admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum CadVerificationVersion {
    /// Version 1, tagged `aegis.p09-p14.cad-verification.v1`.
    #[serde(rename = "aegis.p09-p14.cad-verification.v1")]
    V1,
}

/// One parametric script P09 asks P14 to check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CadVerificationRequest {
    /// The contract version this payload claims.
    pub schema: CadVerificationVersion,
    /// The subsystem-graph edge the payload travels on.
    pub edge: EdgeId,
    /// The reading of that edge the payload claims (decision D27, open).
    pub direction: CadVerificationDirection,
    /// The identifier threading this request to its verdict.
    pub correlation_id: Identity,
    /// The expert that produced the script.
    pub expert: ExpertId,
    /// The digest of the script that was screened.
    pub script_digest: Digest32,
    /// How many constraints the script is counted as carrying.
    pub constraint_count: u32,
    /// What the local screen found. Never a verdict.
    pub screened: ScreenOutcome,
    /// When the request was made.
    pub submitted_at: UnixSeconds,
}

impl CadVerificationRequest {
    /// The contract this type instantiates.
    pub const SCHEMA: SchemaId = SchemaId::CadVerification;

    /// The edge the request travels on, kept from the graph of record.
    pub const EDGE: EdgeId = EdgeId::VerifyCodeCad;

    /// The reading of that edge a submission carries.
    pub const DIRECTION: CadVerificationDirection = CadVerificationDirection::GRAPH_OF_RECORD;

    /// Returns what a refusal of this payload is about.
    #[must_use]
    pub const fn correlation(&self) -> Correlation {
        Correlation::new(Self::SCHEMA, Some(self.correlation_id))
    }

    /// Checks the invariants the field types cannot express.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::WrongDirection`] when the payload claims the
    /// reading this schema does not carry, and
    /// [`ContractError::RejectedScriptSubmitted`] when a script the local
    /// screen already rejected is submitted anyway. The second is the rule
    /// that makes the screen worth running: a script that fails it never
    /// reaches the consumer.
    ///
    /// The edge needs no check of its own: [`EdgeId`] admits exactly the edge
    /// this schema travels on, so a payload naming another is refused by the
    /// decoder before this runs.
    pub fn validate(&self) -> Result<(), ContractError> {
        let correlation = self.correlation();
        if self.direction != Self::DIRECTION {
            return Err(ContractError::WrongDirection { correlation });
        }
        if self.screened.is_rejected() {
            return Err(ContractError::RejectedScriptSubmitted { correlation });
        }
        Ok(())
    }

    /// Encodes a validated request into `buffer`.
    ///
    /// # Errors
    ///
    /// Propagates [`Self::validate`], and returns
    /// [`ContractError::PayloadTooLong`] when the payload does not fit.
    pub fn encode_into<'b>(&self, buffer: &'b mut PayloadBuffer) -> Result<&'b str, ContractError> {
        self.validate()?;
        encode_into(self, self.correlation(), buffer)
    }

    /// Decodes and validates one verification request payload.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::UnknownVersion`] for a payload naming another
    /// contract version, [`ContractError::Malformed`] for anything else the
    /// schema refuses, [`ContractError::PayloadTooLong`] past the byte bound,
    /// [`ContractError::WrongDirection`] for the other reading of D27, and
    /// [`ContractError::RejectedScriptSubmitted`] for a screened-out script.
    pub fn decode(text: &str) -> Result<Self, ContractError> {
        let decoded: Self = decode_text(Self::SCHEMA, text)?;
        decoded.validate()?;
        Ok(decoded)
    }
}
