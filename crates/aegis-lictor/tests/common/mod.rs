// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fixtures shared by the integration tests.
//!
//! Everything here reaches `aegis-lictor` through its public API only, and
//! nothing here uses `unwrap` or `expect`: a test that cannot build its own
//! input returns the failure instead of panicking through a lint the workspace
//! denies.

#![allow(dead_code)]

#[path = "../../../fixture_tamper.rs"]
mod fixture_tamper;
#[allow(unused_imports)]
pub use fixture_tamper::tamper;

use aegis_lictor::{
    CorrelationId, EdgeId, FocusSwitchReport, FocusSwitchVersion, Label, PayloadBuffer, Pid,
    ProcessTier, ResourceBroker, SliceName,
};

/// What every test in this suite returns.
pub type Fallible = Result<(), Box<dyn std::error::Error>>;

/// The correlation identifier the fixtures thread through every payload.
pub const CORRELATION: &str = "m07-p07-fixture-0001";

/// The compositor process the scaffold registers first.
pub const COMPOSITOR_PID: u32 = 1001;

/// The background agent the scaffold registers second.
pub const AGENT_PID: u32 = 2002;

/// A process the broker never hears about.
pub const STRANGER_PID: u32 = 4242;

/// The compositor's process name.
pub const COMPOSITOR: &str = "aegis-compositor";

/// The background agent's process name.
pub const AGENT: &str = "aegis-minerva-slm";

/// The control-group slice a focus switch names.
pub const SLICE: &str = "app-aegis-compositor.slice";

#[path = "../../../fixture_codec.rs"]
mod fixture_codec;

use fixture_codec::encoded;
#[allow(unused_imports)]
pub use fixture_codec::{correlation, label};

/// Returns a validated process identifier.
///
/// # Errors
///
/// Propagates [`Pid::new`].
pub fn pid(raw: u32) -> Result<Pid, Box<dyn std::error::Error>> {
    Ok(Pid::new(raw)?)
}

/// Returns the broker the scaffold's own `main` builds: one compositor and one
/// background agent.
///
/// # Errors
///
/// Propagates the field constructors and the table's capacity refusal.
pub fn scaffold_broker() -> Result<ResourceBroker, Box<dyn std::error::Error>> {
    let mut broker = ResourceBroker::new();
    broker.register(
        pid(COMPOSITOR_PID)?,
        label(COMPOSITOR)?,
        ProcessTier::T0WaylandCompositor,
        256,
    )?;
    broker.register(
        pid(AGENT_PID)?,
        label(AGENT)?,
        ProcessTier::T2BackgroundAgent,
        4096,
    )?;
    Ok(broker)
}

/// Returns a broker holding `count` background agents, identified from 1 up.
///
/// # Errors
///
/// Propagates the field constructors and the table's capacity refusal.
pub fn filled_broker(count: usize) -> Result<ResourceBroker, Box<dyn std::error::Error>> {
    let mut broker = ResourceBroker::new();
    for index in 0..count {
        let raw = u32::try_from(index).unwrap_or(0).saturating_add(1);
        broker.register(pid(raw)?, label(AGENT)?, ProcessTier::T2BackgroundAgent, 0)?;
    }
    Ok(broker)
}

/// Returns a well-formed focus-switch report for `raw_pid`.
///
/// # Errors
///
/// Propagates the field constructors.
pub fn report(raw_pid: u32) -> Result<FocusSwitchReport, Box<dyn std::error::Error>> {
    Ok(FocusSwitchReport {
        schema: FocusSwitchVersion::V1,
        edge: FocusSwitchReport::EDGE,
        correlation_id: correlation()?,
        app_id: label(COMPOSITOR)?,
        cgroup_slice: SliceName::parse(SLICE)?,
        pid: pid(raw_pid)?,
        surface_id: 1,
    })
}

/// Renders a report to owned text.
///
/// # Errors
///
/// Propagates the contract refusal.
pub fn encoded_report(value: &FocusSwitchReport) -> Result<String, Box<dyn std::error::Error>> {
    encoded(value, FocusSwitchReport::encode_into)
}

/// The edge a payload must not claim, given the one it does.
#[must_use]
pub fn other_edge(edge: EdgeId) -> EdgeId {
    match edge {
        EdgeId::FocusSwitchNotify => EdgeId::EnforceRealtimeRtprio,
        _ => EdgeId::FocusSwitchNotify,
    }
}

#[path = "../../../source_effects.rs"]
mod source_effects;

#[allow(unused_imports)]
pub use source_effects::*;
