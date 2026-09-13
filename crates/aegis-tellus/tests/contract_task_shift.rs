// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E05-4, the P13-to-P07 edge: the spatiotemporal task-shift directive.

mod common;

use aegis_tellus::{
    ContractError, EdgeId, EmbodiedCarbon, GridIntensity, PayloadBuffer, SchemaId, SciEngine,
    SliceName, TaskShiftDirective, TaskShiftVersion,
};

use common::{CORRELATION, Fallible, SLICE, correlation, directive, slice};

// --- Positive -------------------------------------------------------------

/// Positive: a directive round-trips through its own encoder and decoder.
#[test]
fn a_directive_round_trips() -> Fallible {
    let payload = directive()?;
    let mut buffer = PayloadBuffer::new();
    let text = payload.encode_into(&mut buffer)?;
    assert!(text.contains(CORRELATION));
    assert!(text.contains(SLICE));
    assert!(text.contains(SchemaId::TaskShiftDirective.tag()));
    assert_eq!(TaskShiftDirective::decode(text)?, payload);
    assert_eq!(TaskShiftDirective::SCHEMA, SchemaId::TaskShiftDirective);
    assert_eq!(TaskShiftDirective::EDGE, EdgeId::SpatiotemporalTaskShift);
    assert_eq!(TaskShiftVersion::V1, payload.schema);
    Ok(())
}

/// Positive: a directive built from an engine carries that engine's verdict.
#[test]
fn a_directive_built_from_an_engine_carries_its_verdict() -> Fallible {
    let clean = SciEngine::new();
    let quiet = TaskShiftDirective::from_engine(&clean, correlation()?, slice()?);
    quiet.validate()?;
    assert!(!quiet.defer);
    assert_eq!(quiet.intensity, clean.intensity());

    let dirty = SciEngine::with_inputs(GridIntensity::new(450.0)?, EmbodiedCarbon::DEFAULT);
    let shift = TaskShiftDirective::from_engine(&dirty, correlation()?, slice()?);
    shift.validate()?;
    assert!(shift.defer);
    assert_eq!(shift.slice, slice()?);
    Ok(())
}

/// Positive: the graph vocabulary carries the three edges, fully described.
#[test]
fn the_graph_vocabulary_describes_every_edge() {
    assert_eq!(EdgeId::ALL.len(), 3);
    for edge in EdgeId::ALL {
        assert!(!edge.name().is_empty());
        assert!(!edge.peer().is_empty());
        assert!(!edge.recorded_transport().is_empty());
    }
    assert_eq!(EdgeId::SpatiotemporalTaskShift.peer(), "P07");
    assert_eq!(EdgeId::EvaluateCandidateCarbonSci.peer(), "P16");
    assert_eq!(EdgeId::EmitCarbonTelemetry.peer(), "P05");
    assert_eq!(
        EdgeId::SpatiotemporalTaskShift.recorded_transport(),
        "D-Bus / kepler_power.bpf"
    );
}

/// Positive: the vocabulary states which edges this milestone types.
///
/// The third edge has a variant and no schema, which is the honest reading:
/// the graph of record carries it and M16 carries its payload.
#[test]
fn the_vocabulary_states_which_edges_are_typed_here() {
    assert!(EdgeId::SpatiotemporalTaskShift.typed_at_m05());
    assert!(EdgeId::EvaluateCandidateCarbonSci.typed_at_m05());
    assert!(
        !EdgeId::EmitCarbonTelemetry.typed_at_m05(),
        "the P05 telemetry payload is milestone M16, not this one"
    );
}

// --- Negative -------------------------------------------------------------

/// Negative: a malformed directive payload is refused.
#[test]
fn a_malformed_directive_is_refused() -> Fallible {
    let payload = directive()?;
    let mut buffer = PayloadBuffer::new();
    let text = payload.encode_into(&mut buffer)?.to_owned();

    assert!(matches!(
        TaskShiftDirective::decode(&text.replace(
            SchemaId::TaskShiftDirective.tag(),
            "aegis.p13-p07.task-shift.v9"
        )),
        Err(ContractError::UnknownVersion { .. })
    ));

    for bad in [
        "{}",
        "still not json",
        &text.replace("\"correlation-id\"", "\"correlationId\""),
        &text.replace(&format!("\"{SLICE}\""), "\"build slice\""),
        &text.replace(&format!("\"{SLICE}\""), "\"\""),
        &text.replace("\"intensity\":220.0", "\"intensity\":-5.0"),
        &text.replace("\"intensity\":220.0", "\"intensity\":\"220\""),
    ] {
        assert!(
            TaskShiftDirective::decode(bad).is_err(),
            "the decoder accepted {bad}"
        );
    }
    Ok(())
}

/// Negative: a directive naming another edge is refused.
#[test]
fn a_directive_naming_another_edge_is_refused() -> Fallible {
    let mut payload = directive()?;
    payload.edge = EdgeId::EmitCarbonTelemetry;
    let refusal = payload.validate();
    assert!(matches!(
        refusal,
        Err(ContractError::WrongEdge {
            found: "EMIT_CARBON_TELEMETRY",
            ..
        })
    ));
    Ok(())
}

/// Negative: a directive whose verdict does not follow from its own numbers is
/// refused rather than obeyed.
#[test]
fn a_directive_that_contradicts_itself_is_refused() -> Fallible {
    let mut lying = directive()?;
    lying.defer = true;
    assert!(
        matches!(lying.validate(), Err(ContractError::Malformed { .. })),
        "a deferral at 220 gCO2eq/kWh does not follow from the threshold"
    );

    let mut other = directive()?;
    other.intensity = GridIntensity::new(500.0)?;
    other.defer = false;
    assert!(
        matches!(other.validate(), Err(ContractError::Malformed { .. })),
        "no deferral at 500 gCO2eq/kWh does not follow either"
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a directive at exactly the threshold must not defer, and one just
/// above it must.
#[test]
fn the_directive_threshold_matches_the_engine_threshold() -> Fallible {
    let mut at_threshold = directive()?;
    at_threshold.intensity = GridIntensity::AT_THRESHOLD;
    at_threshold.defer = false;
    at_threshold.validate()?;

    let mut also = directive()?;
    also.intensity = GridIntensity::AT_THRESHOLD;
    also.defer = true;
    assert!(
        also.validate().is_err(),
        "exactly the threshold is not a deferral, on the wire as in the engine"
    );

    let mut above = directive()?;
    above.intensity = GridIntensity::new(300.001)?;
    above.defer = true;
    above.validate()?;
    assert!((TaskShiftDirective::THRESHOLD - 300.0).abs() < f64::EPSILON);
    Ok(())
}

/// Boundary: a slice name at the identifier bound is carried, and one past it
/// is refused.
#[test]
fn the_slice_name_turns_at_the_identifier_bound() -> Fallible {
    let longest = "a".repeat(aegis_tellus::MAX_IDENTIFIER_LEN);
    let name = SliceName::parse(&longest)?;
    assert_eq!(name.len(), aegis_tellus::MAX_IDENTIFIER_LEN);
    assert!(!name.is_empty());
    assert_eq!(format!("{name}"), longest);
    assert_eq!(name.as_bytes().len(), aegis_tellus::MAX_IDENTIFIER_LEN);

    let too_long = "a".repeat(aegis_tellus::MAX_IDENTIFIER_LEN.saturating_add(1));
    assert!(SliceName::parse(&too_long).is_err());
    assert!(SliceName::parse("").is_err());

    let mut payload = directive()?;
    payload.slice = name;
    let mut buffer = PayloadBuffer::new();
    let text = payload.encode_into(&mut buffer)?;
    assert_eq!(TaskShiftDirective::decode(text)?, payload);
    Ok(())
}
