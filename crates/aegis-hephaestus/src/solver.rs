// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The solver slice declaration, which starts no solver (REQ-P14-03).
//!
//! REQ-P14-03 asks for multi-physics solver execution isolated in cgroups v2
//! slices. The scaffold's stand-in is a `match` on a solver name that prints
//! which slice it would have used and returns `Ok(())`, so a caller cannot tell
//! a dispatch from a refusal except by the string that was printed.
//!
//! What is checkable without a solver and without a cgroup is the **admission**:
//! which names the recorded set holds, and which slice each is declared to run
//! in. [`SolverAdmission::admit`] is that, and its name says what it is not.
//! Nothing here forks, executes, writes a cgroup file or creates a slice; the
//! slice name is a recorded string and [`SolverAdmission`] counts what it was
//! asked about.
//!
//! # Versions are not pinned
//!
//! The three solvers are named by the sources and pinned by nobody: the
//! imported dependency table carries no version for any of them, and admitting
//! one to a gate is `docs/roadmap/toolchain-admission.md`'s business.
//! `register` records that as an unpinned dependency row per solver rather than
//! inventing a version here.

use crate::error::HephaestusError;

/// The systemd slice the sources declare solver work runs in.
pub const SOLVER_SLICE: &str = "agent-solver.slice";

/// The solvers the recorded set admits by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum SolverTarget {
    /// The computational fluid dynamics solver the sources name first.
    OpenFoam,
    /// The structural and thermal finite-element solver.
    CalculiX,
    /// The multiphysics finite-element solver named beside it.
    Elmer,
}

impl SolverTarget {
    /// Every admitted solver, in the order the sources list them.
    pub const ALL: [Self; 3] = [Self::OpenFoam, Self::CalculiX, Self::Elmer];

    /// Returns the tag the scaffold's dispatch matches on, verbatim.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::OpenFoam => "OpenFOAM",
            Self::CalculiX => "CalculiX",
            Self::Elmer => "Elmer",
        }
    }

    /// Returns the physics domain the sources give for this solver.
    #[must_use]
    pub const fn domain(self) -> &'static str {
        match self {
            Self::OpenFoam => "computational fluid dynamics",
            Self::CalculiX | Self::Elmer => "structural and thermal finite elements",
        }
    }

    /// Returns the slice the sources declare this solver runs in.
    ///
    /// Recorded, not created: no cgroup is written and no process is placed.
    #[must_use]
    pub const fn declared_slice(self) -> &'static str {
        SOLVER_SLICE
    }

    /// Returns the solver `tag` names, if the recorded set holds one.
    ///
    /// The comparison is exact and case-sensitive, matching the scaffold's own
    /// `match` arms. The sweep is bounded by [`Self::ALL`].
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .take(Self::ALL.len())
            .find(|target| target.tag() == tag)
    }
}

/// The recorded admission over solver names.
///
/// It counts and it refuses. It does not dispatch: there is deliberately no
/// method here that runs anything, and the crate's effect sweep is what keeps
/// it that way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SolverAdmission {
    admitted: u64,
    refused: u64,
}

impl SolverAdmission {
    /// Builds an admission that has seen nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            admitted: 0,
            refused: 0,
        }
    }

    /// Admits `tag` if the recorded set names it.
    ///
    /// # Errors
    ///
    /// Returns [`HephaestusError::UnknownSolver`] for a name the recorded set
    /// does not hold, which is the scaffold's "Unknown solver target" given a
    /// type.
    pub fn admit(&mut self, tag: &str) -> Result<SolverTarget, HephaestusError> {
        if let Some(target) = SolverTarget::from_tag(tag) {
            self.admitted = self.admitted.saturating_add(1);
            return Ok(target);
        }
        self.refused = self.refused.saturating_add(1);
        Err(HephaestusError::UnknownSolver)
    }

    /// Returns how many names were admitted.
    #[must_use]
    pub const fn admitted(&self) -> u64 {
        self.admitted
    }

    /// Returns how many names were refused.
    #[must_use]
    pub const fn refused(&self) -> u64 {
        self.refused
    }
}
