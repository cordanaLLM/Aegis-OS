// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The P13 claims this crate records but cannot check.
//!
//! Two of this milestone's requirements are statements about documents rather
//! than about behaviour, and one is a measurement of the reference profile.
//! None of them is a property of the code, so each is recorded as a row with
//! its source and its status instead of being asserted somewhere it would look
//! like a test result.
//!
//! `tests/recorded_claims.rs` checks the rows are well formed and that each
//! names its source. It does not check the claims are true; a claim whose
//! status is [`ClaimStatus::Recorded`] has been read out of a source, and a
//! claim whose status is [`ClaimStatus::Measured`] was observed on the
//! reference profile with the command the row names.

/// Where a recorded claim comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ClaimSource {
    /// An imported notebook export, named by its export identifier.
    Export,
    /// A recorded decision in `docs/roadmap/README.md`.
    Decision,
    /// A command run against the reference profile.
    ReferenceProfile,
}

impl ClaimSource {
    /// Returns the source name used in diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Export => "export",
            Self::Decision => "decision",
            Self::ReferenceProfile => "reference-profile",
        }
    }
}

/// How far a recorded claim has been taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ClaimStatus {
    /// Read out of a source. Nothing in this crate checks it.
    Recorded,
    /// Observed on the reference profile with the command the row names.
    Measured,
    /// Held by a test in this crate.
    Held,
}

impl ClaimStatus {
    /// Returns the status name used in diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Recorded => "recorded",
            Self::Measured => "measured",
            Self::Held => "held",
        }
    }
}

/// One claim, its requirement, its source and its status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordedClaim {
    /// The requirement or decision identifier the claim belongs to.
    pub requirement: &'static str,
    /// What is claimed.
    pub claim: &'static str,
    /// Where the claim comes from.
    pub source: ClaimSource,
    /// The source identifier, export digest, or command.
    pub citation: &'static str,
    /// How far the claim has been taken.
    pub status: ClaimStatus,
}

/// The P13 claims recorded at milestone M05.
///
/// This is an enumeration of five rows, not a survey of every P13 claim in the
/// imported material. A requirement that is not here is not thereby satisfied.
pub const P13_RECORDED_CLAIMS: [RecordedClaim; 5] = [
    RecordedClaim {
        requirement: "REQ-P13-07",
        claim: "Tellus's chosen architecture is Kepler Power Telemetry, against a discarded \
                Estimated Power Model, with Calibration as primary impact and the SCI Framework \
                as the regulatory constraint",
        source: ClaimSource::Export,
        citation: "export-004 15831276a058, section 18 comparative matrix",
        status: ClaimStatus::Recorded,
    },
    RecordedClaim {
        requirement: "REQ-P13-08",
        claim: "the developer guide maps crates/aegis-tellus/ to Kepler eBPF power probes and \
                the ISO/IEC 21031:2024 SCI carbon rate engine; this crate is the rate engine \
                half only, and compiles, loads and attaches no eBPF program",
        source: ClaimSource::Export,
        citation: "export-007 84f43472c536",
        status: ClaimStatus::Recorded,
    },
    RecordedClaim {
        requirement: "REQ-P13-02",
        claim: "the kepler_power eBPF probe monitors per-cgroup v2 CPU/DRAM RAPL and ACPI \
                counters; no probe is compiled or loaded here, and the slice table is fed by \
                the caller",
        source: ClaimSource::Export,
        citation: "export-048 293357bac7d3",
        status: ClaimStatus::Recorded,
    },
    RecordedClaim {
        requirement: "D60",
        claim: "the reference profile exposes only the package-0 and core powercap zones; there \
                is no dram and no psys zone, and energy_uj is mode 0400",
        source: ClaimSource::ReferenceProfile,
        citation: "ls -d /sys/class/powercap/*; cat /sys/class/powercap/*/name; \
                   ls -l /sys/class/powercap/*/energy_uj",
        status: ClaimStatus::Measured,
    },
    RecordedClaim {
        requirement: "REQ-P13-01",
        claim: "the SCI rate is ((E * I) + M) / R, and over the domain the constructors admit \
                the result is finite",
        source: ClaimSource::Export,
        citation: "export-036 25813d240733",
        status: ClaimStatus::Held,
    },
];

/// The `max_energy_range_uj` both reference-profile zones report.
///
/// 65532610987, read from `/sys/class/powercap/intel-rapl:0/max_energy_range_uj`
/// and `/sys/class/powercap/intel-rapl:0:0/max_energy_range_uj` on 2026-09-13.
/// It is recorded here so the M21 rollover boundary has a committed figure to
/// test against; nothing in this crate reads a counter.
pub const REFERENCE_MAX_ENERGY_RANGE_UJ: u64 = 65_532_610_987;

/// The file mode the reference profile's `energy_uj` attributes carry.
///
/// `0400`, the CVE-2020-8694 mitigation: root-readable only, so the M21 reader
/// needs a privileged path. Recorded, not enforced.
pub const REFERENCE_ENERGY_UJ_MODE: u32 = 0o400;
