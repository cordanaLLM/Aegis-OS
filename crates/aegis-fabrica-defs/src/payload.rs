// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The payload plumbing both M18 schemas share.
//!
//! One scalar byte bound serves both schemas, and one lenient peek reads the
//! `schema` tag a payload claimed. The peek exists so that a payload naming a
//! contract version this build does not admit is refused as an unknown version
//! rather than as generic malformed input: the two have different fixes, and a
//! consumer that cannot tell them apart cannot report which.
//!
//! # Only the object form decodes
//!
//! serde's derived decoder for a struct reads a JSON object, and it also reads
//! a JSON array of the struct's values in declaration order, through a
//! `visit_seq` it generates alongside `visit_map`. `deny_unknown_fields` reads
//! keys, and an array has none, so a payload written as
//! `["aegis.p01-nucleus.kernel-requirement.v1", "id", ["x86-64"], ...]` decoded
//! with nothing refusing it. No producer writes that form, the published JSON
//! Schemas describe objects only, and nucleus refuses it; this crate refused
//! nothing until the 2026-09-29 probe found it.
//!
//! Every struct of both contracts therefore implements `Deserialize` through
//! the crate-private `object_only!` macro: the deserializer is asked for a
//! map, and only the map's entries reach the decoder serde derives. That
//! decoder lives on a private field mirror of the struct, derived with
//! `#[serde(remote = ...)]`, so the public type has no derived `visit_seq` to
//! reach at all -- at the top of a payload or nested inside one.

use core::fmt;
use core::marker::PhantomData;

/// Scalar bound, in bytes, on one encoded schema payload.
///
/// The larger of the two schemas is the kernel requirement, whose worst case
/// is a full feature list at [`crate::kernel::MAX_FEATURES`] rows plus a
/// signature. The bound leaves headroom above that and is the same for both,
/// so a caller needs one limit rather than two.
pub const MAX_PAYLOAD_BYTES: usize = 16_384;

/// What a refused array form is told it should have been.
const OBJECT_EXPECTED: &str = "a JSON object; the array form of a contract struct is refused";

/// A contract struct that decodes from the entries of one JSON object.
///
/// Implemented by `object_only!`, which forwards to the decoder serde derives
/// on the struct's private field mirror.
pub(crate) trait ObjectFields<'de>: Sized {
    /// Decodes the struct from the entries of one JSON object.
    fn from_fields<A: serde::de::MapAccess<'de>>(map: A) -> Result<Self, A::Error>;
}

/// The visitor that admits a map and nothing else.
///
/// It implements `visit_map` only, so an array, a string or a number is
/// refused by serde's default for every other `visit_*` method, with
/// [`OBJECT_EXPECTED`] as what was expected instead.
pub(crate) struct ObjectVisitor<T>(PhantomData<T>);

impl<'de, T: ObjectFields<'de>> serde::de::Visitor<'de> for ObjectVisitor<T> {
    type Value = T;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(OBJECT_EXPECTED)
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<T, A::Error> {
        T::from_fields(map)
    }
}

/// Decodes `T` from a JSON object, refusing every other JSON value.
///
/// # Errors
///
/// Returns the deserializer's error for anything but an object, and whatever
/// `T`'s derived decoder refuses inside one.
pub(crate) fn object<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: ObjectFields<'de>,
    D: serde::Deserializer<'de>,
{
    deserializer.deserialize_map(ObjectVisitor(PhantomData))
}

/// Implements `Deserialize` for contract structs, object form only.
///
/// Each `Type => Mirror` pair names a public contract struct and its private
/// field mirror, a struct with the same fields and serde attributes derived
/// with `#[serde(remote = "Type")]`. serde's remote derive turns the mirror's
/// decoder into the inherent function `Mirror::deserialize`, which builds a
/// `Type`; the compiler checks that the mirror names every field of `Type`
/// with its type, so the two cannot drift apart on a field. The attributes are
/// held together by the round-trip tests and by the committed JSON Schema,
/// which is generated from the public type.
macro_rules! object_only {
    ($($name:ident => $mirror:ident),+ $(,)?) => {$(
        impl<'de> $crate::payload::ObjectFields<'de> for $name {
            fn from_fields<A: ::serde::de::MapAccess<'de>>(map: A) -> Result<Self, A::Error> {
                $mirror::deserialize(::serde::de::value::MapAccessDeserializer::new(map))
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(
                deserializer: D,
            ) -> Result<Self, D::Error> {
                $crate::payload::object(deserializer)
            }
        }
    )+};
}

pub(crate) use object_only;

/// The lenient view of a payload's `schema` field, and of nothing else.
///
/// Every other field is unknown to this type and is skipped rather than
/// parsed, so reading the claimed version cannot be spoiled by a field
/// elsewhere in the payload that does not decode.
#[derive(serde::Deserialize)]
struct PeekSchema {
    #[serde(default)]
    schema: Option<String>,
}

impl<'de> ObjectFields<'de> for PeekSchema {
    fn from_fields<A: serde::de::MapAccess<'de>>(map: A) -> Result<Self, A::Error> {
        <Self as serde::Deserialize>::deserialize(serde::de::value::MapAccessDeserializer::new(map))
    }
}

/// Returns the contract version a payload claimed, when it claimed a readable one.
///
/// Only an object claims one. An array whose first element is a version tag
/// claims nothing, so it is refused as malformed rather than as an unknown
/// version: the array form is refused whatever it carries.
pub(crate) fn declared_schema(text: &str) -> Option<String> {
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let peeked: PeekSchema = object(&mut deserializer).ok()?;
    deserializer.end().ok()?;
    peeked.schema
}
