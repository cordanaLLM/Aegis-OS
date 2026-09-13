// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E05-4, the P16-to-P02 edge: the promotion trigger, and the sysupdate call
//! it stands in for.

mod common;

use aegis_athena::{
    CallDeadline, CallKind, ContractError, Correlation, EdgeId, Event, InvalidationReason,
    Lifecycle, MAX_CONTRACT_PAYLOAD_BYTES, MAX_RECORDED_CALLS, PayloadBuffer, PortError,
    PromotionAction, PromotionTrigger, PromotionTriggerVersion, STUB_SERVICE_MILLIS, SchemaId,
    Slot, Stage, StubSysupdate, SysupdateCall, SysupdatePort, TriggerStage,
};

use common::{CANDIDATE, CORRELATION, Fallible, candidate, correlation, passing};

/// Returns a lifecycle that has published the fixture candidate.
fn publishing() -> Result<Lifecycle, Box<dyn std::error::Error>> {
    let mut lifecycle = Lifecycle::new();
    lifecycle.step(Event::Challenge)?;
    lifecycle.step(Event::Decompose)?;
    lifecycle.step(Event::Prove)?;
    lifecycle.step(Event::Check {
        metrics: passing()?,
        elapsed_ms: 100,
    })?;
    Ok(lifecycle)
}

/// Returns the trigger a published lifecycle justifies.
fn trigger() -> Result<PromotionTrigger, Box<dyn std::error::Error>> {
    PromotionTrigger::from_lifecycle(&publishing()?, correlation()?, candidate()?, Slot::B)
        .ok_or_else(|| "a published lifecycle justifies a trigger".into())
}

/// Returns a deadline comfortably longer than the stub's service time.
fn deadline() -> Result<CallDeadline, Box<dyn std::error::Error>> {
    CallDeadline::try_from_millis(5).ok_or_else(|| "a deadline must be positive".into())
}

// --- Positive -------------------------------------------------------------

/// Positive: a promotion trigger round-trips through its own codec.
#[test]
fn a_promotion_trigger_round_trips() -> Fallible {
    let payload = trigger()?;
    let mut buffer = PayloadBuffer::new();
    let text = payload.encode_into(&mut buffer)?;
    assert!(text.contains(CORRELATION));
    assert!(text.contains(CANDIDATE));
    assert!(text.contains(SchemaId::PromotionTrigger.tag()));
    assert_eq!(PromotionTrigger::decode(text)?, payload);
    Ok(())
}

/// Positive: a trigger built from a published lifecycle carries that decision.
#[test]
fn a_trigger_carries_the_decision_it_was_built_from() -> Fallible {
    let payload = trigger()?;
    assert_eq!(PromotionTrigger::SCHEMA, SchemaId::PromotionTrigger);
    assert_eq!(PromotionTrigger::EDGE, EdgeId::TriggerSysupdateRollback);
    assert_eq!(payload.action, PromotionAction::Deploy);
    assert_eq!(payload.stage, TriggerStage::Publish);
    assert_eq!(payload.slot, Slot::B);
    assert_eq!(payload.cleared, 4);
    assert_eq!(payload.schema, PromotionTriggerVersion::V1);
    Ok(())
}

/// Positive: an invalidated lifecycle justifies a rollback.
#[test]
fn an_invalidated_lifecycle_justifies_a_rollback() -> Fallible {
    let mut lifecycle = publishing()?;
    lifecycle.step(Event::Invalidate {
        reason: InvalidationReason::PostPublicationRegression,
    })?;
    let payload =
        PromotionTrigger::from_lifecycle(&lifecycle, correlation()?, candidate()?, Slot::A)
            .ok_or("an invalidated lifecycle justifies a trigger")?;
    payload.validate()?;
    assert_eq!(payload.action, PromotionAction::Rollback);
    assert_eq!(payload.stage, TriggerStage::Invalidate);
    assert_eq!(payload.slot, Slot::A);
    assert_eq!(payload.slot.other(), Slot::B);
    Ok(())
}

/// Positive: the P16 graph vocabulary describes all four edges and says where
/// each payload is typed.
#[test]
fn the_graph_vocabulary_says_where_each_payload_is_typed() {
    assert_eq!(EdgeId::ALL.len(), 4);
    for edge in EdgeId::ALL {
        let described = [
            !edge.name().is_empty(),
            !edge.peer().is_empty(),
            !edge.recorded_transport().is_empty(),
            !edge.typed_in().is_empty(),
        ];
        assert_eq!(described, [true; 4], "{} is under-described", edge.name());
    }
    let peers: Vec<&str> = EdgeId::ALL.into_iter().map(EdgeId::peer).collect();
    assert_eq!(peers, vec!["P02", "P06", "P13", "P10"]);
}

/// Positive: exactly the two outbound edges are outbound.
#[test]
fn exactly_two_edges_are_outbound() {
    assert!(EdgeId::TriggerSysupdateRollback.outbound());
    assert!(EdgeId::EvaluateCandidateCarbonSci.outbound());
    assert!(!EdgeId::AuditReconstructiveCandidate.outbound());
    assert!(!EdgeId::SandboxCandidateEvaluation.outbound());
    assert!(
        EdgeId::SandboxCandidateEvaluation
            .typed_in()
            .contains("not typed"),
        "the microVM edge is M06 and M22 work, and says so"
    );
}

// --- Negative -------------------------------------------------------------

/// Negative: a malformed trigger payload is refused.
#[test]
fn a_malformed_trigger_is_refused() -> Fallible {
    let payload = trigger()?;
    let mut buffer = PayloadBuffer::new();
    let text = payload.encode_into(&mut buffer)?.to_owned();

    assert!(matches!(
        PromotionTrigger::decode(&text.replace(
            SchemaId::PromotionTrigger.tag(),
            "aegis.p16-p02.promotion-trigger.v2"
        )),
        Err(ContractError::UnknownVersion { .. })
    ));

    for bad in [
        "{}",
        "not json",
        "[]",
        &text.replace("\"correlation-id\"", "\"correlationId\""),
        &text.replace(&format!("\"{CANDIDATE}\""), "\"\""),
        &text.replace("\"slot\":\"b\"", "\"slot\":\"c\""),
        &text.replace("\"action\":\"deploy\"", "\"action\":\"reboot\""),
        &text.replace('}', ",\"extra\":1}"),
    ] {
        assert!(
            PromotionTrigger::decode(bad).is_err(),
            "the decoder accepted {bad}"
        );
    }
    Ok(())
}

/// Negative: a trigger whose action does not follow from its stage is refused.
///
/// P02 would act on either message, and the action it would take is not
/// reversible by a later one.
#[test]
fn a_trigger_whose_action_does_not_follow_is_refused() -> Fallible {
    let mut lying = trigger()?;
    lying.action = PromotionAction::Rollback;
    let refusal = lying.validate();
    assert!(matches!(
        refusal,
        Err(ContractError::ActionDoesNotFollow {
            action: "rollback",
            stage: "publish",
            ..
        })
    ));

    let mut buffer = PayloadBuffer::new();
    assert!(lying.encode_into(&mut buffer).is_err());
    Ok(())
}

/// Negative: a trigger naming another edge is refused.
#[test]
fn a_trigger_naming_another_edge_is_refused() -> Fallible {
    let mut payload = trigger()?;
    payload.edge = EdgeId::SandboxCandidateEvaluation;
    assert!(matches!(
        payload.validate(),
        Err(ContractError::WrongEdge {
            found: "SANDBOX_CANDIDATE_EVALUATION",
            ..
        })
    ));
    Ok(())
}

/// Negative: an undecided lifecycle justifies no trigger at all.
#[test]
fn an_undecided_lifecycle_justifies_no_trigger() -> Fallible {
    let mut lifecycle = Lifecycle::new();
    for stage in [Stage::Propose, Stage::Challenge, Stage::Decompose] {
        assert!(
            PromotionTrigger::from_lifecycle(&lifecycle, correlation()?, candidate()?, Slot::A)
                .is_none(),
            "{stage} justifies no update action"
        );
        if lifecycle.stage() == Stage::Decompose {
            break;
        }
        lifecycle.step(match lifecycle.stage() {
            Stage::Propose => Event::Challenge,
            _ => Event::Decompose,
        })?;
    }
    assert_eq!(TriggerStage::from_stage(Stage::Prove), None);
    assert_eq!(PromotionAction::for_stage(Stage::Check), None);
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the sysupdate call is stubbed, and the stub records it.
///
/// Nothing runs `systemd-sysupdate`, opens D-Bus, writes a partition or
/// reboots. The stub is what makes the absence checkable rather than merely
/// stated.
#[test]
fn the_sysupdate_call_is_stubbed_and_recorded() -> Fallible {
    let mut port = StubSysupdate::new();
    assert!(port.is_empty());
    assert_eq!(port.service_time().millis(), STUB_SERVICE_MILLIS);

    let call = SysupdateCall {
        kind: CallKind::Deploy,
        candidate: candidate()?,
        reason: None,
    };
    port.call(call, deadline()?)?;
    assert_eq!(port.len(), 1);
    assert_eq!(port.get(0), Some(call));
    assert_eq!(port.get(1), None);

    port.call(
        SysupdateCall {
            kind: CallKind::Rollback,
            candidate: candidate()?,
            reason: Some(InvalidationReason::PostPublicationRegression),
        },
        deadline()?,
    )?;
    assert_eq!(port.len(), 2);
    assert_eq!(CallKind::ALL.len(), 2);
    assert_eq!(CallKind::Deploy.name(), "deploy");
    assert_eq!(CallKind::Rollback.name(), "rollback");
    Ok(())
}

/// Boundary: the stub refuses a call past its recording bound and one under
/// its service time.
#[test]
fn the_stub_refuses_past_its_bound_and_under_its_deadline() -> Fallible {
    let mut port = StubSysupdate::default();
    let call = SysupdateCall {
        kind: CallKind::Deploy,
        candidate: candidate()?,
        reason: None,
    };
    for _ in 0..MAX_RECORDED_CALLS {
        port.call(call, deadline()?)?;
    }
    assert_eq!(port.len(), MAX_RECORDED_CALLS);
    assert!(matches!(
        port.call(call, deadline()?),
        Err(PortError::RecordingFull {
            max: MAX_RECORDED_CALLS
        })
    ));

    assert!(CallDeadline::try_from_millis(0).is_none());
    let exact = CallDeadline::from_millis(
        core::num::NonZeroU32::new(STUB_SERVICE_MILLIS).ok_or("the service time is positive")?,
    );
    assert_eq!(exact.millis(), STUB_SERVICE_MILLIS);
    Ok(())
}

/// Boundary: the slot vocabulary has exactly two values and each is its own
/// other.
#[test]
fn the_slot_vocabulary_is_exactly_two() {
    assert_eq!(Slot::ALL.len(), 2);
    assert_eq!(Slot::A.name(), "a");
    assert_eq!(Slot::B.name(), "b");
    assert_eq!(Slot::A.other(), Slot::B);
    assert_eq!(Slot::B.other().other(), Slot::B);
}

/// Boundary: the action, stage and schema vocabularies are each exactly two.
#[test]
fn the_action_and_stage_vocabularies_are_exactly_two() {
    assert_eq!(PromotionAction::ALL.len(), 2);
    assert_eq!(TriggerStage::ALL.len(), 2);
    assert_eq!(SchemaId::ALL.len(), 2);
    assert_eq!(TriggerStage::Publish.as_stage(), Stage::Publish);
    assert_eq!(TriggerStage::Invalidate.as_stage(), Stage::Invalidate);
    assert_eq!(TriggerStage::Invalidate.name(), "invalidate");
    assert_eq!(
        SchemaId::PromotionTrigger.edge(),
        EdgeId::TriggerSysupdateRollback
    );
}

/// Boundary: the payload bound is 4096, it is the same 4096 `aegis-tellus`
/// declares, and it turns between exactly the bound and one byte past it.
///
/// Both crates state the constant because each owns the buffer on its own side
/// of the edge, and this crate depends on that one, so the two can be compared
/// rather than left to drift: a payload one side would accept and the other
/// would refuse is a wire incompatibility no round-trip test would see, because
/// each round-trip stays inside one crate.
///
/// The refusal below is written against the constant, which is right and which
/// also means it holds for any value of it. Asserting the recorded figure is
/// what gives the number in `planning/roadmap.json` a falsifier.
#[test]
fn the_payload_bound_is_the_recorded_figure_on_both_sides() {
    assert_eq!(
        MAX_CONTRACT_PAYLOAD_BYTES, 4096,
        "the recorded payload bound is 4096 bytes"
    );
    assert_eq!(
        MAX_CONTRACT_PAYLOAD_BYTES,
        aegis_tellus::MAX_CONTRACT_PAYLOAD_BYTES,
        "the two sides of the P16-to-P13 edge must bound a payload identically"
    );

    let at_bound = "x".repeat(MAX_CONTRACT_PAYLOAD_BYTES);
    assert!(
        !matches!(
            PromotionTrigger::decode(&at_bound),
            Err(ContractError::PayloadTooLong { .. })
        ),
        "exactly the bound is inside it: the payload is refused as malformed, not as too long"
    );

    let oversized = "x".repeat(MAX_CONTRACT_PAYLOAD_BYTES.saturating_add(1));
    assert!(matches!(
        PromotionTrigger::decode(&oversized),
        Err(ContractError::PayloadTooLong {
            max: MAX_CONTRACT_PAYLOAD_BYTES,
            ..
        })
    ));
}

/// Boundary: a correlation renders with and without an identifier.
#[test]
fn the_correlation_renders_with_and_without_an_identifier() -> Fallible {
    let named = Correlation::new(SchemaId::PromotionTrigger, Some(correlation()?));
    assert_eq!(named.schema(), SchemaId::PromotionTrigger);
    assert_eq!(named.id(), Some(correlation()?));
    assert!(format!("{named}").contains(CORRELATION));

    let anonymous = Correlation::new(SchemaId::SignedAuditRecord, None);
    assert!(format!("{anonymous}").contains("uncorrelated"));
    assert_eq!(
        trigger()?.correlation().schema(),
        SchemaId::PromotionTrigger
    );
    Ok(())
}
