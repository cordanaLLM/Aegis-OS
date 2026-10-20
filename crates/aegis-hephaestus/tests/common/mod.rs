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

#[path = "../../../source_effects.rs"]
mod source_effects;
#[allow(unused_imports)]
pub use source_effects::{LINE_BOUND, WALK_BOUND, hits, rust_sources, scan_sources, scan_text};

/// Returns the crate's own `src/` directory.
#[must_use]
pub fn source_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}
