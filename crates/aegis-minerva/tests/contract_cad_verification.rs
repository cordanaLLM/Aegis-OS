// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E06-4: the code-CAD verification request P09 submits to P14.
//!
//! Positive: a well-formed request round-trips under a stable encoding.
//! Negative: another contract version, a malformed payload, the other reading
//! of decision D27 and a screened-out script are each refused with a
//! correlated error. Boundary: a payload past the byte bound is refused
//! unparsed.
//!
//! No solver is invoked, because none is admitted.

mod common;

use aegis_minerva::{
    CadVerificationDirection, CadVerificationRequest, ContractError, Correlation, EdgeId,
    MAX_CONTRACT_PAYLOAD_BYTES, PayloadBuffer, SchemaId, ScreenOutcome,
};

use common::{Fallible, encoded, identity, tamper, verification};

/// The exact payload the fixture request encodes to.
const GOLDEN: &str = r#"{"schema":"aegis.p09-p14.cad-verification.v1","edge":"VERIFY_CODE_CAD","direction":"minerva-submits-hephaestus-verifies","correlation-id":"m06-intent-0001","expert":1,"script-digest":"0505050505050505050505050505050505050505050505050505050505050505","constraint-count":2,"screened":"not-rejected","submitted-at":1000}"#;

/// Returns the correlation a refusal of the fixture request must carry.
fn expected_correlation() -> Result<Correlation, Box<dyn std::error::Error>> {
    Ok(Correlation::new(
        SchemaId::CadVerification,
        Some(identity()?),
    ))
}

// --- Positive -------------------------------------------------------------

/// Positive: a well-formed request round-trips, and the encoding is stable.
#[test]
fn a_verification_request_round_trips() -> Fallible {
    let original = verification(ScreenOutcome::NotRejected)?;
    let text = encoded(&original)?;
    assert_eq!(text, GOLDEN, "the field encoding is stable");
    let decoded = CadVerificationRequest::decode(&text)?;
    assert_eq!(decoded, original, "serialise then deserialise is identity");
    assert_eq!(decoded.validate(), Ok(()));
    assert_eq!(decoded.correlation(), expected_correlation()?);
    assert_eq!(decoded.correlation().schema(), SchemaId::CadVerification);
    assert_eq!(decoded.correlation().id(), Some(identity()?));
    Ok(())
}

/// Positive: every field of a decoded request reads back as it was submitted.
#[test]
fn every_field_reads_back_as_submitted() -> Fallible {
    let decoded =
        CadVerificationRequest::decode(&encoded(&verification(ScreenOutcome::NotRejected)?)?)?;
    assert_eq!(decoded.expert.get(), 1);
    assert_eq!(decoded.constraint_count, 2);
    assert_eq!(decoded.submitted_at.get(), 1_000);
    assert_eq!(decoded.script_digest.as_bytes().len(), 32);
    assert_eq!(decoded.screened, ScreenOutcome::NotRejected);
    Ok(())
}

/// Positive: the schema, edge and direction constants agree with the register.
#[test]
fn the_schema_edge_and_direction_constants_agree() {
    assert_eq!(CadVerificationRequest::SCHEMA, SchemaId::CadVerification);
    assert_eq!(CadVerificationRequest::EDGE, EdgeId::VerifyCodeCad);
    assert_eq!(
        CadVerificationRequest::DIRECTION,
        CadVerificationDirection::GRAPH_OF_RECORD
    );
    assert_eq!(
        SchemaId::CadVerification.edge(),
        CadVerificationRequest::EDGE
    );
    assert_eq!(
        SchemaId::CadVerification.tag(),
        "aegis.p09-p14.cad-verification.v1"
    );
    assert_eq!(
        SchemaId::CadVerification.to_string(),
        SchemaId::CadVerification.tag()
    );
    assert_eq!(EdgeId::VerifyCodeCad.name(), "VERIFY_CODE_CAD");
    assert!(EdgeId::VerifyCodeCad.recorded_transport().contains("Z3"));
    assert_eq!(EdgeId::ALL.len(), 1);
}

// --- Negative -------------------------------------------------------------

/// Negative: another contract version is refused, and the refusal still names
/// the payload it was about.
#[test]
fn another_contract_version_is_refused() -> Fallible {
    let text = encoded(&verification(ScreenOutcome::NotRejected)?)?;
    let tampered = tamper(&text, "cad-verification.v1", "cad-verification.v2")?;
    assert_eq!(
        CadVerificationRequest::decode(&tampered),
        Err(ContractError::UnknownVersion {
            correlation: expected_correlation()?
        })
    );
    Ok(())
}

/// Negative: an unknown field, a bad edge and a bad digest are each refused.
#[test]
fn a_malformed_request_is_refused() -> Fallible {
    let text = encoded(&verification(ScreenOutcome::NotRejected)?)?;
    let cases = [
        tamper(&text, r#""expert":1"#, r#""expert":1,"proof":"0x42""#)?,
        tamper(&text, r#""VERIFY_CODE_CAD""#, r#""VERIFY_CAD""#)?,
        tamper(
            &text,
            r#""constraint-count":2"#,
            r#""constraint-count":"two""#,
        )?,
        tamper(&text, "0505050505", "zzzzzzzzzz")?,
    ];
    for tampered in cases {
        assert_eq!(
            CadVerificationRequest::decode(&tampered),
            Err(ContractError::Malformed {
                correlation: expected_correlation()?
            })
        );
    }
    assert!(CadVerificationRequest::decode("{").is_err());
    Ok(())
}

/// Negative: the other reading of decision D27 is representable and is refused
/// by this schema, which is a submission.
#[test]
fn the_other_reading_of_d27_is_refused_by_a_submission() -> Fallible {
    let mut wrong = verification(ScreenOutcome::NotRejected)?;
    wrong.direction = CadVerificationDirection::HephaestusSubmitsMinervaVerifies;
    assert_eq!(
        wrong.validate(),
        Err(ContractError::WrongDirection {
            correlation: expected_correlation()?
        })
    );
    let text = encoded(&verification(ScreenOutcome::NotRejected)?)?;
    let tampered = tamper(
        &text,
        "minerva-submits-hephaestus-verifies",
        "hephaestus-submits-minerva-verifies",
    )?;
    assert_eq!(
        CadVerificationRequest::decode(&tampered),
        Err(ContractError::WrongDirection {
            correlation: expected_correlation()?
        }),
        "the payload decodes and is then refused, so the reading stays representable"
    );
    Ok(())
}

/// Negative: a script the screen rejected is not submitted.
#[test]
fn a_screened_out_script_is_not_submitted() -> Fallible {
    let rejected = verification(ScreenOutcome::RejectedByScreen)?;
    assert_eq!(
        rejected.validate(),
        Err(ContractError::RejectedScriptSubmitted {
            correlation: expected_correlation()?
        })
    );
    let mut buffer = PayloadBuffer::new();
    assert!(rejected.encode_into(&mut buffer).is_err());
    assert!(buffer.is_empty());
    assert_eq!(buffer.len(), 0);
    assert_eq!(buffer.as_bytes(), &[] as &[u8]);
    assert_eq!(buffer.as_str(), Some(""));
    let error = ContractError::RejectedScriptSubmitted {
        correlation: expected_correlation()?,
    };
    assert_eq!(error.correlation(), expected_correlation()?);
    assert!(error.to_string().contains("must not be submitted"));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a payload one byte past the contract bound is refused before it
/// is parsed, and that refusal is the only anonymous one.
#[test]
fn a_payload_past_the_byte_bound_is_refused_unparsed() -> Fallible {
    let filler = "y".repeat(MAX_CONTRACT_PAYLOAD_BYTES.saturating_add(1));
    let refused = CadVerificationRequest::decode(&filler);
    assert_eq!(
        refused,
        Err(ContractError::PayloadTooLong {
            correlation: Correlation::new(SchemaId::CadVerification, None),
            max: MAX_CONTRACT_PAYLOAD_BYTES,
        })
    );
    if let Err(error) = refused {
        assert_eq!(error.correlation().id(), None);
        assert!(error.to_string().contains("uncorrelated"));
    }
    let inside = encoded(&verification(ScreenOutcome::NotRejected)?)?;
    assert!(inside.len() < MAX_CONTRACT_PAYLOAD_BYTES);
    Ok(())
}
