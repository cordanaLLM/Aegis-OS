// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The P11 claims this crate records but cannot check.
//!
//! Four recorded requirements describe P11 in terms no crate-level test
//! reaches: a proprietary SDK binding that ADR-0002 supersedes (REQ-P11-01), a
//! zero-copy video path that needs a compositor and a GPU (REQ-P11-03), TPM2
//! sealing that needs a bound key (REQ-P11-04), and the hardware quote the
//! receipt edge is carried over (REQ-P11-06). This module is a register for
//! them, in the shape `aegis-hestia` uses for D09 and `aegis-vesta` for P10.
//!
//! **It admits and refuses nothing, and it endorses nothing.** The status field
//! is the point: a claim recorded as
//! [`ClaimStatus::ProcurementDependency`] is written down so it is not lost,
//! not so it can be counted as delivered.
//! `tests/recorded_claims.rs` holds the statuses, so a later edit that quietly
//! promotes one fails the gate.
//!
//! # Why FIDO2 has its own status
//!
//! The two credentials P11 names are in different situations on the reference
//! profile, and one status for both would lose that. A TPM2 is present and
//! unbound, which is a deferred hardware requirement: the code to bind a key
//! could be written and is not. No FIDO2 authenticator is present at all, so
//! there is nothing to write code against and no amount of work in this
//! repository changes it. `src/probe.rs` records what each probe returned.

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
    /// A requirement whose evidence needs hardware this milestone does not use,
    /// and which the reference profile does have.
    DeferredHardwareRequirement,
    /// A requirement whose hardware the reference profile does not have at all,
    /// so it has to be bought before any code can be written against it.
    ProcurementDependency,
    /// A requirement a recorded decision has superseded.
    SupersededByDecision,
}

impl ClaimStatus {
    /// Returns the stable name this status is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::DeferredHardwareRequirement => "deferred-hardware-requirement",
            Self::ProcurementDependency => "procurement-dependency",
            Self::SupersededByDecision => "superseded-by-decision",
        }
    }
}

/// One recorded claim about P11.
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

/// The five P11 claims this milestone records and does not discharge.
pub const P11_RECORDED_CLAIMS: [RecordedClaim; 5] = [
    RecordedClaim {
        requirement: "REQ-P11-01",
        summary: "a binding generator wraps proprietary C++ SDK headers so the runtime is \
                  isolated from that ABI",
        status: ClaimStatus::SupersededByDecision,
        settled_by: "nothing further: decision D12 and ADR-0002 exclude the SDK, and the \
                     requirement stays in the set as source evidence marked superseded",
        source: ClaimSource {
            export: "export-019",
            sha256_prefix: "b0aa6e54ed57",
        },
    },
    RecordedClaim {
        requirement: "REQ-P11-03",
        summary: "video frames stay in GPU memory over a zero-copy buffer export between the \
                  compositor and the encoder",
        status: ClaimStatus::DeferredHardwareRequirement,
        settled_by: "a compositor, a GPU and a bound render node, which is the hardware-backed \
                     milestone M25; this crate exports nothing and opens no device",
        source: ClaimSource {
            export: "export-019",
            sha256_prefix: "b0aa6e54ed57",
        },
    },
    RecordedClaim {
        requirement: "REQ-P11-04",
        summary: "payment data and session keys are sealed to specific TPM2 platform \
                  configuration registers",
        status: ClaimStatus::DeferredHardwareRequirement,
        settled_by: "a key bound to the TPM2 the reference profile already has, which is \
                     milestone M20; this crate carries the register selection as a field and \
                     seals nothing",
        source: ClaimSource {
            export: "export-019",
            sha256_prefix: "b0aa6e54ed57",
        },
    },
    RecordedClaim {
        requirement: "REQ-P11-06",
        summary: "the receipt edge to P02 is carried over a hardware TPM2 register quote",
        status: ClaimStatus::DeferredHardwareRequirement,
        settled_by: "a quote from that TPM2 and a transport for it; the receipt this crate \
                     types names its signing as stubbed and unsigned on the wire",
        source: ClaimSource {
            export: "export-062",
            sha256_prefix: "1ce919ed54bb",
        },
    },
    RecordedClaim {
        requirement: "P11-HW-FIDO2",
        summary: "planning/components.json records a FIDO2-capable authenticator as P11's \
                  second hardware requirement; docs/roadmap/requirements.md carries no REQ \
                  identifier for it, so this row is named for the register it comes from",
        status: ClaimStatus::ProcurementDependency,
        settled_by: "buying one: the probe recorded in src/probe.rs found no such device among \
                     the 17 connected to the reference profile, so there is nothing here to \
                     bind to and nothing stubbed that could be mistaken for coverage",
        source: ClaimSource {
            export: "export-019",
            sha256_prefix: "b0aa6e54ed57",
        },
    },
];
