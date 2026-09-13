// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P14 `aegis-hephaestus`: the bounded geometry slice and the P09 edge
//! consumed.
//!
//! The imported P14 scaffold (export-029 `427996186520`) is a daemon that
//! prints what it would do: it declares two bounds and holds neither against
//! anything, walks an evaluation loop with an empty body, compares a hard-coded
//! element count with its own limit so the comparison can only succeed, and
//! announces a symbolic solver it never links. Milestone M08 extracts the part
//! that can be checked without a `CAD` kernel, a mesher, a solver or a cgroup --
//! the bounds, the admissions and the payload -- and turns each string error
//! into a typed one, which is what HISS-07 asks for.
//!
//! # The four things a reviewer should look at
//!
//! * [`BrepEvaluator`] holds both recorded bounds against caller-supplied
//!   input: [`MAX_GEOMETRY_ITERATIONS`] stops a walk that would run past it,
//!   and [`MAX_MESH_ELEMENTS`] accepts the 500,000th element and refuses the
//!   500,001st. A request that names no `STEP` source is refused before either.
//! * [`VerificationIntake`] consumes
//!   [`CadVerificationRequest`](aegis_minerva::CadVerificationRequest) **from
//!   the producer's crate** rather than redefining it, so the shape M06 pinned
//!   stays the one shape and every refusal a consumer sees is the producer's.
//! * [`GeometryViewport`] is the versioned descriptor P14 hands to P15, and it
//!   carries [`MeshElementCount`] so the meshing bound is the same bound on the
//!   wire as in the planner.
//! * [`SolverAdmission`] names three solvers and starts none. There is
//!   deliberately no dispatch method anywhere in this crate.
//!
//! # Nothing here returns a verdict
//!
//! [`VerificationOutcome`] has one variant and it is not "verified"; the
//! evaluation outcome has no `is_watertight`; the viewport carries no proof
//! field. No solver foreign-function interface is admitted at this milestone,
//! so a type that could say a script or a solid is valid would be saying it
//! from a constant. [`register`] records REQ-P14-04 as a deferred dependency
//! and [`UNPINNED_DEPENDENCIES`] records that no `CAD` kernel or solver version
//! is pinned.
//!
//! # Fixed point, not floating point
//!
//! Every length here is an integer count of micrometres. The scaffold carries
//! tolerances and element sizes as `f64` millimetres, and a boundary test on a
//! binary float is a test on rounding.
//!
//! # Design invariants (see `AGENTS.md`)
//!
//! * no recursion: every function here is a flat check, a bounded sweep or a
//!   field access, and no function in the crate calls itself directly or
//!   through another;
//! * every loop carries a scalar upper bound: [`MAX_GEOMETRY_ITERATIONS`],
//!   [`MAX_STEP_PATH_LEN`] and [`MAX_CONTRACT_PAYLOAD_BYTES`];
//! * every value on a decision path is `Copy`, so no such path allocates;
//!   `tests/allocation_bounds.rs` is the falsifier, and the one place that is
//!   deliberately not claimed -- a JSON string carrying an escape, which
//!   `serde_json` unescapes into a heap scratch buffer before any field of ours
//!   sees it -- is tested rather than denied;
//! * no `unwrap`, `expect`, `panic!`, slice indexing or unchecked arithmetic,
//!   and no `unsafe` (forbidden at the workspace root).
//!
//! # What this crate does not do
//!
//! It loads nothing and runs nothing. There is no `CAD` kernel, no `STEP`
//! parser, no mesher, no `Z3` or other solver, no `D-Bus` connection, no
//! process, no cgroup and no filesystem access anywhere in it -- a `STEP`
//! source is a validated name and is never opened. `tests/stubbed_effects.rs`
//! sweeps the crate's own sources for a recorded list of identifiers that would
//! be needed to do any of it and fails if one appears outside a comment -- a
//! regression gate over an enumeration, not a proof over every such identifier.
//!
//! A pass of this crate's tests is evidence about the bounds, the admissions
//! and the payload. It closes no kernel, solver or isolation gate.
//!
//! # Example
//!
//! ```
//! use aegis_hephaestus::{
//!     BrepEvaluator, CadEngine, EvaluationOutcome, MAX_MESH_ELEMENTS, MeshConfig, StepPath,
//!     Tolerance,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let evaluator = BrepEvaluator::new(CadEngine::ExactNurbsBrep, Tolerance::SCAFFOLD);
//! let source = StepPath::parse("/var/lib/aegis/geometry/bracket.step")?;
//!
//! let walked = evaluator.evaluate(Some(source), 120)?;
//! assert_eq!(walked, EvaluationOutcome::Completed { iterations: 120 });
//! assert!(evaluator.evaluate(None, 120).is_err());
//!
//! let config = MeshConfig::new(100, 2_000, true)?;
//! let plan = evaluator.mesh(config, 120_000)?;
//! assert_eq!(plan.elements.get(), 120_000);
//! assert!(evaluator.mesh(config, MAX_MESH_ELEMENTS).is_ok());
//! assert!(evaluator.mesh(config, MAX_MESH_ELEMENTS + 1).is_err());
//! # Ok(())
//! # }
//! ```

pub mod contracts;
pub mod error;
pub mod geometry;
pub mod id;
pub mod register;
pub mod solver;
pub mod verification;

pub use crate::contracts::graph::EdgeId;
pub use crate::contracts::viewport::{GeometryViewport, GeometryViewportVersion};
pub use crate::contracts::{
    ContractError, Correlation, MAX_CONTRACT_PAYLOAD_BYTES, PayloadBuffer, SchemaId,
};
pub use crate::error::HephaestusError;
pub use crate::geometry::{
    BrepEvaluator, CadEngine, EvaluationOutcome, MAX_GEOMETRY_ITERATIONS, MAX_MESH_ELEMENTS,
    MAX_TOLERANCE_MICROMETRES, MIN_TOLERANCE_MICROMETRES, MeshConfig, MeshElementCount, MeshPlan,
    StepSchema, Tolerance,
};
pub use crate::id::{IdError, MAX_STEP_PATH_LEN, StepPath};
pub use crate::register::{
    ClaimSource, ClaimStatus, P14_RECORDED_CLAIMS, RecordedClaim, UNPINNED_DEPENDENCIES,
    UnpinnedDependency,
};
pub use crate::solver::{SOLVER_SLICE, SolverAdmission, SolverTarget};
pub use crate::verification::{AdmittedVerification, VerificationIntake, VerificationOutcome};
