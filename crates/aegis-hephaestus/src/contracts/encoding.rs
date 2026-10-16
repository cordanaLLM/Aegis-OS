// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The field encoding the M08 P14 payload uses.
//!
//! The hand-written `Serialize`/`Deserialize` pairs for [`StepPath`],
//! [`MeshElementCount`] and [`Tolerance`] live in `tornus-geometry`, beside
//! the types: the bounds moved there (cut unit CUT-1632-01), and Rust's orphan
//! rule lets no other crate implement a foreign trait for a foreign type.
//! Three properties hold for the payload:
//!
//! * **Validation happens during decoding.** Element counts, tolerances and
//!   source paths are parsed through the same constructors the library already
//!   uses, so an out-of-range field is refused by the decoder rather than
//!   accepted and checked afterwards. This is why a descriptor naming 500,001
//!   elements does not decode.
//! * **Every value lands in a fixed inline slot.** No field owns heap, which is
//!   why the payload type is `Copy`. What this does not claim is that decoding
//!   is allocation-free in every case: `serde_json` unescapes a JSON string
//!   into a heap scratch buffer before any visitor sees it, so an escaped field
//!   costs a transient copy bounded by [`super::MAX_CONTRACT_PAYLOAD_BYTES`].
//!   See `tests/allocation_bounds.rs`.
//! * **A length is an integer count of micrometres**, never a float. A boundary
//!   test on a binary float is a test on rounding; the scaffold's millimetre
//!   floats do not reach the wire.
//!
//! [`StepPath`]: crate::StepPath
//! [`MeshElementCount`]: crate::MeshElementCount
//! [`Tolerance`]: crate::Tolerance
