// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The P07 claims this crate records but cannot check, and the imported
//! dependency pins it does not inherit.
//!
//! Six recorded requirements describe P07 in terms no crate-level test
//! reaches: a determinism guarantee that needs a real-time kernel
//! (REQ-P07-01), a boot-parameter and affinity configuration (REQ-P07-02), an
//! eBPF library and a loaded program (REQ-P07-03, REQ-P07-05), a version pin
//! that arrived as proposal data (REQ-P01-07), and two sources that declare
//! different tier counts for the same subsystem (REQ-P07-04). This module is a
//! register for them, in the shape `aegis-vesta` uses for P10.
//!
//! **It admits and refuses nothing, and it endorses nothing.** The status
//! field is the point: a claim recorded as
//! [`ClaimStatus::DeferredTimingRequirement`] is written down so it is not
//! lost, not so it can be counted as delivered.
//!
//! # The pins that are not inherited
//!
//! [`P07_PROPOSAL_PINS`] records the versions the imported workspace manifest
//! declares for the two libraries P07 would need. They are **proposal data**:
//! the repository contract says an imported dependency version does not become
//! an active pin by being imported, and `tests/manifest_hygiene.rs` reads this
//! crate's manifest and fails if either name appears in it. A version is
//! selected when the component that needs it is activated, against current
//! upstream at that moment, and not before.

/// A source, cited by export identifier and digest prefix only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClaimSource {
    /// The export identifier of the source.
    pub export: &'static str,
    /// The first twelve hexadecimal characters of that export's sha256.
    pub sha256_prefix: &'static str,
}

/// What standing a recorded claim has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ClaimStatus {
    /// A timing figure that needs a real-time kernel.
    DeferredTimingRequirement,
    /// A requirement that needs a privileged configuration action.
    DeferredPrivilegedConfiguration,
    /// A requirement that needs a toolchain this repository has not admitted.
    DeferredToolchainAdmission,
    /// Two recorded statements that cannot both hold as written.
    UnresolvedSourceConflict,
}

impl ClaimStatus {
    /// Returns the stable name this status is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::DeferredTimingRequirement => "deferred-timing-requirement",
            Self::DeferredPrivilegedConfiguration => "deferred-privileged-configuration",
            Self::DeferredToolchainAdmission => "deferred-toolchain-admission",
            Self::UnresolvedSourceConflict => "unresolved-source-conflict",
        }
    }
}

/// One recorded claim about P07.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordedClaim {
    /// The recorded requirement identifier.
    pub requirement: &'static str,
    /// What the requirement says, in one line.
    pub summary: &'static str,
    /// What standing the claim has.
    pub status: ClaimStatus,
    /// What would be needed to settle it.
    pub settled_by: &'static str,
    /// The source that states it.
    pub source: ClaimSource,
}

/// The six P07 claims this milestone records and does not discharge.
pub const P07_RECORDED_CLAIMS: [RecordedClaim; 6] = [
    RecordedClaim {
        requirement: "REQ-P07-01",
        summary: "Lictor targets a kernel built with CONFIG_PREEMPT_RT and CONFIG_HZ_1000 for \
                  deterministic tier-0 and tier-1 response",
        status: ClaimStatus::DeferredTimingRequirement,
        settled_by: "latency fixtures on a PREEMPT_RT kernel, which is milestone M23. The \
                     reference profile runs PREEMPT_DYNAMIC with CONFIG_HZ_1000, so half the \
                     precondition holds and the determinism claim still cannot be tested \
                     here; no latency or determinism figure is produced by this milestone",
        source: ClaimSource {
            export: "export-016",
            sha256_prefix: "cfa58b5b23b8",
        },
    },
    RecordedClaim {
        requirement: "REQ-P07-02",
        summary: "isolcpus pins the performance-critical threads to physical cores as \
                  Locality Enforcement rule P-002",
        status: ClaimStatus::DeferredPrivilegedConfiguration,
        settled_by: "a boot parameter and an affinity call; CoreMask carries the set of \
                     logical CPUs and nothing here sets an affinity or reads a topology",
        source: ClaimSource {
            export: "export-016",
            sha256_prefix: "cfa58b5b23b8",
        },
    },
    RecordedClaim {
        requirement: "REQ-P07-03",
        summary: "the lictor-d daemon uses the Aya eBPF library to manage the resource-broker \
                  mechanism record tuple",
        status: ClaimStatus::DeferredToolchainAdmission,
        settled_by: "an admitted eBPF toolchain and a loaded object, which is milestone M19; \
                     decision D40 is open and no eBPF library is declared by this crate",
        source: ClaimSource {
            export: "export-016",
            sha256_prefix: "cfa58b5b23b8",
        },
    },
    RecordedClaim {
        requirement: "REQ-P07-05",
        summary: "eBPF-driven fragility probes test shadowed structural changes on a \
                  non-authoritative path before promotion",
        status: ClaimStatus::DeferredToolchainAdmission,
        settled_by: "a loaded program that actually observes something, which is milestone \
                     M19. The ordering half -- shadowed, then observed, then promoted -- is \
                     enforced here by ProbeStage::may_advance_to, and only that half",
        source: ClaimSource {
            export: "export-004",
            sha256_prefix: "15831276a058",
        },
    },
    RecordedClaim {
        requirement: "REQ-P01-07",
        summary: "the imported workspace manifest declares the aya crate at version 0.12 \
                  under its eBPF and kernel-bypass dependencies",
        status: ClaimStatus::DeferredToolchainAdmission,
        settled_by: "an eBPF toolchain admission at milestone M19, which selects a version \
                     against current upstream at that moment. The imported figure is \
                     proposal data and is recorded in P07_PROPOSAL_PINS as not inherited",
        source: ClaimSource {
            export: "export-006",
            sha256_prefix: "6e694e01e136",
        },
    },
    RecordedClaim {
        requirement: "REQ-P07-04",
        summary: "the scheduler source classifies tasks into four burst tiers while the \
                  daemon source classifies processes into three drm_sched tiers, and no \
                  source relates the two",
        status: ClaimStatus::UnresolvedSourceConflict,
        settled_by: "a recorded decision about which classification the broker publishes; \
                     none exists, so this crate carries both -- Tier from the scheduler and \
                     ProcessTier from the daemon -- and states the mapping between them as \
                     this crate's reading rather than as a recorded one",
        source: ClaimSource {
            export: "export-056",
            sha256_prefix: "39af243568ad",
        },
    },
];

/// One dependency version the imported workspace manifest proposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProposalPin {
    /// The crate the imported manifest names.
    pub crate_name: &'static str,
    /// The version requirement the imported manifest declares.
    pub proposed_version: &'static str,
    /// Whether this repository's manifest inherits that requirement.
    pub inherited: bool,
    /// Where the version would be selected instead.
    pub selected_at: &'static str,
    /// The source that declares it.
    pub source: ClaimSource,
}

/// The two eBPF pins the imported manifest proposes, neither of them inherited.
pub const P07_PROPOSAL_PINS: [ProposalPin; 2] = [
    ProposalPin {
        crate_name: "aya",
        proposed_version: "0.12",
        inherited: false,
        selected_at: "M19, against current upstream at that moment",
        source: ClaimSource {
            export: "export-006",
            sha256_prefix: "6e694e01e136",
        },
    },
    ProposalPin {
        crate_name: "aya-bpf",
        proposed_version: "0.1",
        inherited: false,
        selected_at: "M19, against current upstream at that moment",
        source: ClaimSource {
            export: "export-006",
            sha256_prefix: "6e694e01e136",
        },
    },
];
