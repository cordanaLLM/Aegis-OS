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

#[path = "../../../source_effects.rs"]
mod source_effects;

#[allow(unused_imports)]
pub use source_effects::*;
