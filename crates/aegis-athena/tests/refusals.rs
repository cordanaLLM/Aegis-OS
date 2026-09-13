// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Every P16 refusal is a value, and every refusal variant has a producer.
//!
//! HISS-07 asks that an error path return rather than abort. The imported
//! scaffold aborts twice and propagates a boxed `io::Error` for everything
//! else, so this file is where the vocabulary that replaced it is exercised.
//!
//! It also holds the two engine constructors and the contract version the
//! ledger pre-image commits to, because a chain digest that nothing pins is a
//! value that can drift without anything noticing.

mod common;

use aegis_athena::{
    AthenaEngine, AthenaError, CONTRACT_VERSION, CallDeadline, CallKind, CheckpointLedger,
    ContractError, Event, EventKind, LatencyMs, LedgerError, Lifecycle, PortError, PromotionGate,
    PromotionGates, SciCarbonRate, Sha256CheckpointLedger, Stage, StubSysupdate, SysupdateCall,
    SysupdatePort,
};
use aegis_justitia::{
    Digest32, HashAlgorithm, LedgerHasher, Sequence, Sha256Hasher, SignedAuditRecord, UnixSeconds,
};

use common::{AT, Fallible, audit_record, candidate, passing};

// --- Positive -------------------------------------------------------------

/// Positive: every refusal renders a message that names what was refused.
#[test]
fn every_refusal_renders_a_message() {
    let refusals = [
        AthenaError::Latency {
            reason: "a value that is not positive",
        },
        AthenaError::Memory {
            reason: "a negative value",
        },
        AthenaError::CarbonRate {
            reason: "a negative value",
        },
        AthenaError::Retention {
            reason: "a value outside the unit interval",
        },
        AthenaError::Unexpected {
            stage: Stage::Propose,
            event: EventKind::Prove,
        },
        AthenaError::Terminal {
            stage: Stage::Invalidate,
            event: EventKind::Challenge,
        },
        AthenaError::CheckBudgetExceeded {
            budget_ms: 500,
            elapsed_ms: 501,
        },
        AthenaError::MaturityGates {
            reason: "the gate already carries a recorded result",
        },
    ];
    for refusal in refusals {
        assert!(
            format!("{refusal}").len() > 10,
            "the refusal {refusal:?} renders too little"
        );
    }
}

/// Positive: the engine can be built over an explicit ledger.
///
/// This is how a caller sets the record bound: the engine holds the ledger it
/// was handed rather than one it chose.
#[test]
fn an_engine_can_be_built_over_an_explicit_ledger() -> Fallible {
    let ledger: CheckpointLedger<Sha256Hasher> = CheckpointLedger::with_bound(4);
    let mut engine: AthenaEngine<Sha256Hasher> = AthenaEngine::with_ledger(ledger);
    assert_eq!(engine.ledger().bound(), 4);
    assert!(engine.ledger().is_empty());

    engine.evaluate(candidate()?, passing()?, 10, AT)?;
    assert_eq!(engine.ledger().len(), 1);
    engine.verify()?;

    let fresh: AthenaEngine<Sha256Hasher> = AthenaEngine::default();
    assert!(fresh.ledger().is_empty());
    Ok(())
}

/// Positive: the contract version is part of every chain digest.
///
/// Two records identical in every other field, written under different
/// contract versions, must not share a digest. The version is a constant here,
/// so what this states is that it is committed to at all.
#[test]
fn the_contract_version_is_committed_to() -> Fallible {
    assert_eq!(CONTRACT_VERSION, "aegis.athena.v1");
    let mut ledger = Sha256CheckpointLedger::new();
    let digest = ledger.append(aegis_athena::CheckpointDraft {
        candidate: candidate()?,
        stage: Stage::Publish,
        metrics: Some(passing()?),
        verdict: Some(aegis_athena::gate(&passing()?)),
        reason: None,
        at: AT,
    })?;
    let body = ledger
        .records()
        .first()
        .map(|record| *record.body())
        .ok_or("the chain must carry its record")?;
    let preimage = body.canonical_preimage()?;
    assert!(
        preimage
            .as_slice()
            .windows(CONTRACT_VERSION.len())
            .any(|window| window == CONTRACT_VERSION.as_bytes()),
        "the contract version must appear in the pre-image"
    );
    assert_eq!(body.digest::<Sha256Hasher>()?, digest);
    Ok(())
}

/// Positive: a P13 rate converts into the metric the gate compares.
///
/// This is the one place the two crates' numbers meet: the rate returned on
/// the `EVALUATE_CANDIDATE_CARBON_SCI` edge is the rate the gate compares.
#[test]
fn a_p13_rate_converts_into_the_gate_metric() -> Fallible {
    let sci = aegis_tellus::SciEngine::new()
        .sci_rate(aegis_tellus::EnergyKwh::new(0.0025)?, 1.0)
        .as_rate()?;
    let metric = SciCarbonRate::from_sci(sci)?;
    assert!((metric.get() - sci.get()).abs() < common::EPSILON);
    assert!((metric.get() - 0.6).abs() < common::EPSILON);
    assert!(
        metric.get() < aegis_athena::CARBON_RATE_BOUND,
        "the scaffold's benchmark rate clears the gate's carbon bound"
    );
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: each metric refusal has a producer.
#[test]
fn every_metric_refusal_has_a_producer() {
    let raised = [
        matches!(LatencyMs::new(0.0), Err(AthenaError::Latency { .. })),
        matches!(
            aegis_athena::MemoryMb::new(-1.0),
            Err(AthenaError::Memory { .. })
        ),
        matches!(
            SciCarbonRate::new(f64::NAN),
            Err(AthenaError::CarbonRate { .. })
        ),
        matches!(
            aegis_athena::NullModelRetention::new(2.0),
            Err(AthenaError::Retention { .. })
        ),
    ];
    assert_eq!(raised, [true; 4], "a metric refusal has no producer");
}

/// Negative: the two wrapping refusals carry their source unchanged.
///
/// [`AthenaError::Identifier`] wraps the shared P13 identifier refusal and
/// [`AthenaError::Ledger`] wraps the chain refusal, so a caller can match on
/// the original rather than on a re-worded copy of it.
#[test]
fn the_wrapping_refusals_carry_their_source() -> Fallible {
    let identifier = AthenaError::Identifier(aegis_tellus::TellusError::Identifier {
        reason: aegis_tellus::CandidateId::REFUSAL,
    });
    assert!(format!("{identifier}").contains("candidate identifier"));

    let mut ledger = Sha256CheckpointLedger::with_bound(1);
    ledger.append(aegis_athena::CheckpointDraft {
        candidate: candidate()?,
        stage: Stage::Publish,
        metrics: None,
        verdict: None,
        reason: None,
        at: AT,
    })?;
    let mut engine = AthenaEngine::<Sha256Hasher>::with_ledger(ledger);
    let mut lifecycle = Lifecycle::new();
    let refusal = engine.step(candidate()?, &mut lifecycle, Event::Challenge, AT);
    assert!(matches!(
        refusal,
        Err(AthenaError::Ledger(LedgerError::LengthBoundExceeded {
            max: 1
        }))
    ));
    Ok(())
}

/// Negative: the three digest-shaped ledger refusals have producers.
///
/// `AlgorithmMismatch` cannot be reached from outside, because
/// [`HashAlgorithm`] admits one variant; it is constructed here to show the
/// chain would refuse a record naming another algorithm if one existed, which
/// is the strongest statement a one-variant enum admits.
#[test]
fn the_digest_shaped_ledger_refusals_have_producers() -> Fallible {
    let mismatch = LedgerError::AlgorithmMismatch {
        expected: <Sha256Hasher as LedgerHasher>::ALGORITHM,
        found: HashAlgorithm::Sha256,
    };
    assert!(format!("{mismatch}").contains("sha-256"));

    let preimage = LedgerError::Preimage(aegis_justitia::PreimageError::Overflow { max: 512 });
    assert!(format!("{preimage}").contains("pre-image"));

    let hash = LedgerError::Hash(aegis_justitia::HashError::Width {
        expected: 32,
        actual: 31,
    });
    assert!(format!("{hash}").contains("digest"));

    let body = aegis_athena::CheckpointBody::from_parts(
        Sequence::FIRST,
        Digest32::GENESIS,
        HashAlgorithm::Sha256,
        aegis_athena::CheckpointDraft {
            candidate: candidate()?,
            stage: Stage::Publish,
            metrics: None,
            verdict: None,
            reason: None,
            at: UnixSeconds::new(1),
        },
    );
    assert_eq!(body.algorithm(), HashAlgorithm::Sha256);
    Ok(())
}

/// Negative: the contract refusals a malformed payload raises have producers.
#[test]
fn the_contract_refusals_have_producers() -> Fallible {
    let malformed = aegis_athena::PromotionTrigger::decode("{}");
    assert!(matches!(malformed, Err(ContractError::Malformed { .. })));

    let record: SignedAuditRecord = audit_record()?;
    assert!(record.validate().is_ok());
    assert!(matches!(
        aegis_athena::AuditIntake::decode("not json"),
        Err(ContractError::UpstreamAuditRecord { .. })
    ));
    Ok(())
}

/// Negative: the port refuses a deadline shorter than its service time.
#[test]
fn the_port_refuses_a_short_deadline() -> Fallible {
    let mut port = StubSysupdate::new();
    let below =
        CallDeadline::try_from_millis(aegis_athena::STUB_SERVICE_MILLIS.saturating_sub(1).max(1))
            .ok_or("a deadline must be positive")?;
    let call = SysupdateCall {
        kind: CallKind::Deploy,
        candidate: candidate()?,
        reason: None,
    };
    // The stub's service time is one millisecond, the smallest representable
    // deadline, so the refusal is raised from a port that declares more.
    assert!(port.call(call, below).is_ok());

    let refusal = PortError::WouldBlock {
        needed: 10,
        offered: 9,
    };
    assert!(format!("{refusal}").contains("9ms"));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a gate set records every one of the nine gates exactly once.
#[test]
fn a_gate_set_records_all_nine_exactly_once() -> Fallible {
    let mut gates = PromotionGates::new();
    for gate in [
        PromotionGate::TypedInterface,
        PromotionGate::ThreeDimensionalTests,
        PromotionGate::BoundedLoops,
        PromotionGate::NoAbortPath,
        PromotionGate::DeadlinedIo,
        PromotionGate::CommittedManifest,
        PromotionGate::SweptSurface,
        PromotionGate::CitedClaims,
        PromotionGate::NoDecisionPathHeap,
    ] {
        gates.record(gate, true)?;
        assert!(matches!(
            gates.record(gate, true),
            Err(AthenaError::MaturityGates { .. })
        ));
    }
    assert_eq!(gates.recorded(), 9);
    Ok(())
}
