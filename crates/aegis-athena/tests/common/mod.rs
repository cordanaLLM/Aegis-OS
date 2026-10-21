// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fixtures shared by the P16 integration tests.
//!
//! Everything here reaches `aegis-athena` through its public API only, and
//! nothing here uses `unwrap` or `expect`: a test that cannot build its own
//! input returns the failure instead of panicking through a lint the workspace
//! denies. The source-sweep helpers below are the ones `aegis-hestia` and
//! `aegis-tellus` carry, kept identical so a sweep behaves the same in every
//! crate that has one.

#![allow(dead_code)]

use aegis_athena::{
    CandidateMetrics, Event, LatencyMs, Lifecycle, MemoryMb, NullModelRetention, PromotionGate,
    SciCarbonRate, Sha256AthenaEngine,
};
use aegis_justitia::{
    ActionType, AgentId, AuditRecordVersion, Digest32, EventId, HashAlgorithm, Identity,
    OversightSignature, RecordStatus, Sequence, Signature, SignatureAlgorithm, SignedAuditRecord,
    SignerKeyId, UnixSeconds,
};
use aegis_tellus::{CandidateId, CorrelationId};

/// What every test in this suite returns.
pub type Fallible = Result<(), Box<dyn std::error::Error>>;

/// The candidate identifier the imported scaffold evaluates.
pub const CANDIDATE: &str = "candidate-001-scx-cake-v2";

/// The correlation identifier the fixtures thread through every payload.
pub const CORRELATION: &str = "m05-fixture-0001";

/// The moment every fixture record is written at.
pub const AT: UnixSeconds = UnixSeconds::new(1_770_000_000);

/// The tolerance every floating-point comparison in this suite uses.
pub const EPSILON: f64 = 1.0e-12;

/// Returns the fixture candidate identifier.
///
/// # Errors
///
/// Propagates [`CandidateId::parse`].
pub fn candidate() -> Result<CandidateId, Box<dyn std::error::Error>> {
    Ok(CandidateId::parse(CANDIDATE)?)
}

/// Returns the fixture correlation identifier.
///
/// # Errors
///
/// Propagates [`CorrelationId::parse`].
pub fn correlation() -> Result<CorrelationId, Box<dyn std::error::Error>> {
    Ok(CorrelationId::parse(CORRELATION)?)
}

/// Returns the metric set the imported scaffold's sample candidate carries.
///
/// Latency 1.2 ms, memory 32.0 MB, carbon rate 0.45 and retention 0.995: all
/// four inside their bounds, which is why the scaffold's own run publishes.
///
/// # Errors
///
/// Propagates the metric constructors.
pub fn passing() -> Result<CandidateMetrics, Box<dyn std::error::Error>> {
    Ok(CandidateMetrics::new(
        LatencyMs::new(1.2)?,
        MemoryMb::new(32.0)?,
        SciCarbonRate::new(0.45)?,
        NullModelRetention::new(0.995)?,
    ))
}

/// Returns the passing metrics with one objective replaced.
///
/// # Errors
///
/// Propagates the metric constructors.
pub fn breaching(
    objective: aegis_athena::Objective,
) -> Result<CandidateMetrics, Box<dyn std::error::Error>> {
    let mut metrics = passing()?;
    match objective {
        aegis_athena::Objective::Latency => metrics.latency = LatencyMs::new(2.0)?,
        aegis_athena::Objective::Memory => metrics.memory = MemoryMb::new(128.0)?,
        aegis_athena::Objective::Carbon => metrics.carbon = SciCarbonRate::new(0.9)?,
        aegis_athena::Objective::Retention => {
            metrics.retention = NullModelRetention::new(0.5)?;
        }
        // `Objective` is `#[non_exhaustive]`, so a variant added later reaches
        // here. Returning the unbreached metrics would silently turn its
        // negative test into a positive one, so this fails the fixture instead.
        _ => return Err("the fixture does not know how to breach that objective".into()),
    }
    Ok(metrics)
}

/// Returns an engine that has published the fixture candidate once.
///
/// # Errors
///
/// Propagates the evaluation.
pub fn published() -> Result<Sha256AthenaEngine, Box<dyn std::error::Error>> {
    let mut engine = Sha256AthenaEngine::new();
    engine.evaluate(candidate()?, passing()?, 120, AT)?;
    Ok(engine)
}

/// Returns the signed M14 audit record the fixtures consume.
///
/// It is the head of a P06 chain: sequence one with an all-zero predecessor
/// link, approved, and sealed. The seal is a field encoding, not a signature:
/// neither crate produces or verifies one.
///
/// # Errors
///
/// Propagates the field constructors.
pub fn audit_record() -> Result<SignedAuditRecord, Box<dyn std::error::Error>> {
    Ok(SignedAuditRecord {
        schema: AuditRecordVersion::V1,
        edge: SignedAuditRecord::EDGE,
        correlation_id: Identity::parse("intent-m05-0001")?,
        event_id: EventId::parse("event-m05-0001")?,
        sequence: Sequence::FIRST,
        algorithm: HashAlgorithm::Sha256,
        previous: Digest32::GENESIS,
        digest: Digest32::parse_hex(&"ab".repeat(32))?,
        action_type: ActionType::FileModification,
        status: RecordStatus::Approved,
        block_reason: None,
        actor_id: AgentId::parse("agent-athena")?,
        recorded_at: AT,
        oversight_signature: Some(seal()?),
    })
}

/// Returns the field-encoded seal the fixture record carries.
///
/// It is a field encoding and not a signature: neither `aegis-justitia` nor
/// this crate produces or verifies one, and TPM2 sealing is milestone M20.
///
/// # Errors
///
/// Propagates the signature constructors.
pub fn seal() -> Result<OversightSignature, Box<dyn std::error::Error>> {
    let signature = Signature::new(SignerKeyId::parse("tpm2-key-0")?, &[7u8; 64])?;
    Ok(OversightSignature::from_signature(
        &signature,
        SignatureAlgorithm::Tpm2RsaPss,
    ))
}

/// Returns a lifecycle stepped through Challenge, Decompose, and Prove.
///
/// # Errors
///
/// Propagates stepping errors.
pub fn lifecycle_proven() -> Result<Lifecycle, Box<dyn std::error::Error>> {
    let mut lifecycle = Lifecycle::new();
    lifecycle.step(Event::Challenge)?;
    lifecycle.step(Event::Decompose)?;
    lifecycle.step(Event::Prove)?;
    Ok(lifecycle)
}

/// The nine promotion gates in declared sequence.
pub const NINE_PROMOTION_GATES: [PromotionGate; 9] = [
    PromotionGate::TypedInterface,
    PromotionGate::ThreeDimensionalTests,
    PromotionGate::BoundedLoops,
    PromotionGate::NoAbortPath,
    PromotionGate::DeadlinedIo,
    PromotionGate::CommittedManifest,
    PromotionGate::SweptSurface,
    PromotionGate::CitedClaims,
    PromotionGate::NoDecisionPathHeap,
];

#[path = "../../../source_effects.rs"]
mod source_effects;
#[allow(unused_imports)]
pub use source_effects::*;
