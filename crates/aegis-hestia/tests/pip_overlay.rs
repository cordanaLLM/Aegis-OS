// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The picture-in-picture controller, and the descriptor rule it holds.
//!
//! The third scaffold assertion -- the `DMA-BUF` descriptor is not negative --
//! is [`DmaBufFd::new`] here, so the negative case is an ordinary test. The
//! controller adds the state rule the scaffold left implicit: one surface at a
//! time.

mod common;

use aegis_hestia::{
    DmaBufFd, HestiaError, MAX_PIXEL_EXTENT, MIN_PIXEL_EXTENT, PipSurface, PixelExtent, ShellLayer,
    WaylandPipMediaController,
};

use common::{Fallible, SCAFFOLD_FD, registered, surface};

// --- Positive -------------------------------------------------------------

/// Positive: a surface registers, reports its area, and releases.
#[test]
fn a_surface_registers_and_releases() -> Fallible {
    let mut controller = WaylandPipMediaController::new();
    assert!(!controller.is_active());
    assert_eq!(controller.active(), None);

    let pinned: PipSurface = surface()?;
    controller.register(pinned)?;
    assert!(controller.is_active());
    assert_eq!(controller.active(), Some(pinned));
    assert_eq!(pinned.area(), 640 * 360);
    assert_eq!(pinned.layer, ShellLayer::Overlay);
    assert_eq!(pinned.dma_buf.get(), SCAFFOLD_FD);

    assert_eq!(controller.release()?, pinned);
    assert!(!controller.is_active());
    Ok(())
}

/// Positive: the protocol's four layers are all representable and named.
#[test]
fn every_shell_layer_is_representable() {
    assert_eq!(ShellLayer::ALL.len(), 4);
    assert_eq!(ShellLayer::Background.name(), "background");
    assert_eq!(ShellLayer::Bottom.name(), "bottom");
    assert_eq!(ShellLayer::Top.name(), "top");
    assert_eq!(ShellLayer::Overlay.name(), "overlay");
}

// --- Negative -------------------------------------------------------------

/// Negative: a negative descriptor is refused, and names the value.
#[test]
fn a_negative_descriptor_is_refused() {
    assert_eq!(
        DmaBufFd::new(-1),
        Err(HestiaError::InvalidDmaBufFd { fd: -1 })
    );
    assert_eq!(
        DmaBufFd::new(i32::MIN),
        Err(HestiaError::InvalidDmaBufFd { fd: i32::MIN })
    );
}

/// Negative: registering twice is refused, and the first surface stays.
#[test]
fn registering_twice_is_refused() -> Fallible {
    let mut controller = registered()?;
    let second = PipSurface::new(
        DmaBufFd::new(9)?,
        ShellLayer::Overlay,
        PixelExtent::new(320)?,
        PixelExtent::new(180)?,
    );
    assert_eq!(
        controller.register(second),
        Err(HestiaError::OverlayAlreadyActive)
    );
    assert_eq!(controller.active(), Some(surface()?));
    Ok(())
}

/// Negative: releasing with nothing registered is refused, not silent.
#[test]
fn releasing_nothing_is_refused() {
    let mut controller = WaylandPipMediaController::default();
    assert_eq!(controller.release(), Err(HestiaError::OverlayNotActive));
    assert!(!controller.is_active());
}

// --- Boundary -------------------------------------------------------------

/// Boundary: descriptor zero is admissible; minus one is not.
///
/// Zero is a file descriptor like any other, so the rule is `fd >= 0` and not
/// `fd > 0`. Refusing zero would refuse a descriptor a real system can hand
/// over.
#[test]
fn descriptor_zero_is_admissible() {
    assert_eq!(DmaBufFd::new(0).map(DmaBufFd::get), Ok(0));
    assert!(DmaBufFd::new(-1).is_err());
    assert_eq!(DmaBufFd::new(i32::MAX).map(DmaBufFd::get), Ok(i32::MAX));
}

/// Boundary: the extent bound is exact on both sides.
#[test]
fn the_extent_bound_is_exact_on_both_sides() {
    assert_eq!(
        PixelExtent::new(0),
        Err(HestiaError::ExtentOutOfRange { pixels: 0 })
    );
    assert_eq!(PixelExtent::new(1).map(PixelExtent::get), Ok(1));
    assert_eq!(
        PixelExtent::new(MAX_PIXEL_EXTENT).map(PixelExtent::get),
        Ok(MAX_PIXEL_EXTENT)
    );
    assert_eq!(
        PixelExtent::new(MAX_PIXEL_EXTENT.saturating_add(1)),
        Err(HestiaError::ExtentOutOfRange {
            pixels: MAX_PIXEL_EXTENT.saturating_add(1)
        })
    );
    assert_eq!(MIN_PIXEL_EXTENT, 1);
    assert_eq!(PixelExtent::MIN.get(), MIN_PIXEL_EXTENT);
    assert_eq!(PixelExtent::MAX.get(), MAX_PIXEL_EXTENT);
}

/// Boundary: the widest admissible surface does not overflow its area.
#[test]
fn the_widest_admissible_surface_has_a_finite_area() -> Fallible {
    let widest = PipSurface::new(
        DmaBufFd::new(0)?,
        ShellLayer::Overlay,
        PixelExtent::MAX,
        PixelExtent::MAX,
    );
    let bound = u64::from(MAX_PIXEL_EXTENT).saturating_mul(u64::from(MAX_PIXEL_EXTENT));
    assert_eq!(widest.area(), bound);
    assert!(widest.area() < u64::MAX);
    Ok(())
}
