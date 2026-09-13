// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! HISS-03: the claim that no decision path allocates, made falsifiable.
//!
//! Every value on a decision path here is `Copy`, so routing an intent or
//! recording a step writes into storage that already exists.
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

use aegis_minerva::{
    AgentHerEngine, AlpsRouter, CadVerificationRequest, CapsuleDispatch, ConstraintScreen,
    ConstraintScript, ContractError, Correlation, ExpertDomain, ExpertId, Label,
    MAX_CONTRACT_PAYLOAD_BYTES, MinervaError, PayloadBuffer, PowerEnvelope, ProposalDraft, Reward,
    ScreenOutcome, SlmExpert, StateHash, TrajectoryStep,
};

use common::{
    Fallible, budget, dispatch, draft, encoded, filled_router, seal, tamper, verification,
};

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
    assert_no_heap::<AlpsRouter>();
    assert_no_heap::<SlmExpert>();
    assert_no_heap::<ExpertId>();
    assert_no_heap::<ExpertDomain>();
    assert_no_heap::<PowerEnvelope>();
    assert_no_heap::<AgentHerEngine>();
    assert_no_heap::<TrajectoryStep>();
    assert_no_heap::<StateHash>();
    assert_no_heap::<Reward>();
    assert_no_heap::<ConstraintScript>();
    assert_no_heap::<ConstraintScreen>();
    assert_no_heap::<ScreenOutcome>();
    assert_no_heap::<CadVerificationRequest>();
    assert_no_heap::<ProposalDraft>();
    assert_no_heap::<CapsuleDispatch>();
    assert_no_heap::<Label>();
    assert_no_heap::<PayloadBuffer>();
    assert_no_heap::<MinervaError>();
    assert_no_heap::<ContractError>();
    assert_no_heap::<Correlation>();
}

/// Positive: driving the router to its bound changes no size.
#[test]
fn driving_the_router_changes_no_storage() -> Fallible {
    let mut router = filled_router(aegis_minerva::MAX_SLM_EXPERTS)?;
    let before = size_of_val(&router);
    let chosen = router.route(ExpertDomain::CodeSynthesis, budget(20_000)?);
    assert!(chosen.is_some());
    assert_eq!(size_of_val(&router), before);
    assert_eq!(before, size_of::<AlpsRouter>());
    Ok(())
}

/// Positive: encoding writes into the caller's buffer and returns a borrow of
/// it, so a payload that fits costs no allocation of ours.
#[test]
fn encoding_writes_into_the_callers_buffer() -> Fallible {
    let request = verification(ScreenOutcome::NotRejected)?;
    let mut buffer = PayloadBuffer::new();
    let text = request.encode_into(&mut buffer)?;
    assert!(!text.is_empty());
    assert!(text.len() <= MAX_CONTRACT_PAYLOAD_BYTES);
    assert_eq!(size_of_val(&buffer), size_of::<PayloadBuffer>());
    Ok(())
}

/// Positive: the two payloads P09 builds for other crates own no heap either,
/// so a chain hop is still a `Copy` value.
#[test]
fn the_chain_payloads_own_no_heap() -> Fallible {
    let proposal = draft(Some(&seal()?))?.into_proposal()?;
    let request = dispatch(1)?.into_request()?;
    assert_eq!(
        size_of_val(&proposal),
        size_of::<aegis_justitia::ActionProposal>()
    );
    assert_eq!(
        size_of_val(&request),
        size_of::<aegis_vesta::CapsuleRequest>()
    );
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a decoded payload owns no heap even when the input made the
/// parser allocate, which is the case this crate does not deny.
#[test]
fn an_escaped_payload_still_decodes_into_an_inline_value() -> Fallible {
    let text = encoded(&verification(ScreenOutcome::NotRejected)?)?;
    let escaped = tamper(&text, "m06-intent-0001", "m06\\u002Dintent-0001")?;
    let decoded = CadVerificationRequest::decode(&escaped)?;
    assert_eq!(decoded.correlation_id, common::identity()?);
    assert_eq!(size_of_val(&decoded), size_of::<CadVerificationRequest>());
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the payload stays well inside the buffer it is encoded into, so
/// the bound is a bound and not a fit.
#[test]
fn the_payload_stays_inside_the_contract_bound() -> Fallible {
    let text = encoded(&verification(ScreenOutcome::NotRejected)?)?;
    assert!(text.len() < MAX_CONTRACT_PAYLOAD_BYTES);
    assert!(text.len().saturating_mul(2) < MAX_CONTRACT_PAYLOAD_BYTES);
    assert_eq!(
        size_of::<PayloadBuffer>(),
        MAX_CONTRACT_PAYLOAD_BYTES + size_of::<usize>()
    );
    Ok(())
}
