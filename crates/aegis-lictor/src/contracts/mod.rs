// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The one versioned payload P07 Lictor owns, and the two it does not (M07).
//!
//! | Edge | Schema | Where it lives |
//! | :-- | :-- | :-- |
//! | P04 Mercurius to P07 Lictor, `FOCUS_SWITCH_NOTIFY` | [`focus_switch::FocusSwitchReport`] | here, with its consumer |
//! | P07 Lictor to P08 Calliope, `ENFORCE_REALTIME_RTPRIO` | `aegis_calliope::RealtimeGrant` | with its consumer, in P08's crate |
//! | P13 Tellus to P07 Lictor, `SPATIOTEMPORAL_TASK_SHIFT` | `aegis_tellus::TaskShiftDirective` | with its producer, typed at milestone M05 |
//!
//! Only the first is declared here, and that is the convention M14 set rather
//! than an accident: an inbound schema lives with its consumer. The grant P07
//! sends P08 is P08's inbound schema, so P08's crate declares it and
//! [`crate::chain`] builds one. The directive P13 sends P07 was typed by its
//! producer at milestone M05, before this crate existed, so [`crate::chain`]
//! consumes it through P13's own decoder rather than declaring a second shape
//! for the same message.
//!
//! # What this module is not
//!
//! **No transport is implemented here, and none is implied.** There is no
//! shared-memory session, no D-Bus connection, no `cgroup` write and no eBPF
//! map. A schema states the shape and the bounds of a payload; carrying one
//! between two processes is later work.
//!
//! # Versioning
//!
//! The schema carries an explicit `schema` tag as its first field, typed as an
//! enum with one variant per admitted version, so a payload naming any other
//! version is refused by the decoder rather than parsed leniently. It also
//! carries its [`graph::EdgeId`], typed the same way, and a payload naming
//! another of this crate's edges is refused with [`ContractError::WrongEdge`].
//!
//! # Bounds and allocation
//!
//! A payload longer than [`MAX_CONTRACT_PAYLOAD_BYTES`] is refused before it is
//! parsed at all, and every field of a decoded payload lands in a fixed inline
//! slot: the schema type is `Copy`, so a decoded value owns no heap.
//!
//! Decoding is **not** unconditionally allocation-free, and this module does
//! not claim it is. No contract path allocates for an escape-free payload: the
//! encoder writes into a caller-supplied [`PayloadBuffer`] and the decoder
//! reads each field straight out of its input. A payload that spells the same
//! value with a JSON escape is different -- `serde_json` unescapes any such
//! string into a heap scratch buffer before the field is validated -- and a
//! refusal allocates for the decoder's own error value. Both are transient,
//! neither is retained, and both are bounded by
//! [`MAX_CONTRACT_PAYLOAD_BYTES`]. `tests/allocation_bounds.rs` is the
//! executable statement of this.

pub mod encoding;
pub mod focus_switch;
pub mod graph;

use crate::id::CorrelationId;

/// Scalar upper bound, in bytes, on one encoded contract payload.
pub const MAX_CONTRACT_PAYLOAD_BYTES: usize = 2048;

/// The contracts this module defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SchemaId {
    /// The focus-switch report P04 Mercurius sends to P07 Lictor.
    FocusSwitchReport,
}

impl SchemaId {
    /// The contract this module declares.
    pub const ALL: [Self; 1] = [Self::FocusSwitchReport];

    /// Returns the stable version tag the payload's `schema` field carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::FocusSwitchReport => "aegis.p04-p07.focus-switch.v1",
        }
    }

    /// Returns the subsystem-graph edge this schema carries.
    #[must_use]
    pub const fn edge(self) -> graph::EdgeId {
        match self {
            Self::FocusSwitchReport => graph::EdgeId::FocusSwitchNotify,
        }
    }
}

#[macro_use]
#[path = "../../../contract_scaffold.rs"]
mod contract_scaffold;

/// What a refusal is about: the contract, and the payload when it could be read.
pub type Correlation = contract_scaffold::Correlation<SchemaId, CorrelationId>;

scaffold_contract!(
    SchemaId,
    CorrelationId,
    MAX_CONTRACT_PAYLOAD_BYTES,
    ContractError
);

define_contract_error! {
    SchemaId, CorrelationId;
    /// Reasons a contract payload is refused.
    ///
    /// Every variant carries its [`Correlation`], so no refusal is anonymous.
    pub enum ContractError {
        /// The payload travels on an edge this schema does not carry.
        #[error("{correlation}: this schema does not travel on the edge the payload names")]
        WrongEdge {
            /// What the refusal is about.
            correlation: Correlation,
        },
    }
}

/// A fixed buffer holding one encoded contract payload.
pub type PayloadBuffer = contract_scaffold::PayloadBuffer<MAX_CONTRACT_PAYLOAD_BYTES>;
