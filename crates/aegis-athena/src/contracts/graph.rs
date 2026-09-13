// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The subsystem-graph edges P16 sits on.
//!
//! The graph of record (export-062 `1ce919ed54bb`) and the architecture
//! interface table (export-003 `13af15ffc316`) put P16 Athena on four edges,
//! two inbound and two outbound:
//!
//! | Edge | Direction | Where it is typed |
//! | :-- | :-- | :-- |
//! | `TRIGGER_SYSUPDATE_ROLLBACK` | P16 Athena to P02 Janus | [`super::promotion`], this milestone |
//! | `AUDIT_RECONSTRUCTIVE_CANDIDATE` | P06 Justitia to P16 Athena | `aegis_justitia::SignedAuditRecord`, milestone M14; consumed in [`super::audit_intake`] |
//! | `EVALUATE_CANDIDATE_CARBON_SCI` | P16 Athena to P13 Tellus | `aegis_tellus::CandidateSciQuery`, this milestone |
//! | `SANDBOX_CANDIDATE_EVALUATION` | P10 Vesta to P16 Athena | not typed here: REQ-P16-04's microVM transport is M06 and M22 |
//!
//! Only one of the four gets a schema in this crate, because only one is
//! produced by P16 and typed nowhere else. The audit record is M14's type and
//! is consumed rather than redefined; the SCI query lives with its arithmetic
//! in `aegis-tellus`; the sandbox edge has no payload yet.

/// An edge of the subsystem graph that touches P16 Athena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum EdgeId {
    /// P16 Athena to P02 Janus: deploy or roll back a candidate.
    #[serde(rename = "TRIGGER_SYSUPDATE_ROLLBACK")]
    TriggerSysupdateRollback,
    /// P06 Justitia to P16 Athena: a signed audit record (milestone M14).
    #[serde(rename = "AUDIT_RECONSTRUCTIVE_CANDIDATE")]
    AuditReconstructiveCandidate,
    /// P16 Athena to P13 Tellus: evaluate a candidate's SCI rate.
    #[serde(rename = "EVALUATE_CANDIDATE_CARBON_SCI")]
    EvaluateCandidateCarbonSci,
    /// P10 Vesta to P16 Athena: a sandboxed candidate evaluation.
    #[serde(rename = "SANDBOX_CANDIDATE_EVALUATION")]
    SandboxCandidateEvaluation,
}

impl EdgeId {
    /// Every edge, inbound and outbound.
    pub const ALL: [Self; 4] = [
        Self::TriggerSysupdateRollback,
        Self::AuditReconstructiveCandidate,
        Self::EvaluateCandidateCarbonSci,
        Self::SandboxCandidateEvaluation,
    ];

    /// Returns the edge identifier exactly as the graph of record spells it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::TriggerSysupdateRollback => "TRIGGER_SYSUPDATE_ROLLBACK",
            Self::AuditReconstructiveCandidate => "AUDIT_RECONSTRUCTIVE_CANDIDATE",
            Self::EvaluateCandidateCarbonSci => "EVALUATE_CANDIDATE_CARBON_SCI",
            Self::SandboxCandidateEvaluation => "SANDBOX_CANDIDATE_EVALUATION",
        }
    }

    /// Returns the component identifier at the far end of the edge.
    #[must_use]
    pub const fn peer(self) -> &'static str {
        match self {
            Self::TriggerSysupdateRollback => "P02",
            Self::AuditReconstructiveCandidate => "P06",
            Self::EvaluateCandidateCarbonSci => "P13",
            Self::SandboxCandidateEvaluation => "P10",
        }
    }

    /// Returns `true` when P16 produces on the edge rather than consuming it.
    #[must_use]
    pub const fn outbound(self) -> bool {
        matches!(
            self,
            Self::TriggerSysupdateRollback | Self::EvaluateCandidateCarbonSci
        )
    }

    /// Returns the transport the sources name for the edge.
    ///
    /// Recorded, not implemented: no transport exists in this crate.
    #[must_use]
    pub const fn recorded_transport(self) -> &'static str {
        match self {
            Self::TriggerSysupdateRollback => "systemd-sysupdate over local D-Bus, asynchronous",
            Self::AuditReconstructiveCandidate => "signed audit record (M14 schema)",
            Self::EvaluateCandidateCarbonSci => "ISO/IEC 21031:2024 SCI Rate Calculation",
            Self::SandboxCandidateEvaluation => "Firecracker MicroVM / AF_VSOCK",
        }
    }

    /// Returns where the edge's payload is typed, or why it is not.
    #[must_use]
    pub const fn typed_in(self) -> &'static str {
        match self {
            Self::TriggerSysupdateRollback => "aegis_athena::PromotionTrigger (M05)",
            Self::AuditReconstructiveCandidate => "aegis_justitia::SignedAuditRecord (M14)",
            Self::EvaluateCandidateCarbonSci => "aegis_tellus::CandidateSciQuery (M05)",
            Self::SandboxCandidateEvaluation => "not typed; M06 and M22",
        }
    }
}
