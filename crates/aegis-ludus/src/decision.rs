// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The two P11 decisions: D12, settled, and D48, open.
//!
//! # D12, settled: no Steamworks SDK
//!
//! REQ-P11-01 has the P11 runtime wrapping proprietary C++ SDK headers through
//! a binding generator, while the trade-off matrix records free and open-source
//! compliance as the same subsystem's declared constraint. The M01 register
//! carries that as dispute DSP-26; ADR-0002 settled it on 2026-09-13 by
//! excluding the SDK from the Aegis image, and REQ-P11-01 stays in the
//! requirement set as source evidence, marked superseded.
//!
//! Applying it here means the crate declares no such dependency and names no
//! such symbol. `tests/manifest_hygiene.rs` reads the manifest and refuses the
//! binding generator and every SDK crate by name; `tests/no_steamworks.rs`
//! sweeps the crate's own sources for the same names and shows that the sweep
//! reports a planted line. Both are checks over the names they list, not proofs
//! that no proprietary code could ever arrive by another route; what narrows
//! the second half is the dependency set, which the manifest test pins closed,
//! name by name, so a renamed declaration fails it too.
//!
//! ADR-0002 also says the image carries no Steamworks SDK. **This crate cannot
//! check that**, because the repository builds no image: the image gate is
//! blocked and `make readiness` is where its state is recorded. What is checked
//! here is the crate.
//!
//! # D48, open: the chat-platform presence integration
//!
//! The same reasoning applied to the rich-presence path is decision D48, and it
//! is **not settled**. The register asks whether a proprietary,
//! network-dependent integration belongs in a local-first system at all, or
//! whether local inter-process communication to a user-installed client is a
//! different case; it also records that the sources name no library and no
//! version. So this milestone types no rich-presence payload: a schema for an
//! edge that may be removed, built against a library nobody has named, would be
//! work that argues for one reading of an open decision. Both readings stay
//! representable in [`PresenceDisposition`], and the edge stays untyped until
//! D48 closes.

/// Whether a recorded decision is settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DecisionState {
    /// The decision is settled and the register names what was chosen.
    Closed,
    /// The decision is recorded and still open; every reading stays
    /// representable.
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

/// The three ways D12 could have reconciled the SDK with the constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SteamworksDisposition {
    /// The SDK is excluded from the image and from this crate.
    ExcludedFromImage,
    /// The SDK sits behind a build flag with a recorded licence decision.
    OptionalFeatureBehindBuildFlag,
    /// The SDK is wrapped as the blueprint drafted it.
    ProceedAsDrafted,
}

impl SteamworksDisposition {
    /// All three options, in the order the register lists them.
    ///
    /// The rejected options stay representable so the register states a choice
    /// between three readings rather than asserting the only one it can spell.
    pub const ALL: [Self; 3] = [
        Self::ExcludedFromImage,
        Self::OptionalFeatureBehindBuildFlag,
        Self::ProceedAsDrafted,
    ];

    /// Returns the stable name this option is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ExcludedFromImage => "excluded-from-image",
            Self::OptionalFeatureBehindBuildFlag => "optional-feature-behind-build-flag",
            Self::ProceedAsDrafted => "proceed-as-drafted",
        }
    }

    /// Returns `true` when the option would put the proprietary SDK in the
    /// crate's own dependency set under some configuration.
    #[must_use]
    pub const fn admits_proprietary_sdk(self) -> bool {
        !matches!(self, Self::ExcludedFromImage)
    }
}

/// The two readings D48 leaves open for the presence integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PresenceDisposition {
    /// Excluded, by the reasoning ADR-0002 applied to the SDK.
    ExcludedAsProprietaryAndNetworkDependent,
    /// Kept, as local inter-process communication to a user-installed client.
    KeptAsLocalIpcToUserInstalledClient,
}

impl PresenceDisposition {
    /// Both readings, in the order the register lists them.
    pub const BOTH: [Self; 2] = [
        Self::ExcludedAsProprietaryAndNetworkDependent,
        Self::KeptAsLocalIpcToUserInstalledClient,
    ];

    /// Returns the stable name this reading is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ExcludedAsProprietaryAndNetworkDependent => "excluded-as-proprietary",
            Self::KeptAsLocalIpcToUserInstalledClient => "kept-as-local-ipc",
        }
    }
}

/// A source, cited by export identifier and digest prefix only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Citation {
    /// The export identifier of the source.
    pub export: &'static str,
    /// The first twelve hexadecimal characters of that export's sha256.
    pub sha256_prefix: &'static str,
}

/// D12: what P11 does with the proprietary SDK. Settled by ADR-0002.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SdkDecision {
    /// The decision identifier used in `docs/roadmap/README.md`.
    pub id: &'static str,
    /// The question the decision answers.
    pub question: &'static str,
    /// Whether the decision is settled.
    pub state: DecisionState,
    /// The option that was chosen.
    pub chosen: SteamworksDisposition,
    /// The options that were not chosen.
    pub rejected: [SteamworksDisposition; 2],
    /// The architecture decision record that carries it.
    pub adr: &'static str,
    /// The milestone that applies the decision.
    pub applied_at: &'static str,
    /// The requirements the decision touches.
    pub touches: [&'static str; 2],
    /// The dispute this decision closes.
    pub dispute: &'static str,
    /// The sources that posed the question.
    pub sources: [Citation; 2],
    /// What this crate can check, and what it cannot.
    pub scope: &'static str,
}

/// D48: what P11 does with the chat-platform presence integration. Open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresenceDecision {
    /// The decision identifier used in `docs/roadmap/inventory.md`.
    pub id: &'static str,
    /// The question the decision leaves open.
    pub question: &'static str,
    /// Whether the decision is settled.
    pub state: DecisionState,
    /// The readings that stay representable while it is open.
    pub readings: [PresenceDisposition; 2],
    /// What this milestone did about it, and why that is not a decision.
    pub carried: &'static str,
    /// What closing the decision would take.
    pub settled_by: &'static str,
    /// The milestone that records it.
    pub recorded_at: &'static str,
    /// The edge the decision governs, as the graph of record spells it.
    pub edge: &'static str,
}

impl PresenceDecision {
    /// Returns `true` only for a settled decision.
    #[must_use]
    pub const fn is_settled(&self) -> bool {
        matches!(self.state, DecisionState::Closed)
    }
}

/// D12: the proprietary SDK is excluded from the image and from this crate.
pub const D12_STEAMWORKS_EXCLUSION: SdkDecision = SdkDecision {
    id: "D12",
    question: "How does P11 reconcile a proprietary SDK with its own free and open-source \
               compliance constraint?",
    state: DecisionState::Closed,
    chosen: SteamworksDisposition::ExcludedFromImage,
    rejected: [
        SteamworksDisposition::OptionalFeatureBehindBuildFlag,
        SteamworksDisposition::ProceedAsDrafted,
    ],
    adr: "docs/adr/0002-exclude-steamworks-from-image.md",
    applied_at: "M08",
    touches: ["REQ-P11-01", "REQ-P11-05"],
    dispute: "DSP-26",
    sources: [
        Citation {
            export: "export-019",
            sha256_prefix: "b0aa6e54ed57",
        },
        Citation {
            export: "export-004",
            sha256_prefix: "15831276a058",
        },
    ],
    scope: "the crate is checked by tests/manifest_hygiene.rs and tests/no_steamworks.rs; the \
            image is not, because this repository builds none and that gate is blocked",
};

/// D48: whether the presence integration survives ADR-0002's reasoning. Open.
pub const D48_PRESENCE_INTEGRATION: PresenceDecision = PresenceDecision {
    id: "D48",
    question: "Does the reasoning behind ADR-0002 also exclude the chat-platform presence \
               integration in P11?",
    state: DecisionState::Unresolved,
    readings: PresenceDisposition::BOTH,
    carried: "nothing: no rich-presence payload is typed at M08, because a schema for an edge \
              one reading removes would argue for the other reading, and the sources name no \
              library to build it against",
    settled_by: "an owner's decision, and then a named library with a verifiable version; the \
                 register records that the sources name neither",
    recorded_at: "M08",
    edge: "DISPATCH_RICH_PRESENCE",
};
