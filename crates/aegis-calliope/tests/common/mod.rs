// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fixtures shared by the integration tests.
//!
//! Everything here reaches `aegis-calliope` through its public API only, and
//! nothing here uses `unwrap` or `expect`: a test that cannot build its own
//! input returns the failure instead of panicking through a lint the workspace
//! denies.

#![allow(dead_code)]

#[path = "../../../fixture_tamper.rs"]
mod fixture_tamper;
#[allow(unused_imports)]
pub use fixture_tamper::tamper;

use aegis_calliope::{
    BufferHandle, BufferId, CorrelationId, DmaBufStreamDescriptor, DmaBufStreamVersion,
    DmaBufTable, EdgeId, FrameGeometry, GrantedRtPrio, Label, PayloadBuffer, PixelFormat,
    PluginFormat, PluginHost, PluginSlotId, PluginStage, RealtimeGrant, RealtimeGrantVersion,
    SchedPolicy, SyncStrategy, stride_bytes,
};

/// What every test in this suite returns.
pub type Fallible = Result<(), Box<dyn std::error::Error>>;

/// The correlation identifier the fixtures thread through every payload.
pub const CORRELATION: &str = "m07-p08-fixture-0001";

/// The plugin name the fixtures admit.
///
/// Hyphenated, because [`Label`] refuses the space in the scaffold's own
/// `"FabFilter Pro-Q 3"` rather than repairing it.
pub const PLUGIN: &str = "calf-studio-reverb";

/// The stream name the scaffold registers, in a spelling a label admits.
pub const STREAM: &str = "looking-glass-capture";

/// The audio thread group the grant is about.
pub const THREAD: &str = "pipewire-rt-audio";

/// The capture width the scaffold asks for.
pub const CAPTURE_WIDTH: u32 = 2560;

/// The capture height the scaffold asks for.
pub const CAPTURE_HEIGHT: u32 = 1440;

/// Returns the fixture correlation identifier.
///
/// # Errors
///
/// Propagates [`CorrelationId::parse`].
pub fn correlation() -> Result<CorrelationId, Box<dyn std::error::Error>> {
    Ok(CorrelationId::parse(CORRELATION)?)
}

/// Returns a label, so a fixture that stops parsing fails the suite.
///
/// # Errors
///
/// Propagates [`Label::parse`].
pub fn label(raw: &str) -> Result<Label, Box<dyn std::error::Error>> {
    Ok(Label::parse(raw)?)
}

/// Returns a host holding `count` plugins, each in `Recognition`.
///
/// # Errors
///
/// Propagates the label constructor and the table's capacity refusal.
pub fn filled_host(count: usize) -> Result<PluginHost, Box<dyn std::error::Error>> {
    let mut host = PluginHost::new();
    for _ in 0..count {
        host.admit(label(PLUGIN)?, PluginFormat::Vst3)?;
    }
    Ok(host)
}

/// Returns a host with one plugin stepped all the way to `stage`.
///
/// # Errors
///
/// Propagates the lifecycle refusals, so an illegal path fails the fixture
/// rather than silently producing a host in another stage.
pub fn host_at(
    stage: PluginStage,
) -> Result<(PluginHost, PluginSlotId), Box<dyn std::error::Error>> {
    let mut host = PluginHost::new();
    let slot = host.admit(label(PLUGIN)?, PluginFormat::Clap)?;
    if stage == PluginStage::Recognition {
        return Ok((host, slot));
    }
    host.advance(slot, PluginStage::Quarantine)?;
    if stage != PluginStage::Quarantine {
        host.advance(slot, stage)?;
    }
    Ok((host, slot))
}

/// Returns a table holding `count` descriptors.
///
/// # Errors
///
/// Propagates the table's capacity and stride refusals.
pub fn filled_table(count: usize) -> Result<DmaBufTable, Box<dyn std::error::Error>> {
    let mut table = DmaBufTable::new();
    for _ in 0..count {
        table.register(
            FrameGeometry::new(CAPTURE_WIDTH, CAPTURE_HEIGHT),
            PixelFormat::Argb8888,
        )?;
    }
    Ok(table)
}

/// Returns a well-formed grant at `rtprio`.
///
/// # Errors
///
/// Propagates the field constructors.
pub fn grant(rtprio: u8) -> Result<RealtimeGrant, Box<dyn std::error::Error>> {
    Ok(RealtimeGrant {
        schema: RealtimeGrantVersion::V1,
        edge: RealtimeGrant::EDGE,
        correlation_id: correlation()?,
        thread: label(THREAD)?,
        policy: SchedPolicy::ADMITTED,
        rtprio: GrantedRtPrio::new(rtprio)?,
    })
}

/// Returns a well-formed descriptor in `format` at `width` by `height`.
///
/// # Errors
///
/// Propagates the field constructors and the stride arithmetic.
pub fn descriptor(
    format: PixelFormat,
    width: u32,
    height: u32,
) -> Result<DmaBufStreamDescriptor, Box<dyn std::error::Error>> {
    Ok(DmaBufStreamDescriptor {
        schema: DmaBufStreamVersion::V1,
        edge: DmaBufStreamDescriptor::EDGE,
        correlation_id: correlation()?,
        stream: label(STREAM)?,
        buffer_id: BufferId::new(0),
        handle: BufferHandle::new(0),
        width,
        height,
        format,
        stride_bytes: stride_bytes(format, width)?,
        sync: SyncStrategy::ADMITTED,
    })
}

/// Renders a grant to owned text.
///
/// # Errors
///
/// Propagates the contract refusal.
pub fn encoded_grant(value: &RealtimeGrant) -> Result<String, Box<dyn std::error::Error>> {
    let mut buffer = PayloadBuffer::new();
    Ok(value.encode_into(&mut buffer)?.to_owned())
}

/// Renders a descriptor to owned text.
///
/// # Errors
///
/// Propagates the contract refusal.
pub fn encoded_descriptor(
    value: &DmaBufStreamDescriptor,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut buffer = PayloadBuffer::new();
    Ok(value.encode_into(&mut buffer)?.to_owned())
}

/// The edge a payload must not claim, given the one it does.
#[must_use]
pub fn other_edge(edge: EdgeId) -> EdgeId {
    match edge {
        EdgeId::EnforceRealtimeRtprio => EdgeId::ShareDmaBufStream,
        _ => EdgeId::EnforceRealtimeRtprio,
    }
}

#[path = "../../../source_effects.rs"]
mod source_effects;
#[allow(unused_imports)]
pub use source_effects::{LINE_BOUND, WALK_BOUND, hits, rust_sources, scan_sources, scan_text};
