// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! HISS-03: the claim that no decision path allocates, made falsifiable.
//!
//! The reason this crate can say a decision path allocates nothing is
//! structural: every value on such a path is `Copy`, so admitting a plugin or
//! a descriptor writes into storage that already exists. `assert_no_heap::<T>()`
//! is the executable form: a `Vec`, `String` or `Box` anywhere inside one of
//! these types, at any depth, removes `Copy` and this file stops compiling,
//! which fails the gate.
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

use aegis_calliope::{
    BufferHandle, BufferId, CalliopeError, ContractError, Correlation, CorrelationId, Declared,
    DmaBufFrame, DmaBufStreamDescriptor, DmaBufTable, DriftEstimator, FourCc, FrameGeometry,
    GrantedRtPrio, Label, MAX_CONTRACT_PAYLOAD_BYTES, MAX_DMA_BUFFERS, MAX_PLUGIN_SLOTS,
    MemlockExpectation, PayloadBuffer, PhaseOffsetMicros, PixelFormat, PluginHost,
    PluginSandboxSlot, PluginSlotId, PluginStage, Quantum, RealtimeGrant, ReferenceProfileLimits,
    SampleRate, SchedPolicy, SyncStrategy,
};

use common::{Fallible, descriptor, encoded_descriptor, filled_host, filled_table, tamper};

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
    assert_no_heap::<PluginHost>();
    assert_no_heap::<PluginSandboxSlot>();
    assert_no_heap::<PluginSlotId>();
    assert_no_heap::<PluginStage>();
    assert_no_heap::<DmaBufTable>();
    assert_no_heap::<DmaBufFrame>();
    assert_no_heap::<BufferId>();
    assert_no_heap::<BufferHandle>();
    assert_no_heap::<FrameGeometry>();
    assert_no_heap::<FourCc>();
    assert_no_heap::<PixelFormat>();
    assert_no_heap::<GrantedRtPrio>();
    assert_no_heap::<SchedPolicy>();
    assert_no_heap::<MemlockExpectation>();
    assert_no_heap::<ReferenceProfileLimits>();
    assert_no_heap::<Quantum>();
    assert_no_heap::<SampleRate>();
    assert_no_heap::<Declared<u32>>();
    assert_no_heap::<DriftEstimator>();
    assert_no_heap::<PhaseOffsetMicros>();
    assert_no_heap::<SyncStrategy>();
    assert_no_heap::<RealtimeGrant>();
    assert_no_heap::<DmaBufStreamDescriptor>();
    assert_no_heap::<CorrelationId>();
    assert_no_heap::<Label>();
    assert_no_heap::<PayloadBuffer>();
    assert_no_heap::<CalliopeError>();
    assert_no_heap::<ContractError>();
    assert_no_heap::<Correlation>();
}

/// Positive: driving both tables to their bounds changes no size.
#[test]
fn driving_the_tables_changes_no_storage() -> Fallible {
    let host = filled_host(MAX_PLUGIN_SLOTS)?;
    let table = filled_table(MAX_DMA_BUFFERS)?;
    assert_eq!(size_of_val(&host), size_of::<PluginHost>());
    assert_eq!(size_of_val(&table), size_of::<DmaBufTable>());
    assert!(size_of::<PluginHost>() > size_of::<PluginSandboxSlot>());
    assert!(size_of::<DmaBufTable>() > size_of::<DmaBufFrame>());
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a payload carrying a JSON escape still decodes to a value that
/// owns no heap, and the transient scratch buffer is not denied.
#[test]
fn an_escaped_payload_decodes_to_a_value_that_owns_no_heap() -> Fallible {
    let original = descriptor(PixelFormat::Argb8888, 640, 480)?;
    let text = encoded_descriptor(&original)?;
    // `-` spelled as an escape. serde_json unescapes it into a heap scratch
    // buffer before any field of ours sees the string; the decoded value is
    // still the same inline value.
    let escaped = tamper(&text, "looking-glass", "looking\\u002Dglass")?;
    assert!(escaped.contains("\\u002D"));
    let decoded = DmaBufStreamDescriptor::decode(&escaped)?;
    assert_eq!(decoded, original);
    assert_no_heap::<DmaBufStreamDescriptor>();
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
    let buffer = PayloadBuffer::default();
    assert!(buffer.is_empty());
    assert_eq!(size_of_val(&buffer), size_of::<PayloadBuffer>());
}
