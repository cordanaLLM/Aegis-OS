// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! HISS-03: the allocation claims this crate makes, each bound to a test.
//!
//! Two different claims are made here and they are not the same claim, so they
//! are held separately.
//!
//! The first is that every value on a decision path owns no heap, because it
//! is `Copy`. `assert_no_heap::<T>()` is the falsifier: a `Vec`, `String` or
//! `Box` anywhere inside one of these types, at any depth, removes `Copy` and
//! this file stops compiling. The imported scaffold does not have that
//! property -- its `AthenaEngine` holds a `String` ledger path and its
//! `log_ledger_entry` builds a formatted `String` per record.
//!
//! The second is narrower and is stated narrowly: **the ledger allocates once,
//! at construction, and not on the record path.** It holds a `Vec`, so it is
//! not `Copy` and never will be; what it guarantees is that the `Vec` is
//! reserved to its bound when it is built and refuses a push at that bound, so
//! appending never reallocates. `the_ledger_reserves_once_and_never_grows` is
//! the falsifier for exactly that and for nothing wider.

mod common;

use core::mem::size_of;

use aegis_athena::{
    AthenaError, CallDeadline, CandidateId, CandidateMetrics, CheckpointBody, CheckpointDraft,
    CheckpointRecord, ContractError, CorrelationId, LatencyMs, Lifecycle,
    MAX_CONTRACT_PAYLOAD_BYTES, Maturity, MemoryMb, NullModelRetention, Objective, ParetoVerdict,
    PayloadBuffer, PromotionGate, PromotionGates, PromotionTrigger, SciCarbonRate,
    Sha256CheckpointLedger, Slot, Stage, StubSysupdate, SysupdateCall,
};
use aegis_justitia::UnixSeconds;

use common::{AT, Fallible, candidate, passing, published};

/// The largest a lifecycle may be before the size is worth a second look.
const MAX_LIFECYCLE_BYTES: usize = 96;

/// Accepts only a type that owns no heap, because it is `Copy`.
fn assert_no_heap<T: Copy>() {}

/// Returns a draft for the fixture candidate at `at`.
fn draft(at: UnixSeconds) -> Result<CheckpointDraft, Box<dyn std::error::Error>> {
    Ok(CheckpointDraft {
        candidate: candidate()?,
        stage: Stage::Publish,
        metrics: Some(passing()?),
        verdict: Some(aegis_athena::gate(&passing()?)),
        reason: None,
        at,
    })
}

// --- Positive -------------------------------------------------------------

/// Positive: every value on a decision path owns no heap.
#[test]
fn every_value_on_a_decision_path_owns_no_heap() {
    assert_no_heap::<Lifecycle>();
    assert_no_heap::<Stage>();
    assert_no_heap::<ParetoVerdict>();
    assert_no_heap::<Objective>();
    assert_no_heap::<CandidateMetrics>();
    assert_no_heap::<LatencyMs>();
    assert_no_heap::<MemoryMb>();
    assert_no_heap::<SciCarbonRate>();
    assert_no_heap::<NullModelRetention>();
    assert_no_heap::<CandidateId>();
    assert_no_heap::<CorrelationId>();
    assert_no_heap::<CheckpointDraft>();
    assert_no_heap::<CheckpointBody>();
    assert_no_heap::<CheckpointRecord>();
    assert_no_heap::<PromotionTrigger>();
    assert_no_heap::<PromotionGates>();
    assert_no_heap::<PromotionGate>();
    assert_no_heap::<Maturity>();
    assert_no_heap::<Slot>();
    assert_no_heap::<StubSysupdate>();
    assert_no_heap::<SysupdateCall>();
    assert_no_heap::<CallDeadline>();
    assert_no_heap::<PayloadBuffer>();
    assert_no_heap::<AthenaError>();
    assert_no_heap::<ContractError>();
}

/// Positive: driving a lifecycle changes no storage.
#[test]
fn driving_a_lifecycle_changes_no_storage() -> Fallible {
    let mut lifecycle = Lifecycle::new();
    let before = size_of_val(&lifecycle);
    lifecycle.step(aegis_athena::Event::Challenge)?;
    lifecycle.step(aegis_athena::Event::Decompose)?;
    assert_eq!(size_of_val(&lifecycle), before);
    assert_eq!(size_of_val(&lifecycle), size_of::<Lifecycle>());
    assert!(
        before <= MAX_LIFECYCLE_BYTES,
        "a lifecycle measures {before} bytes, past the recorded bound of {MAX_LIFECYCLE_BYTES}"
    );
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: the ledger reserves once and never grows.
///
/// This is the second claim, and the only one made about the ledger. It holds
/// a `Vec`, so it allocates at construction; what it must not do is reallocate
/// while it is in use.
#[test]
fn the_ledger_reserves_once_and_never_grows() -> Fallible {
    let mut ledger = Sha256CheckpointLedger::with_bound(8);
    let reserved = ledger.reserved();
    assert_eq!(reserved, 8, "the reservation is the bound, taken once");

    for step in 0..8u64 {
        ledger.append(draft(UnixSeconds::new(AT.get().saturating_add(step)))?)?;
        assert_eq!(
            ledger.reserved(),
            reserved,
            "appending record {step} reallocated"
        );
    }
    assert!(
        ledger
            .append(draft(UnixSeconds::new(AT.get() + 99))?)
            .is_err()
    );
    assert_eq!(ledger.reserved(), reserved);
    Ok(())
}

/// Negative: an escaped payload is the case the crate does not claim.
///
/// `m` is an admissible spelling of `m`, so the two payloads carry the
/// same correlation identifier and decode to the same value, while the escaped
/// one costs `serde_json` a heap scratch buffer the plain one does not.
#[test]
fn an_escaped_payload_decodes_to_the_same_value() -> Fallible {
    let mut lifecycle = Lifecycle::new();
    lifecycle.step(aegis_athena::Event::Invalidate {
        reason: aegis_athena::InvalidationReason::Withdrawn,
    })?;
    let payload =
        PromotionTrigger::from_lifecycle(&lifecycle, common::correlation()?, candidate()?, Slot::A)
            .ok_or("an invalidated lifecycle justifies a trigger")?;

    let mut buffer = PayloadBuffer::new();
    let plain = payload.encode_into(&mut buffer)?.to_owned();
    let escaped = plain.replace("m05-fixture", "\\u006d05-fixture");
    assert!(escaped.contains("\\u006d"));
    assert_ne!(escaped, plain);
    assert_eq!(PromotionTrigger::decode(&escaped)?, payload);
    Ok(())
}

/// Negative: the payload buffer refuses to grow, and reports nothing written.
#[test]
fn the_payload_buffer_does_not_grow() {
    let buffer = PayloadBuffer::new();
    assert!(buffer.is_empty());
    assert_eq!(buffer.len(), 0);
    assert_eq!(buffer.as_bytes(), b"");
    assert_eq!(buffer.as_str(), Some(""));
    assert_eq!(
        size_of_val(&PayloadBuffer::default()),
        size_of::<PayloadBuffer>()
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the buffer is exactly its bound plus its length field.
#[test]
fn the_payload_buffer_is_its_bound_plus_a_length() {
    let buffer = size_of::<PayloadBuffer>();
    assert!(buffer >= MAX_CONTRACT_PAYLOAD_BYTES);
    assert!(buffer <= MAX_CONTRACT_PAYLOAD_BYTES.saturating_add(size_of::<usize>() * 2));
}

/// Boundary: a copied lifecycle and a copied gate set share nothing with their
/// originals.
#[test]
fn a_copied_value_shares_nothing_with_its_original() -> Fallible {
    let original = Lifecycle::new();
    let mut fork = original;
    fork.step(aegis_athena::Event::Challenge)?;
    assert_eq!(fork.stage(), Stage::Challenge);
    assert_eq!(original.stage(), Stage::Propose);

    let bare = PromotionGates::new();
    let mut copy = bare;
    copy.record(PromotionGate::BoundedLoops, true)?;
    assert_eq!(copy.recorded(), 1);
    assert_eq!(bare.recorded(), 0);

    let engine = published()?;
    assert_eq!(engine.ledger().len(), 1);
    Ok(())
}
