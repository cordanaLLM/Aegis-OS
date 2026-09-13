// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The capsule request P09 Minerva sends to P10 Vesta.
//!
//! # Where the fields come from
//!
//! | Field | Source |
//! | :-- | :-- |
//! | `edge` | export-062 `1ce919ed54bb`, edge `EXECUTE_WASMED_CAPSULE` |
//! | `capsule`, `slot`, `memory-limit-bytes`, `capabilities` | export-037 `ce490c88081f`, `WasmCapsule` and `WasmCapabilities` |
//! | `runtime` | decision D06, recorded in [`decision`](crate::decision) |
//! | `vmm` | decision D58: every measurement records which monitor produced it |
//! | `admission` | decision D04, recorded unresolved at M14 |
//! | `correlation-id` | `docs/integration/stack.md`: every crossing carries a correlation identifier |
//!
//! # Why the monitor is on the request
//!
//! D58 chose Firecracker for P10 and kept `QEMU` `microvm` as the recorded
//! fallback, and attached the rule that every measurement records which
//! monitor produced it. A monitor chosen at build time could not satisfy that:
//! a request carries its own, so a figure taken from the sandbox it asks for
//! can always be attributed. The reference profile has both installed, which is
//! precisely why the field cannot be dropped as obvious -- see
//! [`VMM_PROBES`](crate::VMM_PROBES).
//!
//! # What this module does not do
//!
//! No transport is implemented. Nothing here opens an `AF_VSOCK` socket, talks
//! to a monitor API, loads a `WebAssembly` module or starts a sandbox. The
//! slot a request names is a place in [`CapsuleRegistry`](crate::CapsuleRegistry),
//! and admitting one moves no byte.

use crate::capsule::{CapabilitySet, CapsuleMemoryLimit, CapsuleSlot};
use crate::contracts::graph::EdgeId;
use crate::contracts::{
    ContractError, Correlation, PayloadBuffer, SchemaId, decode_text, encode_into,
};
use crate::decision::AdmittedRuntime;
use crate::id::{CorrelationId, Label};
use crate::vmm::VmmIdentity;

use aegis_justitia::SandboxAdmissionPath;

/// The contract versions of the capsule request this build admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum CapsuleRequestVersion {
    /// Version 1, tagged `aegis.p09-p10.capsule-request.v1`.
    #[serde(rename = "aegis.p09-p10.capsule-request.v1")]
    V1,
}

/// One capsule P09 Minerva asks P10 Vesta to execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CapsuleRequest {
    /// The contract version this payload claims.
    pub schema: CapsuleRequestVersion,
    /// The subsystem-graph edge the payload travels on.
    pub edge: EdgeId,
    /// The gating path the request arrived on (decision D04, unresolved).
    pub admission: SandboxAdmissionPath,
    /// The runtime the capsule is executed by (decision D06, settled).
    pub runtime: AdmittedRuntime,
    /// The monitor the sandbox runs under (decision D58).
    pub vmm: VmmIdentity,
    /// The identifier threading this request to its evaluation.
    pub correlation_id: CorrelationId,
    /// The capsule this request is about.
    pub capsule: Label,
    /// The slot the capsule occupies.
    pub slot: CapsuleSlot,
    /// The memory limit the capsule is admitted with.
    pub memory_limit_bytes: CapsuleMemoryLimit,
    /// The capabilities the capsule is granted.
    pub capabilities: CapabilitySet,
}

impl CapsuleRequest {
    /// The contract this type instantiates.
    pub const SCHEMA: SchemaId = SchemaId::CapsuleRequest;

    /// The edge the request travels on, kept from the graph of record.
    pub const EDGE: EdgeId = EdgeId::ExecuteWasmedCapsule;

    /// The runtime decision D06 settled.
    pub const RUNTIME: AdmittedRuntime = AdmittedRuntime::SETTLED;

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
    /// edge this crate carries. The slot, the memory limit, the runtime and
    /// the capability list need no check here: each is refused by its own
    /// constructor or by its single-variant enum while the payload is decoded.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.edge != Self::EDGE {
            return Err(ContractError::WrongEdge {
                correlation: self.correlation(),
            });
        }
        Ok(())
    }

    /// Returns `true` when the request arrived on a path the graph of record
    /// carries end to end.
    ///
    /// Both readings of D04 are admitted, so this is a report and not a gate:
    /// a direct intercept rests on one source, and answering which reading is
    /// right needs evidence this milestone does not have.
    #[must_use]
    pub fn admission_is_in_graph_of_record(&self) -> bool {
        self.admission.in_graph_of_record()
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

    /// Decodes and validates one capsule request payload.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::UnknownVersion`] for a payload naming another
    /// contract version, [`ContractError::Malformed`] for anything else the
    /// schema refuses -- an unknown field, a missing field, a slot outside
    /// `1..=128`, a memory limit outside its range, an unknown capability name
    /// or the runtime D06 rejected -- [`ContractError::PayloadTooLong`] past
    /// the byte bound, and [`ContractError::WrongEdge`] for the other edge.
    pub fn decode(text: &str) -> Result<Self, ContractError> {
        let decoded: Self = decode_text(Self::SCHEMA, text)?;
        decoded.validate()?;
        Ok(decoded)
    }
}
