// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E08-3, the P14-to-P15 half: the geometry viewport descriptor.
//!
//! Positive: the descriptor round-trips. Negative: a malformed payload is
//! rejected. Boundary: **a viewport descriptor at the mesh bound is accepted**,
//! and one past it does not decode -- the same bound the planner holds, carried
//! by the same type.

mod common;

use aegis_hephaestus::{
    CadEngine, ContractError, EdgeId, GeometryViewport, GeometryViewportVersion,
    MAX_CONTRACT_PAYLOAD_BYTES, MAX_MESH_ELEMENTS, PayloadBuffer, SchemaId, StepSchema, Tolerance,
};

use common::{Fallible, encoded_viewport, scaffold_viewport, tamper, viewport};

// --- Positive -------------------------------------------------------------

/// Positive: the descriptor round-trips with every recorded field intact.
#[test]
fn the_descriptor_round_trips() -> Fallible {
    let original = scaffold_viewport()?;
    let text = encoded_viewport(&original)?;
    let decoded = GeometryViewport::decode(&text)?;
    assert_eq!(decoded, original);
    assert_eq!(decoded.schema, GeometryViewportVersion::V1);
    assert_eq!(decoded.edge, EdgeId::RenderGeometryMicrofrontend);
    assert_eq!(decoded.step_schema, StepSchema::Ap242);
    assert_eq!(decoded.engine, CadEngine::ExactNurbsBrep);
    Ok(())
}

/// Positive: the caller-supplied fields survive the round trip too.
#[test]
fn the_caller_supplied_fields_round_trip() -> Fallible {
    let text = encoded_viewport(&scaffold_viewport()?)?;
    let decoded = GeometryViewport::decode(&text)?;
    assert_eq!(decoded.elements.get(), common::SCAFFOLD_ELEMENTS);
    assert_eq!(decoded.tolerance_micrometres, Tolerance::SCAFFOLD);
    assert_eq!(decoded.source, common::step_source()?);
    assert_eq!(decoded.correlation_id, common::correlation()?);
    assert_eq!(decoded.prepared_at.get(), 2_000);
    Ok(())
}

/// Positive: the wire form names the edge, the protocol and the element count
/// as read.
#[test]
fn the_wire_form_names_the_edge_and_the_protocol() -> Fallible {
    let text = encoded_viewport(&scaffold_viewport()?)?;
    assert!(text.contains("\"edge\":\"RENDER_GEOMETRY_MICROFRONTEND\""));
    assert!(text.contains("\"schema\":\"aegis.p14-p15.geometry-viewport.v1\""));
    assert!(text.contains("\"step-schema\":\"AP242\""));
    assert!(text.contains("\"engine\":\"exact-nurbs-brep\""));
    assert!(text.contains("\"elements\":120000"));
    assert!(text.contains("\"tolerance-micrometres\":1"));
    Ok(())
}

/// Positive: the schema and the payload's own constants agree.
#[test]
fn the_schema_and_the_payload_constants_agree() -> Fallible {
    assert_eq!(GeometryViewport::SCHEMA, SchemaId::GeometryViewport);
    assert_eq!(SchemaId::GeometryViewport.edge(), GeometryViewport::EDGE);
    assert_eq!(
        SchemaId::GeometryViewport.tag(),
        "aegis.p14-p15.geometry-viewport.v1"
    );
    assert_eq!(
        SchemaId::GeometryViewport.to_string(),
        SchemaId::GeometryViewport.tag()
    );
    assert_eq!(GeometryViewport::STEP_SCHEMA, StepSchema::ADMITTED);

    let correlation = scaffold_viewport()?.correlation();
    assert_eq!(correlation.schema(), GeometryViewport::SCHEMA);
    assert_eq!(correlation.id(), Some(common::correlation()?));
    assert!(correlation.to_string().contains(common::CORRELATION));
    Ok(())
}

/// Positive: the edge is the graph of record's, carried on its single source.
#[test]
fn the_edge_is_the_graph_of_records() {
    assert_eq!(EdgeId::ALL, [EdgeId::RenderGeometryMicrofrontend]);
    assert_eq!(
        EdgeId::RenderGeometryMicrofrontend.name(),
        "RENDER_GEOMETRY_MICROFRONTEND"
    );
    assert_eq!(EdgeId::RenderGeometryMicrofrontend.consumer(), "P15");
    assert_eq!(
        EdgeId::RenderGeometryMicrofrontend.recorded_transport(),
        "Svelte 5 Micro-Frontend / PGlite OPFS"
    );
    assert_eq!(EdgeId::RenderGeometryMicrofrontend.sources(), 1);
}

// --- Negative -------------------------------------------------------------

/// Negative: a payload naming the wrong contract, edge or field is rejected.
#[test]
fn a_payload_naming_the_wrong_contract_is_rejected() -> Fallible {
    let text = encoded_viewport(&scaffold_viewport()?)?;

    let other_version = tamper(
        &text,
        "aegis.p14-p15.geometry-viewport.v1",
        "aegis.p14-p15.geometry-viewport.v2",
    )?;
    assert!(matches!(
        GeometryViewport::decode(&other_version),
        Err(ContractError::UnknownVersion { .. })
    ));

    let other_edge = tamper(&text, "RENDER_GEOMETRY_MICROFRONTEND", "VERIFY_CODE_CAD")?;
    assert!(matches!(
        GeometryViewport::decode(&other_edge),
        Err(ContractError::Malformed { .. })
    ));

    let unknown_field = tamper(&text, "{", "{\"triangles\":12,")?;
    assert!(matches!(
        GeometryViewport::decode(&unknown_field),
        Err(ContractError::Malformed { .. })
    ));
    Ok(())
}

/// Negative: a payload carrying an out-of-range or malformed field value is
/// refused by the decoder, not accepted and checked afterwards.
#[test]
fn a_payload_carrying_a_bad_field_value_is_rejected() -> Fallible {
    let text = encoded_viewport(&scaffold_viewport()?)?;

    let other_protocol = tamper(&text, "\"AP242\"", "\"AP203\"")?;
    assert!(matches!(
        GeometryViewport::decode(&other_protocol),
        Err(ContractError::Malformed { .. })
    ));

    let bad_source = tamper(&text, "bracket.step", "bracket file.step")?;
    assert!(matches!(
        GeometryViewport::decode(&bad_source),
        Err(ContractError::Malformed { .. })
    ));

    let bad_tolerance = tamper(
        &text,
        "\"tolerance-micrometres\":1",
        "\"tolerance-micrometres\":0",
    )?;
    assert!(matches!(
        GeometryViewport::decode(&bad_tolerance),
        Err(ContractError::Malformed { .. })
    ));
    Ok(())
}

/// Negative: a descriptor naming the engine that does not read the protocol it
/// claims is refused, in memory and on the wire.
#[test]
fn an_engine_that_cannot_read_the_protocol_is_refused() -> Fallible {
    let mismatched = viewport(common::SCAFFOLD_ELEMENTS, CadEngine::PolyhedralCsg)?;
    assert!(matches!(
        mismatched.validate(),
        Err(ContractError::EngineProtocolMismatch { .. })
    ));
    let mut buffer = PayloadBuffer::new();
    assert!(matches!(
        mismatched.encode_into(&mut buffer),
        Err(ContractError::EngineProtocolMismatch { .. })
    ));
    assert!(buffer.is_empty());

    let text = encoded_viewport(&scaffold_viewport()?)?;
    let swapped = tamper(&text, "exact-nurbs-brep", "polyhedral-csg")?;
    let refusal = GeometryViewport::decode(&swapped);
    assert!(matches!(
        refusal,
        Err(ContractError::EngineProtocolMismatch { .. })
    ));
    let Err(error) = refusal else {
        return Err("a mismatched engine must be refused".into());
    };
    assert_eq!(error.correlation().id(), Some(common::correlation()?));
    Ok(())
}

/// Negative: a payload past the byte bound is refused before it is parsed.
#[test]
fn a_payload_past_the_byte_bound_is_refused() {
    let text = "x".repeat(MAX_CONTRACT_PAYLOAD_BYTES + 1);
    let refusal = GeometryViewport::decode(&text);
    assert!(matches!(
        refusal,
        Err(ContractError::PayloadTooLong {
            max: MAX_CONTRACT_PAYLOAD_BYTES,
            ..
        })
    ));
    let Err(error) = refusal else {
        return;
    };
    assert_eq!(error.correlation().id(), None);
    assert!(error.correlation().to_string().contains("uncorrelated"));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a viewport descriptor at the mesh bound is accepted, and one past
/// it does not decode.
///
/// The element count carries the planner's own type, so the bound on the wire
/// and the bound in `BrepEvaluator::mesh` are the same bound rather than two
/// copies of a number.
#[test]
fn a_descriptor_at_the_mesh_bound_is_accepted() -> Fallible {
    let at_bound = viewport(MAX_MESH_ELEMENTS, CadEngine::ExactNurbsBrep)?;
    let text = encoded_viewport(&at_bound)?;
    assert!(text.contains("\"elements\":500000"));
    let decoded = GeometryViewport::decode(&text)?;
    assert_eq!(decoded.elements.get(), MAX_MESH_ELEMENTS);

    let over = tamper(&text, "\"elements\":500000", "\"elements\":500001")?;
    assert!(matches!(
        GeometryViewport::decode(&over),
        Err(ContractError::Malformed { .. })
    ));
    assert!(viewport(MAX_MESH_ELEMENTS + 1, CadEngine::ExactNurbsBrep).is_err());
    Ok(())
}

/// Boundary: an empty mesh is a descriptor, and the payload at either end of
/// the element range stays inside the byte bound.
#[test]
fn the_element_range_stays_inside_the_payload_bound() -> Fallible {
    let empty = viewport(0, CadEngine::ExactNurbsBrep)?;
    let text = encoded_viewport(&empty)?;
    assert!(text.contains("\"elements\":0"));
    assert_eq!(GeometryViewport::decode(&text)?.elements.get(), 0);

    for elements in [0, common::SCAFFOLD_ELEMENTS, MAX_MESH_ELEMENTS] {
        let payload = encoded_viewport(&viewport(elements, CadEngine::ExactNurbsBrep)?)?;
        assert!(payload.len() < MAX_CONTRACT_PAYLOAD_BYTES);
        assert!(payload.len().saturating_mul(2) < MAX_CONTRACT_PAYLOAD_BYTES);
    }
    Ok(())
}
