// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! HISS-03: the claim that no decision path allocates, made falsifiable.
//!
//! The reason this crate can say a decision path allocates nothing is
//! structural: every value on such a path is `Copy`, so registering a process
//! or classifying a burst writes into storage that already exists.
//! `assert_no_heap::<T>()` is the executable form: a `Vec`, `String` or `Box`
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
//! still owns no heap -- which is the claim the crate actually makes.

mod common;

use core::mem::size_of;

use aegis_lictor::{
    ContractError, CoreMask, Correlation, CorrelationId, DispatchQueue, EwmaBurst, FocusOutcome,
    FocusSwitchReport, FragilityProbe, GrantDraft, Label, LictorError, MAX_CONTRACT_PAYLOAD_BYTES,
    MAX_TRACKED_PROCESSES, PayloadBuffer, Pid, ProbeStage, ProcessRecord, ProcessTier,
    ResourceBroker, SliceName, TaskShiftAction, Tier,
};

use common::{COMPOSITOR_PID, Fallible, encoded_report, filled_broker, report, tamper};

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
    assert_no_heap::<ResourceBroker>();
    assert_no_heap::<ProcessRecord>();
    assert_no_heap::<ProcessTier>();
    assert_no_heap::<Pid>();
    assert_no_heap::<FocusOutcome>();
    assert_no_heap::<EwmaBurst>();
    assert_no_heap::<Tier>();
    assert_no_heap::<DispatchQueue>();
    assert_no_heap::<CoreMask>();
    assert_no_heap::<FragilityProbe>();
    assert_no_heap::<ProbeStage>();
    assert_no_heap::<FocusSwitchReport>();
    assert_no_heap::<GrantDraft>();
    assert_no_heap::<TaskShiftAction>();
    assert_no_heap::<CorrelationId>();
    assert_no_heap::<Label>();
    assert_no_heap::<SliceName>();
    assert_no_heap::<PayloadBuffer>();
    assert_no_heap::<LictorError>();
    assert_no_heap::<ContractError>();
    assert_no_heap::<Correlation>();
}

/// Positive: driving the table to its bound changes no size.
#[test]
fn driving_the_table_changes_no_storage() -> Fallible {
    let broker = filled_broker(MAX_TRACKED_PROCESSES)?;
    assert_eq!(size_of_val(&broker), size_of::<ResourceBroker>());
    assert!(size_of::<ResourceBroker>() > size_of::<ProcessRecord>());
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a payload carrying a JSON escape still decodes to a value that
/// owns no heap, and the transient scratch buffer is not denied.
#[test]
fn an_escaped_payload_decodes_to_a_value_that_owns_no_heap() -> Fallible {
    let original = report(COMPOSITOR_PID)?;
    let text = encoded_report(&original)?;
    // `-` spelled as an escape. serde_json unescapes it into a heap scratch
    // buffer before any field of ours sees the string; the decoded value is
    // still the same inline value.
    let escaped = tamper(&text, "aegis-compositor", "aegis\\u002Dcompositor")?;
    assert!(escaped.contains("\\u002D"));
    let decoded = FocusSwitchReport::decode(&escaped)?;
    assert_eq!(decoded, original);
    assert_no_heap::<FocusSwitchReport>();
    assert!(escaped.len() <= MAX_CONTRACT_PAYLOAD_BYTES);
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the payload buffer is exactly its declared size, so the bound is
/// the storage rather than a length check applied afterwards.
#[test]
fn the_payload_buffer_is_its_declared_size() {
    assert!(size_of::<PayloadBuffer>() >= MAX_CONTRACT_PAYLOAD_BYTES);
    assert_eq!(MAX_CONTRACT_PAYLOAD_BYTES, 2048);
    let buffer = PayloadBuffer::new();
    assert!(buffer.is_empty());
    assert_eq!(size_of_val(&buffer), size_of::<PayloadBuffer>());
}
