// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The field encoding the M05 P13 payloads use.
//!
//! One file holds every hand-written `Serialize`/`Deserialize` pair, so the
//! wire form of a bounded value is reviewed in one place rather than
//! rediscovered per schema. Three properties are deliberate:
//!
//! * **Validation happens during decoding.** Identifiers are parsed through
//!   the same constructors the library uses, and every numeric quantity goes
//!   through its own validating constructor, so a negative energy, a NaN
//!   intensity or an over-bound rate is refused by the decoder rather than
//!   accepted and checked afterwards.
//! * **A list decodes to at most its bound.** Both sequence types refuse the
//!   entry past [`MAX_QUERY_CANDIDATES`] rather than truncating, so a payload
//!   cannot silently lose a candidate on the way in.
//! * **Every value lands in a fixed inline slot.** Nothing in this file owns
//!   heap, which is why the schema types are `Copy`. What this does not claim
//!   is that decoding is allocation-free in every case: `serde_json` unescapes
//!   a JSON string into a heap scratch buffer before any visitor here sees it,
//!   so an escaped field costs a transient copy bounded by
//!   [`super::MAX_CONTRACT_PAYLOAD_BYTES`]. See `tests/allocation_bounds.rs`.

use core::fmt;

use serde::de::{Error as DeError, SeqAccess, Unexpected, Visitor};
use serde::ser::{Error as SerError, SerializeSeq};
use serde::{Deserializer, Serializer};

use crate::contracts::sci_query::{CandidateList, MAX_QUERY_CANDIDATES, RateEntry, RateList};
use crate::id::{CandidateId, CorrelationId, SliceName};
use crate::sci::{EnergyKwh, GridIntensity, SciRate, Seconds, Watts};

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

/// A visitor that parses one numeric field through a validating constructor.
struct Quantity<T> {
    label: &'static str,
    parse: fn(f64) -> Option<T>,
}

impl<T> Visitor<'_> for Quantity<T> {
    type Value = T;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label)
    }

    fn visit_f64<E: DeError>(self, value: f64) -> Result<T, E> {
        match (self.parse)(value) {
            Some(parsed) => Ok(parsed),
            None => Err(E::invalid_value(Unexpected::Float(value), &self)),
        }
    }

    /// Accepts an integer spelling of the same quantity, losslessly.
    ///
    /// JSON spells `0` as an integer, and every quantity here admits zero, so
    /// refusing integers outright would make the zero case unrepresentable.
    /// The conversion goes through `i32`, which `f64` holds exactly; an
    /// integer too large for that is refused rather than rounded, and it is
    /// far past every bound in [`crate::sci`] anyway.
    fn visit_i64<E: DeError>(self, value: i64) -> Result<T, E> {
        match i32::try_from(value) {
            Ok(narrow) => self.visit_f64(f64::from(narrow)),
            Err(_) => Err(E::invalid_value(Unexpected::Signed(value), &self)),
        }
    }

    /// Accepts an unsigned integer spelling, losslessly, through `u32`.
    fn visit_u64<E: DeError>(self, value: u64) -> Result<T, E> {
        match u32::try_from(value) {
            Ok(narrow) => self.visit_f64(f64::from(narrow)),
            Err(_) => Err(E::invalid_value(Unexpected::Unsigned(value), &self)),
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

impl serde::Serialize for CandidateId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let text = core::str::from_utf8(self.as_bytes()).map_err(S::Error::custom)?;
        serializer.serialize_str(text)
    }
}

impl<'de> serde::Deserialize<'de> for CandidateId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(Parsed {
            label: "a bounded candidate identifier",
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
            label: "a bounded cgroup slice name",
            parse: |text| Self::parse(text).ok(),
        })
    }
}

impl serde::Serialize for EnergyKwh {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.get())
    }
}

impl<'de> serde::Deserialize<'de> for EnergyKwh {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_f64(Quantity {
            label: "a bounded, non-negative energy in kWh",
            parse: |value| Self::new(value).ok(),
        })
    }
}

impl serde::Serialize for GridIntensity {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.get())
    }
}

impl<'de> serde::Deserialize<'de> for GridIntensity {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_f64(Quantity {
            label: "a bounded, non-negative grid carbon intensity in gCO2eq/kWh",
            parse: |value| Self::new(value).ok(),
        })
    }
}

impl serde::Serialize for SciRate {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.get())
    }
}

impl<'de> serde::Deserialize<'de> for SciRate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_f64(Quantity {
            label: "a bounded, non-negative SCI rate",
            parse: |value| Self::new(value).ok(),
        })
    }
}

impl serde::Serialize for Watts {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.get())
    }
}

impl<'de> serde::Deserialize<'de> for Watts {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_f64(Quantity {
            label: "a bounded, non-negative draw in watts",
            parse: |value| Self::new(value).ok(),
        })
    }
}

impl serde::Serialize for Seconds {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.get())
    }
}

impl<'de> serde::Deserialize<'de> for Seconds {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_f64(Quantity {
            label: "a bounded, positive interval in seconds",
            parse: |value| Self::new(value).ok(),
        })
    }
}

/// A visitor that reads a bounded sequence of candidate identifiers.
struct CandidateSeq;

impl<'de> Visitor<'de> for CandidateSeq {
    type Value = CandidateList;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "at most {MAX_QUERY_CANDIDATES} candidate identifiers"
        )
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<CandidateList, A::Error> {
        let mut list = CandidateList::new();
        for _ in 0..MAX_QUERY_CANDIDATES {
            let Some(candidate) = access.next_element::<CandidateId>()? else {
                return Ok(list);
            };
            list.push(candidate).map_err(A::Error::custom)?;
        }
        match access.next_element::<CandidateId>()? {
            Some(_) => Err(A::Error::invalid_length(
                MAX_QUERY_CANDIDATES.saturating_add(1),
                &self,
            )),
            None => Ok(list),
        }
    }
}

impl serde::Serialize for CandidateList {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for index in 0..self.len() {
            let candidate = self.get(index).ok_or_else(|| {
                S::Error::custom("the candidate list reported a length it cannot produce")
            })?;
            seq.serialize_element(&candidate)?;
        }
        seq.end()
    }
}

impl<'de> serde::Deserialize<'de> for CandidateList {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(CandidateSeq)
    }
}

/// A visitor that reads a bounded sequence of evaluated rates.
struct RateSeq;

impl<'de> Visitor<'de> for RateSeq {
    type Value = RateList;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "at most {MAX_QUERY_CANDIDATES} rate entries")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<RateList, A::Error> {
        let mut list = RateList::new();
        for _ in 0..MAX_QUERY_CANDIDATES {
            let Some(entry) = access.next_element::<RateEntry>()? else {
                return Ok(list);
            };
            list.push(entry).map_err(A::Error::custom)?;
        }
        match access.next_element::<RateEntry>()? {
            Some(_) => Err(A::Error::invalid_length(
                MAX_QUERY_CANDIDATES.saturating_add(1),
                &self,
            )),
            None => Ok(list),
        }
    }
}

impl serde::Serialize for RateList {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for index in 0..self.len() {
            let entry = self.get(index).ok_or_else(|| {
                S::Error::custom("the rate list reported a length it cannot produce")
            })?;
            seq.serialize_element(&entry)?;
        }
        seq.end()
    }
}

impl<'de> serde::Deserialize<'de> for RateList {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(RateSeq)
    }
}
