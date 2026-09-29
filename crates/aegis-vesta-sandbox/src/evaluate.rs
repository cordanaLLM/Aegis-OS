// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The evaluation itself, and the host's check of what came back.
//!
//! [`answer`] is what runs inside the guest: it binds the request to the
//! sandbox it actually arrived in, through the guest's own `AF_VSOCK` context
//! identifier, and judges the metrics with P16's Pareto gate. [`check_answer`]
//! is the host's side: the decoded payload must name the same correlation,
//! candidate, sandbox and monitor, and carry the verdict the host computes
//! independently from the same metrics -- so a guest that echoed a canned
//! "passed" would fail the breaching candidate.

use aegis_athena::ParetoVerdict;
use aegis_justitia::UnixSeconds;
use aegis_vesta::{
    CandidateEvaluation, CandidateEvaluationVersion, EvaluationVerdict, FIRST_GUEST_CID, VmId,
    VmmIdentity,
};

use crate::error::SandboxError;
use crate::request::EvaluationRequest;

/// Returns the sandbox identifier `aegis-vesta`'s controller derives the
/// context identifier `cid` from: `cid - FIRST_GUEST_CID`.
///
/// # Errors
///
/// Returns [`SandboxError::Refused`] for a reserved context identifier.
pub fn vm_id_for_cid(cid: u32) -> Result<VmId, SandboxError> {
    cid.checked_sub(FIRST_GUEST_CID)
        .map(VmId::new)
        .ok_or_else(|| SandboxError::Refused(format!("context identifier {cid} is reserved")))
}

/// Returns the verdict P16's Pareto gate gives the request's metrics.
///
/// # Errors
///
/// Returns [`SandboxError::Athena`] for metrics P16 refuses.
pub fn expected_verdict(request: &EvaluationRequest) -> Result<EvaluationVerdict, SandboxError> {
    let verdict = ParetoVerdict::evaluate(&request.metrics.metrics()?);
    Ok(if verdict.is_superior() {
        EvaluationVerdict::Passed
    } else {
        EvaluationVerdict::Failed
    })
}

/// Evaluates `request` inside the guest whose context identifier is
/// `local_cid`, at guest time `now`.
///
/// # Errors
///
/// Returns [`SandboxError::Refused`] when the request names another sandbox
/// than the one it arrived in, or a monitor other than Firecracker, which is
/// the only one this path runs; and [`SandboxError::Athena`] for metrics P16
/// refuses.
pub fn answer(
    request: &EvaluationRequest,
    local_cid: u32,
    now: UnixSeconds,
) -> Result<CandidateEvaluation, SandboxError> {
    let here = vm_id_for_cid(local_cid)?;
    if request.vm_id != here {
        return Err(SandboxError::Refused(format!(
            "the request names sandbox {}, and it arrived in sandbox {} (context identifier {local_cid})",
            request.vm_id.get(),
            here.get()
        )));
    }
    if request.vmm != VmmIdentity::Firecracker {
        return Err(SandboxError::Refused(format!(
            "the request names {}, and this guest runs under firecracker",
            request.vmm.tag()
        )));
    }
    Ok(CandidateEvaluation {
        schema: CandidateEvaluationVersion::V1,
        edge: CandidateEvaluation::EDGE,
        correlation_id: request.correlation_id,
        candidate: request.candidate,
        vm_id: request.vm_id,
        vmm: request.vmm,
        verdict: expected_verdict(request)?,
        evaluated_at: now,
    })
}

/// Checks the decoded answer against the request the host sent.
///
/// # Errors
///
/// Returns [`SandboxError::Refused`] naming the first field that differs,
/// the verdict included.
pub fn check_answer(
    request: &EvaluationRequest,
    answer: &CandidateEvaluation,
) -> Result<(), SandboxError> {
    let differs = |field: &str| {
        Err(SandboxError::Refused(format!(
            "the answer's {field} differs"
        )))
    };
    if answer.correlation_id != request.correlation_id {
        return differs("correlation-id");
    }
    if answer.candidate != request.candidate {
        return differs("candidate");
    }
    if answer.vm_id != request.vm_id {
        return differs("vm-id");
    }
    if answer.vmm != request.vmm {
        return differs("vmm");
    }
    let expected = expected_verdict(request)?;
    if answer.verdict != expected {
        return Err(SandboxError::Refused(format!(
            "the answer's verdict is {}, the host computes {}",
            answer.verdict.tag(),
            expected.tag()
        )));
    }
    Ok(())
}
