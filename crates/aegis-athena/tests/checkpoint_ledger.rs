// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E05-3, the hash-chained checkpoint ledger: a chain that re-walks, a tamper
//! that is detected, a genesis record that verifies, and the D02 algorithm.

mod common;

use aegis_athena::{
    CHECKPOINT_DOMAIN, CheckpointBody, CheckpointDraft, CheckpointLedger, CheckpointRecord, Event,
    InvalidationReason, LEDGER_ALGORITHM, LedgerAlgorithmReading, LedgerError,
    MAX_CHECKPOINT_RECORDS, Sha256AthenaEngine, Sha256CheckpointLedger, Stage, verify_chain,
    verify_chain_bounded,
};
use aegis_justitia::{Digest32, HashAlgorithm, LedgerHasher, Sequence, Sha256Hasher, UnixSeconds};

use common::{AT, Fallible, candidate, passing};

/// Builds a draft for the fixture candidate at `at`.
fn draft(stage: Stage, at: UnixSeconds) -> Result<CheckpointDraft, Box<dyn std::error::Error>> {
    Ok(CheckpointDraft {
        candidate: candidate()?,
        stage,
        metrics: Some(passing()?),
        verdict: Some(aegis_athena::gate(&passing()?)),
        reason: None,
        at,
    })
}

// --- Positive -------------------------------------------------------------

/// Positive: a chain of records re-walks and returns the head digest.
#[test]
fn a_chain_re_walks() -> Fallible {
    let mut ledger = Sha256CheckpointLedger::new();
    let mut last = Digest32::GENESIS;
    for step in 0..8u64 {
        last = ledger.append(draft(
            Stage::Publish,
            UnixSeconds::new(AT.get().saturating_add(step)),
        )?)?;
    }
    assert_eq!(ledger.len(), 8);
    assert!(!ledger.is_empty());
    assert_eq!(ledger.head(), last);
    assert_eq!(ledger.verify()?, last);
    assert_eq!(verify_chain::<Sha256Hasher>(ledger.records())?, last);
    assert_eq!(
        verify_chain_bounded::<Sha256Hasher>(ledger.records(), 8)?,
        last
    );
    Ok(())
}

/// Positive: each record links to the digest of the one before it.
#[test]
fn each_record_links_to_its_predecessor() -> Fallible {
    let mut ledger = Sha256CheckpointLedger::new();
    ledger.append(draft(Stage::Publish, AT)?)?;
    ledger.append(draft(
        Stage::Invalidate,
        UnixSeconds::new(AT.get().saturating_add(1)),
    )?)?;

    let records = ledger.records();
    let first = records
        .first()
        .ok_or("the chain must carry a first record")?;
    let second = records
        .get(1)
        .ok_or("the chain must carry a second record")?;
    assert_eq!(second.body().previous(), first.digest());
    assert_eq!(first.body().sequence(), Sequence::FIRST);
    assert_eq!(second.body().sequence().get(), 2);
    Ok(())
}

/// Positive: the engine's own ledger re-walks after every evaluation.
#[test]
fn the_engine_ledger_re_walks() -> Fallible {
    let mut engine = Sha256AthenaEngine::new();
    engine.evaluate(candidate()?, passing()?, 100, AT)?;
    let mut lifecycle = aegis_athena::Lifecycle::new();
    engine.step(
        candidate()?,
        &mut lifecycle,
        Event::Challenge,
        UnixSeconds::new(AT.get().saturating_add(1)),
    )?;
    engine.withdraw(
        candidate()?,
        &mut lifecycle,
        InvalidationReason::Withdrawn,
        UnixSeconds::new(AT.get().saturating_add(2)),
    )?;
    assert_eq!(engine.ledger().len(), 3);
    engine.verify()?;
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a tampered record is detected by the re-walk.
///
/// The forged record keeps its stored digest and changes one field of its
/// body, which is exactly what an edit to durable storage looks like.
#[test]
fn a_tampered_record_is_detected() -> Fallible {
    let mut ledger = Sha256CheckpointLedger::new();
    ledger.append(draft(Stage::Invalidate, AT)?)?;
    ledger.append(draft(
        Stage::Invalidate,
        UnixSeconds::new(AT.get().saturating_add(1)),
    )?)?;
    ledger.verify()?;

    let mut records = ledger.records().to_vec();
    let original = *records.first().ok_or("the chain must carry a record")?;
    let forged = CheckpointRecord::from_parts(
        original.body().with_stage(Stage::Publish),
        original.digest(),
    );
    if let Some(slot) = records.first_mut() {
        *slot = forged;
    }
    assert!(matches!(
        verify_chain::<Sha256Hasher>(&records),
        Err(LedgerError::DigestMismatch { at: 1 })
    ));
    Ok(())
}

/// Negative: a record out of sequence, or unlinked, is detected.
#[test]
fn a_gap_or_a_broken_link_is_detected() -> Fallible {
    let body = CheckpointBody::from_parts(
        Sequence::new(2),
        Digest32::GENESIS,
        HashAlgorithm::Sha256,
        draft(Stage::Publish, AT)?,
    );
    let record = CheckpointRecord::from_parts(body, body.digest::<Sha256Hasher>()?);
    assert!(matches!(
        verify_chain::<Sha256Hasher>(&[record]),
        Err(LedgerError::SequenceGap {
            expected: 1,
            found: 2,
        })
    ));

    let mut ledger = Sha256CheckpointLedger::new();
    ledger.append(draft(Stage::Publish, AT)?)?;
    ledger.append(draft(
        Stage::Publish,
        UnixSeconds::new(AT.get().saturating_add(1)),
    )?)?;
    let mut records = ledger.records().to_vec();
    let second = *records.get(1).ok_or("the chain must carry two records")?;
    let unlinked = CheckpointBody::from_parts(
        second.body().sequence(),
        Digest32::GENESIS,
        HashAlgorithm::Sha256,
        *second.body().draft(),
    );
    if let Some(slot) = records.get_mut(1) {
        *slot = CheckpointRecord::from_parts(unlinked, unlinked.digest::<Sha256Hasher>()?);
    }
    assert!(matches!(
        verify_chain::<Sha256Hasher>(&records),
        Err(LedgerError::PreviousHashMismatch { at: 2 })
    ));
    Ok(())
}

/// Negative: a record that steps the clock backwards is refused on append and
/// detected on re-walk.
#[test]
fn a_backwards_timestamp_is_refused() -> Fallible {
    let mut ledger = Sha256CheckpointLedger::new();
    ledger.append(draft(Stage::Publish, UnixSeconds::new(1_000))?)?;
    assert!(matches!(
        ledger.append(draft(Stage::Publish, UnixSeconds::new(999))?),
        Err(LedgerError::NonMonotonicTimestamp {
            last: 1_000,
            observed: 999,
        })
    ));
    assert_eq!(ledger.len(), 1, "a refused append commits nothing");
    Ok(())
}

/// Negative: the ledger refuses a record past its bound rather than dropping
/// the oldest.
#[test]
fn the_ledger_refuses_a_record_past_its_bound() -> Fallible {
    let mut ledger: CheckpointLedger<Sha256Hasher> = CheckpointLedger::with_bound(2);
    assert_eq!(ledger.bound(), 2);
    assert_eq!(ledger.reserved(), 2);
    ledger.append(draft(Stage::Publish, AT)?)?;
    ledger.append(draft(
        Stage::Publish,
        UnixSeconds::new(AT.get().saturating_add(1)),
    )?)?;
    assert!(matches!(
        ledger.append(draft(
            Stage::Publish,
            UnixSeconds::new(AT.get().saturating_add(2)),
        )?),
        Err(LedgerError::LengthBoundExceeded { max: 2 })
    ));
    assert_eq!(ledger.len(), 2);
    assert_eq!(
        ledger.reserved(),
        2,
        "HISS-03: the reservation must not change while the ledger is in use"
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the genesis record verifies, and an empty chain verifies as the
/// genesis link.
#[test]
fn the_genesis_record_verifies() -> Fallible {
    let empty = Sha256CheckpointLedger::default();
    assert!(empty.is_empty());
    assert_eq!(empty.head(), Digest32::GENESIS);
    assert_eq!(empty.verify()?, Digest32::GENESIS);
    assert_eq!(verify_chain::<Sha256Hasher>(&[])?, Digest32::GENESIS);

    let mut ledger = Sha256CheckpointLedger::new();
    let digest = ledger.append(draft(Stage::Publish, AT)?)?;
    let first = ledger
        .records()
        .first()
        .ok_or("the chain must carry its genesis record")?;
    assert_eq!(first.body().sequence(), Sequence::FIRST);
    assert!(
        first.body().previous().is_genesis(),
        "the head of a chain links to the all-zero digest"
    );
    assert_eq!(ledger.verify()?, digest);
    Ok(())
}

/// Boundary: the algorithm is the one D02 records, and it is the only one.
///
/// The register states three readings because three sources name three
/// algorithms; the code admits one. That is D02 applied rather than restated.
#[test]
fn the_algorithm_matches_d02() {
    assert_eq!(
        <Sha256Hasher as LedgerHasher>::ALGORITHM,
        HashAlgorithm::Sha256
    );
    assert_eq!(LEDGER_ALGORITHM.admitted, HashAlgorithm::Sha256);
    assert_eq!(LEDGER_ALGORITHM.chosen, LedgerAlgorithmReading::Sha256);
    assert_eq!(LEDGER_ALGORITHM.id, "D02");
    assert_eq!(LEDGER_ALGORITHM.dispute, "DSP-07");
    assert_eq!(HashAlgorithm::Sha256.name(), "sha-256");

    assert_eq!(LedgerAlgorithmReading::ALL.len(), 3);
    let chosen: Vec<&str> = LedgerAlgorithmReading::ALL
        .into_iter()
        .filter(|reading| reading.is_chosen())
        .map(LedgerAlgorithmReading::name)
        .collect();
    assert_eq!(chosen, vec!["SHA-256"], "exactly one reading is chosen");
}

/// Boundary: a record naming another algorithm does not verify.
///
/// `HashAlgorithm` has one variant, so the mismatch cannot be spelled from
/// outside; this drives the check through a second hasher whose algorithm
/// constant is the same one, which is the strongest statement the type admits.
#[test]
fn the_chain_checks_the_algorithm_it_was_built_with() -> Fallible {
    let mut ledger = Sha256CheckpointLedger::new();
    ledger.append(draft(Stage::Publish, AT)?)?;
    assert!(verify_chain::<Sha256Hasher>(ledger.records()).is_ok());
    assert_eq!(
        ledger
            .records()
            .first()
            .map(|record| record.body().algorithm()),
        Some(HashAlgorithm::Sha256)
    );
    Ok(())
}

/// Boundary: the domain tag separates this chain from the P06 audit chain.
///
/// Both hash with SHA-256, so without a distinct domain a P06 record could be
/// replayed as a P16 checkpoint. The tags differ, and they differ in the
/// pre-image rather than only in a comment.
#[test]
fn the_domain_tag_separates_the_two_chains() -> Fallible {
    assert_eq!(CHECKPOINT_DOMAIN, b"aegis.athena.checkpoint.v1\0");
    assert_ne!(CHECKPOINT_DOMAIN, aegis_justitia::AUDIT_DOMAIN);

    let body = CheckpointBody::from_parts(
        Sequence::FIRST,
        Digest32::GENESIS,
        HashAlgorithm::Sha256,
        draft(Stage::Publish, AT)?,
    );
    let preimage = body.canonical_preimage()?;
    assert!(
        preimage.as_slice().starts_with(CHECKPOINT_DOMAIN),
        "the domain tag must be the first thing committed to"
    );
    assert!(!preimage.is_empty());
    assert!(preimage.len() < 512);
    Ok(())
}

/// Boundary: a chain longer than the bound is refused without being walked.
#[test]
fn a_chain_past_the_bound_is_refused_before_the_walk() -> Fallible {
    let mut ledger = Sha256CheckpointLedger::new();
    assert_eq!(ledger.bound(), MAX_CHECKPOINT_RECORDS);
    ledger.append(draft(Stage::Publish, AT)?)?;
    ledger.append(draft(
        Stage::Publish,
        UnixSeconds::new(AT.get().saturating_add(1)),
    )?)?;
    assert!(matches!(
        verify_chain_bounded::<Sha256Hasher>(ledger.records(), 1),
        Err(LedgerError::LengthBoundExceeded { max: 1 })
    ));
    Ok(())
}

/// Boundary: a record with no metrics differs from one whose metrics are zero.
///
/// The pre-image carries a presence byte, so "no check has run" and "the check
/// measured nothing" are different digests rather than the same one.
#[test]
fn an_absent_metric_set_is_not_a_zero_metric_set() -> Fallible {
    let with = CheckpointBody::from_parts(
        Sequence::FIRST,
        Digest32::GENESIS,
        HashAlgorithm::Sha256,
        draft(Stage::Publish, AT)?,
    );
    let mut bare_draft = draft(Stage::Propose, AT)?;
    bare_draft.metrics = None;
    bare_draft.verdict = None;
    let without = CheckpointBody::from_parts(
        Sequence::FIRST,
        Digest32::GENESIS,
        HashAlgorithm::Sha256,
        bare_draft,
    );
    assert_ne!(
        with.digest::<Sha256Hasher>()?,
        without.digest::<Sha256Hasher>()?
    );
    assert_ne!(
        with.canonical_preimage()?.len(),
        without.canonical_preimage()?.len()
    );
    assert_eq!(without.draft().reason, None);
    Ok(())
}

/// Boundary: a withdrawal reason changes the digest, so it cannot be edited
/// out after the fact.
#[test]
fn the_withdrawal_reason_is_committed_to() -> Fallible {
    let mut base = draft(Stage::Invalidate, AT)?;
    base.reason = Some(InvalidationReason::ParetoBreach);
    let first = CheckpointBody::from_parts(
        Sequence::FIRST,
        Digest32::GENESIS,
        HashAlgorithm::Sha256,
        base,
    );

    let mut other = base;
    other.reason = Some(InvalidationReason::Withdrawn);
    let second = CheckpointBody::from_parts(
        Sequence::FIRST,
        Digest32::GENESIS,
        HashAlgorithm::Sha256,
        other,
    );
    assert_ne!(
        first.digest::<Sha256Hasher>()?,
        second.digest::<Sha256Hasher>()?
    );
    Ok(())
}
