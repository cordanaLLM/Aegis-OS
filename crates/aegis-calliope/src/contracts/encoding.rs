// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The field encoding the two M07 P08 payloads use.
//!
//! One file holds every hand-written `Serialize`/`Deserialize` pair, so the
//! wire form of a bounded value is reviewed in one place rather than
//! rediscovered per schema. Three properties are deliberate:
//!
//! * **Validation happens during decoding.** Identifiers, labels, priorities,
//!   quanta, sample rates and pixel formats are parsed through the same
//!   constructors the library already uses, so an out-of-range field is refused
//!   by the decoder rather than accepted and checked afterwards. This is why a
//!   grant naming priority 96 does not decode.
//! * **Every value lands in a fixed inline slot.** Nothing in this file owns
//!   heap, which is why both payload types are `Copy`. What this does not claim
//!   is that decoding is allocation-free in every case: `serde_json` unescapes
//!   a JSON string into a heap scratch buffer before any visitor here sees it,
//!   so an escaped field costs a transient copy bounded by
//!   [`super::MAX_CONTRACT_PAYLOAD_BYTES`]. See `tests/allocation_bounds.rs`.
//! * **A pixel format is a name on the wire**, not a raw 32-bit code. The
//!   reader of a descriptor sees which format the stride was computed for, and
//!   an unadmitted name is refused rather than carried -- which is the
//!   scaffold's `0x34325641` defect made undecodable.

#[macro_use]
#[path = "../../../contract_encoding.rs"]
mod contract_encoding;

#[macro_use]
#[path = "../../../contract_encoding_u64.rs"]
mod contract_encoding_u64;

use serde::{Deserializer, Serializer};

use crate::dmabuf::PixelFormat;
use crate::id::{CorrelationId, Label};
use crate::latency::{Quantum, SampleRate};
use crate::rtprio::GrantedRtPrio;

impl_str_serde!(CorrelationId, "a bounded correlation identifier");
impl_str_serde!(Label, "a bounded plugin, thread or stream label");

impl_u64_bounded_serde!(
    GrantedRtPrio,
    "a real-time priority the source authorises",
    |value| u8::try_from(value).ok().and_then(|raw| Self::new(raw).ok())
);

impl_u64_bounded_serde!(
    Quantum,
    "a graph quantum inside the declared range",
    |value| u32::try_from(value)
        .ok()
        .and_then(|raw| Self::new(raw).ok())
);

impl_u64_bounded_serde!(SampleRate, "an admitted sample rate", |value| {
    u32::try_from(value)
        .ok()
        .and_then(|raw| Self::new(raw).ok())
});

impl serde::Serialize for PixelFormat {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.tag())
    }
}

impl<'de> serde::Deserialize<'de> for PixelFormat {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(contract_encoding::Parsed {
            label: "an admitted pixel format name",
            parse: Self::from_tag,
        })
    }
}
