// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Decision D02 applied, and the P16 claims this crate cannot check.
//!
//! # D02, applied
//!
//! Three algorithms are named across the P16 sources for one ledger. The
//! scaffold header and the reference checkpoint ledger both call it a BLAKE3
//! hash chain (REQ-P16-01, REQ-P16-03); the only reference implementation that
//! hashes anything uses SHA-256, and REQ-P16-09 records that the algorithm has
//! to be pinned before the Rust port. The M01 inventory registered the
//! disagreement as DSP-07.
//!
//! **The decision, recorded 2026-09-13 and applied here: SHA-256, behind a
//! hash-algorithm trait; MD5 excluded; BLAKE3 only through a later recorded
//! decision.** Applying it at M05 means three things:
//!
//! * the trait is not redeclared. [`aegis_justitia::LedgerHasher`] was created
//!   at M02 for the P06 audit ledger, and [`crate::ledger::CheckpointLedger`]
//!   is generic over it, so there is one hashing boundary in the workspace
//!   rather than two;
//! * [`aegis_justitia::HashAlgorithm`] has exactly one variant. MD5 and BLAKE3
//!   are excluded structurally -- no variant, no dependency, no lock entry, no
//!   serialisation alias -- so a record naming either does not decode;
//! * the superseded naming is recorded rather than erased. [`LEDGER_ALGORITHM`]
//!   states the three readings, which one was chosen and which requirement
//!   still says otherwise, so a future reader who finds "BLAKE3" in the
//!   requirements table is not left to guess whether it was overlooked.

use aegis_justitia::HashAlgorithm;

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

/// The three algorithms the P16 sources name for one ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LedgerAlgorithmReading {
    /// SHA-256, the only reading with an implementation behind it.
    Sha256,
    /// BLAKE3, named by the scaffold header and the reference ledger.
    Blake3,
    /// MD5, the reference daemon's own choice. Excluded by D02.
    Md5,
}

impl LedgerAlgorithmReading {
    /// All three readings, in the order the register lists them.
    ///
    /// The rejected readings stay representable so the register states a
    /// choice between three rather than asserting the only one it can spell.
    pub const ALL: [Self; 3] = [Self::Sha256, Self::Blake3, Self::Md5];

    /// Returns the algorithm name as the source spells it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Sha256 => "SHA-256",
            Self::Blake3 => "BLAKE3",
            Self::Md5 => "MD5",
        }
    }

    /// Returns `true` when this reading is the one D02 chose.
    #[must_use]
    pub const fn is_chosen(self) -> bool {
        matches!(self, Self::Sha256)
    }
}

/// One citation behind a recorded decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Citation {
    /// The requirement identifier the citation supports.
    pub requirement: &'static str,
    /// The imported source, by export identifier and digest prefix.
    pub source: &'static str,
    /// The reading that source carries.
    pub reading: LedgerAlgorithmReading,
}

/// The recorded D02 decision, as this crate applies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecisionRecord {
    /// The decision identifier.
    pub id: &'static str,
    /// The dispute row in the M01 inventory.
    pub dispute: &'static str,
    /// Whether the decision is settled.
    pub state: DecisionState,
    /// The reading that was chosen.
    pub chosen: LedgerAlgorithmReading,
    /// The algorithm the workspace hashing trait admits.
    pub admitted: HashAlgorithm,
    /// The date the decision was recorded.
    pub recorded_on: &'static str,
    /// The sources, and what each says.
    pub citations: [Citation; 3],
}

/// Decision D02, as milestone M05 applies it to the checkpoint ledger.
pub const LEDGER_ALGORITHM: DecisionRecord = DecisionRecord {
    id: "D02",
    dispute: "DSP-07",
    state: DecisionState::Closed,
    chosen: LedgerAlgorithmReading::Sha256,
    admitted: HashAlgorithm::Sha256,
    recorded_on: "2026-09-13",
    citations: [
        Citation {
            requirement: "REQ-P16-09",
            source: "export-040 84b1ca13cc33",
            reading: LedgerAlgorithmReading::Sha256,
        },
        Citation {
            requirement: "REQ-P16-01",
            source: "export-025 6b23723ddb76",
            reading: LedgerAlgorithmReading::Blake3,
        },
        Citation {
            requirement: "REQ-P06-09",
            source: "export-031 5ff683328148",
            reading: LedgerAlgorithmReading::Md5,
        },
    ],
};

/// How far a recorded claim has been taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ClaimStatus {
    /// Read out of a source. Nothing in this crate checks it.
    Recorded,
    /// Held by a test in this crate.
    Held,
    /// Deferred to a later milestone, which the row names.
    Deferred,
}

impl ClaimStatus {
    /// Returns the status name used in diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Recorded => "recorded",
            Self::Held => "held",
            Self::Deferred => "deferred",
        }
    }
}

/// One P16 claim, its source and its status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordedClaim {
    /// The requirement identifier the claim belongs to.
    pub requirement: &'static str,
    /// What is claimed.
    pub claim: &'static str,
    /// The imported source, by export identifier and digest prefix.
    pub citation: &'static str,
    /// How far the claim has been taken.
    pub status: ClaimStatus,
}

/// The P16 claims recorded at milestone M05.
///
/// This is an enumeration of five rows, not a survey of every P16 claim in the
/// imported material. A requirement that is not here is not thereby satisfied.
pub const P16_RECORDED_CLAIMS: [RecordedClaim; 5] = [
    RecordedClaim {
        requirement: "REQ-P16-01",
        claim: "the candidate lifecycle has seven steps: Propose, Challenge, Decompose, Prove, \
                Check, Publish, Invalidate",
        citation: "export-025 6b23723ddb76",
        status: ClaimStatus::Held,
    },
    RecordedClaim {
        requirement: "REQ-P16-10",
        claim: "Athena carries a 500 ms candidate-check latency budget; a check that overruns it \
                is refused rather than published late",
        citation: "export-062 1ce919ed54bb",
        status: ClaimStatus::Held,
    },
    RecordedClaim {
        requirement: "REQ-P16-04",
        claim: "P10 Vesta performs isolated execution of A/B candidates for Athena over a \
                Firecracker microVM and AF_VSOCK transport",
        citation: "export-062 1ce919ed54bb",
        status: ClaimStatus::Deferred,
    },
    RecordedClaim {
        requirement: "REQ-P16-07",
        claim: "aegis-athena is a declared member of the Aegis OS Rust workspace",
        citation: "export-006 6e694e01e136",
        status: ClaimStatus::Held,
    },
    RecordedClaim {
        requirement: "REQ-P15-04",
        claim: "a feature reaches Established maturity only after passing six of nine \
                machine-checkable structural promotion gates; the two counts are the \
                requirement's, the nine gate names are this milestone's reading",
        citation: "export-022 46cea660df63",
        status: ClaimStatus::Held,
    },
];

/// The milestone that supplies the P10 Vesta sandbox REQ-P16-04 names.
///
/// Recorded so the deferral names where it goes rather than merely saying it
/// is deferred. M06 types the P09-to-P10 capsule request; M22 is the microVM
/// half that would actually run a candidate.
pub const SANDBOX_DEFERRED_TO: &str = "M06 (typed request), M22 (microVM execution)";
