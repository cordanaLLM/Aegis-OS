// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The P14 claims this crate records but cannot check, and the versions nobody
//! has pinned.
//!
//! Four recorded requirements describe P14 in terms no crate-level test
//! reaches: a dual-engine kernel approach with no engine linked (REQ-P14-01),
//! cgroup isolation with no cgroup written (REQ-P14-03), a symbolic-solver hard
//! gate with no solver admitted (REQ-P14-04), and an imported test target that
//! prints simulated output (REQ-P14-08). This module is a register for them, in
//! the shape `aegis-vesta` uses for P10.
//!
//! **It admits and refuses nothing, and it endorses nothing.** The status field
//! is the point: a claim recorded as [`ClaimStatus::DeferredDependency`] is
//! written down so it is not lost, not so it can be counted as delivered.
//! `tests/recorded_claims.rs` holds the statuses, so a later edit that quietly
//! promotes one fails the gate.
//!
//! # Why REQ-P14-08 is here and is not a defect of this crate
//!
//! The imported `Makefile` test target runs `cargo test` and then a script that
//! prints simulated boot and measurement passes. That script is not in this
//! repository's gate and nothing in this crate runs it; the claim is recorded
//! so the imported target is not mistaken for the one `make verify-all` runs.
//! The `Makefile` recipe is the authoritative list of what that gate runs.
//!
//! # Unpinned versions
//!
//! [`UNPINNED_DEPENDENCIES`] records the five tools the P14 sources name and
//! the imported dependency table gives no version for. Recording a name is not
//! selecting a package and is certainly not pinning one: admission is
//! `docs/roadmap/toolchain-admission.md`'s business, and none of these is
//! admitted by any gate this milestone touches. The row exists so that "the
//! CAD and solver versions are unpinned" is a value a test can read rather than
//! a sentence in a commit message.

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
    /// A requirement whose evidence needs a dependency no gate has admitted.
    DeferredDependency,
    /// A requirement whose evidence needs a running kernel feature.
    DeferredRuntimeIsolation,
    /// A recorded artefact that is excluded from every activation gate.
    ExcludedFromGates,
}

impl ClaimStatus {
    /// Returns the stable name this status is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::DeferredDependency => "deferred-dependency",
            Self::DeferredRuntimeIsolation => "deferred-runtime-isolation",
            Self::ExcludedFromGates => "excluded-from-gates",
        }
    }
}

/// One recorded claim about P14.
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

/// One tool the P14 sources name and nobody has pinned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnpinnedDependency {
    /// The tool as the sources name it.
    pub name: &'static str,
    /// What it would be used for.
    pub role: &'static str,
    /// Why no version is recorded.
    pub why_unpinned: &'static str,
}

impl UnpinnedDependency {
    /// Returns `false`; no row here carries a version.
    ///
    /// Written as a method rather than as a missing field so that the absence
    /// is something a test reads, and so that adding a version means adding a
    /// field and revisiting every reader.
    #[must_use]
    pub const fn is_pinned(&self) -> bool {
        false
    }
}

/// The four P14 claims this milestone records and does not discharge.
pub const P14_RECORDED_CLAIMS: [RecordedClaim; 4] = [
    RecordedClaim {
        requirement: "REQ-P14-01",
        summary: "a dual-engine approach, exact NURBS and polyhedral CSG, closes the \
                  proprietary-kernel gap while keeping watertight meshes",
        status: ClaimStatus::DeferredDependency,
        settled_by: "two kernels selected through the template matrix and admitted by a gate; \
                     this crate links neither and carries the two engines as names",
        source: ClaimSource {
            export: "export-021",
            sha256_prefix: "6a3cda152e6c",
        },
    },
    RecordedClaim {
        requirement: "REQ-P14-03",
        summary: "multi-physics solver execution is isolated in cgroups v2 slices under the \
                  architecture's resource-isolation contract",
        status: ClaimStatus::DeferredRuntimeIsolation,
        settled_by: "a running kernel with the slice created and a process placed in it; this \
                     crate writes no cgroup file and starts no solver, and carries the slice \
                     name as a recorded string",
        source: ClaimSource {
            export: "export-021",
            sha256_prefix: "6a3cda152e6c",
        },
    },
    RecordedClaim {
        requirement: "REQ-P14-04",
        summary: "generated code-CAD must pass symbolic solver verification as a hard gate \
                  before promotion",
        status: ClaimStatus::DeferredDependency,
        settled_by: "a solver foreign-function interface admitted by a gate; neither this \
                     crate nor aegis-minerva admits one, which is why the intake outcome has \
                     no variant meaning verified",
        source: ClaimSource {
            export: "export-021",
            sha256_prefix: "6a3cda152e6c",
        },
    },
    RecordedClaim {
        requirement: "REQ-P14-08",
        summary: "the imported test target runs a script whose steps print simulated boot and \
                  measurement passes",
        status: ClaimStatus::ExcludedFromGates,
        settled_by: "nothing further: the imported target is not this repository's gate, the \
                     script is not run here, and the Makefile recipe is the authoritative \
                     list of what the gate does run",
        source: ClaimSource {
            export: "export-009",
            sha256_prefix: "3068c85b768a",
        },
    },
];

/// The five tools the P14 sources name with no version recorded anywhere.
pub const UNPINNED_DEPENDENCIES: [UnpinnedDependency; 5] = [
    UnpinnedDependency {
        name: "exact NURBS B-Rep kernel",
        role: "boundary representation and STEP interchange",
        why_unpinned: "the imported dependency table carries no version for it, and no gate \
                       has admitted a kernel",
    },
    UnpinnedDependency {
        name: "polyhedral CSG kernel",
        role: "watertight boolean operations",
        why_unpinned: "the imported dependency table carries no version for it, and no gate \
                       has admitted a kernel",
    },
    UnpinnedDependency {
        name: "OpenFOAM",
        role: "computational fluid dynamics solver",
        why_unpinned: "named by the source and by no version; a licence decision for solver \
                       bindings is still recorded as an activation blocker for P14",
    },
    UnpinnedDependency {
        name: "CalculiX",
        role: "structural and thermal finite-element solver",
        why_unpinned: "named by the source and by no version; the same licence decision is \
                       outstanding",
    },
    UnpinnedDependency {
        name: "Elmer",
        role: "multiphysics finite-element solver",
        why_unpinned: "named by the source and by no version; the same licence decision is \
                       outstanding",
    },
];
