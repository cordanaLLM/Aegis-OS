// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fixtures shared by the integration tests.
//!
//! Everything here reaches `aegis-lictor` through its public API only, and
//! nothing here uses `unwrap` or `expect`: a test that cannot build its own
//! input returns the failure instead of panicking through a lint the workspace
//! denies.

#![allow(dead_code)]

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
    let mut buffer = PayloadBuffer::new();
    Ok(value.encode_into(&mut buffer)?.to_owned())
}

/// Returns `text` with the first occurrence of `from` replaced by `to`.
///
/// # Errors
///
/// Returns a failure when `from` does not occur, so a tamper that stops
/// applying fails the suite instead of testing the untampered payload.
pub fn tamper(text: &str, from: &str, to: &str) -> Result<String, Box<dyn std::error::Error>> {
    if !text.contains(from) {
        return Err(format!("the fixture payload does not contain {from:?}").into());
    }
    Ok(text.replacen(from, to, 1))
}

/// The edge a payload must not claim, given the one it does.
#[must_use]
pub fn other_edge(edge: EdgeId) -> EdgeId {
    match edge {
        EdgeId::FocusSwitchNotify => EdgeId::EnforceRealtimeRtprio,
        _ => EdgeId::FocusSwitchNotify,
    }
}

/// Scalar bound on directories visited and entries read per directory.
pub const WALK_BOUND: usize = 64;

/// Scalar bound on the lines read from any one source file.
pub const LINE_BOUND: usize = 4096;

/// Returns the `.rs` files under `root`, as (file name, body) pairs.
///
/// The walk is iterative and doubly bounded: at most [`WALK_BOUND`]
/// directories and at most [`WALK_BOUND`] entries per directory.
///
/// # Errors
///
/// Returns the first read failure, so a sweep cannot pass by reading nothing.
pub fn rust_sources(
    root: &std::path::Path,
) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = vec![root.to_path_buf()];
    for _ in 0..WALK_BOUND {
        let Some(directory) = stack.pop() else { break };
        for entry in std::fs::read_dir(&directory)?.take(WALK_BOUND) {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            let name = path
                .file_name()
                .map(|raw| raw.to_string_lossy().into_owned())
                .unwrap_or_default();
            out.push((name, std::fs::read_to_string(&path)?));
        }
    }
    if out.is_empty() {
        return Err(format!("no Rust source was read under {}", root.display()).into());
    }
    Ok(out)
}

/// Returns the `"<file>:<line>: <identifier>"` hits `text` carries.
fn hits(name: &str, number: usize, text: &str, watched: &[&str]) -> Vec<String> {
    watched
        .iter()
        .filter(|needle| text.contains(**needle))
        .map(|needle| format!("{name}:{}: {needle}", number.saturating_add(1)))
        .collect()
}

/// Returns the hits one body of text carries, skipping comment lines.
///
/// Comment lines are skipped whole: the crate documentation names the very
/// identifiers a sweep watches for, precisely in order to say it does not use
/// them, so a sweep that read comments would fail on its own documentation.
///
/// This is public so a watched list can be exercised against planted text.
pub fn scan_text(name: &str, body: &str, watched: &[&str]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for (number, line) in body.lines().take(LINE_BOUND).enumerate() {
        let text = line.trim();
        if text.starts_with("//") {
            continue;
        }
        found.extend(hits(name, number, text, watched));
    }
    found
}

/// Returns every watched identifier the sources under `root` carry.
///
/// # Errors
///
/// Propagates [`rust_sources`], so a sweep cannot pass by reading nothing.
pub fn scan_sources(
    root: &std::path::Path,
    watched: &[&str],
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut found: Vec<String> = Vec::new();
    for (name, body) in rust_sources(root)? {
        found.extend(scan_text(&name, &body, watched));
    }
    Ok(found)
}
