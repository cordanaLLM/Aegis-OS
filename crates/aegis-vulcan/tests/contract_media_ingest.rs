// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E17-3, the P03-to-P15 half: the media-ingest descriptor round-trips, a
//! malformed payload is rejected, and the block-count boundary is exact on the
//! wire.
//!
//! REQ-P15-07 is the edge. The `BAR` window travels in the payload because the
//! graph of record names a `PCIe` `BAR` memory mapping as the transport, so a
//! misaligned address is refused by the decoder rather than by the consumer.
//! No transport is exercised: the payload is text, and nothing here maps
//! anything.

mod common;

use aegis_vulcan::{
    ContractError, EdgeId, MediaIngestDescriptor, ModesetState, PayloadBuffer, SchemaId,
};

use common::{ALIGNED_BAR, CORRELATION, Fallible, media_ingest};

/// Returns the refusal a payload produced, or a message when it decoded.
fn refusal(text: &str) -> Result<ContractError, Box<dyn std::error::Error>> {
    match MediaIngestDescriptor::decode(text) {
        Ok(_) => Err("the payload decoded and was expected to be refused".into()),
        Err(error) => Ok(error),
    }
}

// --- Positive -------------------------------------------------------------

/// Positive: a descriptor round-trips through its own encoding.
#[test]
fn a_media_ingest_descriptor_round_trips() -> Fallible {
    let descriptor = media_ingest(64)?;
    let mut buffer = PayloadBuffer::new();
    let text = descriptor.encode_into(&mut buffer)?.to_owned();
    assert_eq!(MediaIngestDescriptor::decode(&text)?, descriptor);
    assert_eq!(descriptor.transfer_bytes(), 64 * 512);
    assert_eq!(descriptor.bar.address.get(), ALIGNED_BAR);
    Ok(())
}

/// Positive: the payload names its contract, its edge and its correlation.
#[test]
fn the_payload_names_its_contract_and_edge() -> Fallible {
    let descriptor = media_ingest(1)?;
    let mut buffer = PayloadBuffer::new();
    let text = descriptor.encode_into(&mut buffer)?.to_owned();
    assert!(text.starts_with("{\"schema\":\"aegis.p03-p15.media-ingest.v1\""));
    assert!(text.contains("\"edge\":\"ZERO_COPY_MEDIA_INGEST\""));
    assert!(text.contains(CORRELATION));
    assert_eq!(MediaIngestDescriptor::SCHEMA, SchemaId::MediaIngest);
    assert_eq!(MediaIngestDescriptor::EDGE, EdgeId::ZeroCopyMediaIngest);
    assert_eq!(SchemaId::MediaIngest.edge(), EdgeId::ZeroCopyMediaIngest);
    assert_eq!(EdgeId::ZeroCopyMediaIngest.name(), "ZERO_COPY_MEDIA_INGEST");
    assert_eq!(EdgeId::ZeroCopyMediaIngest.consumer(), "P15");
    assert_eq!(
        EdgeId::ZeroCopyMediaIngest.recorded_transport(),
        "PCIe BAR / Memory Mapping"
    );
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a malformed payload is rejected rather than parsed leniently.
#[test]
fn a_malformed_media_ingest_payload_is_rejected() -> Fallible {
    for text in [
        "",
        "{}",
        "[]",
        "{\"schema\":\"aegis.p03-p15.media-ingest.v1\",\"edge\":\"ZERO_COPY_MEDIA_INGEST\"}",
    ] {
        assert!(MediaIngestDescriptor::decode(text).is_err());
    }
    let descriptor = media_ingest(8)?;
    let mut buffer = PayloadBuffer::new();
    let text = descriptor.encode_into(&mut buffer)?.to_owned();

    let extra = text.replace("{\"schema\"", "{\"stowaway\":1,\"schema\"");
    assert!(matches!(refusal(&extra)?, ContractError::Malformed { .. }));

    let renamed = text.replace("media-ingest.v1", "media-ingest.v0");
    assert!(matches!(
        refusal(&renamed)?,
        ContractError::UnknownVersion { .. }
    ));
    Ok(())
}

/// Negative: a misaligned `BAR` address is refused by the decoder.
#[test]
fn a_misaligned_bar_does_not_decode() -> Fallible {
    let descriptor = media_ingest(8)?;
    let mut buffer = PayloadBuffer::new();
    let text = descriptor.encode_into(&mut buffer)?.replace(
        &ALIGNED_BAR.to_string(),
        &ALIGNED_BAR.saturating_add(1).to_string(),
    );
    assert!(matches!(refusal(&text)?, ContractError::Malformed { .. }));
    Ok(())
}

/// Negative: an unusable export path is refused with a named reason.
#[test]
fn an_unusable_export_path_is_rejected() -> Fallible {
    let mut descriptor = media_ingest(8)?;
    descriptor.export.modeset = ModesetState::Disabled;
    let mut buffer = PayloadBuffer::new();
    assert!(matches!(
        descriptor.encode_into(&mut buffer),
        Err(ContractError::ExportPathUnusable { .. })
    ));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a descriptor at the maximum block count is accepted on the wire,
/// and one block over is rejected by the decoder.
#[test]
fn the_wire_block_count_bound_is_exact() -> Fallible {
    let descriptor = media_ingest(8192)?;
    let mut buffer = PayloadBuffer::new();
    let text = descriptor.encode_into(&mut buffer)?.to_owned();
    assert!(text.contains("\"blocks\":8192"));
    assert_eq!(MediaIngestDescriptor::decode(&text)?, descriptor);

    for over in ["\"blocks\":8193", "\"blocks\":0"] {
        let bad = text.replace("\"blocks\":8192", over);
        assert!(matches!(refusal(&bad)?, ContractError::Malformed { .. }));
    }
    Ok(())
}

/// Boundary: the smallest admissible run still round-trips.
#[test]
fn the_smallest_admissible_run_round_trips() -> Fallible {
    let descriptor = media_ingest(1)?;
    let mut buffer = PayloadBuffer::new();
    let text = descriptor.encode_into(&mut buffer)?.to_owned();
    assert_eq!(MediaIngestDescriptor::decode(&text)?, descriptor);
    assert_eq!(descriptor.transfer_bytes(), 512);
    Ok(())
}
