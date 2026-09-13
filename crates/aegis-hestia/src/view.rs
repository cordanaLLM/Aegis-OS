// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The D09 typed boundary: what the Rust half tells the Svelte half.
//!
//! Decision D09 splits P15 into a Rust crate for the storage and vector logic
//! and a Svelte package for the user interface, joined by a typed boundary.
//! [`HestiaView`] is that boundary: a versioned snapshot of everything the
//! interface needs to render, and nothing else.
//!
//! # Why a payload rather than an interface
//!
//! A boundary made of methods would give the interface a handle on the store
//! and the store a way to call back into the interface. A boundary made of one
//! flat, `Copy`, versioned payload cannot: it carries no function, no handle,
//! no descriptor and no path into the crate. The package reads a snapshot; if
//! the snapshot is stale, it asks for another one.
//!
//! # Versioning
//!
//! The payload carries an explicit `schema` tag as its first field, typed as
//! an enum with one variant per admitted version, so a package built against
//! another version fails to decode rather than mis-reading a field.
//!
//! # What this module does not do
//!
//! It renders no markup, ships no component, declares no JavaScript dependency
//! and names no package manager. Whether a Svelte package exists that reads
//! this payload is not a fact this crate can establish, and this milestone
//! adds none.

use crate::overlay::WaylandPipMediaController;
use crate::store::{MAX_QUERY_LIMIT, MIN_QUERY_LIMIT, PgliteVectorStore, StoreState};

/// The versions of the P15 view payload this build admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum ViewVersion {
    /// Version 1, tagged `aegis.p15.ui-view.v1`.
    #[serde(rename = "aegis.p15.ui-view.v1")]
    V1,
}

impl ViewVersion {
    /// Returns the stable version tag the payload's `schema` field carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::V1 => "aegis.p15.ui-view.v1",
        }
    }
}

/// What the interface is told about the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct StoreView {
    /// Whether the store will answer a query.
    pub initialised: bool,
    /// The buffer the recorded requirement allocates, in megabytes.
    pub buffer_mb: usize,
    /// The smallest query limit the interface may ask for.
    pub min_query_limit: usize,
    /// The largest query limit the interface may ask for.
    pub max_query_limit: usize,
}

/// What the interface is told about the picture-in-picture surface.
///
/// The surface's `DMA-BUF` descriptor is deliberately **not** here: a
/// descriptor is meaningful only inside the process that holds it, so handing
/// its number to a user interface would be misleading rather than useful.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PipView {
    /// Whether a surface is registered.
    pub active: bool,
    /// The surface width in pixels, when one is registered.
    pub width: Option<u32>,
    /// The surface height in pixels, when one is registered.
    pub height: Option<u32>,
}

/// The whole D09 boundary payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct HestiaView {
    /// The payload version this snapshot claims.
    pub schema: ViewVersion,
    /// The store half of the snapshot.
    pub store: StoreView,
    /// The picture-in-picture half of the snapshot.
    pub pip: PipView,
}

impl HestiaView {
    /// Renders the boundary payload from the two models.
    ///
    /// Reading is one pass over two `Copy` values, so rendering a snapshot
    /// allocates nothing; encoding it to JSON is a separate step the caller
    /// makes.
    #[must_use]
    pub fn snapshot(store: &PgliteVectorStore, pip: &WaylandPipMediaController) -> Self {
        let surface = pip.active();
        Self {
            schema: ViewVersion::V1,
            store: StoreView {
                initialised: matches!(store.state(), StoreState::Initialised),
                buffer_mb: store.buffer_mb(),
                min_query_limit: MIN_QUERY_LIMIT,
                max_query_limit: MAX_QUERY_LIMIT,
            },
            pip: PipView {
                active: surface.is_some(),
                width: surface.map(|value| value.width.get()),
                height: surface.map(|value| value.height.get()),
            },
        }
    }
}
