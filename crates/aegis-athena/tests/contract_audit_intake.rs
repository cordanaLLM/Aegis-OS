// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E05-4, consuming the M14 audit record.
//!
//! The record type is `aegis_justitia::SignedAuditRecord` and it is not
//! redefined here. Every refusal M14 wrote applies unchanged; what this side
//! adds is the one rule that belongs to the consumer, which recorded decisions
//! admit a promotion.

mod common;

use aegis_athena::{AuditIntake, ContractError, EdgeId, SchemaId};
use aegis_justitia::{
    Digest32, PayloadBuffer as JustitiaBuffer, RecordStatus, Sequence, SignedAuditRecord,
};

use common::{AT, Fallible, audit_record};

/// Renders a record through its own M14 encoder.
fn encode(record: &SignedAuditRecord) -> Result<String, Box<dyn std::error::Error>> {
    let mut buffer = JustitiaBuffer::new();
    Ok(record.encode_into(&mut buffer)?.to_owned())
}

// --- Positive -------------------------------------------------------------

/// Positive: an approved record decodes through the M14 schema and is read.
#[test]
fn an_approved_record_is_consumed() -> Fallible {
    let record = audit_record()?;
    let text = encode(&record)?;
    let intake = AuditIntake::decode(&text)?;

    assert_eq!(intake.record(), &record);
    assert_eq!(intake.sequence(), Sequence::FIRST);
    assert_eq!(intake.digest(), record.digest);
    assert_eq!(intake.recorded_at(), AT);
    assert!(intake.admits_promotion());
    intake.accept_for_promotion()?;
    assert_eq!(AuditIntake::SCHEMA, SchemaId::SignedAuditRecord);
    Ok(())
}

/// Positive: an already-decoded record can be wrapped without re-encoding.
#[test]
fn an_already_decoded_record_can_be_wrapped() -> Fallible {
    let intake = AuditIntake::from_record(audit_record()?);
    assert!(intake.admits_promotion());
    assert_eq!(intake.record().status, RecordStatus::Approved);
    Ok(())
}

/// Positive: the consumer names the edge the graph of record names.
#[test]
fn the_consumer_names_the_recorded_edge() {
    assert_eq!(
        SchemaId::SignedAuditRecord.edge(),
        EdgeId::AuditReconstructiveCandidate
    );
    assert_eq!(
        SchemaId::SignedAuditRecord.tag(),
        "aegis.p06-p16.audit-record.v1",
        "the tag is M14's, not a second spelling of it"
    );
    assert_eq!(
        EdgeId::AuditReconstructiveCandidate.typed_in(),
        "aegis_justitia::SignedAuditRecord (M14)"
    );
}

// --- Negative -------------------------------------------------------------

/// Negative: a malformed payload is refused, and the refusal is the producer's.
///
/// The consumer does not reclassify which of M14's rules a payload broke; it
/// reports that M14 refused it and carries the correlation it could still read.
#[test]
fn a_malformed_record_is_refused() -> Fallible {
    let text = encode(&audit_record()?)?;

    for bad in [
        "{}",
        "not json",
        "[]",
        &text.replace(
            "aegis.p06-p16.audit-record.v1",
            "aegis.p06-p16.audit-record.v2",
        ),
        &text.replace("\"actor-id\"", "\"actorId\""),
        &text.replace("\"sequence\":1", "\"sequence\":2"),
        &text.replace('}', ",\"extra\":1}"),
    ] {
        let refusal = AuditIntake::decode(bad);
        assert!(
            matches!(refusal, Err(ContractError::UpstreamAuditRecord { .. })),
            "the consumer accepted {bad}"
        );
    }
    Ok(())
}

/// Negative: an unsigned record is refused by M14, so it never reaches the
/// promotion rule.
#[test]
fn an_unsigned_record_never_reaches_the_promotion_rule() -> Fallible {
    let mut unsigned = audit_record()?;
    unsigned.oversight_signature = None;
    let text = serde_json::to_string(&unsigned)?;
    assert!(matches!(
        AuditIntake::decode(&text),
        Err(ContractError::UpstreamAuditRecord { .. })
    ));
    Ok(())
}

/// Negative: a record recording a refusal does not admit a promotion.
///
/// P06 records approvals and refusals alike and both are valid records; P16
/// may promote on the first and must not on the second.
#[test]
fn a_refusal_record_does_not_admit_a_promotion() -> Fallible {
    for status in [
        RecordStatus::PendingApproval,
        RecordStatus::Rejected,
        RecordStatus::Blocked,
        RecordStatus::Halted,
    ] {
        let mut record = audit_record()?;
        record.status = status;
        if matches!(status, RecordStatus::Blocked | RecordStatus::Halted) {
            record.block_reason = Some(aegis_justitia::BlockReason::AuditUnavailable);
        }
        let intake = AuditIntake::from_record(record);
        assert!(
            !intake.admits_promotion(),
            "{} must not admit a promotion",
            status.name()
        );
        assert!(matches!(
            intake.accept_for_promotion(),
            Err(ContractError::PromotionNotAdmitted { .. })
        ));
    }
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the genesis record is the head of the chain and is consumed.
///
/// Sequence one with an all-zero predecessor link. M14's own validation
/// refuses every other pairing, so this reads the rule rather than restating
/// it.
#[test]
fn the_genesis_record_is_consumed_as_the_chain_head() -> Fallible {
    let record = audit_record()?;
    assert_eq!(record.sequence, Sequence::FIRST);
    assert!(record.previous.is_genesis());

    let intake = AuditIntake::decode(&encode(&record)?)?;
    assert!(intake.is_chain_head());
    assert!(intake.record().previous.is_genesis());
    Ok(())
}

/// Boundary: a record in the middle of the chain is consumed and is not the
/// head.
#[test]
fn a_mid_chain_record_is_consumed_and_is_not_the_head() -> Fallible {
    let mut record = audit_record()?;
    record.sequence = Sequence::new(9);
    record.previous = Digest32::parse_hex(&"1f".repeat(32))?;

    let intake = AuditIntake::decode(&encode(&record)?)?;
    assert!(!intake.is_chain_head());
    assert_eq!(intake.sequence().get(), 9);
    assert!(intake.admits_promotion());
    Ok(())
}

/// Boundary: a genesis link out of place is refused upstream, not here.
///
/// Sequence nine with an all-zero predecessor is M14's
/// `GenesisLinkOutOfPlace`; this side sees it as an upstream refusal and does
/// not attempt to name which rule it broke.
#[test]
fn a_genesis_link_out_of_place_is_an_upstream_refusal() -> Fallible {
    let mut record = audit_record()?;
    record.sequence = Sequence::new(9);
    let text = serde_json::to_string(&record)?;
    assert!(matches!(
        AuditIntake::decode(&text),
        Err(ContractError::UpstreamAuditRecord { .. })
    ));
    Ok(())
}
