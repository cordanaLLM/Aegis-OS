// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The one versioned payload P11 Ludus produces at milestone M08.
//!
//! | Edge | Schema | Module |
//! | :-- | :-- | :-- |
//! | P11 Ludus to P02 Janus and Vallum, `ATTEST_GAME_TRANSACTION` | [`transaction_receipt::TransactionReceipt`] | [`transaction_receipt`] |
//!
//! P11's other outbound edge, `DISPATCH_RICH_PRESENCE` to P04 Mercurius, is
//! **deliberately not typed here**: decision D48 is open on whether that
//! integration survives the reasoning ADR-0002 applied to the SDK, and the
//! sources name no library to build it against.
//! [`D48_PRESENCE_INTEGRATION`](crate::D48_PRESENCE_INTEGRATION) is the record.
//!
//! The receipt is typed in the producer's crate, which is where every schema in
//! this workspace lives except one: `aegis-vulcan` types both descriptors P03
//! hands out, `aegis-hestia` types the registration P15 hands to P04, and
//! `aegis-minerva` types the request P09 submits to P14. The exception is the
//! P09-to-P10 capsule request, which `aegis-vesta` owns because its bounds and
//! its monitor identity are P10's rather than P09's. Nothing about a receipt is
//! P02's in that way, so this one stays with its producer, and it is retired if
//! P02 ever types its own.
//!
//! # What this module is not
//!
//! **No transport is implemented here, and none is implied.** There is no
//! socket, no bus connection, no TPM2 and no quote. A schema states the shape
//! and the bounds of a payload; carrying one between two processes is later
//! work.
//!
//! # Versioning and bounds
//!
//! The schema carries an explicit `schema` tag as its first field, typed as an
//! enum with one variant per admitted version, so a payload naming any other
//! version is refused by the decoder rather than parsed leniently. A payload
//! longer than [`MAX_CONTRACT_PAYLOAD_BYTES`] is refused before it is parsed at
//! all, and every field of a decoded payload lands in a fixed inline slot: the
//! schema type is `Copy`, so a decoded value owns no heap.
//!
//! Decoding is **not** unconditionally allocation-free, and this module does
//! not claim it is: `serde_json` unescapes a JSON string into a heap scratch
//! buffer before the field is validated, and a refusal allocates for the
//! decoder's own error value. Both are transient, neither is retained, and both
//! are bounded by [`MAX_CONTRACT_PAYLOAD_BYTES`].
//! `tests/allocation_bounds.rs` is the executable statement of this.

pub mod encoding;
pub mod graph;
pub mod transaction_receipt;

use aegis_justitia::Identity;

/// Scalar upper bound, in bytes, on one encoded contract payload.
pub const MAX_CONTRACT_PAYLOAD_BYTES: usize = 1024;

/// The consumer contract this module defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SchemaId {
    /// The microtransaction receipt P11 hands to P02.
    TransactionReceipt,
}

impl SchemaId {
    /// Returns the stable version tag the payload's `schema` field carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::TransactionReceipt => "aegis.p11-p02.transaction-receipt.v1",
        }
    }

    /// Returns the subsystem-graph edge this schema carries.
    #[must_use]
    pub const fn edge(self) -> graph::EdgeId {
        match self {
            Self::TransactionReceipt => graph::EdgeId::AttestGameTransaction,
        }
    }
}

#[macro_use]
#[path = "../../../contract_scaffold.rs"]
mod contract_scaffold;

scaffold_contract!(
    SchemaId,
    Identity,
    MAX_CONTRACT_PAYLOAD_BYTES,
    ContractError
);

define_contract_error! {
    SchemaId, Identity;
    /// Reasons a contract payload is refused.
    ///
    /// Every variant carries its [`Correlation`], so no refusal is anonymous.
    pub enum ContractError {
        /// A receipt claimed to be sealed and named no register to seal to.
        #[error("{correlation}: a sealed receipt must name at least one platform register")]
        NoSealedRegisters {
            /// What the refusal is about.
            correlation: Correlation,
        },
    }
}

/// A fixed buffer holding one encoded contract payload.
pub type PayloadBuffer = contract_scaffold::PayloadBuffer<MAX_CONTRACT_PAYLOAD_BYTES>;

/// What a refusal is about: the contract, and the payload when it could be read.
pub type Correlation = contract_scaffold::Correlation<SchemaId, Identity>;
