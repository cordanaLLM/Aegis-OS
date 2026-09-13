// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The subsystem-graph edges P10's contracts carry.
//!
//! Both identifiers are the graph's own (export-062 `1ce919ed54bb`), kept
//! verbatim so an edge stays traceable to its source. The graph's transport
//! string for the capsule edge names the Go runtime decision D06 rejected; the
//! register annotates that string rather than editing the graph, and nothing
//! here carries a runtime name at all.
//!
//! # The admission path is carried, not decided
//!
//! Whether P06 Justitia gates a sandbox execution directly or transitively
//! through P09 Minerva is decision D04, which milestone M14 recorded as
//! **unresolved** with both readings representable as
//! [`SandboxAdmissionPath`](aegis_justitia::SandboxAdmissionPath). This crate
//! consumes that type rather than declaring a third reading of its own: a
//! capsule request names the path it arrived on, both paths are admitted, and
//! which one is correct stays open. A crate that accepted only one would be
//! settling D04 by implementation.

/// The subsystem-graph edges this crate's contracts touch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum EdgeId {
    /// P09 to P10: capsule execution inside a Vesta sandbox.
    #[serde(rename = "EXECUTE_WASMED_CAPSULE")]
    ExecuteWasmedCapsule,
    /// P10 to P16: isolated execution and verification of A/B candidates.
    #[serde(rename = "SANDBOX_CANDIDATE_EVALUATION")]
    SandboxCandidateEvaluation,
}

impl EdgeId {
    /// Every edge this enum names, in declaration order.
    ///
    /// Exported rather than re-declared by each sweep that needs one, because
    /// `#[non_exhaustive]` stops a consumer crate from matching the enum
    /// exhaustively: a test holding its own copy of the list cannot be broken
    /// by a new variant at all, and would quietly stop testing what its name
    /// claims.
    pub const ALL: [Self; 2] = [Self::ExecuteWasmedCapsule, Self::SandboxCandidateEvaluation];

    /// Returns the edge identifier as it is written in the graph of record.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ExecuteWasmedCapsule => "EXECUTE_WASMED_CAPSULE",
            Self::SandboxCandidateEvaluation => "SANDBOX_CANDIDATE_EVALUATION",
        }
    }

    /// Returns the transport the graph of record records for this edge.
    ///
    /// Recorded verbatim, including the capsule edge's runtime name, which
    /// decision D06 superseded. It is here so a reader can see what the graph
    /// says and what was decided about it in the same place; nothing in this
    /// crate acts on the string.
    #[must_use]
    pub const fn recorded_transport(self) -> &'static str {
        match self {
            Self::ExecuteWasmedCapsule => "Wazero WASM Runtime / WIT Interfaces",
            Self::SandboxCandidateEvaluation => "Firecracker MicroVM / AF_VSOCK",
        }
    }

    /// Returns `true` when the graph of record contains this edge.
    ///
    /// Both do. The method exists so a later edge that rests on one source
    /// only -- the shape decision D04 is in -- has somewhere to say so.
    #[must_use]
    pub const fn in_graph_of_record(self) -> bool {
        matches!(
            self,
            Self::ExecuteWasmedCapsule | Self::SandboxCandidateEvaluation
        )
    }
}
