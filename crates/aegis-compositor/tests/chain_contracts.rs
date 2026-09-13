// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E07-5, the compositor's two edges: the focus-switch report P04 builds
//! and the DMA-BUF descriptor it consumes.
//!
//! Positive: a focus-switch report built from a registry row is P07's own type
//! and round-trips through P07's own decoder, carrying the control-group slice
//! REQ-P04-07 requires; a descriptor P08 produced becomes an overlay surface
//! request. Negative: a surface row carrying process zero produces no report,
//! and a stream name this crate's registry cannot hold is refused even though
//! P08 accepted it. Boundary: the overlay geometry is carried across at zero
//! and at the widest representable frame.

mod common;

use aegis_calliope::{
    BufferHandle, BufferId, ContractError, CorrelationId as StreamCorrelationId,
    DmaBufStreamDescriptor, DmaBufStreamVersion, Label as StreamLabel, PayloadBuffer, PixelFormat,
    SyncStrategy, stride_bytes,
};
use aegis_compositor::{
    FocusSwitchDraft, OverlayRegistration, SurfaceGeometry, SurfaceId, SurfaceLayer,
    SurfaceRegistry, SurfaceRequest, accept_dma_buf_stream,
};
use aegis_lictor::{
    CorrelationId as ReportCorrelationId, FocusSwitchReport, FocusSwitchVersion,
    Label as ReportLabel, SliceName,
};

use common::Fallible;

/// The largest width whose packed 32-bit stride still fits in `u32`.
const LAST_PACKED_WIDTH: u32 = u32::MAX / 4;

/// Builds the draft the compositor would carry for a focus switch.
fn draft() -> Result<FocusSwitchDraft, Box<dyn std::error::Error>> {
    Ok(FocusSwitchDraft {
        correlation_id: ReportCorrelationId::parse(common::CORRELATION)?,
        app_id: ReportLabel::parse(common::APP_ID)?,
        cgroup_slice: SliceName::parse(common::SLICE)?,
    })
}

/// Renders a descriptor as P08 would produce it.
fn descriptor_text(
    stream: &str,
    width: u32,
    height: u32,
) -> Result<String, Box<dyn std::error::Error>> {
    let descriptor = DmaBufStreamDescriptor {
        schema: DmaBufStreamVersion::V1,
        edge: DmaBufStreamDescriptor::EDGE,
        correlation_id: StreamCorrelationId::parse(common::CORRELATION)?,
        stream: StreamLabel::parse(stream)?,
        buffer_id: BufferId::new(3),
        handle: BufferHandle::new(3),
        width,
        height,
        format: PixelFormat::Argb8888,
        stride_bytes: stride_bytes(PixelFormat::Argb8888, width)?,
        sync: SyncStrategy::ADMITTED,
    };
    let mut buffer = PayloadBuffer::new();
    Ok(descriptor.encode_into(&mut buffer)?.to_owned())
}

// --- Positive -------------------------------------------------------------

/// Positive: the focus-switch report is P07's own type, and P07's decoder
/// reads it back.
#[test]
fn the_focus_switch_report_is_built_from_the_consumers_type() -> Fallible {
    let mut registry = common::scaffold_registry()?;
    let focused = registry.focus(SurfaceId::new(1))?;
    let report: FocusSwitchReport = draft()?.report_for(focused)?;
    let mut buffer = aegis_lictor::PayloadBuffer::new();
    let text = report.encode_into(&mut buffer)?.to_owned();
    let decoded = FocusSwitchReport::decode(&text)?;
    assert_eq!(decoded, report);
    assert_eq!(decoded.schema, FocusSwitchVersion::V1);
    assert_eq!(decoded.edge, FocusSwitchReport::EDGE);
    Ok(())
}

/// Positive: the report carries the control-group slice and the surface that
/// gained focus, which is what REQ-P04-07 is about.
#[test]
fn the_report_carries_the_slice_and_the_surface() -> Fallible {
    let mut registry = common::scaffold_registry()?;
    let focused = registry.focus(SurfaceId::new(1))?;
    let report = draft()?.report_for(focused)?;
    let fields = (
        report.cgroup_slice.to_string(),
        report.app_id.to_string(),
        report.pid.get(),
        report.surface_id,
    );
    assert_eq!(
        fields,
        (
            common::SLICE.to_owned(),
            common::APP_ID.to_owned(),
            common::SHELL_PID,
            1,
        )
    );
    Ok(())
}

/// Positive: a descriptor P08 produced becomes an overlay surface request.
#[test]
fn a_descriptor_becomes_an_overlay_surface_request() -> Fallible {
    let text = descriptor_text(common::STREAM, 2560, 1440)?;
    let registration: OverlayRegistration = accept_dma_buf_stream(&text, SurfaceId::new(7), 1003)?;
    let request: SurfaceRequest = registration.request;
    assert_eq!(request.id, SurfaceId::new(7));
    assert_eq!(request.layer, SurfaceLayer::Overlay);
    assert_eq!(request.geometry, SurfaceGeometry::new(2560, 1440));
    assert_eq!(request.title.to_string(), common::STREAM);
    assert_eq!(request.pid, 1003);
    assert_eq!(
        registration.stride_bytes,
        stride_bytes(PixelFormat::Argb8888, 2560)?
    );
    Ok(())
}

/// Positive: the resulting request registers, on the overlay layer.
#[test]
fn the_resulting_request_registers_on_the_overlay_layer() -> Fallible {
    let text = descriptor_text(common::STREAM, 640, 480)?;
    let registration = accept_dma_buf_stream(&text, SurfaceId::new(9), 1004)?;
    let mut registry = SurfaceRegistry::new();
    registry.register(registration.request)?;
    assert_eq!(registry.layer_count(SurfaceLayer::Overlay), 1);
    assert_eq!(registry.count(), 1);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a surface row carrying process zero produces no report.
///
/// P07's own [`Pid::new`](aegis_lictor::Pid::new) refuses it, so the refusal is
/// the consumer's rather than one of ours.
#[test]
fn a_surface_owned_by_process_zero_produces_no_report() -> Fallible {
    let mut registry = SurfaceRegistry::new();
    let id = SurfaceId::new(5);
    registry.register(SurfaceRequest {
        id,
        title: common::label(common::SHELL)?,
        geometry: SurfaceGeometry::new(1, 1),
        layer: SurfaceLayer::Top,
        pid: 0,
    })?;
    let focused = registry.focus(id)?;
    assert!(draft()?.report_for(focused).is_err());
    Ok(())
}

/// Negative: a stream name P08 accepts is refused here when this crate's
/// registry cannot hold it.
///
/// P08's label is bounded at 64 bytes and this crate's at 32. The consumer
/// validating what it must store is the point of the consumer acting on it.
#[test]
fn a_stream_name_this_registry_cannot_hold_is_refused() -> Fallible {
    let long = "s".repeat(48);
    // P08's own decoder is satisfied: the name fits its bound.
    let text = descriptor_text(&long, 640, 480)?;
    assert!(DmaBufStreamDescriptor::decode(&text).is_ok());
    let refusal = accept_dma_buf_stream(&text, SurfaceId::new(11), 1005);
    assert!(matches!(refusal, Err(ContractError::Malformed { .. })));
    Ok(())
}

/// Negative: the producer's own refusals stay the producer's -- a descriptor
/// whose stride contradicts its format never reaches this crate's mapping.
#[test]
fn the_producers_refusals_stay_the_producers() -> Fallible {
    let text = descriptor_text(common::STREAM, 640, 480)?;
    let tampered = common::tamper(&text, "\"stride-bytes\":2560", "\"stride-bytes\":640")?;
    let refusal = accept_dma_buf_stream(&tampered, SurfaceId::new(12), 1006);
    assert!(matches!(refusal, Err(ContractError::Malformed { .. })));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the overlay geometry is carried across at zero and at the widest
/// representable frame.
#[test]
fn the_overlay_geometry_is_carried_at_both_extremes() -> Fallible {
    let degenerate = descriptor_text(common::STREAM, 0, 0)?;
    let zero = accept_dma_buf_stream(&degenerate, SurfaceId::new(13), 1007)?;
    assert_eq!(zero.request.geometry, SurfaceGeometry::new(0, 0));
    assert_eq!(zero.stride_bytes, 0);

    let widest = descriptor_text(common::STREAM, LAST_PACKED_WIDTH, 1)?;
    let full = accept_dma_buf_stream(&widest, SurfaceId::new(14), 1008)?;
    assert_eq!(
        full.request.geometry,
        SurfaceGeometry::new(LAST_PACKED_WIDTH, 1)
    );
    assert_eq!(full.stride_bytes, 4_294_967_292);
    Ok(())
}

/// Boundary: a report at the label bound is built, and one byte past it is
/// not.
#[test]
fn the_report_label_bound_is_exact() -> Fallible {
    let mut registry = common::scaffold_registry()?;
    let focused = registry.focus(SurfaceId::new(1))?;
    let longest = "a".repeat(aegis_lictor::MAX_LABEL_LEN);
    let at_bound = FocusSwitchDraft {
        correlation_id: ReportCorrelationId::parse(common::CORRELATION)?,
        app_id: ReportLabel::parse(&longest)?,
        cgroup_slice: SliceName::parse(common::SLICE)?,
    };
    assert!(at_bound.report_for(focused).is_ok());
    let over = format!("a{longest}");
    assert!(ReportLabel::parse(&over).is_err());
    Ok(())
}
