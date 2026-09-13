// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Decision D09, applied: where P15 Hestia lives, and what joins the halves.
//!
//! Two imported sources place P15 in two different trees. One puts a Rust
//! daemon under `crates/aegis-hestia`; the other's directory map lists only
//! `ui/hestia-app` and carries no crate row at all. The proposed workspace
//! lists neither (REQ-WS-01). The M01 inventory registered the disagreement as
//! DSP-24 and closed it.
//!
//! **The decision, recorded 2026-09-13: both.** A Rust crate holds the storage
//! and vector logic; a Svelte package holds the user interface; and the two
//! are joined by a typed boundary rather than by a shared implementation.
//! Milestone M17 is where that is applied, and this crate is the Rust half of
//! it.
//!
//! # What applying it means here
//!
//! * The storage and vector logic is in this crate and nowhere else:
//!   [`PgliteVectorStore`](crate::PgliteVectorStore), [`QueryLimit`](crate::QueryLimit)
//!   and [`StoragePath`](crate::StoragePath) carry the rules REQ-P15-05 and
//!   REQ-P15-08 state.
//! * The boundary is a payload, not a linkage: [`HestiaView`](crate::HestiaView)
//!   is a versioned, bounded snapshot the crate renders and the package reads.
//!   It carries no method, no handle and no callback, so neither half can
//!   reach into the other.
//! * The user interface is **not** here. This crate renders no markup, ships
//!   no component and declares no JavaScript dependency. The Svelte package,
//!   its manifest, its lockfile and its accessibility test are separate work;
//!   D10 has still to pin the toolchain that would build it.
//!
//! # Scope
//!
//! Recording and applying D09 is not evidence that a Svelte package exists or
//! consumes this boundary. No package is added by this milestone, and nothing
//! here is proof about one.

/// Whether a recorded decision is settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DecisionState {
    /// The decision is settled and the register names what was chosen.
    Closed,
    /// The decision is recorded but not settled.
    Unresolved,
}

impl DecisionState {
    /// Returns the stable name this state is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Unresolved => "unresolved",
        }
    }
}

/// The three places D09 could have put P15.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum HestiaLocation {
    /// A Rust crate and nothing else.
    RustCrateOnly,
    /// A Svelte package and nothing else.
    SveltePackageOnly,
    /// Both, joined by a typed boundary.
    BothWithTypedBoundary,
}

impl HestiaLocation {
    /// All three options, in the order the register lists them.
    ///
    /// The rejected options stay representable so the register states a choice
    /// between three readings rather than asserting the only one it can spell.
    pub const ALL: [Self; 3] = [
        Self::RustCrateOnly,
        Self::SveltePackageOnly,
        Self::BothWithTypedBoundary,
    ];

    /// Returns the stable name this option is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::RustCrateOnly => "rust-crate-only",
            Self::SveltePackageOnly => "svelte-package-only",
            Self::BothWithTypedBoundary => "both-with-typed-boundary",
        }
    }

    /// Returns `true` when the option puts storage and vector logic in Rust.
    #[must_use]
    pub const fn includes_rust_crate(self) -> bool {
        matches!(self, Self::RustCrateOnly | Self::BothWithTypedBoundary)
    }

    /// Returns `true` when the option puts the user interface in Svelte.
    #[must_use]
    pub const fn includes_ui_package(self) -> bool {
        matches!(self, Self::SveltePackageOnly | Self::BothWithTypedBoundary)
    }
}

/// A private source, cited by export identifier and digest prefix only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Citation {
    /// The export identifier of the private source.
    pub export: &'static str,
    /// The first twelve hexadecimal characters of that export's sha256.
    pub sha256_prefix: &'static str,
}

/// One recorded decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecisionRecord {
    /// The decision identifier used in `docs/roadmap/README.md`.
    pub id: &'static str,
    /// The question the decision answers.
    pub question: &'static str,
    /// Whether the decision is settled.
    pub state: DecisionState,
    /// The option that was chosen.
    pub chosen: HestiaLocation,
    /// The options that were not chosen.
    pub rejected: [HestiaLocation; 2],
    /// What joins the two halves.
    pub boundary: &'static str,
    /// The milestone that applies the decision.
    pub applied_at: &'static str,
    /// The requirements the decision touches.
    pub touches: [&'static str; 3],
    /// The dispute this decision closes.
    pub dispute: &'static str,
    /// The private sources that posed the question.
    pub sources: [Citation; 2],
}

/// D09: where P15 Hestia lives.
pub const D09_HESTIA_LOCATION: DecisionRecord = DecisionRecord {
    id: "D09",
    question: "Where does P15 Hestia live: a Rust crate, a Svelte package, or both?",
    state: DecisionState::Closed,
    chosen: HestiaLocation::BothWithTypedBoundary,
    rejected: [
        HestiaLocation::RustCrateOnly,
        HestiaLocation::SveltePackageOnly,
    ],
    boundary: "a versioned, bounded snapshot payload the crate renders and the package \
               reads; no method, handle or callback crosses it",
    applied_at: "M17",
    touches: ["REQ-P15-05", "REQ-P15-08", "REQ-WS-01"],
    dispute: "DSP-24",
    sources: [
        Citation {
            export: "export-030",
            sha256_prefix: "689d175667d6",
        },
        Citation {
            export: "export-007",
            sha256_prefix: "84f43472c536",
        },
    ],
};
