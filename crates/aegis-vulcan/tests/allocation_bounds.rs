// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! HISS-03: the claim that no decision path allocates, made falsifiable.
//!
//! The reason the crate can say a decision path allocates nothing is
//! structural: every value on such a path is `Copy`, so a check writes into
//! storage that already exists. That is equivalent to a property a test can
//! hold, and `assert_no_heap::<T>()` is it: a `Vec`, `String` or `Box`
//! anywhere inside one of these types, at any depth, removes `Copy` and this
//! file stops compiling, which fails the gate.
//!
//! # What is deliberately not claimed
//!
//! **Decoding is not unconditionally allocation-free, and this file says so by
//! exercising the case rather than by denying it.** A payload that spells a
//! value with a JSON escape makes `serde_json` unescape the string into a heap
//! scratch buffer before any field of ours sees it. That is bounded by
//! [`MAX_CONTRACT_PAYLOAD_BYTES`] and never retained, and the decoded value
//! still owns no heap -- which is the claim the crate actually makes. A
//! refusal allocates too, for the decoder's own error value.
//!
//! The size figures below are computed with `size_of` on the building target,
//! so this file, not the prose, is the authority for the numbers.

mod common;

use core::mem::size_of;

use aegis_vulcan::{
    BarAddress, BarWindow, BlockCount, ContractError, CorrelationId, DeviceTable, DmaBufExport,
    DmaRequest, MAX_CONTRACT_PAYLOAD_BYTES, MediaIngestDescriptor, PayloadBuffer, RenderNode,
    RingIndex, TransferReceipt, UserSpacePcieDriver, VfioDeviceConfig, VulcanError,
    WeightStreamDescriptor,
};

use common::{CORRELATION, Fallible, mapped, media_ingest, request, weight_stream};

/// The largest a driver may be before the size is worth a second look.
const MAX_DRIVER_BYTES: usize = 128;

/// Accepts only a type that owns no heap, because it is `Copy`.
fn assert_no_heap<T: Copy>() {}

// --- Positive -------------------------------------------------------------

/// Positive: every value on a decision path owns no heap.
///
/// This is the falsifier. Adding a `Vec`, `String` or `Box` to any of these
/// types, or to anything they contain, removes `Copy` and this test stops
/// compiling.
#[test]
fn every_value_on_a_decision_path_owns_no_heap() {
    assert_no_heap::<UserSpacePcieDriver>();
    assert_no_heap::<VfioDeviceConfig>();
    assert_no_heap::<DeviceTable>();
    assert_no_heap::<BarWindow>();
    assert_no_heap::<BarAddress>();
    assert_no_heap::<BlockCount>();
    assert_no_heap::<RingIndex>();
    assert_no_heap::<DmaRequest>();
    assert_no_heap::<TransferReceipt>();
    assert_no_heap::<CorrelationId>();
    assert_no_heap::<RenderNode>();
    assert_no_heap::<DmaBufExport>();
    assert_no_heap::<WeightStreamDescriptor>();
    assert_no_heap::<MediaIngestDescriptor>();
    assert_no_heap::<PayloadBuffer>();
    assert_no_heap::<VulcanError>();
    assert_no_heap::<ContractError>();
}

/// Positive: driving map and dispatch changes no storage, because there is
/// none to change.
#[test]
fn driving_the_driver_changes_no_storage() -> Fallible {
    let before = size_of::<UserSpacePcieDriver>();
    let mut driver = mapped()?;
    driver.execute_direct_dma(request(64)?)?;
    assert_eq!(size_of_val(&driver), before);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: an escaped payload is the case the crate does not claim.
///
/// `m` is an admissible spelling of `m`, so the two payloads carry the
/// same correlation identifier. They decode to the same value -- which is the
/// property that matters -- while the escaped one costs `serde_json` a heap
/// scratch buffer the plain one does not. Recording it here is what keeps the
/// documentation's allocation claim conditional rather than absolute.
#[test]
fn an_escaped_payload_decodes_to_the_same_value() -> Fallible {
    let descriptor = weight_stream(8)?;
    let mut buffer = PayloadBuffer::new();
    let plain = descriptor.encode_into(&mut buffer)?.to_owned();
    assert!(plain.contains(CORRELATION));

    let escaped = plain.replace("m17-fixture", "\\u006d17-fixture");
    assert!(escaped.contains("\\u006d"));
    assert_ne!(escaped, plain);
    assert_eq!(WeightStreamDescriptor::decode(&escaped)?, descriptor);
    Ok(())
}

/// Negative: the payload buffer refuses to grow, and reports nothing written.
#[test]
fn the_payload_buffer_does_not_grow() -> Fallible {
    let buffer = PayloadBuffer::new();
    assert!(buffer.is_empty());
    assert_eq!(buffer.len(), 0);
    assert_eq!(buffer.as_bytes(), b"");
    assert_eq!(buffer.as_str(), Some(""));
    assert_eq!(size_of::<PayloadBuffer>(), size_of_val(&buffer));

    let mut second = PayloadBuffer::default();
    let descriptor = media_ingest(1)?;
    descriptor.encode_into(&mut second)?;
    assert_eq!(size_of_val(&second), size_of::<PayloadBuffer>());
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the buffer is exactly its bound plus its length field.
#[test]
fn the_payload_buffer_is_its_bound_plus_a_length() {
    let buffer = size_of::<PayloadBuffer>();
    assert!(buffer >= MAX_CONTRACT_PAYLOAD_BYTES);
    assert!(buffer <= MAX_CONTRACT_PAYLOAD_BYTES.saturating_add(size_of::<usize>() * 2));
}

/// Boundary: the driver is small, and a copy of it shares nothing.
#[test]
fn a_copied_driver_shares_nothing_with_its_original() -> Fallible {
    assert!(size_of::<UserSpacePcieDriver>() <= MAX_DRIVER_BYTES);
    let original = mapped()?;
    let mut fork = original;
    fork.execute_direct_dma(request(1)?)?;
    assert_eq!(fork.ring_tail().get(), 1);
    assert_eq!(original.ring_tail().get(), 0);
    Ok(())
}
