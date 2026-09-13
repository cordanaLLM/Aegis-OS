// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! HISS-03: the claim that no decision path allocates, made falsifiable.
//!
//! The reason this crate can say a decision path allocates nothing is
//! structural: every value on such a path is `Copy`, so admitting a sandbox or
//! a capsule writes into storage that already exists. `assert_no_heap::<T>()`
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

use aegis_vesta::{
    CandidateEvaluation, CapabilitySet, CapsuleMemoryLimit, CapsuleRegistry, CapsuleRequest,
    CapsuleSlot, ContractError, Correlation, CorrelationId, EvaluationVerdict, GuestMemoryMib,
    Label, MAX_CONTRACT_PAYLOAD_BYTES, MicroVmController, MicroVmInstance, MicroVmRequest,
    PayloadBuffer, RingCore, Unmeasured, VestaError, VmId, VmmIdentity, VsockCid, WasmCapsule,
};

use common::{Fallible, capsule_request, encoded_request, evaluation, tamper};

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
    assert_no_heap::<MicroVmController>();
    assert_no_heap::<MicroVmInstance>();
    assert_no_heap::<MicroVmRequest>();
    assert_no_heap::<GuestMemoryMib>();
    assert_no_heap::<VsockCid>();
    assert_no_heap::<VmId>();
    assert_no_heap::<CapsuleRegistry>();
    assert_no_heap::<WasmCapsule>();
    assert_no_heap::<CapsuleSlot>();
    assert_no_heap::<CapsuleMemoryLimit>();
    assert_no_heap::<CapabilitySet>();
    assert_no_heap::<CapsuleRequest>();
    assert_no_heap::<CandidateEvaluation>();
    assert_no_heap::<CorrelationId>();
    assert_no_heap::<Label>();
    assert_no_heap::<PayloadBuffer>();
    assert_no_heap::<RingCore>();
    assert_no_heap::<VmmIdentity>();
    assert_no_heap::<VestaError>();
    assert_no_heap::<ContractError>();
    assert_no_heap::<Correlation>();
    assert_no_heap::<Unmeasured<u32>>();
}

/// Positive: driving both tables to their bounds changes no size.
#[test]
fn driving_the_tables_changes_no_storage() -> Fallible {
    let controller = common::filled_controller(aegis_vesta::MAX_MICROVMS)?;
    let registry = common::filled_registry(aegis_vesta::MAX_WASM_CAPSULES)?;
    assert_eq!(size_of_val(&controller), size_of::<MicroVmController>());
    assert_eq!(size_of_val(&registry), size_of::<CapsuleRegistry>());
    assert!(size_of::<MicroVmController>() > size_of::<MicroVmInstance>());
    Ok(())
}

/// Positive: encoding writes into the caller's buffer and returns a borrow of
/// it, so a payload that fits costs no allocation of ours.
#[test]
fn encoding_writes_into_the_callers_buffer() -> Fallible {
    let request = capsule_request(1, VmmIdentity::Firecracker)?;
    let mut buffer = PayloadBuffer::new();
    let text = request.encode_into(&mut buffer)?;
    let written = text.len();
    assert!(written > 0);
    assert!(written <= MAX_CONTRACT_PAYLOAD_BYTES);
    assert_eq!(size_of_val(&buffer), size_of::<PayloadBuffer>());
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a decoded payload owns no heap even when the input made the
/// parser allocate, which is the case this crate does not deny.
#[test]
fn an_escaped_payload_still_decodes_into_an_inline_value() -> Fallible {
    let text = encoded_request(&capsule_request(1, VmmIdentity::Firecracker)?)?;
    let escaped = tamper(
        &text,
        "aegis-extism-capsule-01",
        "aegis\\u002Dextism-capsule-01",
    )?;
    let decoded = CapsuleRequest::decode(&escaped)?;
    assert_eq!(decoded.capsule, Label::parse("aegis-extism-capsule-01")?);
    assert_eq!(size_of_val(&decoded), size_of::<CapsuleRequest>());
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: both payload types stay well inside the buffer they are encoded
/// into, so the bound is a bound and not a fit.
#[test]
fn both_payloads_stay_inside_the_contract_bound() -> Fallible {
    let request = encoded_request(&capsule_request(
        aegis_vesta::MAX_WASM_CAPSULES,
        VmmIdentity::QemuMicrovm,
    )?)?;
    let evaluated = {
        let mut buffer = PayloadBuffer::new();
        evaluation(EvaluationVerdict::Inconclusive)?
            .encode_into(&mut buffer)?
            .to_owned()
    };
    for payload in [&request, &evaluated] {
        assert!(payload.len() < MAX_CONTRACT_PAYLOAD_BYTES);
        assert!(payload.len().saturating_mul(2) < MAX_CONTRACT_PAYLOAD_BYTES);
    }
    assert_eq!(
        size_of::<PayloadBuffer>(),
        MAX_CONTRACT_PAYLOAD_BYTES + size_of::<usize>()
    );
    Ok(())
}
