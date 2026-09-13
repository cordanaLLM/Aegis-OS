// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The field encoding the M08 P11 payload uses.
//!
//! One file holds every hand-written `Serialize`/`Deserialize` pair, so the
//! wire form of a bounded value is reviewed in one place rather than
//! rediscovered per schema. Three properties are deliberate:
//!
//! * **Validation happens during decoding.** Amounts and register indices are
//!   parsed through the same constructors the library already uses, so an
//!   out-of-range field is refused by the decoder rather than accepted and
//!   checked afterwards. This is why a receipt naming register 24 does not
//!   decode.
//! * **Every value lands in a fixed inline slot.** Nothing in this file owns
//!   heap, which is why the payload type is `Copy`. What this does not claim is
//!   that decoding is allocation-free in every case: `serde_json` unescapes a
//!   JSON string into a heap scratch buffer before any visitor here sees it, so
//!   an escaped field costs a transient copy bounded by
//!   [`super::MAX_CONTRACT_PAYLOAD_BYTES`]. See `tests/allocation_bounds.rs`.
//! * **A register selection is a list of indices on the wire**, not a bitmask
//!   integer. The reader of a payload sees which registers were named; an
//!   out-of-range index is refused rather than masked away, and the list is
//!   bounded at [`MAX_PCR_ENTRIES`], so a payload cannot make the decoder walk
//!   an unbounded sequence.

use core::fmt;

use serde::de::{Error as DeError, SeqAccess, Unexpected, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserializer, Serializer};

use crate::receipt::{AmountCents, MAX_PCR_ENTRIES, MAX_PCR_INDEX, PcrIndex, PcrSelection};

impl serde::Serialize for AmountCents {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(self.get())
    }
}

/// A visitor that parses one unsigned field through a validating constructor.
struct BoundedAmount;

impl Visitor<'_> for BoundedAmount {
    type Value = AmountCents;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a receipt amount inside the declared range")
    }

    fn visit_u64<E: DeError>(self, value: u64) -> Result<AmountCents, E> {
        match AmountCents::new(value) {
            Ok(parsed) => Ok(parsed),
            Err(_) => Err(E::invalid_value(Unexpected::Unsigned(value), &self)),
        }
    }
}

impl<'de> serde::Deserialize<'de> for AmountCents {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_u64(BoundedAmount)
    }
}

impl serde::Serialize for PcrSelection {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(usize::try_from(self.count()).ok())?;
        for raw in 0..=MAX_PCR_INDEX {
            let Ok(index) = PcrIndex::new(raw) else {
                continue;
            };
            if self.contains(index) {
                seq.serialize_element(&raw)?;
            }
        }
        seq.end()
    }
}

/// A visitor that reads a bounded list of register indices.
struct PcrList;

impl<'de> Visitor<'de> for PcrList {
    type Value = PcrSelection;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded list of platform configuration register indices")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<PcrSelection, A::Error> {
        let mut selection = PcrSelection::new();
        for _ in 0..MAX_PCR_ENTRIES {
            let Some(raw) = access.next_element::<u8>()? else {
                return Ok(selection);
            };
            let index = PcrIndex::new(raw).map_err(|_| {
                A::Error::invalid_value(Unexpected::Unsigned(u64::from(raw)), &self)
            })?;
            selection = selection.with(index);
        }
        if access.next_element::<u8>()?.is_some() {
            return Err(A::Error::invalid_length(
                MAX_PCR_ENTRIES.saturating_add(1),
                &self,
            ));
        }
        Ok(selection)
    }
}

impl<'de> serde::Deserialize<'de> for PcrSelection {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(PcrList)
    }
}
