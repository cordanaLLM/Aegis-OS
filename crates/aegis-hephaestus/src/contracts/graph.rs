// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The subsystem-graph edge P14's own contract carries.
//!
//! The graph of record (export-062 `1ce919ed54bb`) names one outbound edge for
//! P14: `RENDER_GEOMETRY_MICROFRONTEND`, from P14 Hephaestus to P15 Hestia,
//! carrying `STEP AP242` and mesh viewports (REQ-P14-06). The direction is not
//! disputed -- but the edge is one of the eight the M01 register records as
//! appearing in the graph of record and in **neither** architecture document,
//! which is dispute DSP-19 against open decision D37. So it is typed on a
//! single supporting source, and [`EdgeId::sources`] records that.
//!
//! P14's inbound edge is not here. `VERIFY_CODE_CAD` is
//! [`EdgeId::VerifyCodeCad`](aegis_minerva::EdgeId::VerifyCodeCad) in the
//! producer's crate, and this crate reads that constant rather than declaring
//! its own spelling of the same edge.

/// An edge of the subsystem graph that P14 Hephaestus produces on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum EdgeId {
    /// P14 to P15: a `STEP` and mesh viewport for the geometry surface.
    #[serde(rename = "RENDER_GEOMETRY_MICROFRONTEND")]
    RenderGeometryMicrofrontend,
}

impl EdgeId {
    /// Every edge this enum names.
    pub const ALL: [Self; 1] = [Self::RenderGeometryMicrofrontend];

    /// Returns the edge identifier as it is written in the graph of record.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::RenderGeometryMicrofrontend => "RENDER_GEOMETRY_MICROFRONTEND",
        }
    }

    /// Returns the component identifier at the far end of the edge.
    #[must_use]
    pub const fn consumer(self) -> &'static str {
        match self {
            Self::RenderGeometryMicrofrontend => "P15",
        }
    }

    /// Returns the transport the graph of record records for this edge.
    ///
    /// Recorded verbatim. Neither half of that transport is admitted by any
    /// gate this milestone touches, nothing in this crate acts on the string,
    /// and decision D09 is what splits P15 into the two halves it names.
    #[must_use]
    pub const fn recorded_transport(self) -> &'static str {
        match self {
            Self::RenderGeometryMicrofrontend => "Svelte 5 Micro-Frontend / PGlite OPFS",
        }
    }

    /// Returns how many sources of the imported bundle draw this edge.
    ///
    /// One, which is dispute DSP-19: the graph of record draws it and neither
    /// architecture document names it. A schema on a single-source edge is
    /// still a schema; it is not a two-source agreement, and open decision D37
    /// is where that is settled.
    #[must_use]
    pub const fn sources(self) -> u8 {
        match self {
            Self::RenderGeometryMicrofrontend => 1,
        }
    }
}
