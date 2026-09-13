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

use core::fmt;

use serde::de::{Error as DeError, Unexpected, Visitor};
use serde::ser::Error as SerError;
use serde::{Deserializer, Serializer};

use crate::dmabuf::PixelFormat;
use crate::id::{CorrelationId, Label};
use crate::latency::{Quantum, SampleRate};
use crate::rtprio::GrantedRtPrio;

/// A visitor that parses one string field through a validating constructor.
struct Parsed<T> {
    label: &'static str,
    parse: fn(&str) -> Option<T>,
}

impl<T> Visitor<'_> for Parsed<T> {
    type Value = T;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label)
    }

    fn visit_str<E: DeError>(self, value: &str) -> Result<T, E> {
        match (self.parse)(value) {
            Some(parsed) => Ok(parsed),
            None => Err(E::invalid_value(Unexpected::Str(value), &self)),
        }
    }
}

/// A visitor that parses one unsigned field through a validating constructor.
struct Bounded<T> {
    label: &'static str,
    parse: fn(u64) -> Option<T>,
}

impl<T> Visitor<'_> for Bounded<T> {
    type Value = T;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label)
    }

    fn visit_u64<E: DeError>(self, value: u64) -> Result<T, E> {
        match (self.parse)(value) {
            Some(parsed) => Ok(parsed),
            None => Err(E::invalid_value(Unexpected::Unsigned(value), &self)),
        }
    }
}

impl serde::Serialize for CorrelationId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let text = core::str::from_utf8(self.as_bytes()).map_err(S::Error::custom)?;
        serializer.serialize_str(text)
    }
}

impl<'de> serde::Deserialize<'de> for CorrelationId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(Parsed {
            label: "a bounded correlation identifier",
            parse: |text| Self::parse(text).ok(),
        })
    }
}

impl serde::Serialize for Label {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let text = core::str::from_utf8(self.as_bytes()).map_err(S::Error::custom)?;
        serializer.serialize_str(text)
    }
}

impl<'de> serde::Deserialize<'de> for Label {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(Parsed {
            label: "a bounded plugin, thread or stream label",
            parse: |text| Self::parse(text).ok(),
        })
    }
}

impl serde::Serialize for GrantedRtPrio {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(u64::from(self.get()))
    }
}

impl<'de> serde::Deserialize<'de> for GrantedRtPrio {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_u64(Bounded {
            label: "a real-time priority the source authorises",
            parse: |value| u8::try_from(value).ok().and_then(|raw| Self::new(raw).ok()),
        })
    }
}

impl serde::Serialize for Quantum {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(u64::from(self.get()))
    }
}

impl<'de> serde::Deserialize<'de> for Quantum {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_u64(Bounded {
            label: "a graph quantum inside the declared range",
            parse: |value| {
                u32::try_from(value)
                    .ok()
                    .and_then(|raw| Self::new(raw).ok())
            },
        })
    }
}

impl serde::Serialize for SampleRate {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(u64::from(self.get()))
    }
}

impl<'de> serde::Deserialize<'de> for SampleRate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_u64(Bounded {
            label: "an admitted sample rate",
            parse: |value| {
                u32::try_from(value)
                    .ok()
                    .and_then(|raw| Self::new(raw).ok())
            },
        })
    }
}

impl serde::Serialize for PixelFormat {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.tag())
    }
}

impl<'de> serde::Deserialize<'de> for PixelFormat {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(Parsed {
            label: "an admitted pixel format name",
            parse: Self::from_tag,
        })
    }
}
