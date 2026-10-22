// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! D116: shared contract field encoding visitors and helpers across subsystem crates.
//!
//! One file, included by path into the contracts encoding module of every crate
//! that defines or consumes cross-subsystem versioned payloads
//! (`#[path = "../../../contract_encoding.rs"]`), extending the D78 and D116 precedents
//! to contract field encoding. It is not a crate and has no manifest: a workspace member
//! would enter every dependency closure.
//!
//! Included by path into:
//! - `aegis-calliope`
//! - `aegis-hestia`
//! - `aegis-justitia`
//! - `aegis-lictor`
//! - `aegis-tellus`
//! - `aegis-vulcan`
//!
//! What this shared file folds:
//! - [`Parsed`]: a visitor that parses one string field through a validating constructor.
//! - [`serialize_text`]: serialises a validated byte string as a UTF-8 string.
//! - [`impl_str_serde!`]: macro generating `Serialize` and `Deserialize` implementations for
//!   bounded text identifier types backed by `as_bytes()` and `parse()`.

#![allow(dead_code)]

use core::fmt;

use serde::Serializer;
use serde::de::{Error as DeError, Unexpected, Visitor};
use serde::ser::Error as SerError;

/// A visitor that parses one string field through a validating constructor.
pub(crate) struct Parsed<T> {
    pub(crate) label: &'static str,
    pub(crate) parse: fn(&str) -> Option<T>,
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

/// Serialises a validated byte string as a UTF-8 string.
pub(crate) fn serialize_text<S: Serializer>(
    bytes: &[u8],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let text = core::str::from_utf8(bytes).map_err(S::Error::custom)?;
    serializer.serialize_str(text)
}

/// Implements `Serialize` and `Deserialize` for a bounded string identifier type.
macro_rules! impl_str_serde {
    ($type:ty, $label:expr) => {
        impl serde::Serialize for $type {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                contract_encoding::serialize_text(self.as_bytes(), serializer)
            }
        }

        impl<'de> serde::Deserialize<'de> for $type {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                deserializer.deserialize_str(contract_encoding::Parsed {
                    label: $label,
                    parse: |text| <$type>::parse(text).ok(),
                })
            }
        }
    };
}
