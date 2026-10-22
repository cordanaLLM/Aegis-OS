// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The field encoding the P07 payload uses.
//!
//! One file holds every hand-written `Serialize`/`Deserialize` pair, so the
//! wire form of a bounded value is reviewed in one place rather than
//! rediscovered per schema. Two properties are deliberate:
//!
//! * **Validation happens during decoding.** Identifiers, labels and slice
//!   names are parsed through the same constructors the library already uses,
//!   so an out-of-range field is refused by the decoder rather than accepted
//!   and checked afterwards. This is why a report naming
//!   `"background.service"` as its control group does not decode.
//! * **Every value lands in a fixed inline slot.** Nothing in this file owns
//!   heap, which is why the payload type is `Copy`. What this does not claim is
//!   that decoding is allocation-free in every case: `serde_json` unescapes a
//!   JSON string into a heap scratch buffer before any visitor here sees it, so
//!   an escaped field costs a transient copy bounded by
//!   [`super::MAX_CONTRACT_PAYLOAD_BYTES`]. See `tests/allocation_bounds.rs`.

#[macro_use]
#[path = "../../../contract_encoding.rs"]
mod contract_encoding;

#[macro_use]
#[path = "../../../contract_encoding_u64.rs"]
mod contract_encoding_u64;

use crate::broker::Pid;
use crate::id::{CorrelationId, Label, SliceName};

impl_str_serde!(CorrelationId, "a bounded correlation identifier");
impl_str_serde!(Label, "a bounded process or application label");
impl_str_serde!(SliceName, "a control-group slice name ending in .slice");

impl_u64_bounded_serde!(
    Pid,
    "a process identifier inside the admissible range",
    |value| u32::try_from(value)
        .ok()
        .and_then(|raw| Self::new(raw).ok())
);
