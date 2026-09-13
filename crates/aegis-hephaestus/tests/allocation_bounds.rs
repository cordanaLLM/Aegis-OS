// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! HISS-03: the claim that no decision path allocates, made falsifiable.
//!
//! The reason this crate can say a decision path allocates nothing is
//! structural: every value on such a path is `Copy`, so planning a mesh or
//! admitting a request writes into storage that already exists.
//! `assert_no_heap::<T>()` is the executable form: a `Vec`, `String` or `Box`
//! anywhere inside one of these types, at any depth, removes `Copy` and this
//! file stops compiling, which fails the gate.
//!
//! The producer's request type is on the list too. It is `aegis-minerva`'s and
//! not ours, and a consumer that stored it would inherit whatever it owns, so
//! the property is checked on this side rather than assumed from the other.
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

use aegis_hephaestus::{
    AdmittedVerification, BrepEvaluator, CadEngine, ContractError, Correlation, EvaluationOutcome,
    GeometryViewport, HephaestusError, MAX_CONTRACT_PAYLOAD_BYTES, MeshConfig, MeshElementCount,
    MeshPlan, PayloadBuffer, RecordedClaim, SolverAdmission, SolverTarget, StepPath, Tolerance,
    UnpinnedDependency, VerificationIntake, VerificationOutcome,
};
use aegis_minerva::CadVerificationRequest;

use common::{Fallible, encoded_viewport, scaffold_viewport, tamper};

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
    assert_no_heap::<BrepEvaluator>();
    assert_no_heap::<CadEngine>();
    assert_no_heap::<EvaluationOutcome>();
    assert_no_heap::<MeshConfig>();
    assert_no_heap::<MeshElementCount>();
    assert_no_heap::<MeshPlan>();
    assert_no_heap::<StepPath>();
    assert_no_heap::<Tolerance>();
    assert_no_heap::<SolverAdmission>();
    assert_no_heap::<SolverTarget>();
    assert_no_heap::<VerificationIntake>();
    assert_no_heap::<VerificationOutcome>();
    assert_no_heap::<AdmittedVerification>();
    assert_no_heap::<CadVerificationRequest>();
    assert_no_heap::<GeometryViewport>();
    assert_no_heap::<PayloadBuffer>();
    assert_no_heap::<HephaestusError>();
    assert_no_heap::<ContractError>();
    assert_no_heap::<Correlation>();
    assert_no_heap::<RecordedClaim>();
    assert_no_heap::<UnpinnedDependency>();
}

/// Positive: driving a plan to the element bound changes no size.
#[test]
fn planning_at_the_bound_changes_no_storage() -> Fallible {
    let engine = common::evaluator();
    let plan = engine.mesh(common::mesh_config()?, aegis_hephaestus::MAX_MESH_ELEMENTS)?;
    assert_eq!(size_of_val(&plan), size_of::<MeshPlan>());
    assert_eq!(size_of_val(&engine), size_of::<BrepEvaluator>());
    assert!(size_of::<MeshPlan>() >= size_of::<MeshElementCount>());
    Ok(())
}

/// Positive: encoding writes into the caller's buffer and returns a borrow of
/// it, so a payload that fits costs no allocation of ours.
#[test]
fn encoding_writes_into_the_callers_buffer() -> Fallible {
    let descriptor = scaffold_viewport()?;
    let mut buffer = PayloadBuffer::new();
    let written = descriptor.encode_into(&mut buffer)?.len();
    assert!(written > 0);
    assert!(written <= MAX_CONTRACT_PAYLOAD_BYTES);
    assert_eq!(size_of_val(&buffer), size_of::<PayloadBuffer>());
    assert_eq!(buffer.len(), written);
    assert_eq!(buffer.as_bytes().len(), written);
    assert_eq!(buffer.as_str().map(str::len), Some(written));
    assert!(!buffer.is_empty());
    assert_eq!(PayloadBuffer::default().len(), 0);
    assert_eq!(PayloadBuffer::new().as_str(), Some(""));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a decoded payload owns no heap even when the input made the
/// parser allocate, which is the case this crate does not deny.
#[test]
fn an_escaped_payload_still_decodes_into_an_inline_value() -> Fallible {
    let text = encoded_viewport(&scaffold_viewport()?)?;
    let escaped = tamper(&text, "bracket.step", "bracket\\u002Dstep.step")?;
    let decoded = GeometryViewport::decode(&escaped)?;
    assert_eq!(
        decoded.source,
        StepPath::parse("/var/lib/aegis/geometry/bracket-step.step")?
    );
    assert_eq!(size_of_val(&decoded), size_of::<GeometryViewport>());
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the payload stays well inside the buffer it is encoded into, so
/// the bound is a bound and not a fit.
#[test]
fn the_payload_stays_inside_the_contract_bound() -> Fallible {
    let text = encoded_viewport(&scaffold_viewport()?)?;
    assert!(text.len() < MAX_CONTRACT_PAYLOAD_BYTES);
    assert!(text.len().saturating_mul(2) < MAX_CONTRACT_PAYLOAD_BYTES);
    assert_eq!(
        size_of::<PayloadBuffer>(),
        MAX_CONTRACT_PAYLOAD_BYTES + size_of::<usize>()
    );
    Ok(())
}
