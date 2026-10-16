// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The field encoding the two M06 payloads use.
//!
//! The hand-written `Serialize`/`Deserialize` pairs for the bounded values
//! live in `saeptum-capsule`, beside the types: the controllers moved there
//! (cut unit CUT-1632-03), and Rust's orphan rule lets no other crate
//! implement a foreign trait for a foreign type. Three properties hold for
//! both payloads:
//!
//! * **Validation happens during decoding.** Identifiers, labels, slots,
//!   memory limits and context identifiers are parsed through the same
//!   constructors the library already uses, so an out-of-range field is refused
//!   by the decoder rather than accepted and checked afterwards. This is why a
//!   capsule request naming slot 129 does not decode.
//! * **Every value lands in a fixed inline slot.** No field owns heap, which
//!   is why both payload types are `Copy`. What this does not claim is that
//!   decoding is allocation-free in every case: `serde_json` unescapes a JSON
//!   string into a heap scratch buffer before any visitor sees it, so an
//!   escaped field costs a transient copy bounded by
//!   [`super::MAX_CONTRACT_PAYLOAD_BYTES`]. See `tests/allocation_bounds.rs`.
//! * **A capability set is a list of names on the wire**, not four booleans.
//!   The reader of a payload sees what was granted; an unknown name is refused
//!   rather than ignored, and the list is bounded at
//!   [`MAX_CAPABILITY_ENTRIES`], so a payload cannot make the decoder walk an
//!   unbounded sequence.

pub use saeptum_capsule::encoding::MAX_CAPABILITY_ENTRIES;
