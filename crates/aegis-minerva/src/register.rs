// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The P09 claims this crate records but cannot check.
//!
//! Four recorded requirements describe P09 in terms no crate-level test
//! reaches: a 20 W power envelope and a 1.5 ms routing cap that need a running
//! system to measure (REQ-P09-01, REQ-P09-02), an architectural thesis about
//! fast and slow memory that no type here models (REQ-P09-06), and a solver
//! the developer guide maps to this directory and this milestone excludes
//! (REQ-P09-08). This module is a register for them, in the shape
//! `aegis-hestia` uses for D09.
//!
//! **It admits and refuses nothing, and it endorses nothing.** The status
//! field is the point: a claim recorded as
//! [`ClaimStatus::UnmeasuredTarget`] is written down so it is not lost, not so
//! it can be cited as a property this crate has.
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
    /// A dependency the milestone deliberately excludes.
    DeferredDependency,
    /// A structural disagreement between two recorded sources.
    UnresolvedGraphConflict,
    /// A statement about the architecture that no type here models.
    UnmodelledArchitecturalClaim,
}

impl ClaimStatus {
    /// Returns the stable name this status is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::UnmeasuredTarget => "unmeasured-target",
            Self::DeferredDependency => "deferred-dependency",
            Self::UnresolvedGraphConflict => "unresolved-graph-conflict",
            Self::UnmodelledArchitecturalClaim => "unmodelled-architectural-claim",
        }
    }
}

/// One recorded claim about P09.
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

/// The five P09 claims this milestone records and does not discharge.
pub const P09_RECORDED_CLAIMS: [RecordedClaim; 5] = [
    RecordedClaim {
        requirement: "REQ-P09-01",
        summary: "the router works inside a strict 20 W power envelope",
        status: ClaimStatus::UnmeasuredTarget,
        settled_by: "a running system with power metering; this crate holds the cap as a \
                     constant in milliwatts and refuses a budget above it, which is a bound \
                     on a request and not a watt anybody measured",
        source: ClaimSource {
            export: "export-034",
            sha256_prefix: "213a95fc0d02",
        },
    },
    RecordedClaim {
        requirement: "REQ-P09-02",
        summary: "the router stays under a 1.5 ms routing-overhead latency cap",
        status: ClaimStatus::UnmeasuredTarget,
        settled_by: "a measured routing path on real hardware; this crate has no clock, so \
                     the cap is a predicate over a number a caller supplies",
        source: ClaimSource {
            export: "export-034",
            sha256_prefix: "213a95fc0d02",
        },
    },
    RecordedClaim {
        requirement: "REQ-P09-06",
        summary: "the 20 W thesis is implemented by separating rapid acquisition in fast \
                  memory from stable structure in a slow model",
        status: ClaimStatus::UnmodelledArchitecturalClaim,
        settled_by: "an implementation with both memories; the trajectory buffer here is \
                     the replay half of that picture and nothing in this crate holds a \
                     model at all",
        source: ClaimSource {
            export: "export-004",
            sha256_prefix: "15831276a058",
        },
    },
    RecordedClaim {
        requirement: "REQ-P09-08",
        summary: "the developer guide maps this directory to a subsystem owning a 20 W \
                  policy router, a trajectory relabeller and a Z3 solver",
        status: ClaimStatus::DeferredDependency,
        settled_by: "a decision admitting a solver and a toolchain to build it; milestone \
                     M06 admits no Z3 foreign-function interface, so the crate carries a \
                     screen that can only refuse and submits everything else to P14",
        source: ClaimSource {
            export: "export-007",
            sha256_prefix: "84f43472c536",
        },
    },
    RecordedClaim {
        requirement: "REQ-GRAPH-05",
        summary: "the dataflow document draws the code-CAD verification edge from P14 to \
                  P09, opposite to the graph of record",
        status: ClaimStatus::UnresolvedGraphConflict,
        settled_by: "decision D27, which is recorded and open; both readings stay \
                     representable and the submission carries the graph's",
        source: ClaimSource {
            export: "export-002",
            sha256_prefix: "7e0c95f4ea05",
        },
    },
];
