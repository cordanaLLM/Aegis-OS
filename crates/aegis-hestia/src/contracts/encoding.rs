// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The field encoding the M17 overlay registration uses.
//!
//! One file holds every hand-written `Serialize`/`Deserialize` pair, so the
//! wire form of a bounded value is reviewed in one place rather than
//! rediscovered per schema. Two properties are deliberate:
//!
//! * **Validation happens during decoding.** A correlation identifier and a
//!   surface identifier are parsed through the same constructors the library
//!   already uses, so an over-long or ill-formed field is refused by the
//!   decoder rather than accepted and checked afterwards. Every other field is
//!   a newtype over an integer wired to its own `TryFrom`, which is why a
//!   negative `DMA-BUF` descriptor does not decode.
//! * **Every value lands in a fixed inline slot.** Nothing in this file owns
//!   heap, which is why the registration type is `Copy`. What this does not
//!   claim is that decoding is allocation-free in every case: `serde_json`
//!   unescapes a JSON string into a heap scratch buffer before any visitor
//!   here sees it, so an escaped field costs a transient copy bounded by
//!   [`super::MAX_CONTRACT_PAYLOAD_BYTES`]. See `tests/allocation_bounds.rs`.

#[macro_use]
#[path = "../../../contract_encoding.rs"]
mod contract_encoding;

use crate::id::{CorrelationId, SurfaceId};

impl_str_serde!(CorrelationId, "a bounded correlation identifier");
impl_str_serde!(SurfaceId, "a bounded surface identifier");
