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
