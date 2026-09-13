// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E06-4: the candidate evaluation P10 Vesta hands to P16 Athena
//! (REQ-P10-07, REQ-P16-04).
//!
//! Positive: every verdict round-trips and the monitor travels with it.
//! Negative: a malformed payload and the wrong edge are refused. Boundary: an
//! inconclusive verdict does not admit a candidate, which is the one place a
//! three-valued verdict could quietly become two-valued.

mod common;

use aegis_vesta::{
    CandidateEvaluation, ContractError, Correlation, CorrelationId, EdgeId, EvaluationVerdict,
    SchemaId, VmId, VmmIdentity,
};

use common::{CORRELATION, Fallible, encoded_evaluation, evaluation, other_edge, tamper};

/// The exact payload the fixture evaluation encodes to.
const GOLDEN: &str = r#"{"schema":"aegis.p10-p16.candidate-evaluation.v1","edge":"SANDBOX_CANDIDATE_EVALUATION","correlation-id":"m06-fixture-0001","candidate":"aegis-candidate-0007","vm-id":100,"vmm":"firecracker","verdict":"passed","evaluated-at":1000}"#;

/// Returns the correlation a refusal of the fixture evaluation must carry.
fn expected_correlation() -> Result<Correlation, Box<dyn std::error::Error>> {
    Ok(Correlation::new(
        SchemaId::CandidateEvaluation,
        Some(CorrelationId::parse(CORRELATION)?),
    ))
}

// --- Positive -------------------------------------------------------------

/// Positive: a well-formed evaluation round-trips, and the encoding is stable.
#[test]
fn a_candidate_evaluation_round_trips() -> Fallible {
    let original = evaluation(EvaluationVerdict::Passed)?;
    let text = encoded_evaluation(&original)?;
    assert_eq!(text, GOLDEN, "the field encoding is stable");
    let decoded = CandidateEvaluation::decode(&text)?;
    assert_eq!(decoded, original);
    assert_eq!(decoded.validate(), Ok(()));
    assert_eq!(decoded.correlation(), expected_correlation()?);
    assert_eq!(decoded.vm_id, VmId::new(100));
    assert_eq!(decoded.vmm, VmmIdentity::Firecracker);
    assert_eq!(decoded.evaluated_at.get(), 1_000);
    Ok(())
}

/// Positive: the schema and edge constants agree with the graph of record.
#[test]
fn the_schema_and_edge_constants_agree() {
    assert_eq!(CandidateEvaluation::SCHEMA, SchemaId::CandidateEvaluation);
    assert_eq!(
        CandidateEvaluation::EDGE,
        EdgeId::SandboxCandidateEvaluation
    );
    assert_eq!(
        SchemaId::CandidateEvaluation.edge(),
        CandidateEvaluation::EDGE
    );
    assert!(
        EdgeId::SandboxCandidateEvaluation
            .recorded_transport()
            .contains("AF_VSOCK")
    );
    assert!(EdgeId::SandboxCandidateEvaluation.in_graph_of_record());
}

/// Positive: every verdict round-trips under its recorded tag.
#[test]
fn every_verdict_round_trips() -> Fallible {
    for verdict in EvaluationVerdict::ALL {
        let text = encoded_evaluation(&evaluation(verdict)?)?;
        assert!(text.contains(verdict.tag()));
        assert_eq!(CandidateEvaluation::decode(&text)?.verdict, verdict);
    }
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: an unknown verdict, an unknown field and a bad version are each
/// refused.
#[test]
fn a_malformed_evaluation_is_refused() -> Fallible {
    let text = encoded_evaluation(&evaluation(EvaluationVerdict::Failed)?)?;
    let malformed = [
        tamper(&text, r#""failed""#, r#""probably-fine""#)?,
        tamper(&text, r#""vm-id":100"#, r#""vm-id":100,"boot-ms":112"#)?,
        tamper(&text, r#""evaluated-at":1000"#, r#""evaluated-at":"soon""#)?,
    ];
    for payload in malformed {
        assert_eq!(
            CandidateEvaluation::decode(&payload),
            Err(ContractError::Malformed {
                correlation: expected_correlation()?
            })
        );
    }
    let version = tamper(&text, "candidate-evaluation.v1", "candidate-evaluation.v9")?;
    assert_eq!(
        CandidateEvaluation::decode(&version),
        Err(ContractError::UnknownVersion {
            correlation: expected_correlation()?
        })
    );
    Ok(())
}

/// Negative: an evaluation that names the other edge is refused.
#[test]
fn an_evaluation_on_the_other_edge_is_refused() -> Fallible {
    let mut wrong = evaluation(EvaluationVerdict::Passed)?;
    wrong.edge = other_edge(CandidateEvaluation::EDGE);
    assert_eq!(
        wrong.validate(),
        Err(ContractError::WrongEdge {
            correlation: expected_correlation()?
        })
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: only a pass admits a candidate; an inconclusive evaluation is not
/// a quiet pass.
#[test]
fn only_a_pass_admits_a_candidate() {
    assert!(EvaluationVerdict::Passed.admits_candidate());
    assert!(!EvaluationVerdict::Failed.admits_candidate());
    assert!(!EvaluationVerdict::Inconclusive.admits_candidate());
    assert_eq!(EvaluationVerdict::ALL.len(), 3);
    assert_eq!(EvaluationVerdict::Inconclusive.tag(), "inconclusive");
}
