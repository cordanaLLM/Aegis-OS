// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The payload plumbing both M18 schemas share.
//!
//! One scalar byte bound serves both schemas, and one lenient peek reads the
//! `schema` tag a payload claimed. The peek exists so that a payload naming a
//! contract version this build does not admit is refused as an unknown version
//! rather than as generic malformed input: the two have different fixes, and a
//! consumer that cannot tell them apart cannot report which.

/// Scalar bound, in bytes, on one encoded schema payload.
///
/// The larger of the two schemas is the kernel requirement, whose worst case
/// is a full feature list at [`crate::kernel::MAX_FEATURES`] rows plus a
/// signature. The bound leaves headroom above that and is the same for both,
/// so a caller needs one limit rather than two.
pub const MAX_PAYLOAD_BYTES: usize = 16_384;

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

/// Returns the contract version a payload claimed, when it claimed a readable one.
pub(crate) fn declared_schema(text: &str) -> Option<String> {
    serde_json::from_str::<PeekSchema>(text)
        .ok()
        .and_then(|row| row.schema)
}
