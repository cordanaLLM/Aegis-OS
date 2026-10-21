// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fixtures shared by the integration tests.
//!
//! Everything here reaches `aegis-minerva` through its public API only, and
//! nothing here uses `unwrap` or `expect`: a test that cannot build its own
//! input returns the failure instead of panicking through a lint the workspace
//! denies.

#![allow(dead_code)]

use aegis_justitia::{
    ActionType, AgentId, DIGEST_LEN, Digest32, Identity, MakerId, OversightClass,
    OversightSignature, RiskTier, SandboxAdmissionPath, Signature, SignatureAlgorithm,
    SignatureBytes, SignerKeyId, TargetResource, UnixSeconds,
};
use aegis_minerva::{
    AlpsRouter, CadVerificationRequest, CadVerificationVersion, CapsuleDispatch, ConstraintScript,
    ExpertDomain, ExpertId, Label, PayloadBuffer, PowerEnvelope, ProposalDraft, Reward,
    ScreenOutcome, SlmExpert, StateHash, TrajectoryStep,
};
use aegis_vesta::{
    Capability, CapabilitySet, CapsuleMemoryLimit, CapsuleSlot, CorrelationId, VmmIdentity,
};

/// What every test in this suite returns.
pub type Fallible = Result<(), Box<dyn std::error::Error>>;

/// Anything a fixture can fail with.
pub type Built<T> = Result<T, Box<dyn std::error::Error>>;

/// The correlation identifier the fixtures thread through every payload.
pub const CORRELATION: &str = "m06-intent-0001";

/// The capsule the dispatch fixture asks P10 to run.
pub const CAPSULE: &str = "aegis-extism-capsule-01";

/// The parametric script the fixtures screen and submit.
pub const SCRIPT: &str = "x > 0 && y <= 100";

/// Returns the fixture correlation identifier, as P06 validates it.
///
/// # Errors
///
/// Propagates [`Identity::parse`].
pub fn identity() -> Built<Identity> {
    Ok(Identity::parse(CORRELATION)?)
}

/// Returns the same text, as P10 validates it.
///
/// # Errors
///
/// Propagates [`CorrelationId::parse`].
pub fn capsule_correlation() -> Built<CorrelationId> {
    Ok(CorrelationId::parse(CORRELATION)?)
}

/// Returns an expert with the given identifier, draw and cost inputs.
///
/// # Errors
///
/// Propagates [`Label::parse`].
pub fn expert(
    id: u32,
    domain: ExpertDomain,
    draw_milliwatts: u32,
    latency_us: u32,
) -> Built<SlmExpert> {
    Ok(SlmExpert {
        id: ExpertId::new(id),
        name: Label::parse("aegis-slm-code-v1")?,
        domain,
        parameter_count_millions: 1_500,
        vram_footprint_mib: 1_200,
        avg_latency_us: latency_us,
        energy_per_token_ujoule: 120,
        draw_milliwatts,
        active: true,
    })
}

/// Returns a router holding `count` active code-synthesis experts.
///
/// # Errors
///
/// Propagates [`expert`] and the table's capacity refusal.
pub fn filled_router(count: usize) -> Built<AlpsRouter> {
    let mut router = AlpsRouter::new();
    for index in 0..count {
        let id = u32::try_from(index).unwrap_or(u32::MAX);
        router.register(expert(id, ExpertDomain::CodeSynthesis, 18_500, 450)?)?;
    }
    Ok(router)
}

/// Returns a trajectory step with `reward`, in thousandths.
///
/// # Errors
///
/// Propagates [`Label::parse`].
pub fn step(index: u16, reward_thousandths: i32) -> Built<TrajectoryStep> {
    Ok(TrajectoryStep {
        index,
        state: StateHash::from_bytes([index.to_le_bytes()[0]; 32]),
        action: Label::parse("write-file")?,
        reward: Reward::from_thousandths(reward_thousandths),
        terminal: false,
        relabelled_goal: None,
    })
}

/// Returns the state hash a relabelling is performed against.
#[must_use]
pub fn achieved() -> StateHash {
    StateHash::from_bytes([9u8; 32])
}

/// Returns the fixture constraint script.
///
/// # Errors
///
/// Propagates [`ConstraintScript::parse`].
pub fn script() -> Built<ConstraintScript> {
    Ok(ConstraintScript::parse(SCRIPT)?)
}

/// Returns a seal a proposal can carry.
///
/// The bytes are a fixture, not a signature: nothing in either crate produces
/// or verifies one, and the field is a wire encoding.
///
/// # Errors
///
/// Propagates the signature constructors.
pub fn seal() -> Built<OversightSignature> {
    let key = SignerKeyId::parse("fixture-key")?;
    let signature = Signature::new(key, &[7u8; 32])?;
    Ok(OversightSignature::from_signature(
        &signature,
        SignatureAlgorithm::Tpm2RsaPss,
    ))
}

/// Returns a seal that carries no bytes at all.
///
/// # Errors
///
/// Propagates [`SignerKeyId::parse`].
pub fn empty_seal() -> Built<OversightSignature> {
    Ok(OversightSignature {
        key_id: SignerKeyId::parse("fixture-key")?,
        algorithm: SignatureAlgorithm::Tpm2RsaPss,
        bytes: SignatureBytes::EMPTY,
    })
}

/// Returns a draft of the action P09 proposes, sealed with `seal`.
///
/// # Errors
///
/// Propagates the field constructors.
pub fn draft(seal: Option<&OversightSignature>) -> Built<ProposalDraft> {
    Ok(ProposalDraft {
        correlation_id: identity()?,
        agent_id: AgentId::parse("agent-01")?,
        maker: MakerId::parse("maker-alice")?,
        action_type: ActionType::FileDeletion,
        target: TargetResource::parse("/var/lib/aegis/example")?,
        declared_tier: RiskTier::TierAConsequential,
        oversight: OversightClass::Standard,
        payload_hash: Digest32::from_bytes([3u8; DIGEST_LEN]),
        proposed_at: UnixSeconds::new(1_000),
        seal: seal.copied(),
    })
}

/// Returns a dispatch of the capsule P09 asks P10 to run at `slot`.
///
/// # Errors
///
/// Propagates the field constructors.
pub fn dispatch(slot: usize) -> Built<CapsuleDispatch> {
    Ok(CapsuleDispatch {
        correlation_id: capsule_correlation()?,
        capsule: aegis_vesta::Label::parse(CAPSULE)?,
        slot: CapsuleSlot::new(slot)?,
        memory_limit: CapsuleMemoryLimit::new(16 * 1024 * 1024)?,
        capabilities: CapabilitySet::new()
            .with(Capability::FilesystemRead)
            .with(Capability::ZenohIpc),
        vmm: VmmIdentity::Firecracker,
        admission: SandboxAdmissionPath::TransitiveThroughMinerva,
    })
}

/// Returns a well-formed verification request carrying `screened`.
///
/// # Errors
///
/// Propagates the field constructors.
pub fn verification(screened: ScreenOutcome) -> Built<CadVerificationRequest> {
    Ok(CadVerificationRequest {
        schema: CadVerificationVersion::V1,
        edge: CadVerificationRequest::EDGE,
        direction: CadVerificationRequest::DIRECTION,
        correlation_id: identity()?,
        expert: ExpertId::new(1),
        script_digest: Digest32::from_bytes([5u8; DIGEST_LEN]),
        constraint_count: script()?.constraint_count(),
        screened,
        submitted_at: UnixSeconds::new(1_000),
    })
}

/// Renders a verification request to owned text.
///
/// # Errors
///
/// Propagates the contract refusal.
pub fn encoded(request: &CadVerificationRequest) -> Built<String> {
    let mut buffer = PayloadBuffer::new();
    Ok(request.encode_into(&mut buffer)?.to_owned())
}

/// Returns the budget the scaffold routes with.
///
/// # Errors
///
/// Propagates [`PowerEnvelope::new`].
pub fn budget(milliwatts: u32) -> Built<PowerEnvelope> {
    Ok(PowerEnvelope::new(milliwatts)?)
}

#[path = "../../../fixture_tamper.rs"]
mod fixture_tamper;
#[allow(unused_imports)]
pub use fixture_tamper::tamper;

#[path = "../../../source_effects.rs"]
mod source_effects;

#[allow(unused_imports)]
pub use source_effects::{LINE_BOUND, WALK_BOUND, hits, rust_sources, scan_sources, scan_text};
