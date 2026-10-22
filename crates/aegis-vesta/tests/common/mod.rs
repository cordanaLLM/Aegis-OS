// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fixtures shared by the integration tests.
//!
//! Everything here reaches `aegis-vesta` through its public API only, and
//! nothing here uses `unwrap` or `expect`: a test that cannot build its own
//! input returns the failure instead of panicking through a lint the workspace
//! denies.

#![allow(dead_code)]

#[path = "../../../fixture_tamper.rs"]
mod fixture_tamper;
#[allow(unused_imports)]
pub use fixture_tamper::tamper;

use aegis_justitia::{SandboxAdmissionPath, UnixSeconds};
use aegis_vesta::{
    CandidateEvaluation, CandidateEvaluationVersion, Capability, CapabilitySet, CapsuleMemoryLimit,
    CapsuleRegistry, CapsuleRequest, CapsuleRequestVersion, CapsuleSlot, CorrelationId, EdgeId,
    EvaluationVerdict, GuestMemoryMib, Label, MicroVmController, MicroVmRequest, PayloadBuffer,
    VmId, VmmIdentity,
};

/// What every test in this suite returns.
pub type Fallible = Result<(), Box<dyn std::error::Error>>;

/// The correlation identifier the fixtures thread through every payload.
pub const CORRELATION: &str = "m06-fixture-0001";

/// The sandbox name the scaffold spawns.
pub const SANDBOX: &str = "aegis-agent-microvm-01";

/// The capsule name the scaffold registers.
pub const CAPSULE: &str = "aegis-extism-capsule-01";

/// The candidate a sandbox evaluates for P16.
pub const CANDIDATE: &str = "aegis-candidate-0007";

/// The guest memory size the scaffold asks for, in mebibytes.
pub const GUEST_MIB: u32 = 4;

/// The capsule memory limit the scaffold asks for, in bytes.
pub const CAPSULE_BYTES: u64 = 16 * 1024 * 1024;

#[path = "../../../fixture_codec.rs"]
mod fixture_codec;

use fixture_codec::encoded;
#[allow(unused_imports)]
pub use fixture_codec::{correlation, label};

/// Returns the scaffold's sandbox request under `vmm`.
///
/// # Errors
///
/// Propagates the field constructors and the accelerator refusal.
pub fn sandbox_request(vmm: VmmIdentity) -> Result<MicroVmRequest, Box<dyn std::error::Error>> {
    Ok(MicroVmRequest::new(
        label(SANDBOX)?,
        GuestMemoryMib::new(GUEST_MIB)?,
        false,
        vmm,
    )?)
}

/// Returns a controller holding `count` sandboxes.
///
/// # Errors
///
/// Propagates the request constructors and the table's capacity refusal.
pub fn filled_controller(count: usize) -> Result<MicroVmController, Box<dyn std::error::Error>> {
    let mut controller = MicroVmController::new();
    for _ in 0..count {
        controller.spawn(sandbox_request(VmmIdentity::Firecracker)?)?;
    }
    Ok(controller)
}

/// Returns the capability set the scaffold grants its capsule.
#[must_use]
pub fn scaffold_capabilities() -> CapabilitySet {
    CapabilitySet::new()
        .with(Capability::FilesystemRead)
        .with(Capability::ZenohIpc)
}

/// Returns a registry holding `count` capsules.
///
/// # Errors
///
/// Propagates the field constructors and the table's capacity refusal.
pub fn filled_registry(count: usize) -> Result<CapsuleRegistry, Box<dyn std::error::Error>> {
    let mut registry = CapsuleRegistry::new();
    for _ in 0..count {
        registry.register(
            label(CAPSULE)?,
            CapsuleMemoryLimit::new(CAPSULE_BYTES)?,
            scaffold_capabilities(),
        )?;
    }
    Ok(registry)
}

/// Returns a well-formed capsule request at `slot` under `vmm`.
///
/// # Errors
///
/// Propagates the field constructors.
pub fn capsule_request(
    slot: usize,
    vmm: VmmIdentity,
) -> Result<CapsuleRequest, Box<dyn std::error::Error>> {
    Ok(CapsuleRequest {
        schema: CapsuleRequestVersion::V1,
        edge: CapsuleRequest::EDGE,
        admission: SandboxAdmissionPath::TransitiveThroughMinerva,
        runtime: CapsuleRequest::RUNTIME,
        vmm,
        correlation_id: correlation()?,
        capsule: label(CAPSULE)?,
        slot: CapsuleSlot::new(slot)?,
        memory_limit_bytes: CapsuleMemoryLimit::new(CAPSULE_BYTES)?,
        capabilities: scaffold_capabilities(),
    })
}

/// Returns a well-formed candidate evaluation with `verdict`.
///
/// # Errors
///
/// Propagates the field constructors.
pub fn evaluation(
    verdict: EvaluationVerdict,
) -> Result<CandidateEvaluation, Box<dyn std::error::Error>> {
    Ok(CandidateEvaluation {
        schema: CandidateEvaluationVersion::V1,
        edge: CandidateEvaluation::EDGE,
        correlation_id: correlation()?,
        candidate: label(CANDIDATE)?,
        vm_id: VmId::new(100),
        vmm: VmmIdentity::Firecracker,
        verdict,
        evaluated_at: UnixSeconds::new(1_000),
    })
}

/// Renders a capsule request to owned text.
///
/// # Errors
///
/// Propagates the contract refusal.
pub fn encoded_request(request: &CapsuleRequest) -> Result<String, Box<dyn std::error::Error>> {
    encoded(request, CapsuleRequest::encode_into)
}

/// Renders a candidate evaluation to owned text.
///
/// # Errors
///
/// Propagates the contract refusal.
pub fn encoded_evaluation(
    evaluation: &CandidateEvaluation,
) -> Result<String, Box<dyn std::error::Error>> {
    encoded(evaluation, CandidateEvaluation::encode_into)
}

/// The edge a payload must not claim, given the one it does.
#[must_use]
pub fn other_edge(edge: EdgeId) -> EdgeId {
    match edge {
        EdgeId::ExecuteWasmedCapsule => EdgeId::SandboxCandidateEvaluation,
        _ => EdgeId::ExecuteWasmedCapsule,
    }
}

#[path = "../../../source_effects.rs"]
mod source_effects;

#[allow(unused_imports)]
pub use source_effects::{LINE_BOUND, WALK_BOUND, hits, rust_sources, scan_sources, scan_text};
