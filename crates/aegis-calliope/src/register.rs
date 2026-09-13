// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The P08 claims this crate records but cannot check.
//!
//! Five recorded requirements describe P08 in terms no crate-level test
//! reaches: zero-copy buffer sharing between two real applications over a
//! hardware path (REQ-P08-01), two resource limits that only a privileged
//! configuration can grant (REQ-P08-02), a rate-limited restricted execution
//! environment (REQ-P08-03), a 5 ms round-trip latency (REQ-P08-08), and a
//! descriptor whose three fields contradict one another (REQ-P08-06). This
//! module is a register for them, in the shape `aegis-vesta` uses for P10.
//!
//! **It admits and refuses nothing, and it endorses nothing.** The status
//! field is the point: a claim recorded as
//! [`ClaimStatus::DeferredTimingRequirement`] is written down so it is not
//! lost, not so it can be counted as delivered. `tests/recorded_claims.rs`
//! holds the statuses, so a later edit that quietly promotes one fails the
//! gate.

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
    /// A timing figure that needs a real-time kernel and a running graph.
    DeferredTimingRequirement,
    /// A requirement whose evidence needs hardware this milestone does not use.
    DeferredHardwareRequirement,
    /// A requirement that needs a privileged configuration action.
    DeferredPrivilegedConfiguration,
    /// Two recorded statements that cannot both hold as written.
    UnresolvedSourceConflict,
}

impl ClaimStatus {
    /// Returns the stable name this status is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::DeferredTimingRequirement => "deferred-timing-requirement",
            Self::DeferredHardwareRequirement => "deferred-hardware-requirement",
            Self::DeferredPrivilegedConfiguration => "deferred-privileged-configuration",
            Self::UnresolvedSourceConflict => "unresolved-source-conflict",
        }
    }
}

/// One recorded claim about P08.
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

/// The five P08 claims this milestone records and does not discharge.
pub const P08_RECORDED_CLAIMS: [RecordedClaim; 5] = [
    RecordedClaim {
        requirement: "REQ-P08-01",
        summary: "DMA-BUF zero-copy sharing between Looking Glass and OBS Studio over \
                  hardware-accelerated paths, avoiding CPU-side copies",
        status: ClaimStatus::DeferredHardwareRequirement,
        settled_by: "an accelerator, a running PipeWire graph and two real clients, which is \
                     the GPU path at milestone M12; this crate exports no buffer and holds \
                     no file descriptor",
        source: ClaimSource {
            export: "export-017",
            sha256_prefix: "afbc0af8056d",
        },
    },
    RecordedClaim {
        requirement: "REQ-P08-02",
        summary: "the audio threads run with RLIMIT_RTPRIO 95 and RLIMIT_MEMLOCK infinity, \
                  both stated as non-negotiable",
        status: ClaimStatus::DeferredPrivilegedConfiguration,
        settled_by: "a privileged limits configuration; the reference profile reading in \
                     REFERENCE_PROFILE_LIMITS already clears the 95 with a ceiling of 99 and \
                     does not meet the memlock requirement, whose ceiling reads 8192 KiB",
        source: ClaimSource {
            export: "export-017",
            sha256_prefix: "afbc0af8056d",
        },
    },
    RecordedClaim {
        requirement: "REQ-P08-03",
        summary: "a third-party plugin passes through a rate-limited, restricted Quarantine \
                  stage before activation into the real-time graph",
        status: ClaimStatus::DeferredHardwareRequirement,
        settled_by: "a sandbox that actually rate-limits and restricts something, which needs \
                     a process, a bridge and a graph; the ordering half of the requirement is \
                     enforced here by PluginStage::may_advance_to, and only that half",
        source: ClaimSource {
            export: "export-017",
            sha256_prefix: "afbc0af8056d",
        },
    },
    RecordedClaim {
        requirement: "REQ-P08-08",
        summary: "this crate is the designated location for PipeWire 5 ms round-trip-latency \
                  tuning and zero-copy DMA-BUF video capture",
        status: ClaimStatus::DeferredTimingRequirement,
        settled_by: "latency fixtures on a PREEMPT_RT kernel, which is milestone M23; the \
                     reference profile runs PREEMPT_DYNAMIC, so no latency or determinism \
                     figure is produced by this milestone at all",
        source: ClaimSource {
            export: "export-007",
            sha256_prefix: "84f43472c536",
        },
    },
    RecordedClaim {
        requirement: "REQ-P08-06",
        summary: "the scaffold bounds DMA-BUF buffers at 64 and fills each descriptor with a \
                  width-times-four stride and the code 0x34325641 under a comment naming both \
                  NV12 and ARGB",
        status: ClaimStatus::UnresolvedSourceConflict,
        settled_by: "a recorded decision about which pixel format the shared stream carries; \
                     none exists, so this crate admits the two formats the comment names, \
                     each with its own stride rule, and refuses the literal code, which \
                     spells AV24 and is neither",
        source: ClaimSource {
            export: "export-026",
            sha256_prefix: "c2f1e433cd32",
        },
    },
];
