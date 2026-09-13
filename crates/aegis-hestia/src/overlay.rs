// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The picture-in-picture overlay controller.
//!
//! The scaffold's `WaylandPipMediaController` asserts one rule -- the
//! `DMA-BUF` descriptor it is handed is not negative -- and then records it.
//! Here that rule is [`DmaBufFd::new`], so a negative descriptor never becomes
//! a value, and the controller is a small state machine: one surface may be
//! registered at a time, and releasing one that was never registered is a
//! refusal rather than a silent success.
//!
//! # What this module does not do
//!
//! It connects to no Wayland compositor, binds no `wlr-layer-shell` protocol,
//! creates no surface, imports no `DMA-BUF` and receives no file descriptor
//! over a socket. A [`DmaBufFd`] is a validated integer. On a real system a
//! descriptor travels as an `SCM_RIGHTS` ancillary message and is meaningful
//! only inside the process that holds it; the number here names a descriptor
//! positionally so the contract has the field, and binding it to a real
//! descriptor is later work.

use crate::error::HestiaError;

/// The smallest admissible surface extent, in pixels.
pub const MIN_PIXEL_EXTENT: u32 = 1;

/// The largest admissible surface extent, in pixels.
///
/// This is a bound chosen here, not a recorded requirement. `DRM` states no
/// framebuffer dimension of its own: each driver sets
/// `mode_config.max_width` and `max_height`, and 16384 is the largest value
/// the two drivers on the reference profile set --
/// `drivers/gpu/drm/i915/display/intel_display_driver.c` from `DISPLAY_VER`
/// 7 upward, and `drivers/gpu/drm/amd/display/amdgpu_dm/amdgpu_dm.c`
/// unconditionally. Read on 2026-09-13 against Linux 7.2.4. An older or
/// smaller driver admits less -- the same `i915` arm falls to 8192, 4096 and
/// 2048 for earlier display generations -- so this bounds the field against a
/// decoding mistake; it does not promise that a compositor would accept it.
pub const MAX_PIXEL_EXTENT: u32 = 16384;

/// A non-negative `DMA-BUF` file descriptor.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(into = "i32", try_from = "i32")]
pub struct DmaBufFd(i32);

impl DmaBufFd {
    /// Validates a `DMA-BUF` file descriptor.
    ///
    /// # Errors
    ///
    /// Returns [`HestiaError::InvalidDmaBufFd`] for a negative descriptor.
    /// Zero is admitted: it is a descriptor like any other.
    pub const fn new(fd: i32) -> Result<Self, HestiaError> {
        if fd < 0 {
            return Err(HestiaError::InvalidDmaBufFd { fd });
        }
        Ok(Self(fd))
    }

    /// Returns the validated descriptor.
    #[must_use]
    pub const fn get(self) -> i32 {
        self.0
    }
}

impl From<DmaBufFd> for i32 {
    fn from(value: DmaBufFd) -> Self {
        value.0
    }
}

impl TryFrom<i32> for DmaBufFd {
    type Error = HestiaError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// A validated surface extent, within `1..=16384` pixels.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(into = "u32", try_from = "u32")]
pub struct PixelExtent(u32);

impl PixelExtent {
    /// The largest admissible extent, as a value.
    pub const MAX: Self = Self(MAX_PIXEL_EXTENT);

    /// The smallest admissible extent, as a value.
    pub const MIN: Self = Self(MIN_PIXEL_EXTENT);

    /// Validates a surface extent.
    ///
    /// # Errors
    ///
    /// Returns [`HestiaError::ExtentOutOfRange`] for zero and for anything
    /// past [`MAX_PIXEL_EXTENT`].
    pub const fn new(pixels: u32) -> Result<Self, HestiaError> {
        if pixels < MIN_PIXEL_EXTENT || pixels > MAX_PIXEL_EXTENT {
            return Err(HestiaError::ExtentOutOfRange { pixels });
        }
        Ok(Self(pixels))
    }

    /// Returns the validated extent.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl From<PixelExtent> for u32 {
    fn from(value: PixelExtent) -> Self {
        value.0
    }
}

impl TryFrom<u32> for PixelExtent {
    type Error = HestiaError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// A `wlr-layer-shell` layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ShellLayer {
    /// Behind every other surface.
    Background,
    /// Between the background and the ordinary surfaces.
    Bottom,
    /// Above the ordinary surfaces.
    Top,
    /// Above everything, which is where a pinned picture-in-picture goes.
    Overlay,
}

impl ShellLayer {
    /// Every layer the protocol defines, bottom to top.
    pub const ALL: [Self; 4] = [Self::Background, Self::Bottom, Self::Top, Self::Overlay];

    /// Returns the stable name this layer is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Bottom => "bottom",
            Self::Top => "top",
            Self::Overlay => "overlay",
        }
    }
}

/// One pinned picture-in-picture surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PipSurface {
    /// The imported buffer the surface displays.
    pub dma_buf: DmaBufFd,
    /// The layer the surface is pinned to.
    pub layer: ShellLayer,
    /// The surface width, in pixels.
    pub width: PixelExtent,
    /// The surface height, in pixels.
    pub height: PixelExtent,
}

impl PipSurface {
    /// Builds a surface from validated parts.
    #[must_use]
    pub const fn new(
        dma_buf: DmaBufFd,
        layer: ShellLayer,
        width: PixelExtent,
        height: PixelExtent,
    ) -> Self {
        Self {
            dma_buf,
            layer,
            width,
            height,
        }
    }

    /// Returns the pixels the surface covers.
    ///
    /// The product cannot overflow: both extents are bounded by
    /// [`MAX_PIXEL_EXTENT`].
    #[must_use]
    pub const fn area(&self) -> u64 {
        (self.width.get() as u64).saturating_mul(self.height.get() as u64)
    }
}

/// The P15 picture-in-picture controller, as a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WaylandPipMediaController {
    active: Option<PipSurface>,
}

impl WaylandPipMediaController {
    /// Builds a controller with no surface registered.
    #[must_use]
    pub const fn new() -> Self {
        Self { active: None }
    }

    /// Returns the registered surface, when one is registered.
    #[must_use]
    pub const fn active(&self) -> Option<PipSurface> {
        self.active
    }

    /// Returns whether a surface is registered.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active.is_some()
    }

    /// Registers one picture-in-picture surface.
    ///
    /// # Errors
    ///
    /// Returns [`HestiaError::OverlayAlreadyActive`] when one is already
    /// registered. The descriptor and both extents were already refused if
    /// they were out of range, because [`DmaBufFd`] and [`PixelExtent`] are
    /// the only ways to spell them.
    pub const fn register(&mut self, surface: PipSurface) -> Result<(), HestiaError> {
        if self.active.is_some() {
            return Err(HestiaError::OverlayAlreadyActive);
        }
        self.active = Some(surface);
        Ok(())
    }

    /// Releases the registered surface and returns it.
    ///
    /// # Errors
    ///
    /// Returns [`HestiaError::OverlayNotActive`] when none is registered.
    pub const fn release(&mut self) -> Result<PipSurface, HestiaError> {
        match self.active.take() {
            Some(surface) => Ok(surface),
            None => Err(HestiaError::OverlayNotActive),
        }
    }
}
