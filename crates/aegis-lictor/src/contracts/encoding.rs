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

use core::fmt;

use serde::de::{Error as DeError, Unexpected, Visitor};
use serde::ser::Error as SerError;
use serde::{Deserializer, Serializer};

use crate::broker::Pid;
use crate::id::{CorrelationId, Label, SliceName};

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
            label: "a bounded process or application label",
            parse: |text| Self::parse(text).ok(),
        })
    }
}

impl serde::Serialize for SliceName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let text = core::str::from_utf8(self.as_bytes()).map_err(S::Error::custom)?;
        serializer.serialize_str(text)
    }
}

impl<'de> serde::Deserialize<'de> for SliceName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(Parsed {
            label: "a control-group slice name ending in .slice",
            parse: |text| Self::parse(text).ok(),
        })
    }
}

impl serde::Serialize for Pid {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(u64::from(self.get()))
    }
}

impl<'de> serde::Deserialize<'de> for Pid {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_u64(Bounded {
            label: "a process identifier inside the admissible range",
            parse: |value| {
                u32::try_from(value)
                    .ok()
                    .and_then(|raw| Self::new(raw).ok())
            },
        })
    }
}
