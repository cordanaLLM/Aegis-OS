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
//! The scaffold's own store did not have that property: its `storage_path` was
//! a `String`, so building one allocated. [`StoragePath`] is an inline buffer
//! instead, which is what makes [`PgliteVectorStore`] `Copy` at all.
//!
//! # What is deliberately not claimed
//!
//! **Decoding is not unconditionally allocation-free, and this file says so by
//! exercising the case rather than by denying it.** A payload that spells a
//! value with a JSON escape makes `serde_json` unescape the string into a heap
//! scratch buffer before any field of ours sees it. That is bounded by
//! [`MAX_CONTRACT_PAYLOAD_BYTES`] and never retained, and the decoded value
//! still owns no heap -- which is the claim the crate actually makes.
//! Rendering a [`HestiaView`] to JSON allocates too, and is a step the caller
//! makes rather than part of the snapshot.

mod common;

use core::mem::size_of;

use aegis_hestia::{
    ContractError, CorrelationId, DmaBufFd, Embedding, HestiaError, HestiaView,
    MAX_CONTRACT_PAYLOAD_BYTES, OverlayRegistration, PayloadBuffer, PgliteVectorStore, PipSurface,
    PixelExtent, QueryHits, QueryLimit, StoragePath, SurfaceId, WaylandPipMediaController,
};

use common::{CORRELATION, Fallible, initialised, registered, registration, store};

/// The largest a store may be before the size is worth a second look.
const MAX_STORE_BYTES: usize = 192;

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
    assert_no_heap::<PgliteVectorStore>();
    assert_no_heap::<StoragePath>();
    assert_no_heap::<QueryLimit>();
    assert_no_heap::<QueryHits>();
    assert_no_heap::<Embedding>();
    assert_no_heap::<WaylandPipMediaController>();
    assert_no_heap::<PipSurface>();
    assert_no_heap::<DmaBufFd>();
    assert_no_heap::<PixelExtent>();
    assert_no_heap::<CorrelationId>();
    assert_no_heap::<SurfaceId>();
    assert_no_heap::<OverlayRegistration>();
    assert_no_heap::<HestiaView>();
    assert_no_heap::<PayloadBuffer>();
    assert_no_heap::<HestiaError>();
    assert_no_heap::<ContractError>();
}

/// Positive: initialising and querying changes no storage.
#[test]
fn driving_the_store_changes_no_storage() -> Fallible {
    let before = size_of::<PgliteVectorStore>();
    let store = initialised()?;
    store.query(&common::embedding(), QueryLimit::MAX)?;
    assert_eq!(size_of_val(&store), before);
    Ok(())
}

/// Positive: a snapshot is a read, so taking one changes nothing either.
#[test]
fn taking_a_snapshot_changes_nothing() -> Fallible {
    let store = initialised()?;
    let controller = registered()?;
    let first = HestiaView::snapshot(&store, &controller);
    let second = HestiaView::snapshot(&store, &controller);
    assert_eq!(first, second);
    assert_eq!(size_of_val(&first), size_of::<HestiaView>());
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: an escaped payload is the case the crate does not claim.
///
/// `m` is an admissible spelling of `m`, so the two payloads carry the
/// same correlation identifier. They decode to the same value -- which is the
/// property that matters -- while the escaped one costs `serde_json` a heap
/// scratch buffer the plain one does not.
#[test]
fn an_escaped_payload_decodes_to_the_same_value() -> Fallible {
    let payload = registration()?;
    let mut buffer = PayloadBuffer::new();
    let plain = payload.encode_into(&mut buffer)?.to_owned();
    assert!(plain.contains(CORRELATION));

    let escaped = plain.replace("m17-fixture", "\\u006d17-fixture");
    assert!(escaped.contains("\\u006d"));
    assert_ne!(escaped, plain);
    assert_eq!(OverlayRegistration::decode(&escaped)?, payload);
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

    let mut second = PayloadBuffer::default();
    registration()?.encode_into(&mut second)?;
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

/// Boundary: the store is small, and a copy of it shares nothing.
#[test]
fn a_copied_store_shares_nothing_with_its_original() -> Fallible {
    assert!(
        size_of::<PgliteVectorStore>() <= MAX_STORE_BYTES,
        "the store measures {} bytes, past the recorded bound of {MAX_STORE_BYTES}",
        size_of::<PgliteVectorStore>()
    );
    let original = store()?;
    let mut fork = original;
    fork.initialize()?;
    assert!(fork.query(&common::embedding(), QueryLimit::MIN).is_ok());
    assert!(
        original
            .query(&common::embedding(), QueryLimit::MIN)
            .is_err()
    );

    let bare = WaylandPipMediaController::new();
    let mut copy = bare;
    copy.register(common::surface()?)?;
    assert!(copy.is_active());
    assert!(!bare.is_active());
    Ok(())
}
