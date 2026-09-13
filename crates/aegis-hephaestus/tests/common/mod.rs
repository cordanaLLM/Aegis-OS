// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fixtures shared by the integration tests.
//!
//! Everything here reaches `aegis-hephaestus` through its public API only, and
//! nothing here uses `unwrap` or `expect`: a test that cannot build its own
//! input returns the failure instead of panicking through a lint the workspace
//! denies.
//!
//! The producer fixtures reach `aegis-minerva` the same way, because the P09
//! request is the producer's type: a test that built its own copy of it would
//! be testing a copy.

#![allow(dead_code)]

use aegis_hephaestus::{
    BrepEvaluator, CadEngine, GeometryViewport, GeometryViewportVersion, MeshConfig,
    MeshElementCount, PayloadBuffer, StepPath, StepSchema, Tolerance,
};
use aegis_justitia::{Digest32, Identity, UnixSeconds};
use aegis_minerva::{
    CadVerificationRequest, CadVerificationVersion, ExpertId, PayloadBuffer as MinervaBuffer,
    ScreenOutcome,
};

/// What every test in this suite returns.
pub type Fallible = Result<(), Box<dyn std::error::Error>>;

/// The correlation identifier the fixtures thread through every payload.
pub const CORRELATION: &str = "m08-fixture-0002";

/// The `STEP` source the fixtures name and never open.
pub const STEP_SOURCE: &str = "/var/lib/aegis/geometry/bracket.step";

/// The element count the scaffold hard-codes as its meshing result.
pub const SCAFFOLD_ELEMENTS: usize = 120_000;

/// Returns the fixture correlation identifier.
///
/// # Errors
///
/// Propagates [`Identity::parse`].
pub fn correlation() -> Result<Identity, Box<dyn std::error::Error>> {
    Ok(Identity::parse(CORRELATION)?)
}

/// Returns the fixture `STEP` source path.
///
/// # Errors
///
/// Propagates [`StepPath::parse`].
pub fn step_source() -> Result<StepPath, Box<dyn std::error::Error>> {
    Ok(StepPath::parse(STEP_SOURCE)?)
}

/// Returns an evaluator for the exact boundary-representation engine.
#[must_use]
pub fn evaluator() -> BrepEvaluator {
    BrepEvaluator::new(CadEngine::ExactNurbsBrep, Tolerance::SCAFFOLD)
}

/// Returns the mesh configuration the fixtures use, in micrometres.
///
/// # Errors
///
/// Propagates [`MeshConfig::new`].
pub fn mesh_config() -> Result<MeshConfig, Box<dyn std::error::Error>> {
    Ok(MeshConfig::new(100, 2_000, true)?)
}

/// Returns a well-formed viewport descriptor carrying `elements`.
///
/// # Errors
///
/// Propagates the field constructors.
pub fn viewport(
    elements: usize,
    engine: CadEngine,
) -> Result<GeometryViewport, Box<dyn std::error::Error>> {
    Ok(GeometryViewport {
        schema: GeometryViewportVersion::V1,
        edge: GeometryViewport::EDGE,
        correlation_id: correlation()?,
        source: step_source()?,
        step_schema: StepSchema::ADMITTED,
        engine,
        tolerance_micrometres: Tolerance::SCAFFOLD,
        elements: MeshElementCount::new(elements)?,
        prepared_at: UnixSeconds::new(2_000),
    })
}

/// Returns the viewport the fixtures use by default.
///
/// # Errors
///
/// Propagates the field constructors.
pub fn scaffold_viewport() -> Result<GeometryViewport, Box<dyn std::error::Error>> {
    viewport(SCAFFOLD_ELEMENTS, CadEngine::ExactNurbsBrep)
}

/// Renders a viewport descriptor to owned text.
///
/// # Errors
///
/// Propagates the contract refusal.
pub fn encoded_viewport(value: &GeometryViewport) -> Result<String, Box<dyn std::error::Error>> {
    let mut buffer = PayloadBuffer::new();
    Ok(value.encode_into(&mut buffer)?.to_owned())
}

/// Returns the producer's verification request, built through P09's own type.
///
/// # Errors
///
/// Propagates the producer's field constructors.
pub fn producer_request(
    screened: ScreenOutcome,
) -> Result<CadVerificationRequest, Box<dyn std::error::Error>> {
    Ok(CadVerificationRequest {
        schema: CadVerificationVersion::V1,
        edge: CadVerificationRequest::EDGE,
        direction: CadVerificationRequest::DIRECTION,
        correlation_id: correlation()?,
        expert: ExpertId::new(1),
        script_digest: Digest32::from_bytes([0x22u8; 32]),
        constraint_count: 4,
        screened,
        submitted_at: UnixSeconds::new(1_500),
    })
}

/// Renders a producer request to owned text through the producer's encoder.
///
/// # Errors
///
/// Propagates the producer's contract refusal.
pub fn encoded_request(
    request: &CadVerificationRequest,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut buffer = MinervaBuffer::new();
    Ok(request.encode_into(&mut buffer)?.to_owned())
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

/// Returns the crate's own `src/` directory.
#[must_use]
pub fn source_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}
