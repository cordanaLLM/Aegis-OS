// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The subsystem-graph edge P15 produces on.
//!
//! The graph of record (export-062 `1ce919ed54bb`) names one outbound edge for
//! P15: `REGISTER_PIP_OVERLAY`, from P15 Hestia to P04 Mercurius, carried by
//! the `wlr-layer-shell` overlay surface protocol. The direction is not
//! disputed.
//!
//! The edge is a field of the registration and a constant of the registration
//! type. [`EdgeId`] admits that edge and no other, so a payload asserting a
//! different edge does not decode.

/// An edge of the subsystem graph that P15 Hestia produces on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum EdgeId {
    /// P15 Hestia to P04 Mercurius: pin a picture-in-picture overlay surface.
    #[serde(rename = "REGISTER_PIP_OVERLAY")]
    RegisterPipOverlay,
}

impl EdgeId {
    /// Every edge, in the order the graph of record lists them.
    pub const ALL: [Self; 1] = [Self::RegisterPipOverlay];

    /// Returns the edge identifier exactly as the graph of record spells it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::RegisterPipOverlay => "REGISTER_PIP_OVERLAY",
        }
    }

    /// Returns the component identifier at the far end of the edge.
    #[must_use]
    pub const fn consumer(self) -> &'static str {
        match self {
            Self::RegisterPipOverlay => "P04",
        }
    }

    /// Returns the transport the graph of record names for the edge.
    ///
    /// Recorded, not implemented: no transport exists in this crate, and the
    /// roadmap keeps it stubbed until M12.
    #[must_use]
    pub const fn recorded_transport(self) -> &'static str {
        match self {
            Self::RegisterPipOverlay => "wlr-layer-shell Overlay Surface Protocol",
        }
    }
}
