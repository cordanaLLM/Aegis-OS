// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E16-2: the two consumers decode into the producers' own Rust types and
//! update the shell state.
//!
//! Positive: a `DecisionRequest` and a `CarbonTelemetry` update each decode
//! into the producer crate's type and update the state. Negative: an unknown
//! schema version is refused with the producer's typed error. Boundary: a
//! zero-watt update decodes and updates the state without error. The epic's
//! token-edge decision (D05) is held against the component register.

mod common;

use aegis_forum_shell::a11y::{DECISIONS_ID, TELEMETRY_ID, export};
use aegis_forum_shell::canvas::seed;
use aegis_forum_shell::consumers::{
    ConsumeError, DISPATCH_DECISION_REQUEST, EMIT_CARBON_TELEMETRY, Inbound, decode,
};
use aegis_forum_shell::lifecycle::WindowVerdict;
use aegis_forum_shell::state::{Applied, MAX_PENDING_DECISIONS, ShellState, StateError};
use aegis_justitia::{DecisionRequest, SchemaId as JustitiaSchema};
use aegis_tellus::{CarbonTelemetry, Provenance, SchemaId as TellusSchema};
use serde_json::Value;

use common::{Fallible, decision_payload, decision_request, telemetry, telemetry_payload};

/// Parses a payload into the `params` value a message carries.
fn params(text: &str) -> Fallible<Value> {
    Ok(serde_json::from_str(text)?)
}

/// A shell over the empty canvas.
fn shell() -> Fallible<ShellState> {
    Ok(ShellState::new(seed::empty()?))
}

/// The label the exported tree gives node `id`.
fn label_of(state: &ShellState, id: accesskit::NodeId) -> Option<String> {
    export(state)
        .nodes
        .into_iter()
        .find(|(found, _)| *found == id)
        .and_then(|(_, node)| node.label().map(str::to_owned))
}

// --- Positive -------------------------------------------------------------

/// Positive: a decision request decodes into `aegis_justitia::DecisionRequest`
/// itself, and joins the pending decisions the tree shows.
#[test]
fn a_decision_request_decodes_into_the_producers_type_and_updates_the_state() -> Fallible {
    let original = decision_request("request-0001")?;
    let value = params(&decision_payload(&original)?)?;
    let inbound = decode(DISPATCH_DECISION_REQUEST, Some(&value))?;
    let Inbound::Decision(decoded) = inbound.clone() else {
        return Err("a decision request decoded as something else".into());
    };
    let decoded: DecisionRequest = *decoded;
    assert_eq!(decoded, original);
    assert_eq!(DecisionRequest::SCHEMA, JustitiaSchema::DecisionRequest);
    let mut state = shell()?;
    assert_eq!(state.apply(inbound)?, Applied::Decision(0));
    assert_eq!(state.decisions(), &[original]);
    let tree = export(&state);
    let list = tree
        .nodes
        .iter()
        .find(|(id, _)| *id == DECISIONS_ID)
        .map(|(_, node)| node);
    assert_eq!(list.map(|node| node.children().len()), Some(1));
    Ok(())
}

/// Positive: a telemetry update decodes into `aegis_tellus::CarbonTelemetry`
/// itself, replaces the latest reading and is what the status bar reads out.
#[test]
fn a_telemetry_update_decodes_into_the_producers_type_and_updates_the_state() -> Fallible {
    let original = telemetry(14.2, 0.6)?;
    let value = params(&telemetry_payload(&original)?)?;
    let inbound = decode(EMIT_CARBON_TELEMETRY, Some(&value))?;
    assert_eq!(inbound, Inbound::Telemetry(original));
    assert_eq!(CarbonTelemetry::SCHEMA, TellusSchema::CarbonTelemetry);
    let mut state = shell()?;
    assert_eq!(
        label_of(&state, TELEMETRY_ID).as_deref(),
        Some("No carbon telemetry received yet")
    );
    assert_eq!(
        state.apply(inbound)?,
        Applied::Telemetry(WindowVerdict::WithinBudget)
    );
    assert_eq!(state.telemetry(), Some(&original));
    let label = label_of(&state, TELEMETRY_ID).unwrap_or_default();
    assert!(label.contains("14.2 W (simulated)"), "{label}");
    assert_eq!(original.provenance, Provenance::Simulated);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: an unknown schema version is refused with the producer's own
/// typed error, for both consumers.
#[test]
fn an_unknown_schema_version_is_refused_with_a_typed_error() -> Fallible {
    let decision = decision_payload(&decision_request("request-0001")?)?.replacen(
        "decision-request.v1",
        "decision-request.v2",
        1,
    );
    let refused = decode(DISPATCH_DECISION_REQUEST, Some(&params(&decision)?));
    assert!(
        matches!(
            refused,
            Err(ConsumeError::Decision(
                aegis_justitia::ContractError::UnknownVersion { .. }
            ))
        ),
        "{refused:?}"
    );
    let update = telemetry_payload(&telemetry(14.2, 0.6)?)?.replacen(
        "carbon-telemetry.v1",
        "carbon-telemetry.v2",
        1,
    );
    let refused = decode(EMIT_CARBON_TELEMETRY, Some(&params(&update)?));
    assert!(
        matches!(
            refused,
            Err(ConsumeError::Telemetry(
                aegis_tellus::ContractError::UnknownVersion { .. }
            ))
        ),
        "{refused:?}"
    );
    Ok(())
}

/// Negative: a payload sent on the other edge, an unknown method and a
/// missing params object are refused, each with its own error and code.
#[test]
fn a_crossed_payload_an_unknown_method_and_missing_params_are_refused() -> Fallible {
    let update = params(&telemetry_payload(&telemetry(14.2, 0.6)?)?)?;
    assert!(matches!(
        decode(DISPATCH_DECISION_REQUEST, Some(&update)),
        Err(ConsumeError::Decision(_))
    ));
    let unknown = decode("SYNC_DESKTOP_SHELL", Some(&update));
    assert_eq!(
        unknown,
        Err(ConsumeError::UnknownMethod("SYNC_DESKTOP_SHELL".to_owned()))
    );
    assert_eq!(unknown.map_err(|error| error.code()), Err(-32_601));
    let missing = decode(EMIT_CARBON_TELEMETRY, None);
    assert_eq!(
        missing,
        Err(ConsumeError::MissingParams {
            method: EMIT_CARBON_TELEMETRY
        })
    );
    let array = decode(EMIT_CARBON_TELEMETRY, Some(&Value::Array(Vec::new())));
    assert_eq!(array.map_err(|error| error.code()), Err(-32_602));
    Ok(())
}

/// Negative: a request id already pending is refused by the state.
#[test]
fn a_duplicate_decision_request_is_refused() -> Fallible {
    let mut state = shell()?;
    let request = decision_request("request-0001")?;
    state.apply(Inbound::Decision(Box::new(request)))?;
    assert_eq!(
        state.apply(Inbound::Decision(Box::new(request))),
        Err(StateError::DuplicateDecision("request-0001".to_owned()))
    );
    assert_eq!(state.decisions().len(), 1);
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a zero-watt update decodes and updates the state without error,
/// and the readout shows zero rather than nothing.
#[test]
fn a_zero_watt_update_decodes_and_updates_the_state() -> Fallible {
    let original = telemetry(0.0, 0.05)?;
    let value = params(&telemetry_payload(&original)?)?;
    let mut state = shell()?;
    let applied = state.apply(decode(EMIT_CARBON_TELEMETRY, Some(&value))?)?;
    assert_eq!(applied, Applied::Telemetry(WindowVerdict::WithinBudget));
    let stored = state
        .telemetry()
        .map(|update| update.power_watts.get().to_bits());
    assert_eq!(stored, Some(0.0_f64.to_bits()));
    let label = label_of(&state, TELEMETRY_ID).unwrap_or_default();
    assert!(label.starts_with("Power draw 0 W (simulated)"), "{label}");
    Ok(())
}

/// Boundary: the pending list holds exactly `MAX_PENDING_DECISIONS` and
/// refuses one more.
#[test]
fn the_pending_list_is_full_at_its_bound() -> Fallible {
    let mut state = shell()?;
    for index in 0..MAX_PENDING_DECISIONS {
        let request = decision_request(&format!("request-{index:04}"))?;
        assert_eq!(
            state.apply(Inbound::Decision(Box::new(request)))?,
            Applied::Decision(index)
        );
    }
    let extra = decision_request("request-9999")?;
    assert_eq!(
        state.apply(Inbound::Decision(Box::new(extra))),
        Err(StateError::DecisionsFull)
    );
    Ok(())
}

/// Positive (D05, REQ-GRAPH-01): the token edge keeps the graph's identifier
/// and direction and annotates that P12 produces and P05 consumes, as the
/// component register records the edge.
#[test]
fn the_token_edge_direction_is_recorded_as_d05_decides() -> Fallible {
    use aegis_forum_shell::tokens::TOKEN_EDGE;
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../planning/components.json"
    );
    let register: Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    let components = register
        .get("components")
        .and_then(Value::as_array)
        .ok_or("no components")?;
    let forum = components
        .iter()
        .find(|row| row.get("id") == Some(&Value::from("P05")))
        .ok_or("no P05 row")?;
    let edge = serde_json::json!({ "to": TOKEN_EDGE.graph_to, "relation": TOKEN_EDGE.edge });
    let sends = forum
        .get("sends_to")
        .and_then(Value::as_array)
        .ok_or("no sends_to")?;
    assert!(sends.contains(&edge), "{sends:?}");
    assert_eq!((TOKEN_EDGE.graph_from, TOKEN_EDGE.graph_to), ("P05", "P12"));
    assert_eq!((TOKEN_EDGE.producer, TOKEN_EDGE.consumer), ("P12", "P05"));
    assert!(
        TOKEN_EDGE
            .form
            .contains("ui/concordia-tokens/concordia-tokens.css")
    );
    Ok(())
}
