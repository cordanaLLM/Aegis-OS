// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E17-3, the P15-to-P04 half: the overlay registration round-trips, a
//! malformed payload is rejected, and the extent bound is exact on the wire.
//!
//! The graph of record names this edge `REGISTER_PIP_OVERLAY`, carried by the
//! `wlr-layer-shell` overlay surface protocol. No transport is exercised: the
//! payload is text, and nothing here connects to a compositor.

mod common;

use aegis_hestia::{
    ContractError, EdgeId, MAX_CONTRACT_PAYLOAD_BYTES, MAX_PIXEL_EXTENT, OverlayRegistration,
    PayloadBuffer, SchemaId, ShellLayer,
};

use common::{CORRELATION, Fallible, SURFACE, correlation, registration, surface};

/// Returns the refusal a payload produced, or a message when it decoded.
fn refusal(text: &str) -> Result<ContractError, Box<dyn std::error::Error>> {
    match OverlayRegistration::decode(text) {
        Ok(_) => Err("the payload decoded and was expected to be refused".into()),
        Err(error) => Ok(error),
    }
}

// --- Positive -------------------------------------------------------------

/// Positive: a registration round-trips through its own encoding.
#[test]
fn an_overlay_registration_round_trips() -> Fallible {
    let payload = registration()?;
    let mut buffer = PayloadBuffer::new();
    let text = payload.encode_into(&mut buffer)?.to_owned();
    assert_eq!(OverlayRegistration::decode(&text)?, payload);
    assert_eq!(payload.as_surface(), surface()?);
    assert_eq!(buffer.len(), text.len());
    assert!(!buffer.is_empty());
    Ok(())
}

/// Positive: the payload names its contract, its edge and its correlation.
#[test]
fn the_payload_names_its_contract_and_edge() -> Fallible {
    let payload = registration()?;
    let mut buffer = PayloadBuffer::new();
    let text = payload.encode_into(&mut buffer)?.to_owned();
    assert!(text.starts_with("{\"schema\":\"aegis.p15-p04.overlay-registration.v1\""));
    assert!(text.contains("\"edge\":\"REGISTER_PIP_OVERLAY\""));
    assert!(text.contains("\"layer\":\"overlay\""));
    assert!(text.contains(CORRELATION));
    assert!(text.contains(SURFACE));
    Ok(())
}

/// Positive: the schema and the edge agree about what this contract is.
#[test]
fn the_schema_and_the_edge_agree() {
    assert_eq!(OverlayRegistration::SCHEMA, SchemaId::OverlayRegistration);
    assert_eq!(OverlayRegistration::EDGE, EdgeId::RegisterPipOverlay);
    assert_eq!(OverlayRegistration::LAYER, ShellLayer::Overlay);
    assert_eq!(
        SchemaId::OverlayRegistration.edge(),
        EdgeId::RegisterPipOverlay
    );
    assert_eq!(
        SchemaId::OverlayRegistration.to_string(),
        SchemaId::OverlayRegistration.tag()
    );
    assert_eq!(EdgeId::RegisterPipOverlay.name(), "REGISTER_PIP_OVERLAY");
    assert_eq!(EdgeId::RegisterPipOverlay.consumer(), "P04");
    assert_eq!(
        EdgeId::RegisterPipOverlay.recorded_transport(),
        "wlr-layer-shell Overlay Surface Protocol"
    );
    assert_eq!(EdgeId::ALL.len(), 1);
}

/// Positive: a refusal carries the correlation the payload declared.
#[test]
fn a_refusal_is_correlated_to_its_payload() -> Fallible {
    let error = refusal(
        "{\"schema\":\"aegis.p15-p04.overlay-registration.v9\",\
         \"correlation-id\":\"m17-fixture-0001\"}",
    )?;
    let about = error.correlation();
    assert_eq!(about.schema(), SchemaId::OverlayRegistration);
    assert_eq!(about.id(), Some(correlation()?));
    assert!(matches!(error, ContractError::UnknownVersion { .. }));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a malformed payload is rejected rather than parsed leniently.
#[test]
fn a_malformed_registration_payload_is_rejected() -> Fallible {
    for text in [
        "",
        "{}",
        "null",
        "{\"schema\":\"aegis.p15-p04.overlay-registration.v1\"}",
    ] {
        assert!(OverlayRegistration::decode(text).is_err());
    }
    let payload = registration()?;
    let mut buffer = PayloadBuffer::new();
    let text = payload.encode_into(&mut buffer)?.to_owned();

    let extra = text.replace("{\"schema\"", "{\"stowaway\":1,\"schema\"");
    assert!(matches!(refusal(&extra)?, ContractError::Malformed { .. }));

    let negative = text.replace("\"dma-buf\":7", "\"dma-buf\":-1");
    assert!(matches!(
        refusal(&negative)?,
        ContractError::Malformed { .. }
    ));
    Ok(())
}

/// Positive: a well-formed registration validates on its own.
///
/// `validate` is what `encode_into` and `decode` both run, so calling it
/// directly is what separates "the payload is well formed" from "the encoder
/// happened to accept it".
#[test]
fn a_well_formed_registration_validates() -> Fallible {
    let payload = registration()?;
    assert_eq!(payload.validate(), Ok(()));
    Ok(())
}

/// Positive: the edge needs no check, because the type admits only one.
///
/// P15 produces on exactly one edge, so there is no wrong-edge refusal to
/// make: a payload naming another edge does not decode at all. This case is
/// what would have to change if P15 gained a second outbound edge.
#[test]
fn the_edge_is_guarded_by_its_type_rather_than_by_a_check() -> Fallible {
    assert_eq!(EdgeId::ALL, [EdgeId::RegisterPipOverlay]);
    let mut payload = registration()?;
    payload.edge = EdgeId::RegisterPipOverlay;
    assert_eq!(payload.validate(), Ok(()));
    Ok(())
}

/// Negative: a registration on another layer is refused by name.
#[test]
fn a_registration_on_another_layer_is_rejected() -> Fallible {
    let payload = registration()?;
    let mut buffer = PayloadBuffer::new();
    let text = payload
        .encode_into(&mut buffer)?
        .replace("\"layer\":\"overlay\"", "\"layer\":\"top\"");
    assert!(matches!(refusal(&text)?, ContractError::WrongLayer { .. }));

    let mut wrong = payload;
    wrong.layer = ShellLayer::Background;
    let mut second = PayloadBuffer::new();
    assert!(matches!(
        wrong.encode_into(&mut second),
        Err(ContractError::WrongLayer { .. })
    ));
    Ok(())
}

/// Negative: a payload asserting an unknown edge does not decode.
#[test]
fn a_payload_on_an_unknown_edge_is_rejected() -> Fallible {
    let payload = registration()?;
    let mut buffer = PayloadBuffer::new();
    let text = payload
        .encode_into(&mut buffer)?
        .replace("REGISTER_PIP_OVERLAY", "ZERO_COPY_MEDIA_INGEST");
    assert!(matches!(refusal(&text)?, ContractError::Malformed { .. }));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: an extent at the bound is accepted on the wire, one over is not.
#[test]
fn the_wire_extent_bound_is_exact() -> Fallible {
    let mut payload = registration()?;
    payload.width = aegis_hestia::PixelExtent::MAX;
    let mut buffer = PayloadBuffer::new();
    let text = payload.encode_into(&mut buffer)?.to_owned();
    assert!(text.contains("\"width\":16384"));
    assert_eq!(OverlayRegistration::decode(&text)?, payload);

    for over in ["\"width\":16385", "\"width\":0"] {
        let bad = text.replace("\"width\":16384", over);
        assert!(matches!(refusal(&bad)?, ContractError::Malformed { .. }));
    }
    assert_eq!(MAX_PIXEL_EXTENT, 16384);
    Ok(())
}

/// Boundary: a payload past the byte bound is refused before it is parsed.
#[test]
fn a_payload_past_the_byte_bound_is_refused_unparsed() -> Fallible {
    let padding = "y".repeat(MAX_CONTRACT_PAYLOAD_BYTES.saturating_add(1));
    match refusal(&padding)? {
        ContractError::PayloadTooLong { max, correlation } => {
            assert_eq!(max, MAX_CONTRACT_PAYLOAD_BYTES);
            assert_eq!(correlation.id(), None);
            assert!(correlation.to_string().ends_with("[uncorrelated]"));
        }
        other => return Err(format!("expected a length refusal, got {other:?}").into()),
    }
    Ok(())
}
