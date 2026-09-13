// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded Wayland surface registry (REQ-P04-04, REQ-P04-07).
//!
//! This is the scaffold's `AegisCompositor` surface half with the three things
//! it lacked: the capacity refusal is a typed error rather than a string, a
//! duplicate identifier is refused rather than stored twice, and focusing a
//! surface the registry does not hold is a refusal rather than a sweep that
//! quietly clears every focus flag and sets none.
//!
//! # What this module does not do
//!
//! **Nothing is composited, rendered, mapped or displayed.** No Wayland
//! connection is opened, no `wl_surface` exists, no layer-shell protocol is
//! bound, no buffer is attached and no output is driven. A surface is a row in
//! a fixed array, and a "layer" is a value of [`SurfaceLayer`]. Milestone M12
//! is where a real display path would be demonstrated.
//!
//! The registry is backend-agnostic on purpose: decision D08 selects a pure
//! Rust compositor and admits no compositor library at all at this milestone,
//! so nothing here names one. See [`crate::decision`].

use crate::error::CompositorError;
use crate::id::Label;

/// Scalar upper bound on the surface registry.
///
/// The scaffold's `MAX_SURFACES`, labelled there as a NASA JPL P10 compliance
/// bound (export-027 `f19640d7a7da`, REQ-P04-04).
pub const MAX_SURFACES: usize = 256;

/// The memory ceiling the compositor unit declares, in mebibytes.
///
/// Recorded from export-028 `d74a93eac654`, where the systemd unit sets
/// `MemoryMax=256M` (REQ-P04-05). Recorded, not applied: this crate writes no
/// control group and allocates nothing that a ceiling would bound.
pub const UNIT_MEMORY_MAX_MIB: u32 = 256;

/// The four `wlr-layer-shell` rendering layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum SurfaceLayer {
    /// Ambient surfaces, lowest.
    Background,
    /// Standard application surfaces.
    Bottom,
    /// Focused user interface and active agent interfaces.
    Top,
    /// High-priority safety alerts and the admission gate, highest.
    Overlay,
}

impl SurfaceLayer {
    /// All four layers, lowest first.
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

    /// Returns the stacking index, lowest first.
    #[must_use]
    pub const fn index(self) -> u32 {
        match self {
            Self::Background => 0,
            Self::Bottom => 1,
            Self::Top => 2,
            Self::Overlay => 3,
        }
    }

    /// Returns `true` when this layer carries the safety-alert admission gate.
    #[must_use]
    pub const fn is_admission_gate(self) -> bool {
        matches!(self, Self::Overlay)
    }
}

/// A surface identifier.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct SurfaceId(u32);

impl SurfaceId {
    /// Names a surface by its raw identifier.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw identifier.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// The pixel dimensions of one surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceGeometry {
    /// The width in pixels.
    pub width: u32,
    /// The height in pixels.
    pub height: u32,
}

impl SurfaceGeometry {
    /// Names a geometry.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// Everything needed to register one surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceRequest {
    /// The surface identifier.
    pub id: SurfaceId,
    /// The surface title.
    pub title: Label,
    /// The surface dimensions.
    pub geometry: SurfaceGeometry,
    /// The layer the surface is placed on.
    pub layer: SurfaceLayer,
    /// The process that owns the surface.
    pub pid: u32,
}

/// One registry row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaylandSurface {
    /// The surface identifier.
    pub id: SurfaceId,
    /// The surface title.
    pub title: Label,
    /// The surface dimensions.
    pub geometry: SurfaceGeometry,
    /// The layer the surface is placed on.
    pub layer: SurfaceLayer,
    /// The process that owns the surface.
    pub pid: u32,
    /// Whether the surface currently holds focus.
    pub focused: bool,
}

/// The bounded surface registry.
///
/// `Copy`, like every value on a decision path in this crate: the whole
/// registry is a fixed array, so registering a surface allocates nothing.
#[derive(Debug, Clone, Copy)]
pub struct SurfaceRegistry {
    surfaces: [Option<WaylandSurface>; MAX_SURFACES],
    count: usize,
    focused: Option<SurfaceId>,
}

impl Default for SurfaceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SurfaceRegistry {
    /// Builds an empty registry.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            surfaces: [None; MAX_SURFACES],
            count: 0,
            focused: None,
        }
    }

    /// Registers one layer surface.
    ///
    /// # Errors
    ///
    /// Returns [`CompositorError::DuplicateSurface`] when the registry already
    /// holds that identifier -- the scaffold stores a second row instead --
    /// and [`CompositorError::SurfaceRegistryFull`] at [`MAX_SURFACES`].
    pub fn register(&mut self, request: SurfaceRequest) -> Result<SurfaceId, CompositorError> {
        if self.get(request.id).is_some() {
            return Err(CompositorError::DuplicateSurface {
                id: request.id.get(),
            });
        }
        if self.count >= MAX_SURFACES {
            return Err(CompositorError::SurfaceRegistryFull { max: MAX_SURFACES });
        }
        let surface = WaylandSurface {
            id: request.id,
            title: request.title,
            geometry: request.geometry,
            layer: request.layer,
            pid: request.pid,
            focused: false,
        };
        let cell = self
            .surfaces
            .get_mut(self.count)
            .ok_or(CompositorError::SurfaceRegistryFull { max: MAX_SURFACES })?;
        *cell = Some(surface);
        self.count = self.count.saturating_add(1);
        Ok(request.id)
    }

    /// Moves focus to `id` and returns the surface that now holds it.
    ///
    /// # Errors
    ///
    /// Returns [`CompositorError::UnknownSurface`] when the registry holds no
    /// such surface, and changes nothing. The scaffold instead sweeps the
    /// whole table clearing every focus flag, so an unknown identifier leaves
    /// the session with no focused surface at all.
    pub fn focus(&mut self, id: SurfaceId) -> Result<WaylandSurface, CompositorError> {
        if self.get(id).is_none() {
            return Err(CompositorError::UnknownSurface { id: id.get() });
        }
        let mut focused = None;
        for cell in self.surfaces.iter_mut().take(MAX_SURFACES) {
            let Some(row) = cell.as_mut() else { continue };
            row.focused = row.id == id;
            if row.focused {
                focused = Some(*row);
            }
        }
        self.focused = Some(id);
        focused.ok_or(CompositorError::UnknownSurface { id: id.get() })
    }

    /// Returns the row for `id`, when the registry holds one.
    #[must_use]
    pub fn get(&self, id: SurfaceId) -> Option<WaylandSurface> {
        self.surfaces
            .iter()
            .take(MAX_SURFACES)
            .flatten()
            .find(|row| row.id == id)
            .copied()
    }

    /// Returns the surface that currently holds focus, if any.
    #[must_use]
    pub const fn focused(&self) -> Option<SurfaceId> {
        self.focused
    }

    /// Returns how many surfaces the registry holds.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Returns `true` when the registry holds no surface.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Returns how many surfaces sit on `layer`.
    #[must_use]
    pub fn layer_count(&self, layer: SurfaceLayer) -> usize {
        self.surfaces
            .iter()
            .take(MAX_SURFACES)
            .flatten()
            .filter(|row| row.layer == layer)
            .count()
    }
}
