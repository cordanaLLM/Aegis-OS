// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The P15 claims this crate records but cannot check.
//!
//! Three recorded requirements describe P15 in terms no crate-level test
//! reaches: an idle memory and processor footprint for the micro-frontend host
//! (REQ-P15-01), a third-party accuracy figure the register itself marks as an
//! unverified proposal claim (REQ-P15-06), and a storage edge the dataflow
//! document draws but the graph of record does not carry (REQ-GRAPH-03). This
//! module is a register for them, in the shape `aegis-janus-lifecycle` uses
//! for decision D13: static data naming what was recorded, by whom, and with
//! what standing.
//!
//! **It admits and refuses nothing, and it endorses nothing.** The status
//! field is the point: a claim recorded as
//! [`ClaimStatus::UnverifiedProposalClaim`] is written down so it is not lost,
//! not so it can be cited as true. `tests/recorded_claims.rs` holds the
//! statuses, so a later edit that quietly promotes one fails the gate.

/// A private source, cited by export identifier and digest prefix only.
///
/// Deliberately a separate type from [`decision::Citation`](crate::decision::Citation):
/// a decision cites the sources that posed a question, while a claim cites the
/// one source that states it, so the two carry different arities and must not
/// be interchangeable at a call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClaimSource {
    /// The export identifier of the private source.
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
    /// A figure the register marks as unverified proposal data.
    UnverifiedProposalClaim,
    /// A structural disagreement between two recorded sources.
    UnresolvedGraphConflict,
}

impl ClaimStatus {
    /// Returns the stable name this status is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::UnmeasuredTarget => "unmeasured-target",
            Self::UnverifiedProposalClaim => "unverified-proposal-claim",
            Self::UnresolvedGraphConflict => "unresolved-graph-conflict",
        }
    }
}

/// One recorded claim about P15.
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
    /// The private source that states it.
    pub source: ClaimSource,
}

/// The three P15 claims recorded at milestone M01 and carried here.
pub const P15_RECORDED_CLAIMS: [RecordedClaim; 3] = [
    RecordedClaim {
        requirement: "REQ-P15-01",
        summary: "the micro-frontend host targets a 30-50 MB idle memory footprint \
                  at roughly no idle processor use",
        status: ClaimStatus::UnmeasuredTarget,
        settled_by: "a running host measured on real hardware; no code in this crate \
                     starts one or measures anything",
        source: ClaimSource {
            export: "export-022",
            sha256_prefix: "46cea660df63",
        },
    },
    RecordedClaim {
        requirement: "REQ-P15-06",
        summary: "a trade-off guide claims that grounding agents in governed context \
                  improves generated query accuracy by 38 per cent",
        status: ClaimStatus::UnverifiedProposalClaim,
        settled_by: "nothing in this repository: the figure is third-party proposal \
                     data, recorded so it is not lost and not relied upon",
        source: ClaimSource {
            export: "export-004",
            sha256_prefix: "15831276a058",
        },
    },
    RecordedClaim {
        requirement: "REQ-GRAPH-03",
        summary: "the dataflow document places the local-first store on a Btrfs \
                  subvolume owned by P02, an edge the graph of record does not carry",
        status: ClaimStatus::UnresolvedGraphConflict,
        settled_by: "a recorded decision that either adds the edge to the graph of \
                     record or drops it; none exists, so the crate types the path \
                     and claims no edge",
        source: ClaimSource {
            export: "export-002",
            sha256_prefix: "7e0c95f4ea05",
        },
    },
];
