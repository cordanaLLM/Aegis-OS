// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The P10 claims this crate records but cannot check.
//!
//! Four recorded requirements describe P10 in terms no crate-level test
//! reaches: a boot time and a footprint the register itself marks as unmeasured
//! (REQ-P10-01, REQ-P10-05), accelerator traffic priced through a protocol that
//! needs a real accelerator (REQ-P10-02), and `AF_VSOCK` traffic priced on a
//! transport that needs a kernel and a running monitor (REQ-P10-03). This
//! module is a register for them, in the shape `aegis-hestia` uses for D09.
//!
//! **It admits and refuses nothing, and it endorses nothing.** The status
//! field is the point: a claim recorded as
//! [`ClaimStatus::DeferredHardwareRequirement`] is written down so it is not
//! lost, not so it can be counted as delivered.
//! `tests/recorded_claims.rs` holds the statuses, so a later edit that quietly
//! promotes one fails the gate.

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
    /// A design target this crate cannot measure.
    UnmeasuredTarget,
    /// A requirement whose evidence needs hardware this milestone does not use.
    DeferredHardwareRequirement,
    /// Two recorded bounds that cannot both be satisfied as written.
    UnresolvedSourceConflict,
}

impl ClaimStatus {
    /// Returns the stable name this status is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::UnmeasuredTarget => "unmeasured-target",
            Self::DeferredHardwareRequirement => "deferred-hardware-requirement",
            Self::UnresolvedSourceConflict => "unresolved-source-conflict",
        }
    }
}

/// One recorded claim about P10.
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

/// The five P10 claims this milestone records and does not discharge.
pub const P10_RECORDED_CLAIMS: [RecordedClaim; 5] = [
    RecordedClaim {
        requirement: "REQ-P10-01",
        summary: "a microVM sandbox targets a boot time under 125 ms and a resident \
                  footprint under 5 MB",
        status: ClaimStatus::UnmeasuredTarget,
        settled_by: "a real boot under a named monitor, which is milestone M22; this crate \
                     starts no process and measures nothing",
        source: ClaimSource {
            export: "export-018",
            sha256_prefix: "5dbc6d071bbb",
        },
    },
    RecordedClaim {
        requirement: "REQ-P10-05",
        summary: "the crate header lists the same two figures among its standards targets, \
                  and the register records that the values are unmeasured",
        status: ClaimStatus::UnmeasuredTarget,
        settled_by: "the same measurement; the scaffold's 112 ms literal is carried here as \
                     an Unmeasured value so it cannot be read back as an observation",
        source: ClaimSource {
            export: "export-037",
            sha256_prefix: "ce490c88081f",
        },
    },
    RecordedClaim {
        requirement: "REQ-P10-02",
        summary: "accelerator workloads inside a sandbox go through the Venus protocol so \
                  memory movement is priced and tagged",
        status: ClaimStatus::DeferredHardwareRequirement,
        settled_by: "an accelerator, a monitor and a running Venus transport, which is the \
                     hardware-backed milestone M21; nothing here prices anything",
        source: ClaimSource {
            export: "export-018",
            sha256_prefix: "5dbc6d071bbb",
        },
    },
    RecordedClaim {
        requirement: "REQ-P10-03",
        summary: "the physical-computation boundaries include explicit pricing of all \
                  AF_VSOCK traffic",
        status: ClaimStatus::DeferredHardwareRequirement,
        settled_by: "a kernel vsock transport under a running monitor, which is the \
                     hardware-backed milestone M21; this crate validates a context \
                     identifier and opens no socket",
        source: ClaimSource {
            export: "export-018",
            sha256_prefix: "5dbc6d071bbb",
        },
    },
    RecordedClaim {
        requirement: "REQ-P10-07",
        summary: "the scaffold bounds sandboxes at 64 and vsock connections at 32 while \
                  giving every sandbox its own context identifier, so the two bounds \
                  cannot both hold as written",
        status: ClaimStatus::UnresolvedSourceConflict,
        settled_by: "a recorded decision about which bound governs; none exists, so this \
                     crate bounds the sandbox table and declares no connection bound at all \
                     rather than inventing a reconciliation",
        source: ClaimSource {
            export: "export-037",
            sha256_prefix: "ce490c88081f",
        },
    },
];
