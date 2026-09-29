// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E16-2, the P13-to-P05 edge: the carbon telemetry update (milestone M16).
//!
//! The encoding is pinned by golden strings in this file: the Forum shell
//! (`crates/aegis-forum-shell`) decodes this Rust type directly (D101), so no
//! fixture file, schema file or copy in a second language exists to drift, and
//! the goldens only hold the wire spelling still. The update is the `params`
//! of the one line-delimited JSON-RPC 2.0 message D77 frames the edge in; no
//! socket and no D-Bus connection are opened here.

mod common;

use aegis_tellus::{
    CarbonTelemetry, CarbonTelemetryVersion, ContractError, Correlation, CorrelationId, EdgeId,
    GridIntensity, MAX_CONTRACT_PAYLOAD_BYTES, PayloadBuffer, Provenance, SchemaId, SciRate, Watts,
};

use common::{EPSILON, Fallible};

/// The correlation identifier the telemetry goldens carry.
const TELEMETRY_CORRELATION: &str = "m16-telemetry-0001";

/// A contract version this build does not admit.
const UNADMITTED: &str = "aegis.p13-p05.carbon-telemetry.v2";

/// The scaffold status bar's draw (export-043), in watts.
const SCAFFOLD_WATTS: f64 = 14.2;

/// The scaffold benchmark's SCI rate, and with no energy the embodied share alone.
const SCAFFOLD_RATE: f64 = 0.6;
const EMBODIED_ONLY_RATE: f64 = 0.05;

/// The exact payload the scaffold update encodes to.
const GOLDEN: &str = r#"{"schema":"aegis.p13-p05.carbon-telemetry.v1","edge":"EMIT_CARBON_TELEMETRY","correlation-id":"m16-telemetry-0001","power-watts":14.2,"sci-rate":0.6,"grid-intensity":220.0,"provenance":"simulated"}"#;

/// The exact payload a zero-watt update encodes to.
const GOLDEN_ZERO_WATTS: &str = r#"{"schema":"aegis.p13-p05.carbon-telemetry.v1","edge":"EMIT_CARBON_TELEMETRY","correlation-id":"m16-telemetry-0001","power-watts":0.0,"sci-rate":0.05,"grid-intensity":220.0,"provenance":"simulated"}"#;

/// The scaffold update with only its version tag changed.
const GOLDEN_UNKNOWN_VERSION: &str = r#"{"schema":"aegis.p13-p05.carbon-telemetry.v2","edge":"EMIT_CARBON_TELEMETRY","correlation-id":"m16-telemetry-0001","power-watts":14.2,"sci-rate":0.6,"grid-intensity":220.0,"provenance":"simulated"}"#;

/// A simulated update at the scaffold's grid intensity.
fn update(watts: f64, rate: f64) -> Result<CarbonTelemetry, Box<dyn std::error::Error>> {
    Ok(CarbonTelemetry::new(
        CorrelationId::parse(TELEMETRY_CORRELATION)?,
        Watts::new(watts)?,
        SciRate::new(rate)?,
        GridIntensity::DEFAULT,
        Provenance::Simulated,
    ))
}

/// Encodes an update through the contract's own bounded encoder.
fn encoded(payload: &CarbonTelemetry) -> Result<String, Box<dyn std::error::Error>> {
    let mut buffer = PayloadBuffer::new();
    Ok(payload.encode_into(&mut buffer)?.to_owned())
}

/// The correlation a refusal of a fixture update must carry.
fn expected_correlation() -> Result<Correlation, Box<dyn std::error::Error>> {
    Ok(Correlation::new(
        SchemaId::CarbonTelemetry,
        Some(CorrelationId::parse(TELEMETRY_CORRELATION)?),
    ))
}

// --- Positive -------------------------------------------------------------

/// Positive: an update round-trips and is the golden payload, byte for byte.
#[test]
fn the_update_is_the_golden_payload_byte_for_byte() -> Fallible {
    let payload = update(SCAFFOLD_WATTS, SCAFFOLD_RATE)?;
    let text = encoded(&payload)?;
    assert_eq!(text, GOLDEN);
    let decoded = CarbonTelemetry::decode(&text)?;
    assert_eq!(decoded, payload);
    assert_eq!(decoded.correlation(), expected_correlation()?);
    assert_eq!(decoded.correlation_id.to_string(), TELEMETRY_CORRELATION);
    assert!((decoded.power_watts.get() - SCAFFOLD_WATTS).abs() < EPSILON);
    assert!((decoded.sci_rate.get() - SCAFFOLD_RATE).abs() < EPSILON);
    assert_eq!(decoded.grid_intensity, GridIntensity::DEFAULT);
    Ok(())
}

/// Positive: the update is version 1 of its schema, on the graph's edge, and
/// says its draw is simulated.
#[test]
fn the_update_names_its_version_edge_and_provenance() -> Fallible {
    let payload = update(SCAFFOLD_WATTS, SCAFFOLD_RATE)?;
    assert_eq!(payload.schema, CarbonTelemetryVersion::V1);
    assert_eq!(payload.edge, EdgeId::EmitCarbonTelemetry);
    assert_eq!(payload.provenance, Provenance::Simulated);
    assert_eq!(payload.validate(), Ok(()));
    assert_eq!(CarbonTelemetry::SCHEMA, SchemaId::CarbonTelemetry);
    assert_eq!(CarbonTelemetry::EDGE, EdgeId::EmitCarbonTelemetry);
    assert_eq!(
        SchemaId::CarbonTelemetry.tag(),
        "aegis.p13-p05.carbon-telemetry.v1"
    );
    assert_eq!(
        SchemaId::CarbonTelemetry.edge(),
        EdgeId::EmitCarbonTelemetry
    );
    Ok(())
}

/// Positive: every provenance travels by its stable name, so the shell can
/// label a simulated or modelled draw as what it is.
#[test]
fn every_provenance_round_trips_by_its_name() -> Fallible {
    for provenance in Provenance::ALL {
        let mut payload = update(SCAFFOLD_WATTS, SCAFFOLD_RATE)?;
        payload.provenance = provenance;
        let text = encoded(&payload)?;
        assert!(text.contains(&format!("\"provenance\":\"{}\"", provenance.name())));
        assert_eq!(CarbonTelemetry::decode(&text)?, payload);
    }
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: the unknown-version golden is the valid update with only its
/// version tag changed, and it is refused as an unknown version.
#[test]
fn the_unknown_version_payload_is_refused_as_such() -> Fallible {
    let text = encoded(&update(SCAFFOLD_WATTS, SCAFFOLD_RATE)?)?.replacen(
        SchemaId::CarbonTelemetry.tag(),
        UNADMITTED,
        1,
    );
    assert_eq!(text, GOLDEN_UNKNOWN_VERSION);
    assert_eq!(
        CarbonTelemetry::decode(&text),
        Err(ContractError::UnknownVersion {
            correlation: expected_correlation()?
        })
    );
    Ok(())
}

/// Negative: a negative draw, an unknown provenance or an unknown field is
/// refused by the decoder, with the payload's correlation.
#[test]
fn a_negative_draw_an_unknown_provenance_or_an_extra_field_is_malformed() -> Fallible {
    let valid = encoded(&update(SCAFFOLD_WATTS, SCAFFOLD_RATE)?)?;
    let malformed = ContractError::Malformed {
        correlation: expected_correlation()?,
    };
    for text in [
        valid.replacen("\"power-watts\":14.2", "\"power-watts\":-0.1", 1),
        valid.replacen("\"simulated\"", "\"guessed\"", 1),
        valid.replacen("\"provenance\"", "\"watts\":1.0,\"provenance\"", 1),
    ] {
        assert_ne!(text, valid);
        assert_eq!(CarbonTelemetry::decode(&text), Err(malformed));
    }
    Ok(())
}

/// Negative: an update that names another edge is refused, on the way out and
/// on the way in.
#[test]
fn an_update_on_another_edge_is_refused() -> Fallible {
    let mut payload = update(SCAFFOLD_WATTS, SCAFFOLD_RATE)?;
    payload.edge = EdgeId::SpatiotemporalTaskShift;
    let refusal = ContractError::WrongEdge {
        correlation: expected_correlation()?,
        found: "SPATIOTEMPORAL_TASK_SHIFT",
    };
    assert_eq!(payload.validate(), Err(refusal));
    let mut buffer = PayloadBuffer::new();
    assert_eq!(payload.encode_into(&mut buffer), Err(refusal));
    let valid = encoded(&update(SCAFFOLD_WATTS, SCAFFOLD_RATE)?)?;
    let moved = valid.replacen("EMIT_CARBON_TELEMETRY", "SPATIOTEMPORAL_TASK_SHIFT", 1);
    assert_eq!(CarbonTelemetry::decode(&moved), Err(refusal));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary (E16-2): a zero-watt update is admitted, is the golden payload,
/// and decodes from JSON's integer spelling of zero too.
#[test]
fn a_zero_watt_update_is_admitted_and_is_the_golden_payload() -> Fallible {
    let payload = update(0.0, EMBODIED_ONLY_RATE)?;
    let text = encoded(&payload)?;
    assert_eq!(text, GOLDEN_ZERO_WATTS);
    let decoded = CarbonTelemetry::decode(&text)?;
    assert_eq!(decoded, payload);
    assert_eq!(decoded.power_watts.get().to_bits(), 0.0_f64.to_bits());
    let integer = text.replacen("\"power-watts\":0.0", "\"power-watts\":0", 1);
    assert_ne!(integer, text);
    assert_eq!(CarbonTelemetry::decode(&integer)?, payload);
    Ok(())
}

/// Boundary: the draw bound is admitted and a draw past it is refused.
#[test]
fn the_draw_bound_is_admitted_and_one_past_it_refused() -> Fallible {
    let payload = update(Watts::MAX, SCAFFOLD_RATE)?;
    let text = encoded(&payload)?;
    assert_eq!(CarbonTelemetry::decode(&text)?, payload);
    let past = text.replacen("\"power-watts\":100000.0", "\"power-watts\":100000.5", 1);
    assert_ne!(past, text);
    assert_eq!(
        CarbonTelemetry::decode(&past),
        Err(ContractError::Malformed {
            correlation: expected_correlation()?
        })
    );
    Ok(())
}

/// Boundary: every golden is one line within the payload bound, so each fits
/// one JSON-RPC line as its `params`.
#[test]
fn every_golden_is_one_line_within_the_payload_bound() {
    for line in [GOLDEN, GOLDEN_ZERO_WATTS, GOLDEN_UNKNOWN_VERSION] {
        assert!(line.len() <= MAX_CONTRACT_PAYLOAD_BYTES);
        assert!(!line.contains('\n'));
        assert!(line.contains(TELEMETRY_CORRELATION));
    }
}
