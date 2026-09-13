// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E06-4 and exit criterion 3: the two payloads P09 builds from other
//! crates' types.
//!
//! Positive: a sealed draft becomes the M14 action proposal and round-trips
//! through P06's own decoder; a dispatch becomes P10's capsule request and
//! round-trips through P10's. Negative: an unsealed or empty-sealed draft is
//! refused, with P06's refusal rather than one of ours. Boundary: a dispatch
//! at the capsule bound is accepted and one over never becomes a slot.
//!
//! Neither type is defined in this crate. That is the point of the file: what
//! is exercised here is P09 filling in a consumer's type and reading the
//! consumer's constants.

mod common;

use aegis_justitia::{
    ActionProposal, ContractError as GateError, Correlation as GateCorrelation, EdgeId as GateEdge,
    GateDirection, SandboxAdmissionPath, SchemaId as GateSchema,
};
use aegis_vesta::{
    AdmittedRuntime, CapsuleRequest, CapsuleSlot, MAX_WASM_CAPSULES, VestaError, VmmIdentity,
};

use common::{Fallible, dispatch, draft, empty_seal, identity, seal};

// --- Positive -------------------------------------------------------------

/// Positive: a sealed draft becomes a proposal P06's own decoder accepts.
#[test]
fn a_sealed_draft_becomes_a_proposal_p06_accepts() -> Fallible {
    let proposal: ActionProposal = draft(Some(&seal()?))?.into_proposal()?;
    assert_eq!(proposal.edge, ActionProposal::EDGE);
    assert_eq!(proposal.edge, GateEdge::ActionGateIntercept);
    assert_eq!(proposal.direction, GateDirection::SETTLED);
    assert_eq!(proposal.correlation_id, identity()?);
    assert_eq!(proposal.validate(), Ok(()));

    Ok(())
}

/// Positive: what P09 produced is what P06's own decoder reads back, and the
/// settled direction travels on the wire.
#[test]
fn p06_decodes_what_p09_produced() -> Fallible {
    let proposal = draft(Some(&seal()?))?.into_proposal()?;
    let mut buffer = aegis_justitia::PayloadBuffer::new();
    let text = proposal.encode_into(&mut buffer)?.to_owned();
    let decoded = ActionProposal::decode(&text)?;
    assert_eq!(decoded, proposal, "P06 decodes what P09 produced");
    assert_eq!(decoded.correlation().schema(), GateSchema::ActionProposal);
    assert!(text.contains(GateDirection::SETTLED.tag()));
    assert_eq!(GateDirection::SETTLED.proposer(), "P09_Minerva");
    assert_eq!(GateDirection::SETTLED.interceptor(), "P06_Justitia");
    Ok(())
}

/// Positive: a dispatch becomes a capsule request P10's own decoder accepts,
/// carrying the runtime D06 settled and the monitor D58 requires.
#[test]
fn a_dispatch_becomes_a_request_p10_accepts() -> Fallible {
    let request: CapsuleRequest = dispatch(1)?.into_request()?;
    assert_eq!(request.edge, CapsuleRequest::EDGE);
    assert_eq!(request.runtime, AdmittedRuntime::SETTLED);
    assert_eq!(request.vmm, VmmIdentity::Firecracker);
    assert_eq!(request.slot, CapsuleSlot::FIRST);
    assert_eq!(
        request.admission,
        SandboxAdmissionPath::TransitiveThroughMinerva
    );
    assert!(request.admission_is_in_graph_of_record());

    let mut buffer = aegis_vesta::PayloadBuffer::new();
    let text = buffer_text(&request, &mut buffer)?;
    let decoded = CapsuleRequest::decode(&text)?;
    assert_eq!(decoded, request, "P10 decodes what P09 produced");
    Ok(())
}

/// Renders a capsule request into `buffer` and returns owned text.
fn buffer_text(
    request: &CapsuleRequest,
    buffer: &mut aegis_vesta::PayloadBuffer,
) -> Result<String, Box<dyn std::error::Error>> {
    Ok(request.encode_into(buffer)?.to_owned())
}

// --- Negative -------------------------------------------------------------

/// Negative: an unsealed draft produces no proposal, and the refusal is P06's.
#[test]
fn an_unsealed_draft_produces_no_proposal() -> Fallible {
    let refused = draft(None)?.into_proposal();
    let expected = GateError::Unsigned {
        correlation: GateCorrelation::new(GateSchema::ActionProposal, Some(identity()?)),
    };
    assert_eq!(refused, Err(expected));
    assert_eq!(
        draft(Some(&empty_seal()?))?.into_proposal(),
        Err(expected),
        "a seal carrying no bytes is not a seal"
    );
    if let Err(error) = refused {
        assert_eq!(error.correlation().id(), Some(identity()?));
        assert!(error.to_string().contains("no oversight signature"));
    }
    Ok(())
}

/// Negative: P09 cannot name the rejected runtime or the wrong edge, because
/// it reads both from P10's own constants rather than taking them.
#[test]
fn the_producer_cannot_name_a_rejected_constant() -> Fallible {
    let request = dispatch(2)?.into_request()?;
    assert_eq!(request.runtime.choice().name(), "rust-native-runtime");
    assert_eq!(request.edge.name(), "EXECUTE_WASMED_CAPSULE");
    assert_eq!(request.validate(), Ok(()));
    let proposal = draft(Some(&seal()?))?.into_proposal()?;
    assert_eq!(proposal.edge.evidence(), "export-062");
    assert!(proposal.edge.in_graph_of_record());
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a dispatch at the capsule bound is accepted and one over never
/// becomes a slot at all.
#[test]
fn a_dispatch_at_the_capsule_bound_is_accepted_and_one_over_is_not() -> Fallible {
    let at_bound = dispatch(MAX_WASM_CAPSULES)?.into_request()?;
    assert_eq!(at_bound.slot, CapsuleSlot::LAST);
    assert_eq!(at_bound.slot.get(), MAX_WASM_CAPSULES);

    let over = CapsuleSlot::new(MAX_WASM_CAPSULES.saturating_add(1));
    assert_eq!(
        over,
        Err(VestaError::SlotOutOfRange {
            slot: 129,
            max: MAX_WASM_CAPSULES
        })
    );
    assert!(
        dispatch(MAX_WASM_CAPSULES.saturating_add(1)).is_err(),
        "the fixture cannot even build a dispatch past the bound"
    );
    Ok(())
}

/// Boundary: the same correlation text is validated by each consumer's own
/// constructor, and one that is too long for P10 is refused there even though
/// P06 admits it.
#[test]
fn each_consumer_validates_the_correlation_with_its_own_bound() {
    let long = "c".repeat(aegis_justitia::MAX_IDENTITY_LEN);
    assert!(aegis_justitia::Identity::parse(&long).is_ok());
    assert!(aegis_vesta::CorrelationId::parse(&long).is_ok());
    let longer = "c".repeat(aegis_vesta::MAX_CORRELATION_LEN.saturating_add(1));
    assert!(aegis_vesta::CorrelationId::parse(&longer).is_err());
    assert!(aegis_justitia::Identity::parse(&longer).is_err());
    let at_bound = "c".repeat(aegis_vesta::MAX_CORRELATION_LEN);
    assert!(
        aegis_justitia::Identity::parse(&at_bound).is_ok(),
        "the two bounds agree today, and each is still checked by its own crate"
    );
}
