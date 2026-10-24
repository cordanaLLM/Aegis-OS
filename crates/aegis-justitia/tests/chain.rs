// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Integration tests for the generic hash-linked chain and record traits.

mod common;

use aegis_justitia::{
    ActionType, AgentId, AuditRecord, Chain, ChainRecord, Digest32, HashAlgorithm, IntentId,
    KillswitchState, LedgerError, MakerId, RecordBody, RecordDraft, RecordPrincipals, RecordStatus,
    RecordSubject, RiskLevel, Sequence, Sha256Hasher, Signature, SignerKeyId, Timestamped,
    UnixSeconds,
};

use common::Fallible;

fn chain_draft(at: u64) -> Fallible<RecordDraft> {
    Ok(RecordDraft {
        subject: Some(RecordSubject {
            intent_id: IntentId::parse("intent-chain")?,
            agent: AgentId::parse("agent-chain")?,
            action_type: ActionType::FileDeletion,
        }),
        principals: Some(RecordPrincipals {
            level: RiskLevel::LOWEST,
            maker: MakerId::parse("maker-chain")?,
            checkers: None,
        }),
        status: RecordStatus::Approved,
        block_reason: None,
        killswitch: KillswitchState::Armed,
        at: UnixSeconds::new(at),
    })
}

fn sample_record(
    sequence: Sequence,
    previous: Digest32,
    draft: &RecordDraft,
) -> Fallible<AuditRecord> {
    let body = RecordBody::from_parts(sequence, previous, HashAlgorithm::Sha256, *draft);
    let digest = body.digest::<Sha256Hasher>()?;
    let key = SignerKeyId::parse("signer-01")?;
    let signature = Signature::new(key, digest.as_bytes())?;
    Ok(AuditRecord::from_parts(body, signature, digest))
}

// --- Positive -------------------------------------------------------------

#[test]
fn positive_chain_initial_state_accessors() {
    let chain = Chain::<AuditRecord>::with_bound(10);
    assert_eq!(chain.bound(), 10);
    assert!(chain.reserved() >= 10);
    assert!(chain.is_empty());
    assert_eq!(chain.len(), 0);
    assert_eq!(chain.head(), Digest32::GENESIS);
    assert_eq!(chain.next_sequence(), Sequence::FIRST);
    assert_eq!(chain.records(), []);
}

#[test]
fn positive_chain_seal_and_advance() -> Fallible<()> {
    let mut chain = Chain::<AuditRecord>::with_bound(10);
    assert_eq!(chain.check_append(UnixSeconds::new(100)), Ok(()));
    let (seq, head) = chain.next_link(UnixSeconds::new(100))?;
    assert_eq!(seq, Sequence::FIRST);
    assert_eq!(head, Digest32::GENESIS);

    let rec = sample_record(seq, head, &chain_draft(100)?)?;
    let digest = chain.seal(rec);

    assert_eq!(chain.len(), 1);
    assert!(!chain.is_empty());
    assert_eq!((digest, chain.head()), (rec.digest(), rec.digest()));
    assert_eq!(chain.next_sequence(), Sequence::new(2));
    assert_eq!(chain.records().len(), 1);
    assert_eq!(chain.records().first(), Some(&rec));
    Ok(())
}

#[test]
fn positive_chain_seal_and_verify() -> Fallible<()> {
    let mut chain = Chain::<AuditRecord>::with_bound(5);

    let (s1, h1) = chain.next_link(UnixSeconds::new(100))?;
    let r1 = sample_record(s1, h1, &chain_draft(100)?)?;
    let d1 = chain.seal(r1);
    assert_eq!(d1, r1.digest());
    assert_eq!(chain.head(), d1);

    let (s2, h2) = chain.next_link(UnixSeconds::new(105))?;
    let r2 = sample_record(s2, h2, &chain_draft(105)?)?;
    let d2 = chain.seal(r2);
    assert_eq!(d2, r2.digest());
    assert_eq!(chain.head(), d2);

    let verified_head = chain.verify::<Sha256Hasher>()?;
    assert_eq!(verified_head, chain.head());
    assert_eq!(chain.len(), 2);
    Ok(())
}

#[test]
fn positive_chain_record_provided_methods_and_timestamped() -> Fallible<()> {
    let d = chain_draft(200)?;
    assert_eq!(d.at(), UnixSeconds::new(200));

    let rec = sample_record(Sequence::FIRST, Digest32::GENESIS, &d)?;
    assert_eq!(rec.algorithm(), HashAlgorithm::Sha256);
    assert_eq!(rec.sequence(), Sequence::FIRST);
    assert_eq!(rec.previous(), Digest32::GENESIS);
    assert_eq!(rec.at(), UnixSeconds::new(200));
    assert_eq!(rec.verify::<Sha256Hasher>()?, rec.digest());

    let records = [rec];
    let head = AuditRecord::verify_chain_bounded::<Sha256Hasher>(&records, 5)?;
    assert_eq!(head, rec.digest());
    Ok(())
}

// --- Negative -------------------------------------------------------------

#[test]
fn negative_full_chain_refuses_append() -> Fallible<()> {
    let mut chain = Chain::<AuditRecord>::with_bound(2);

    let (s1, h1) = chain.next_link(UnixSeconds::new(100))?;
    chain.seal(sample_record(s1, h1, &chain_draft(100)?)?);

    let (s2, h2) = chain.next_link(UnixSeconds::new(101))?;
    chain.seal(sample_record(s2, h2, &chain_draft(101)?)?);

    assert_eq!(chain.len(), 2);
    assert_eq!(
        chain.check_append(UnixSeconds::new(102)),
        Err(LedgerError::LengthBoundExceeded { max: 2 })
    );
    assert_eq!(
        chain.next_link(UnixSeconds::new(102)),
        Err(LedgerError::LengthBoundExceeded { max: 2 })
    );
    Ok(())
}

#[test]
fn negative_non_monotonic_timestamp_refused() -> Fallible<()> {
    let mut chain = Chain::<AuditRecord>::with_bound(5);

    let (s1, h1) = chain.next_link(UnixSeconds::new(200))?;
    chain.seal(sample_record(s1, h1, &chain_draft(200)?)?);

    assert_eq!(
        chain.check_append(UnixSeconds::new(199)),
        Err(LedgerError::NonMonotonicTimestamp {
            last: 200,
            observed: 199,
        })
    );
    assert_eq!(
        chain.next_link(UnixSeconds::new(199)),
        Err(LedgerError::NonMonotonicTimestamp {
            last: 200,
            observed: 199,
        })
    );
    Ok(())
}

#[test]
fn negative_tampered_digest_detected() -> Fallible<()> {
    let d1 = chain_draft(100)?;
    let r1 = sample_record(Sequence::FIRST, Digest32::GENESIS, &d1)?;

    let mut bytes = r1.digest().to_bytes();
    if let Some(first) = bytes.first_mut() {
        *first ^= 0x01;
    }
    let forged = AuditRecord::from_parts(*r1.body(), *r1.signature(), Digest32::from_bytes(bytes));

    let tampered = [forged];
    assert_eq!(
        AuditRecord::verify_chain_bounded::<Sha256Hasher>(&tampered, 5),
        Err(LedgerError::DigestMismatch { at: 1 })
    );
    Ok(())
}

#[test]
fn negative_broken_link_predecessor_detected() -> Fallible<()> {
    let d1 = chain_draft(100)?;
    let r1 = sample_record(Sequence::FIRST, Digest32::GENESIS, &d1)?;

    let d2 = chain_draft(101)?;
    let bad_previous = Digest32::from_bytes([0xff; 32]);
    let r2 = sample_record(Sequence::new(2), bad_previous, &d2)?;

    let broken = [r1, r2];
    assert_eq!(
        AuditRecord::verify_chain_bounded::<Sha256Hasher>(&broken, 5),
        Err(LedgerError::PreviousHashMismatch { at: 2 })
    );
    Ok(())
}

#[test]
fn negative_verify_chain_bounded_exceeds_max() -> Fallible<()> {
    let d1 = chain_draft(100)?;
    let r1 = sample_record(Sequence::FIRST, Digest32::GENESIS, &d1)?;

    let (s2, h2) = (Sequence::new(2), r1.digest());
    let d2 = chain_draft(101)?;
    let r2 = sample_record(s2, h2, &d2)?;

    let records = [r1, r2];
    assert_eq!(
        AuditRecord::verify_chain_bounded::<Sha256Hasher>(&records, 1),
        Err(LedgerError::LengthBoundExceeded { max: 1 })
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

#[test]
fn boundary_zero_capacity_chain() -> Fallible<()> {
    let chain = Chain::<AuditRecord>::with_bound(0);
    assert_eq!(chain.bound(), 0);
    assert_eq!(chain.len(), 0);
    assert!(chain.is_empty());
    assert_eq!(
        chain.check_append(UnixSeconds::new(100)),
        Err(LedgerError::LengthBoundExceeded { max: 0 })
    );
    assert_eq!(
        chain.next_link(UnixSeconds::new(100)),
        Err(LedgerError::LengthBoundExceeded { max: 0 })
    );
    assert_eq!(chain.verify::<Sha256Hasher>()?, Digest32::GENESIS);
    Ok(())
}

#[test]
fn boundary_exactly_at_bound_lifecycle() -> Fallible<()> {
    let mut chain = Chain::<AuditRecord>::with_bound(1);
    assert_eq!(chain.check_append(UnixSeconds::new(50)), Ok(()));

    let (seq, head) = chain.next_link(UnixSeconds::new(50))?;
    let rec = sample_record(seq, head, &chain_draft(50)?)?;
    let digest = chain.seal(rec);
    assert_eq!(digest, rec.digest());

    assert_eq!(chain.len(), 1);
    assert_eq!(
        chain.check_append(UnixSeconds::new(51)),
        Err(LedgerError::LengthBoundExceeded { max: 1 })
    );
    assert_eq!(chain.verify::<Sha256Hasher>()?, rec.digest());
    Ok(())
}

#[test]
fn boundary_empty_chain_verifies_to_genesis() -> Fallible<()> {
    let chain = Chain::<AuditRecord>::with_bound(16);
    assert_eq!(chain.verify::<Sha256Hasher>()?, Digest32::GENESIS);
    assert_eq!(
        AuditRecord::verify_chain_bounded::<Sha256Hasher>(&[], 16)?,
        Digest32::GENESIS
    );
    Ok(())
}
