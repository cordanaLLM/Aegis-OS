// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fixtures shared by the integration tests.
//!
//! Everything here reaches `aegis-hestia` through its public API only, and
//! nothing here uses `unwrap` or `expect`: a test that cannot build its own
//! input returns the failure instead of panicking through a lint the workspace
//! denies.

#![allow(dead_code)]

use aegis_hestia::{
    CorrelationId, DmaBufFd, EdgeId, Embedding, OverlayRegistration, OverlayRegistrationVersion,
    PgliteVectorStore, PipSurface, PixelExtent, ShellLayer, StoragePath, SurfaceId,
    WaylandPipMediaController,
};

/// What every test in this suite returns.
pub type Fallible = Result<(), Box<dyn std::error::Error>>;

/// The storage path the scaffold opens the store at.
pub const STORAGE: &str = "/var/pglite";

/// The correlation identifier the fixtures thread through every payload.
pub const CORRELATION: &str = "m17-fixture-0001";

/// The surface the fixtures register.
pub const SURFACE: &str = "hestia-pip-0";

/// The `DMA-BUF` descriptor the scaffold hands the controller.
pub const SCAFFOLD_FD: i32 = 7;

/// Returns the fixture storage path.
///
/// # Errors
///
/// Returns the refusal, so a fixture that stops being a valid path fails the
/// suite instead of being quietly replaced.
pub fn path() -> Result<StoragePath, Box<dyn std::error::Error>> {
    Ok(StoragePath::parse(STORAGE)?)
}

/// Returns an uninitialised store at the fixture path.
///
/// # Errors
///
/// Propagates [`path`].
pub fn store() -> Result<PgliteVectorStore, Box<dyn std::error::Error>> {
    Ok(PgliteVectorStore::new(path()?))
}

/// Returns an initialised store at the fixture path.
///
/// # Errors
///
/// Propagates [`store`] and the initialisation refusal.
pub fn initialised() -> Result<PgliteVectorStore, Box<dyn std::error::Error>> {
    let mut store = store()?;
    store.initialize()?;
    Ok(store)
}

/// Returns the embedding the scaffold queries with.
#[must_use]
pub fn embedding() -> Embedding {
    Embedding::new([0.12, 0.45, 0.88, 0.03])
}

/// Returns the fixture correlation identifier.
///
/// # Errors
///
/// Propagates [`CorrelationId::parse`].
pub fn correlation() -> Result<CorrelationId, Box<dyn std::error::Error>> {
    Ok(CorrelationId::parse(CORRELATION)?)
}

/// Returns the fixture surface identifier.
///
/// # Errors
///
/// Propagates [`SurfaceId::parse`].
pub fn surface_id() -> Result<SurfaceId, Box<dyn std::error::Error>> {
    Ok(SurfaceId::parse(SURFACE)?)
}

/// Returns a 640 by 360 overlay surface on the scaffold's descriptor.
///
/// # Errors
///
/// Propagates the field constructors.
pub fn surface() -> Result<PipSurface, Box<dyn std::error::Error>> {
    Ok(PipSurface::new(
        DmaBufFd::new(SCAFFOLD_FD)?,
        ShellLayer::Overlay,
        PixelExtent::new(640)?,
        PixelExtent::new(360)?,
    ))
}

/// Returns a controller with the fixture surface registered.
///
/// # Errors
///
/// Propagates [`surface`] and the registration refusal.
pub fn registered() -> Result<WaylandPipMediaController, Box<dyn std::error::Error>> {
    let mut controller = WaylandPipMediaController::new();
    controller.register(surface()?)?;
    Ok(controller)
}

/// Returns an overlay registration for the fixture surface.
///
/// # Errors
///
/// Propagates the field constructors.
pub fn registration() -> Result<OverlayRegistration, Box<dyn std::error::Error>> {
    Ok(OverlayRegistration {
        schema: OverlayRegistrationVersion::V1,
        edge: EdgeId::RegisterPipOverlay,
        correlation_id: correlation()?,
        surface: surface_id()?,
        layer: ShellLayer::Overlay,
        dma_buf: DmaBufFd::new(SCAFFOLD_FD)?,
        width: PixelExtent::new(640)?,
        height: PixelExtent::new(360)?,
    })
}

/// Scalar bound on directories visited and entries read per directory.
pub const WALK_BOUND: usize = 64;

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

/// Scalar bound on the lines read from any one source file.
pub const LINE_BOUND: usize = 4096;

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
/// A list of constructs is a gate only if each entry is one the scan reports,
/// and that is checkable without writing a file.
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
