// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E17-3, the P03-to-P09 half: the weight-streaming descriptor
//! round-trips, a malformed payload is rejected, and the block-count boundary
//! is exact on the wire.
//!
//! REQ-P03-07 and REQ-P09-04 are the same edge read from both ends. No
//! transport is exercised: the payload is text, and nothing here opens a
//! device.

mod common;

use aegis_vulcan::{
    ContractError, EdgeId, MAX_CONTRACT_PAYLOAD_BYTES, PayloadBuffer, SchemaId,
    WeightStreamDescriptor,
};

use common::{CORRELATION, Fallible, correlation, weight_stream};

/// Returns the refusal a payload produced, or a message when it decoded.
fn refusal(text: &str) -> Result<ContractError, Box<dyn std::error::Error>> {
    match WeightStreamDescriptor::decode(text) {
        Ok(_) => Err("the payload decoded and was expected to be refused".into()),
        Err(error) => Ok(error),
    }
}

// --- Positive -------------------------------------------------------------

/// Positive: a descriptor round-trips through its own encoding.
#[test]
fn a_weight_stream_descriptor_round_trips() -> Fallible {
    let descriptor = weight_stream(64)?;
    let mut buffer = PayloadBuffer::new();
    let text = descriptor.encode_into(&mut buffer)?.to_owned();
    assert_eq!(WeightStreamDescriptor::decode(&text)?, descriptor);
    assert_eq!(descriptor.transfer_bytes(), 64 * 512);
    assert!(!buffer.is_empty());
    assert_eq!(buffer.len(), text.len());
    Ok(())
}

/// Positive: the payload names its contract, its edge and its correlation.
#[test]
fn the_payload_names_its_contract_and_edge() -> Fallible {
    let descriptor = weight_stream(1)?;
    let mut buffer = PayloadBuffer::new();
    let text = descriptor.encode_into(&mut buffer)?.to_owned();
    assert!(text.starts_with("{\"schema\":\"aegis.p03-p09.weight-stream.v1\""));
    assert!(text.contains("\"edge\":\"GPUDIRECT_WEIGHT_STREAMING\""));
    assert!(text.contains(CORRELATION));
    Ok(())
}

/// Positive: the schema and the edge agree about what this contract is.
#[test]
fn the_schema_and_the_edge_agree() {
    assert_eq!(WeightStreamDescriptor::SCHEMA, SchemaId::WeightStream);
    assert_eq!(
        WeightStreamDescriptor::EDGE,
        EdgeId::GpudirectWeightStreaming
    );
    assert_eq!(
        SchemaId::WeightStream.edge(),
        EdgeId::GpudirectWeightStreaming
    );
    assert_eq!(
        SchemaId::WeightStream.to_string(),
        SchemaId::WeightStream.tag()
    );
    assert_eq!(
        EdgeId::GpudirectWeightStreaming.name(),
        "GPUDIRECT_WEIGHT_STREAMING"
    );
    assert_eq!(EdgeId::GpudirectWeightStreaming.consumer(), "P09");
    assert_eq!(
        EdgeId::GpudirectWeightStreaming.recorded_transport(),
        "PCIe P2PDMA / CUDA GPUDirect"
    );
    assert_eq!(EdgeId::ALL.len(), 2);
}

/// Positive: a refusal carries the correlation the payload declared.
#[test]
fn a_refusal_is_correlated_to_its_payload() -> Fallible {
    let error = refusal(
        "{\"schema\":\"aegis.p03-p09.weight-stream.v9\",\"correlation-id\":\"m17-fixture-0001\"}",
    )?;
    let about = error.correlation();
    assert_eq!(about.schema(), SchemaId::WeightStream);
    assert_eq!(about.id(), Some(correlation()?));
    assert!(error.to_string().contains(CORRELATION));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a malformed payload is rejected rather than parsed leniently.
#[test]
fn a_malformed_weight_stream_payload_is_rejected() -> Fallible {
    for text in [
        "",
        "{}",
        "not json at all",
        "{\"schema\":\"aegis.p03-p09.weight-stream.v1\"}",
    ] {
        assert!(WeightStreamDescriptor::decode(text).is_err());
    }
    let descriptor = weight_stream(8)?;
    let mut buffer = PayloadBuffer::new();
    let text = descriptor.encode_into(&mut buffer)?.to_owned();

    let extra = text.replace("{\"schema\"", "{\"stowaway\":1,\"schema\"");
    assert!(matches!(refusal(&extra)?, ContractError::Malformed { .. }));

    let renamed = text.replace(
        "aegis.p03-p09.weight-stream.v1",
        "aegis.p03-p09.weight-stream.v2",
    );
    assert!(matches!(
        refusal(&renamed)?,
        ContractError::UnknownVersion { .. }
    ));
    Ok(())
}

/// Negative: a payload asserting the other P03 edge does not decode.
#[test]
fn a_payload_on_the_other_p03_edge_is_rejected() -> Fallible {
    let descriptor = weight_stream(8)?;
    let mut buffer = PayloadBuffer::new();
    let text = descriptor
        .encode_into(&mut buffer)?
        .replace("GPUDIRECT_WEIGHT_STREAMING", "ZERO_COPY_MEDIA_INGEST");
    assert!(matches!(refusal(&text)?, ContractError::WrongEdge { .. }));

    let mut wrong = descriptor;
    wrong.edge = EdgeId::ZeroCopyMediaIngest;
    let mut second = PayloadBuffer::new();
    assert!(matches!(
        wrong.encode_into(&mut second),
        Err(ContractError::WrongEdge { .. })
    ));
    Ok(())
}

/// Negative: an unusable export path is refused with a named reason.
#[test]
fn an_unusable_export_path_is_rejected() -> Fallible {
    let mut descriptor = weight_stream(8)?;
    descriptor.export.modeset = aegis_vulcan::ModesetState::Disabled;
    let mut buffer = PayloadBuffer::new();
    match descriptor.encode_into(&mut buffer) {
        Err(ContractError::ExportPathUnusable { reason, .. }) => {
            assert!(reason.contains("modesetting"));
        }
        other => return Err(format!("expected an export refusal, got {other:?}").into()),
    }
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a descriptor at the maximum block count is accepted on the wire,
/// and one block over is rejected by the decoder.
#[test]
fn the_wire_block_count_bound_is_exact() -> Fallible {
    let descriptor = weight_stream(8192)?;
    let mut buffer = PayloadBuffer::new();
    let text = descriptor.encode_into(&mut buffer)?.to_owned();
    assert!(text.contains("\"blocks\":8192"));
    assert_eq!(WeightStreamDescriptor::decode(&text)?, descriptor);

    for over in ["\"blocks\":8193", "\"blocks\":0"] {
        let bad = text.replace("\"blocks\":8192", over);
        assert!(matches!(refusal(&bad)?, ContractError::Malformed { .. }));
    }
    Ok(())
}

/// Boundary: a payload past the byte bound is refused before it is parsed.
#[test]
fn a_payload_past_the_byte_bound_is_refused_unparsed() -> Fallible {
    let padding = "x".repeat(MAX_CONTRACT_PAYLOAD_BYTES.saturating_add(1));
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
