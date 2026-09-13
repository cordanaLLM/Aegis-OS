// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! What the P04 registry, client table and mesh refuse, and why.
//!
//! The imported scaffold (export-027 `f19640d7a7da`) returns
//! `Result<_, String>` from every entry point, so `"Surface registry full"`
//! and `"Payload exceeds Zenoh SHM buffer limit"` are the same type and
//! neither carries the bound it hit. Two of its refusals are not refusals at
//! all: the client table silently drops a connection past its bound, and
//! `focus_surface` returns nothing for an identifier the registry does not
//! hold. Each is a variant here instead, and each carries the bound or the
//! value it refused.

use crate::id::IdError;

/// Reasons a P04 table, constructor or mesh operation refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CompositorError {
    /// The surface registry is full.
    #[error("the surface registry holds its maximum of {max} surfaces")]
    SurfaceRegistryFull {
        /// The scalar bound that was hit.
        max: usize,
    },
    /// The registry already holds a surface with that identifier.
    #[error("the surface registry already holds surface {id}")]
    DuplicateSurface {
        /// The identifier that was offered.
        id: u32,
    },
    /// The registry holds no surface with that identifier.
    #[error("the surface registry holds no surface {id}")]
    UnknownSurface {
        /// The identifier that was named.
        id: u32,
    },
    /// The Tier-1 client table is full.
    ///
    /// The scaffold's own handling of this case is to accept the connection
    /// and then drop it without telling anyone, which is why this is a typed
    /// refusal rather than a silent `if`.
    #[error("the Tier-1 client table holds its maximum of {max} clients")]
    ClientTableFull {
        /// The scalar bound that was hit.
        max: usize,
    },
    /// The mesh topic table is full.
    #[error("the mesh holds its maximum of {max} retained topics")]
    MeshTopicTableFull {
        /// The scalar bound that was hit.
        max: usize,
    },
    /// A sample does not fit the mesh's retained-sample bound.
    #[error("a sample of {bytes} bytes exceeds the retained-sample bound of {max}")]
    SampleTooLarge {
        /// The size that was offered.
        bytes: usize,
        /// The scalar bound.
        max: usize,
    },
    /// A subscriber tried to publish.
    ///
    /// REQ-P04-03's immutable-read control: attaching a reader must not let it
    /// act back on the stream it is reading.
    #[error("a subscriber may not publish: immutable-read controls forbid back-action")]
    RetroactivityRefused,
    /// The mesh holds no subscription with that identifier.
    #[error("the mesh holds no subscription {id}")]
    UnknownSubscription {
        /// The identifier that was named.
        id: u32,
    },
    /// A refresh rate was outside the range a frame period can be taken of.
    #[error("a refresh rate of {hz} Hz is outside {min}..={max}")]
    RefreshRateOutOfRange {
        /// The rate that was offered.
        hz: u32,
        /// The lowest admissible rate.
        min: u32,
        /// The highest admissible rate.
        max: u32,
    },
    /// An identifier was refused by its validating constructor.
    #[error("an identifier was refused: {0}")]
    Identifier(#[from] IdError),
}
