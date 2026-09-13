// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `ENFORCE_REALTIME_RTPRIO`: the grant P07 Lictor hands P08 Calliope
//! (REQ-P08-05, REQ-P08-02).
//!
//! The graph of record draws `P07_Lictor -> P08_Calliope` over
//! `Linux cgroups v2 / RLIMIT_RTPRIO`, carrying an `RLIMIT_RTPRIO=95` priority
//! reservation for the `PipeWire` audio threads (export-062 `1ce919ed54bb`).
//! This is the consumer's type for that message, so P07's crate builds it from
//! here rather than declaring a second shape for the same edge.
//!
//! # What a grant is, and is not
//!
//! It is a statement about a priority someone intends to set. It is not the
//! setting: nothing in this crate calls `setrlimit` or
//! `sched_setscheduler`, and decoding a grant changes no thread's scheduling
//! class. [`GrantedRtPrio`] refuses anything above the source's own 95, and
//! [`SchedPolicy`] has one variant, so a grant naming `SCHED_FIFO` does not
//! decode.
//!
//! The memlock half of REQ-P08-02 is deliberately **not** a field here. The
//! report asks for infinity and the reference profile offers 8192 kibibytes;
//! carrying that as a payload field would let a sender assert a limit it
//! cannot grant. It is recorded instead, once, in
//! [`REFERENCE_PROFILE_LIMITS`](crate::rtprio::REFERENCE_PROFILE_LIMITS).

use crate::contracts::graph::EdgeId;
use crate::contracts::{
    ContractError, Correlation, PayloadBuffer, SchemaId, decode_text, encode_into,
};
use crate::id::{CorrelationId, Label};
use crate::rtprio::{GrantedRtPrio, SchedPolicy};

/// The contract versions this build admits for the grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum RealtimeGrantVersion {
    /// Version 1.
    #[serde(rename = "aegis.p07-p08.realtime-grant.v1")]
    V1,
}

/// The real-time grant P07 Lictor sends to P08 Calliope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct RealtimeGrant {
    /// The contract version.
    pub schema: RealtimeGrantVersion,
    /// The graph edge this payload travels on.
    pub edge: EdgeId,
    /// The identifier that correlates this crossing.
    pub correlation_id: CorrelationId,
    /// The thread group the grant is about.
    pub thread: Label,
    /// The scheduling policy the grant names.
    pub policy: SchedPolicy,
    /// The real-time priority the grant reserves.
    pub rtprio: GrantedRtPrio,
}

impl RealtimeGrant {
    /// The contract this payload satisfies.
    pub const SCHEMA: SchemaId = SchemaId::RealtimeGrant;

    /// The graph edge this payload travels on.
    pub const EDGE: EdgeId = EdgeId::EnforceRealtimeRtprio;

    /// The scheduling policy every admitted grant names.
    pub const POLICY: SchedPolicy = SchedPolicy::ADMITTED;

    /// Returns what a refusal of this payload would be about.
    #[must_use]
    pub const fn correlation(&self) -> Correlation {
        Correlation::new(Self::SCHEMA, Some(self.correlation_id))
    }

    /// Decodes one grant from `text`.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::PayloadTooLong`] above the byte bound,
    /// [`ContractError::UnknownVersion`] for another contract version,
    /// [`ContractError::Malformed`] for anything else the decoder refuses --
    /// an unknown field, a priority above the source's 95, a policy other than
    /// `SCHED_RR` -- and [`ContractError::WrongEdge`] when the payload names
    /// the other edge this crate carries.
    pub fn decode(text: &str) -> Result<Self, ContractError> {
        let grant: Self = decode_text(Self::SCHEMA, text)?;
        if grant.edge != Self::EDGE {
            return Err(ContractError::WrongEdge {
                correlation: grant.correlation(),
            });
        }
        Ok(grant)
    }

    /// Encodes this grant into `buffer`.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::PayloadTooLong`] when the rendered payload
    /// does not fit the buffer's scalar bound.
    pub fn encode_into<'b>(&self, buffer: &'b mut PayloadBuffer) -> Result<&'b str, ContractError> {
        encode_into(self, self.correlation(), buffer)
    }

    /// Returns `true` when the grant asks for exactly the source's priority.
    #[must_use]
    pub fn is_source_priority(&self) -> bool {
        self.rtprio == GrantedRtPrio::REQUIRED
    }
}
