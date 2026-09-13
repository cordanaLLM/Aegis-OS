// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E07-5: the `SHARE_DMA_BUF_STREAM` descriptor, round-tripped and
//! refused.
//!
//! Positive: a descriptor encodes and decodes to the value it started as, in
//! both admitted formats. Negative: a malformed descriptor is refused, and so
//! is one whose stride does not follow from its own width and format -- which
//! is the scaffold's defect, made undecodable. Boundary: the stride is checked
//! on the wire at zero and at the packed ceiling.

mod common;

use aegis_calliope::{
    BufferHandle, BufferId, ContractError, DmaBufStreamDescriptor, DmaBufStreamVersion, EdgeId,
    FrameGeometry, PixelFormat, SchemaId, SyncStrategy, stride_bytes,
};

use common::{
    CAPTURE_HEIGHT, CAPTURE_WIDTH, Fallible, descriptor, encoded_descriptor, other_edge, tamper,
};

/// The largest width whose packed 32-bit stride still fits in `u32`.
const LAST_PACKED_WIDTH: u32 = u32::MAX / 4;

// --- Positive -------------------------------------------------------------

/// Positive: a descriptor round-trips in both admitted formats.
#[test]
fn a_descriptor_round_trips_in_both_formats() -> Fallible {
    for format in PixelFormat::ALL {
        let original = descriptor(format, CAPTURE_WIDTH, CAPTURE_HEIGHT)?;
        let text = encoded_descriptor(&original)?;
        let decoded = DmaBufStreamDescriptor::decode(&text)?;
        let fields = (
            decoded.schema,
            decoded.edge,
            decoded.sync,
            decoded.format,
            decoded.stride_bytes,
            decoded.buffer_id,
            decoded.handle,
            decoded.geometry(),
            decoded.stream.to_string(),
        );
        assert_eq!(
            fields,
            (
                DmaBufStreamVersion::V1,
                EdgeId::ShareDmaBufStream,
                SyncStrategy::ADMITTED,
                format,
                stride_bytes(format, CAPTURE_WIDTH)?,
                BufferId::new(0),
                BufferHandle::new(0),
                FrameGeometry::new(CAPTURE_WIDTH, CAPTURE_HEIGHT),
                common::STREAM.to_owned(),
            )
        );
        assert_eq!(decoded, original);
        assert_eq!(decoded.edge, DmaBufStreamDescriptor::EDGE);
        assert_eq!(decoded.sync, DmaBufStreamDescriptor::SYNC);
    }
    assert_eq!(DmaBufStreamDescriptor::SCHEMA, SchemaId::DmaBufStream);
    assert_eq!(
        SchemaId::DmaBufStream.tag(),
        "aegis.p08-p04.dma-buf-stream.v1"
    );
    assert_eq!(SchemaId::DmaBufStream.edge(), EdgeId::ShareDmaBufStream);
    Ok(())
}

/// Positive: the edges name their recorded transports, and the one no schema
/// carries is marked as such.
#[test]
fn the_edges_name_their_recorded_transports() {
    assert_eq!(EdgeId::ALL.len(), 3);
    assert_eq!(
        EdgeId::ShareDmaBufStream.recorded_transport(),
        "PipeWire pipewiresrc / SPA_DATA_DmaBuf"
    );
    assert_eq!(
        EdgeId::EnforceRealtimeRtprio.recorded_transport(),
        "Linux cgroups v2 / RLIMIT_RTPRIO"
    );
    assert_eq!(
        EdgeId::RemotePlayCapture.recorded_transport(),
        "Gamescope / PipeWire DMA-BUF"
    );
    assert!(EdgeId::ShareDmaBufStream.is_carried());
    assert!(EdgeId::EnforceRealtimeRtprio.is_carried());
    assert!(!EdgeId::RemotePlayCapture.is_carried());
    assert_eq!(
        EdgeId::RemotePlayCapture.tag(),
        "P08_Calliope->P11_Ludus:REMOTE_PLAY_CAPTURE"
    );
    assert_eq!(
        EdgeId::ShareDmaBufStream.tag(),
        "P08_Calliope->P04_Mercurius:SHARE_DMA_BUF_STREAM"
    );
}

// --- Negative -------------------------------------------------------------

/// Negative: the scaffold's own descriptor -- a width-times-four stride under
/// a planar format -- does not decode.
#[test]
fn a_packed_stride_under_a_planar_format_is_refused() -> Fallible {
    let planar = descriptor(PixelFormat::Nv12, CAPTURE_WIDTH, CAPTURE_HEIGHT)?;
    let text = encoded_descriptor(&planar)?;
    let packed = stride_bytes(PixelFormat::Argb8888, CAPTURE_WIDTH)?;
    let tampered = tamper(
        &text,
        &format!("\"stride-bytes\":{}", planar.stride_bytes),
        &format!("\"stride-bytes\":{packed}"),
    )?;
    assert!(matches!(
        DmaBufStreamDescriptor::decode(&tampered),
        Err(ContractError::Malformed { .. })
    ));
    Ok(())
}

/// Negative: an unadmitted format name, an unknown field, an empty stream name
/// and a global master clock are each refused.
#[test]
fn malformed_descriptors_are_refused() -> Fallible {
    let text = encoded_descriptor(&descriptor(PixelFormat::Argb8888, 640, 480)?)?;
    for (from, to) in [
        ("\"argb8888\"", "\"av24\""),
        ("\"width\"", "\"pixel-width\""),
        (common::STREAM, ""),
        ("\"local-phase-and-drift\"", "\"global-master-clock\""),
        ("dma-buf-stream.v1", "dma-buf-stream.v0"),
    ] {
        let tampered = tamper(&text, from, to)?;
        assert!(
            matches!(
                DmaBufStreamDescriptor::decode(&tampered),
                Err(ContractError::Malformed { .. } | ContractError::UnknownVersion { .. })
            ),
            "the decoder accepted {from:?} replaced by {to:?}"
        );
    }
    Ok(())
}

/// Negative: a payload naming the other edge this crate carries is refused as
/// travelling on the wrong crossing.
#[test]
fn the_other_edge_is_refused_as_the_wrong_edge() -> Fallible {
    let text = encoded_descriptor(&descriptor(PixelFormat::Argb8888, 640, 480)?)?;
    let tampered = tamper(
        &text,
        EdgeId::ShareDmaBufStream.tag(),
        other_edge(EdgeId::ShareDmaBufStream).tag(),
    )?;
    assert!(matches!(
        DmaBufStreamDescriptor::decode(&tampered),
        Err(ContractError::WrongEdge { .. })
    ));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the wire stride is exact at zero and at the packed ceiling.
#[test]
fn the_wire_stride_is_exact_at_zero_and_at_the_ceiling() -> Fallible {
    let degenerate = descriptor(PixelFormat::Argb8888, 0, 0)?;
    assert_eq!(degenerate.stride_bytes, 0);
    let text = encoded_descriptor(&degenerate)?;
    assert_eq!(DmaBufStreamDescriptor::decode(&text)?, degenerate);

    let widest = descriptor(PixelFormat::Argb8888, LAST_PACKED_WIDTH, 1)?;
    assert_eq!(widest.stride_bytes, 4_294_967_292);
    let text = encoded_descriptor(&widest)?;
    assert_eq!(DmaBufStreamDescriptor::decode(&text)?, widest);

    // One pixel wider has no representable stride, so no descriptor exists to
    // encode: the refusal happens at construction rather than on the wire.
    assert!(stride_bytes(PixelFormat::Argb8888, LAST_PACKED_WIDTH.saturating_add(1)).is_err());

    // And a payload that asserts one anyway does not decode.
    let overflowing = tamper(
        &text,
        &format!("\"width\":{LAST_PACKED_WIDTH}"),
        &format!("\"width\":{}", LAST_PACKED_WIDTH.saturating_add(1)),
    )?;
    assert!(matches!(
        DmaBufStreamDescriptor::decode(&overflowing),
        Err(ContractError::Malformed { .. })
    ));
    Ok(())
}
