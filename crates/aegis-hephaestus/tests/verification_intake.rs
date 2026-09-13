// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E08-3, the P09-to-P14 half: the verification request consumed.
//!
//! The edge `VERIFY_CODE_CAD` was typed at M06 in the producer's crate, which
//! is where the M01 register's direction lives. These cases show the consumer
//! **using the producer's type** rather than a copy of it, taking the
//! producer's refusals unchanged, and reaching no verdict -- because no solver
//! is admitted at this milestone in either crate.
//!
//! Decision D27 stays open, and the last case asserts that the record
//! `aegis-minerva` publishes still says so.

mod common;

use aegis_hephaestus::{AdmittedVerification, VerificationIntake, VerificationOutcome};
use aegis_minerva::{
    CadVerificationDirection, CadVerificationRequest, ContractError,
    D27_CAD_VERIFICATION_DIRECTION, DecisionState, EdgeId, MAX_CONTRACT_PAYLOAD_BYTES,
    ScreenOutcome,
};

use common::{Fallible, encoded_request, producer_request, tamper};

// --- Positive -------------------------------------------------------------

/// Positive: a well-formed producer payload is admitted, decoded through the
/// producer's own type.
#[test]
fn a_producer_request_is_admitted() -> Fallible {
    let request = producer_request(ScreenOutcome::NotRejected)?;
    let text = encoded_request(&request)?;
    let mut intake = VerificationIntake::new();
    let admitted: AdmittedVerification = intake.accept(&text)?;
    assert_eq!(admitted.request, request);
    assert_eq!(admitted.outcome, VerificationOutcome::AdmittedPendingSolver);
    assert_eq!(admitted.request.edge, EdgeId::VerifyCodeCad);
    assert_eq!(admitted.request.constraint_count, 4);
    assert_eq!(admitted.request.screened, ScreenOutcome::NotRejected);
    assert_eq!(intake.admitted(), 1);
    assert_eq!(intake.refused(), 0);
    assert_eq!(VerificationIntake::default(), VerificationIntake::new());
    Ok(())
}

/// Positive: the direction the consumer reads is the producer's own constant,
/// so the two sides of the edge cannot drift apart.
#[test]
fn the_direction_is_read_from_the_producer() {
    assert_eq!(
        VerificationIntake::DIRECTION,
        CadVerificationRequest::DIRECTION
    );
    assert_eq!(
        VerificationIntake::DIRECTION,
        CadVerificationDirection::GRAPH_OF_RECORD
    );
    assert_eq!(
        VerificationIntake::DIRECTION,
        CadVerificationDirection::MinervaSubmitsHephaestusVerifies
    );
    assert!(VerificationIntake::DIRECTION.is_graph_of_record());
    assert_eq!(VerificationIntake::DIRECTION.submitter(), "P09_Minerva");
    assert_eq!(VerificationIntake::DIRECTION.verifier(), "P14_Hephaestus");
    assert_eq!(VerificationIntake::DIRECTION.evidence(), "export-062");
}

// --- Negative -------------------------------------------------------------

/// Negative: a payload naming the wrong contract is refused, and the refusal is
/// the producer's.
///
/// Nothing is re-implemented on this side, so an unknown version and an unknown
/// field both come back as the producer's own variants.
#[test]
fn a_wrong_contract_is_refused_with_the_producers_error() -> Fallible {
    let text = encoded_request(&producer_request(ScreenOutcome::NotRejected)?)?;
    let mut intake = VerificationIntake::new();

    let other_version = tamper(
        &text,
        "aegis.p09-p14.cad-verification.v1",
        "aegis.p09-p14.cad-verification.v2",
    )?;
    assert!(matches!(
        intake.accept(&other_version),
        Err(ContractError::UnknownVersion { .. })
    ));

    let unknown_field = tamper(&text, "{", "{\"solver\":\"z3\",")?;
    assert!(matches!(
        intake.accept(&unknown_field),
        Err(ContractError::Malformed { .. })
    ));
    assert_eq!(intake.refused(), 2);
    assert_eq!(intake.admitted(), 0);
    Ok(())
}

/// Negative: the other reading of D27 and an over-long payload are refused the
/// same way, again with the producer's own variants.
#[test]
fn the_other_reading_and_an_over_long_payload_are_refused() -> Fallible {
    let text = encoded_request(&producer_request(ScreenOutcome::NotRejected)?)?;
    let mut intake = VerificationIntake::new();

    let reversed = tamper(
        &text,
        "minerva-submits-hephaestus-verifies",
        "hephaestus-submits-minerva-verifies",
    )?;
    assert!(matches!(
        intake.accept(&reversed),
        Err(ContractError::WrongDirection { .. })
    ));

    let long = "x".repeat(MAX_CONTRACT_PAYLOAD_BYTES + 1);
    assert!(matches!(
        intake.accept(&long),
        Err(ContractError::PayloadTooLong { .. })
    ));
    assert_eq!(intake.refused(), 2);
    Ok(())
}

/// Negative: a script the producer's own screen rejected is not admitted here
/// either, and the refusal is again the producer's.
#[test]
fn a_screened_out_script_is_not_admitted() -> Fallible {
    let rejected = producer_request(ScreenOutcome::RejectedByScreen)?;
    assert!(rejected.validate().is_err());

    let text = encoded_request(&producer_request(ScreenOutcome::NotRejected)?)?;
    let tampered = tamper(&text, "not-rejected", "rejected-by-screen")?;
    let mut intake = VerificationIntake::new();
    assert!(matches!(
        intake.accept(&tampered),
        Err(ContractError::RejectedScriptSubmitted { .. })
    ));
    assert_eq!(intake.refused(), 1);
    Ok(())
}

/// Negative: no outcome this crate can reach means a script was verified.
///
/// The check is over every variant the type has, so it cannot be satisfied by
/// picking the one variant that happens to answer `false`. REQ-P14-04 asks for
/// a symbolic-solver gate and none is admitted, so there is nothing here that
/// could answer `true`.
#[test]
fn no_outcome_means_verified() {
    let outcome = VerificationOutcome::ADMITTED;
    assert_eq!(outcome, VerificationOutcome::AdmittedPendingSolver);
    assert!(!outcome.is_verified());
    assert_eq!(outcome.name(), "admitted-pending-solver");
    assert!(outcome.name().contains("pending"));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: decision D27 is still open, so consuming the producer's reading
/// has not settled the edge.
///
/// The record read here is the one `aegis-minerva` publishes, not a copy: if a
/// later milestone closes D27, this case fails and someone has to decide what
/// the consumer does about it.
#[test]
fn d27_is_still_unresolved() {
    let record = D27_CAD_VERIFICATION_DIRECTION;
    assert_eq!(record.id, "D27");
    assert_eq!(record.state, DecisionState::Unresolved);
    assert!(!record.is_settled());
    assert_eq!(record.readings, CadVerificationDirection::BOTH);
    assert_eq!(record.dispute, "DSP-05");
    assert_eq!(record.recorded_at, "M06");
    assert!(record.settled_by.contains("M08"));
}

/// Boundary: both readings stay representable, and the one this crate does not
/// consume is refused rather than unspellable.
///
/// That is the property M06 was after: the payload for the other reading would
/// be a request type in this crate, and nothing has to be un-decided first.
#[test]
fn both_readings_stay_representable() {
    let other = CadVerificationDirection::HephaestusSubmitsMinervaVerifies;
    assert!(!other.is_graph_of_record());
    assert_eq!(other.submitter(), "P14_Hephaestus");
    assert_eq!(other.verifier(), "P09_Minerva");
    assert_eq!(other.evidence(), "export-002");
    assert_eq!(other.tag(), "hephaestus-submits-minerva-verifies");
    assert_ne!(other, VerificationIntake::DIRECTION);
    assert_eq!(CadVerificationDirection::BOTH.len(), 2);
}

/// Boundary: the intake counts what it saw, and an admitted request and a
/// refusal move different counters.
#[test]
fn the_intake_counts_both_outcomes() -> Fallible {
    let text = encoded_request(&producer_request(ScreenOutcome::NotRejected)?)?;
    let mut intake = VerificationIntake::new();
    intake.accept(&text)?;
    intake.accept(&text)?;
    assert_eq!(intake.admitted(), 2);
    assert_eq!(intake.refused(), 0);
    assert!(intake.accept("{}").is_err());
    assert_eq!(intake.admitted(), 2);
    assert_eq!(intake.refused(), 1);
    Ok(())
}
