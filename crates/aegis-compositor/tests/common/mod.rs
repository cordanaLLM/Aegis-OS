// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fixtures shared by the integration tests.
//!
//! Everything here reaches `aegis-compositor` through its public API only, and
//! nothing here uses `unwrap` or `expect`: a test that cannot build its own
//! input returns the failure instead of panicking through a lint the workspace
//! denies.

#![allow(dead_code)]

use aegis_compositor::{
    ClientTable, KeyExpr, Label, MeshRole, MockedMesh, SurfaceGeometry, SurfaceId, SurfaceLayer,
    SurfaceRegistry, SurfaceRequest,
};

/// What every test in this suite returns.
pub type Fallible = Result<(), Box<dyn std::error::Error>>;

/// The correlation identifier the fixtures thread through every payload.
pub const CORRELATION: &str = "m07-p04-fixture-0001";

/// The shell surface the scaffold seeds, in a spelling a label admits.
pub const SHELL: &str = "forum-desktop-shell";

/// The overlay surface the scaffold seeds.
pub const OVERLAY: &str = "hestia-pip-media-player";

/// The stream name the P08 descriptor carries.
pub const STREAM: &str = "looking-glass-capture";

/// The application identifier a focus-switch report carries.
pub const APP_ID: &str = "aegis-compositor";

/// The control-group slice a focus-switch report carries.
pub const SLICE: &str = "app-aegis-compositor.slice";

/// The key expression the scaffold publishes a focus change on.
pub const FOCUS_KEY: &str = "aegis/compositor/focus";

/// A key expression from the report's own topic hierarchy.
pub const METRICS_KEY: &str = "aegis/agents/athena/candidate_a/metrics";

/// The process that owns the shell surface.
pub const SHELL_PID: u32 = 1001;

/// Returns a label, so a fixture that stops parsing fails the suite.
///
/// # Errors
///
/// Propagates [`Label::parse`].
pub fn label(raw: &str) -> Result<Label, Box<dyn std::error::Error>> {
    Ok(Label::parse(raw)?)
}

/// Returns a key expression.
///
/// # Errors
///
/// Propagates [`KeyExpr::parse`].
pub fn key(raw: &str) -> Result<KeyExpr, Box<dyn std::error::Error>> {
    Ok(KeyExpr::parse(raw)?)
}

/// Returns the surface request the scaffold seeds for the shell.
///
/// # Errors
///
/// Propagates [`Label::parse`].
pub fn shell_request(id: u32) -> Result<SurfaceRequest, Box<dyn std::error::Error>> {
    Ok(SurfaceRequest {
        id: SurfaceId::new(id),
        title: label(SHELL)?,
        geometry: SurfaceGeometry::new(1920, 1080),
        layer: SurfaceLayer::Top,
        pid: SHELL_PID,
    })
}

/// Returns the registry the scaffold seeds: a shell surface and an overlay.
///
/// # Errors
///
/// Propagates the field constructors and the registry's refusals.
pub fn scaffold_registry() -> Result<SurfaceRegistry, Box<dyn std::error::Error>> {
    let mut registry = SurfaceRegistry::new();
    registry.register(shell_request(1)?)?;
    registry.register(SurfaceRequest {
        id: SurfaceId::new(2),
        title: label(OVERLAY)?,
        geometry: SurfaceGeometry::new(480, 270),
        layer: SurfaceLayer::Overlay,
        pid: 1002,
    })?;
    Ok(registry)
}

/// Returns a registry holding `count` surfaces, identified from 1 up.
///
/// # Errors
///
/// Propagates the field constructors and the registry's capacity refusal.
pub fn filled_registry(count: usize) -> Result<SurfaceRegistry, Box<dyn std::error::Error>> {
    let mut registry = SurfaceRegistry::new();
    for index in 0..count {
        let id = u32::try_from(index).unwrap_or(0).saturating_add(1);
        registry.register(shell_request(id)?)?;
    }
    Ok(registry)
}

/// Returns a client table holding `count` admitted clients.
///
/// # Errors
///
/// Propagates the table's capacity refusal.
pub fn filled_clients(count: usize) -> Result<ClientTable, Box<dyn std::error::Error>> {
    let mut clients = ClientTable::new();
    for _ in 0..count {
        clients.admit()?;
    }
    Ok(clients)
}

/// Returns a mesh with one publication retained on the focus key.
///
/// # Errors
///
/// Propagates the key constructor and the mesh's refusals.
pub fn seeded_mesh() -> Result<MockedMesh, Box<dyn std::error::Error>> {
    let mut mesh = MockedMesh::new();
    mesh.publish(MeshRole::Publisher, key(FOCUS_KEY)?, b"focus-change")?;
    Ok(mesh)
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

#[path = "../../../source_effects.rs"]
mod source_effects;
#[allow(unused_imports)]
pub use source_effects::{LINE_BOUND, WALK_BOUND, hits, rust_sources, scan_sources, scan_text};
