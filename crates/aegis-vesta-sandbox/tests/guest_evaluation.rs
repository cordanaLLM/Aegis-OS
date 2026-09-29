// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The evaluation a guest runs and the host's check of it, without a guest.
//!
//! [`respond`] is the function the guest init calls for each line it reads;
//! here it is called directly with the context identifier the controller
//! derives for sandbox 100. Positive: a superior candidate passes and one at
//! the 1.5 ms bound fails, as P16's gate decides. Negative: a request for
//! another sandbox, another monitor, a malformed request and a tampered answer
//! are each refused. Boundary: a request line of exactly the byte bound is
//! read, one byte more is refused.

mod common;

use aegis_justitia::UnixSeconds;
use aegis_vesta::{CandidateEvaluation, EvaluationVerdict, VmId, VmmIdentity};
use aegis_vesta_sandbox::{
    AT_LATENCY_BOUND, EvaluationRequest, MAX_REQUEST_BYTES, REQUEST_SCHEMA_TAG, RequestVersion,
    SUPERIOR, SandboxError, WireMetrics, answer, check_answer, expected_verdict, request, respond,
};

use common::{FIRST_CID, FIRST_VM, Fallible, first_request};

/// Returns the guest's decoded answer to `request`.
fn guest_answer(
    request: &EvaluationRequest,
) -> Result<CandidateEvaluation, Box<dyn std::error::Error>> {
    let line = respond(&request.encode_line()?, FIRST_CID)?;
    Ok(CandidateEvaluation::decode(line.trim_end_matches('\n'))?)
}

// --- Positive -------------------------------------------------------------

/// Positive (E21-2): the guest's verdicts are the Pareto gate's, and the host
/// accepts them.
#[test]
fn the_guest_judges_with_the_pareto_gate() -> Fallible {
    let superior = first_request("superior", SUPERIOR)?;
    let passed = guest_answer(&superior)?;
    assert_eq!(passed.verdict, EvaluationVerdict::Passed);
    check_answer(&superior, &passed)?;
    let bound = first_request("latency-bound", AT_LATENCY_BOUND)?;
    let failed = guest_answer(&bound)?;
    assert_eq!(failed.verdict, EvaluationVerdict::Failed);
    check_answer(&bound, &failed)?;
    assert_eq!(expected_verdict(&bound)?, EvaluationVerdict::Failed);
    Ok(())
}

/// Positive: the answer carries the request's identity and the guest's time.
#[test]
fn the_answer_carries_the_request() -> Fallible {
    let superior = first_request("superior", SUPERIOR)?;
    let evaluation = answer(&superior, FIRST_CID, UnixSeconds::new(1_790_712_268))?;
    assert_eq!(evaluation.vm_id, VmId::new(FIRST_VM));
    assert_eq!(evaluation.vmm, VmmIdentity::Firecracker);
    assert_eq!(evaluation.correlation_id, superior.correlation_id);
    assert_eq!(evaluation.candidate, superior.candidate);
    assert_eq!(evaluation.evaluated_at, UnixSeconds::new(1_790_712_268));
    assert_eq!(superior.schema, RequestVersion::V1);
    assert!(superior.encode_line()?.contains(REQUEST_SCHEMA_TAG));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a request for another sandbox is refused by the guest it
/// reached, and so is one naming the fallback monitor.
#[test]
fn the_guest_refuses_a_request_for_another_sandbox() -> Fallible {
    let other = request(VmId::new(FIRST_VM.saturating_add(1)), "superior", SUPERIOR)?;
    assert!(matches!(
        respond(&other.encode_line()?, FIRST_CID),
        Err(SandboxError::Refused(_))
    ));
    let mut qemu = first_request("superior", SUPERIOR)?;
    qemu.vmm = VmmIdentity::QemuMicrovm;
    assert!(matches!(
        respond(&qemu.encode_line()?, FIRST_CID),
        Err(SandboxError::Refused(_))
    ));
    Ok(())
}

/// Negative: a malformed request is refused before any evaluation.
#[test]
fn a_malformed_request_is_refused() -> Fallible {
    let superior = first_request("superior", SUPERIOR)?.encode_line()?;
    let unknown = superior.replacen("\"vmm\"", "\"monitor\"", 1);
    let version = superior.replacen(
        REQUEST_SCHEMA_TAG,
        "aegis.m21.guest-evaluation-request.v2",
        1,
    );
    for line in [unknown.as_str(), version.as_str(), "{}\n", "not json\n"] {
        assert!(
            matches!(
                EvaluationRequest::decode_line(line),
                Err(SandboxError::Protocol(_))
            ),
            "{line}"
        );
    }
    let negative = WireMetrics {
        latency_ms: -1.0,
        ..SUPERIOR
    };
    assert!(matches!(
        first_request("negative", negative)?.encode_line(),
        Err(SandboxError::Athena(_))
    ));
    Ok(())
}

/// Negative: the host refuses an answer whose verdict or identity differs.
#[test]
fn a_tampered_answer_is_refused() -> Fallible {
    let bound = first_request("latency-bound", AT_LATENCY_BOUND)?;
    let mut forged = guest_answer(&bound)?;
    forged.verdict = EvaluationVerdict::Passed;
    assert!(matches!(
        check_answer(&bound, &forged),
        Err(SandboxError::Refused(_))
    ));
    let mut moved = guest_answer(&bound)?;
    moved.vm_id = VmId::new(FIRST_VM.saturating_add(1));
    assert!(matches!(
        check_answer(&bound, &moved),
        Err(SandboxError::Refused(_))
    ));
    let mut renamed = guest_answer(&bound)?;
    renamed.candidate = first_request("superior", SUPERIOR)?.candidate;
    assert!(matches!(
        check_answer(&bound, &renamed),
        Err(SandboxError::Refused(_))
    ));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a request line of exactly the byte bound is read; one byte more
/// is refused. JSON admits trailing whitespace, which pads the line.
#[test]
fn the_request_bound_is_closed_at_its_edge() -> Fallible {
    let line = first_request("superior", SUPERIOR)?.encode_line()?;
    let body = line.trim_end_matches('\n');
    let pad = MAX_REQUEST_BYTES
        .checked_sub(body.len())
        .and_then(|room| room.checked_sub(1))
        .ok_or("the request fits the bound")?;
    let exact = format!("{body}{}\n", " ".repeat(pad));
    assert_eq!(exact.len(), MAX_REQUEST_BYTES);
    assert!(EvaluationRequest::decode_line(&exact).is_ok());
    let over = format!("{body}{} \n", " ".repeat(pad));
    assert!(matches!(
        EvaluationRequest::decode_line(&over),
        Err(SandboxError::Protocol(_))
    ));
    Ok(())
}
