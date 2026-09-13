// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The P04 claims this crate records but cannot check.
//!
//! Five recorded requirements describe P04 in terms no crate-level test
//! reaches: a shared-memory latency and throughput target (REQ-P04-02), a
//! control-group memory ceiling set by a unit file (REQ-P04-05), a retrieval
//! pipeline whose modules are swapped per task (REQ-P04-06), a sidecar
//! dispatch whose owner two sources disagree about (dispute DSP-21), and a
//! socket endpoint and budget only one source states (dispute DSP-14). This
//! module is a register for them, in the shape `aegis-vesta` uses for P10.
//!
//! **It admits and refuses nothing, and it endorses nothing.** The status
//! field is the point: a claim recorded as
//! [`ClaimStatus::DeferredTimingRequirement`] is written down so it is not
//! lost, not so it can be counted as delivered.

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
    /// A timing or throughput figure that needs a real transport.
    DeferredTimingRequirement,
    /// A requirement that needs a privileged configuration action.
    DeferredPrivilegedConfiguration,
    /// A requirement whose evidence needs a subsystem this milestone leaves
    /// unbuilt.
    DeferredSubsystem,
    /// Two recorded statements that cannot both hold as written, or a claim
    /// only one source makes.
    UnresolvedSourceConflict,
}

impl ClaimStatus {
    /// Returns the stable name this status is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::DeferredTimingRequirement => "deferred-timing-requirement",
            Self::DeferredPrivilegedConfiguration => "deferred-privileged-configuration",
            Self::DeferredSubsystem => "deferred-subsystem",
            Self::UnresolvedSourceConflict => "unresolved-source-conflict",
        }
    }
}

/// One recorded claim about P04.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordedClaim {
    /// The recorded requirement or dispute identifier.
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

/// The five P04 claims this milestone records and does not discharge.
pub const P04_RECORDED_CLAIMS: [RecordedClaim; 5] = [
    RecordedClaim {
        requirement: "REQ-P04-02",
        summary: "Tier 2 of the agent mesh is Eclipse Zenoh pub/sub with shared-memory \
                  backends targeting 5 to 35 microseconds and 50 or more gigabits per second",
        status: ClaimStatus::DeferredTimingRequirement,
        settled_by: "a real transport carrying real traffic, measured. MockedMesh is an \
                     in-process table and produces no latency or throughput figure of any \
                     kind; the reference kernel is PREEMPT_DYNAMIC rather than PREEMPT_RT, \
                     so a figure taken here would describe a machine the requirement does \
                     not target. No such figure is produced by this milestone",
        source: ClaimSource {
            export: "export-013",
            sha256_prefix: "7f4c22813332",
        },
    },
    RecordedClaim {
        requirement: "REQ-P04-05",
        summary: "the compositor systemd unit sets a control-group memory ceiling of 256M",
        status: ClaimStatus::DeferredPrivilegedConfiguration,
        settled_by: "a unit installed on a booted system; UNIT_MEMORY_MAX_MIB records the \
                     figure and this crate writes no control group and installs no unit",
        source: ClaimSource {
            export: "export-028",
            sha256_prefix: "d74a93eac654",
        },
    },
    RecordedClaim {
        requirement: "REQ-P04-06",
        summary: "the user-space agent mesh uses a Modular RAG pipeline whose Retrieval, \
                  Rerank and Memory modules are swapped per task demand",
        status: ClaimStatus::DeferredSubsystem,
        settled_by: "a retrieval pipeline and the modules to swap, none of which exists here \
                     or is in this milestone's scope; the mesh half this crate models is the \
                     key expression and the retained sample, not what is published on one",
        source: ClaimSource {
            export: "export-004",
            sha256_prefix: "15831276a058",
        },
    },
    RecordedClaim {
        requirement: "DSP-21",
        summary: "the graph of record declares sixteen subsystem nodes and no transport \
                  nodes, while the architecture document adds a mesh transport and a sidecar \
                  protocol as first-class nodes, leaving the owner of sidecar dispatch \
                  unsettled",
        status: ClaimStatus::UnresolvedSourceConflict,
        settled_by: "a recorded decision naming the owner; none exists, so Tier 3 is not \
                     modelled in this crate at all rather than being given an owner by \
                     implementation",
        source: ClaimSource {
            export: "export-062",
            sha256_prefix: "1ce919ed54bb",
        },
    },
    RecordedClaim {
        requirement: "DSP-14",
        summary: "only one source names the P04 to P05 socket path and its sub-100 \
                  microsecond budget; the graph of record draws the edge with neither",
        status: ClaimStatus::UnresolvedSourceConflict,
        settled_by: "decision D32 and the P05 consumer slice at milestone M16. This crate \
                     declares no P04 to P05 schema and binds no socket, so it asserts \
                     neither the path nor the budget",
        source: ClaimSource {
            export: "export-003",
            sha256_prefix: "13af15ffc316",
        },
    },
];
