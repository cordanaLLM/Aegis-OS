// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The subsystem-graph edge P11's own contract touches.
//!
//! The graph of record (export-062 `1ce919ed54bb`) names two outbound edges for
//! P11: `ATTEST_GAME_TRANSACTION` to P02 Janus and Vallum, and
//! `DISPATCH_RICH_PRESENCE` to P04 Mercurius. Neither direction is disputed --
//! but both are among the eight edges the M01 register records as appearing in
//! the graph of record and in **neither** architecture document, which is
//! dispute DSP-19 against open decision D37. So each is typed on a single
//! supporting source, and that is what [`EdgeId::sources`] records.
//!
//! Only the first is admitted here. [`EdgeId`] has one variant because this
//! crate types one payload: the presence edge is held back by open decision
//! D48, so there is no schema for it and no edge value that could travel on
//! one. A payload naming the presence edge therefore does not decode, which is
//! the same device the rejected runtime gets in `aegis-vesta` -- not a
//! judgement that the edge does not exist, but the absence of a schema for it.

/// An edge of the subsystem graph that P11 Ludus produces on and this crate
/// types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum EdgeId {
    /// P11 to P02: a microtransaction receipt for non-repudiation.
    #[serde(rename = "ATTEST_GAME_TRANSACTION")]
    AttestGameTransaction,
}

impl EdgeId {
    /// Every edge this enum names.
    pub const ALL: [Self; 1] = [Self::AttestGameTransaction];

    /// The edge the graph of record draws and this crate holds back.
    ///
    /// Recorded as a name rather than as a variant: a variant would be a value
    /// a payload could carry, and D48 has not decided that the edge survives.
    pub const UNTYPED_PRESENCE_EDGE: &'static str = "DISPATCH_RICH_PRESENCE";

    /// Returns the edge identifier as it is written in the graph of record.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::AttestGameTransaction => "ATTEST_GAME_TRANSACTION",
        }
    }

    /// Returns the component identifier at the far end of the edge.
    #[must_use]
    pub const fn consumer(self) -> &'static str {
        match self {
            Self::AttestGameTransaction => "P02",
        }
    }

    /// Returns the transport the graph of record records for this edge.
    ///
    /// Recorded verbatim. No TPM2 is opened and no quote is taken anywhere in
    /// this crate, and nothing acts on the string.
    #[must_use]
    pub const fn recorded_transport(self) -> &'static str {
        match self {
            Self::AttestGameTransaction => "Hardware TPM2 PCR Quote",
        }
    }

    /// Returns how many sources of the imported bundle draw this edge.
    ///
    /// One, which is dispute DSP-19: the graph of record draws it and neither
    /// architecture document names it. A schema on a single-source edge is
    /// still a schema; it is not a two-source agreement, and open decision D37
    /// is where that is settled.
    #[must_use]
    pub const fn sources(self) -> u8 {
        match self {
            Self::AttestGameTransaction => 1,
        }
    }
}
