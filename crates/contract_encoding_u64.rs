// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Shared contract field encoding helper for bounded integer fields.
//!
//! Included by path (`#[path = "../../../contract_encoding_u64.rs"]`) into the
//! contract encoding modules of `aegis-calliope` and `aegis-lictor`, the two
//! crates with bounded integer fields; `contract_encoding.rs` holds the text
//! helpers every encoding module shares.

#![allow(dead_code)]

use core::fmt;

use serde::de::{Error as DeError, Unexpected, Visitor};

/// A visitor that parses one unsigned field through a validating constructor.
pub(crate) struct Bounded<T> {
    pub(crate) label: &'static str,
    pub(crate) parse: fn(u64) -> Option<T>,
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

/// Implements `Serialize` and `Deserialize` for an unsigned integer type using a bounded validator.
macro_rules! impl_u64_bounded_serde {
    ($type:ty, $label:expr, $parse:expr) => {
        impl serde::Serialize for $type {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_u64(u64::from(self.get()))
            }
        }

        impl<'de> serde::Deserialize<'de> for $type {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                deserializer.deserialize_u64(contract_encoding_u64::Bounded {
                    label: $label,
                    parse: $parse,
                })
            }
        }
    };
}
