// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The two edges P04 stands on that another crate owns (REQ-P04-07,
//! REQ-P08-04).
//!
//! Milestone M14 set the rule this module follows: a crate that sends a
//! message another subsystem consumes **builds that subsystem's type**, and a
//! crate that receives one **decodes it with the owner's own decoder**. Both
//! halves appear here.
//!
//! | Direction | Type | Owner |
//! | :-- | :-- | :-- |
//! | P04 to P07, `FOCUS_SWITCH_NOTIFY` | [`FocusSwitchReport`] | `aegis-lictor`, the consumer |
//! | P08 to P04, `SHARE_DMA_BUF_STREAM` | [`DmaBufStreamDescriptor`] | `aegis-calliope`, the producer |
//!
//! The second is the exception the crate graph forces, and it is worth being
//! explicit about. The convention would put an inbound schema in the
//! consumer's crate, which is this one. This crate already depends on
//! `aegis-calliope` for the plugin and buffer bounds, so declaring the
//! descriptor here would close a cycle -- and Cargo refuses a cyclic crate
//! graph outright, so the alternative is not a style question. The descriptor
//! therefore lives with its producer and is decoded here through the
//! producer's own decoder, which keeps the one property the convention exists
//! for: there is exactly one definition of the message.
//!
//! # What the consumer side adds
//!
//! P08's decoder already refuses a descriptor whose stride does not follow
//! from its width and its format. What it cannot refuse is a stream name P04
//! cannot hold: this crate's [`Label`] is bounded at 32 bytes
//! and P08's at 64, so a longer name decodes on the producer's side and is
//! refused here. That is the consumer validating what it must store, not a
//! disagreement between the two crates.
//!
//! # What this module does not do
//!
//! **Nothing is sent, received, focused or displayed.** Building a payload is
//! not sending it, and decoding one is not acting on it; registering an
//! overlay surface adds a row to a fixed array.

use aegis_calliope::{ContractError, DmaBufStreamDescriptor};
use aegis_lictor::{
    CorrelationId as ReportCorrelationId, FocusSwitchReport, FocusSwitchVersion,
    Label as ReportLabel, Pid, SliceName,
};

use crate::error::CompositorError;
use crate::id::Label;
use crate::surface::{SurfaceGeometry, SurfaceId, SurfaceLayer, SurfaceRequest, WaylandSurface};

/// Everything P04 needs in order to build P07's focus-switch report.
///
/// The application identifier and the control-group slice are the two
/// arguments the shell protocol's own `report_focus_switch` request takes
/// (export-035 `4c2146ffed32`, REQ-P04-07); the surface and the process come
/// from the registry row that gained focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusSwitchDraft {
    /// The identifier that correlates this crossing.
    pub correlation_id: ReportCorrelationId,
    /// The application identifier the shell protocol carries.
    pub app_id: ReportLabel,
    /// The control-group slice the broker is to re-allocate.
    pub cgroup_slice: SliceName,
}

impl FocusSwitchDraft {
    /// Builds P07's report for the surface that gained focus.
    ///
    /// The edge comes from P07's own [`FocusSwitchReport::EDGE`], so a change
    /// on the consumer's side reaches this builder rather than being
    /// duplicated here.
    ///
    /// # Errors
    ///
    /// Propagates P07's own [`Pid::new`] refusal, so a surface row carrying
    /// process zero produces no report at all rather than one the broker would
    /// have to reject.
    pub fn report_for(
        self,
        surface: WaylandSurface,
    ) -> Result<FocusSwitchReport, aegis_lictor::LictorError> {
        Ok(FocusSwitchReport {
            schema: FocusSwitchVersion::V1,
            edge: FocusSwitchReport::EDGE,
            correlation_id: self.correlation_id,
            app_id: self.app_id,
            cgroup_slice: self.cgroup_slice,
            pid: Pid::new(surface.pid)?,
            surface_id: surface.id.get(),
        })
    }
}

/// What P04 does with one decoded DMA-BUF stream descriptor.
///
/// The graph of record says the stream carries zero-copy video for overlay
/// picture-in-picture windows, so the surface request this produces is on
/// [`SurfaceLayer::Overlay`] and nowhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayRegistration {
    /// The surface request the descriptor turns into.
    pub request: SurfaceRequest,
    /// The row stride the producer computed, carried for the reader.
    pub stride_bytes: u32,
}

/// Reads one descriptor P08 Calliope produced, as P04 Mercurius would act on it.
///
/// The descriptor is decoded by P08's own decoder, so the stride rule, the
/// edge guard and the version guard are the producer's. What this adds is the
/// consumer's own bound on what it can store.
///
/// # Errors
///
/// Returns [`ContractError`] when P08's decoder refuses the payload. The
/// consumer-side refusal -- a stream name this crate's registry cannot hold --
/// is reported as [`ContractError::Malformed`] carrying the descriptor's own
/// correlation, so the refusal is not anonymous.
pub fn accept_dma_buf_stream(
    text: &str,
    id: SurfaceId,
    pid: u32,
) -> Result<OverlayRegistration, ContractError> {
    let descriptor = DmaBufStreamDescriptor::decode(text)?;
    let registration =
        overlay_request(&descriptor, id, pid).map_err(|_| ContractError::Malformed {
            correlation: descriptor.correlation(),
        })?;
    Ok(registration)
}

/// Turns a decoded descriptor into the overlay request P04 would register.
///
/// # Errors
///
/// Returns [`CompositorError::Identifier`] when the stream name does not fit
/// this crate's label bound or carries a byte it does not admit.
fn overlay_request(
    descriptor: &DmaBufStreamDescriptor,
    id: SurfaceId,
    pid: u32,
) -> Result<OverlayRegistration, CompositorError> {
    let bytes = descriptor.stream.as_bytes();
    let text = core::str::from_utf8(bytes).map_err(|_| crate::id::IdError::Charset)?;
    let title = Label::parse(text)?;
    Ok(OverlayRegistration {
        request: SurfaceRequest {
            id,
            title,
            geometry: SurfaceGeometry::new(descriptor.width, descriptor.height),
            layer: SurfaceLayer::Overlay,
            pid,
        },
        stride_bytes: descriptor.stride_bytes,
    })
}
