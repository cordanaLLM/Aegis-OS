// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded geometry and meshing limits (REQ-P14-01, REQ-P14-02).
//!
//! The scaffold declares the two bounds this module keeps --
//! [`MAX_GEOMETRY_ITERATIONS`] and [`MAX_MESH_ELEMENTS`] -- and then holds
//! neither of them against anything real. Its evaluation loop is
//!
//! ```text
//! loop { iterations += 1; if iterations > MAX { break } }
//! ```
//!
//! with an empty body and no work to bound, and its meshing check compares a
//! hard-coded `120_000` with the element bound, so the comparison can only ever
//! succeed. Both bounds are kept here and given inputs that can cross them:
//! [`BrepEvaluator::evaluate`] walks a caller-supplied face count and
//! [`BrepEvaluator::mesh`] a caller-supplied element count, so the 500,000th
//! element is accepted and the 500,001st is refused.
//!
//! # Fixed point, not floating point
//!
//! The scaffold carries tolerances and element sizes as `f64` millimetres. A
//! boundary test on a binary float is a test on rounding, so every length here
//! is an integer count of micrometres and every comparison is exact. That is
//! the same move `aegis-minerva` made for its reward scale.
//!
//! # What this module does not do
//!
//! No `CAD` kernel is linked, no `STEP` file is opened or parsed, no mesher
//! runs and no element is generated. An evaluation is arithmetic over a count,
//! and a mesh is a validated number. The engines are recorded as names in
//! [`CadEngine`] and `register` records that no version of either is pinned.

use crate::error::HephaestusError;
use crate::id::StepPath;

/// Scalar upper bound on the iterations one geometry evaluation may take.
///
/// One thousand is the scaffold's own `MAX_GEOMETRY_ITERATIONS`.
pub const MAX_GEOMETRY_ITERATIONS: usize = 1_000;

/// Scalar upper bound on the elements one mesh may hold.
///
/// Five hundred thousand is the scaffold's own `MAX_MESH_ELEMENTS`, recorded as
/// REQ-P14-02, which gives preventing out-of-memory failures as the reason.
pub const MAX_MESH_ELEMENTS: usize = 500_000;

/// The smallest admissible geometry tolerance, in micrometres.
///
/// Chosen here, not recorded: no P14 requirement states a tolerance range. One
/// micrometre is the finest figure the fixed-point unit can express, and it is
/// also the scaffold's own tolerance.
pub const MIN_TOLERANCE_MICROMETRES: u32 = 1;

/// The largest admissible geometry tolerance, in micrometres.
///
/// Chosen here, not recorded, for the same reason as the lower end. One
/// millimetre. The scaffold's own tolerance is `0.001` millimetres, which is
/// the bottom of this range rather than the top.
pub const MAX_TOLERANCE_MICROMETRES: u32 = 1_000;

/// The `STEP` application protocol the scaffold names.
///
/// One variant, on purpose, and the same device the rest of this workspace
/// uses: `AP242` is the only protocol either source names, so a payload
/// claiming another does not decode. It has a consequence on the P14-to-P15
/// wire that is worth stating rather than discovering. Because the protocol
/// field cannot vary, the viewport's engine-against-protocol check turns on
/// the engine alone -- see
/// [`GeometryViewport::validate`](crate::contracts::viewport::GeometryViewport::validate)
/// -- and [`CadEngine::PolyhedralCsg`], the second half of REQ-P14-01's
/// recorded dual-engine approach, therefore has no representable viewport at
/// this milestone at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum StepSchema {
    /// `AP242`, the protocol export-021 and export-029 both name.
    #[serde(rename = "AP242")]
    Ap242,
}

impl StepSchema {
    /// The only protocol this build admits.
    pub const ADMITTED: Self = Self::Ap242;

    /// Returns the stable tag the wire form carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Ap242 => "AP242",
        }
    }
}

/// Which of the two recorded engines a geometry request names (REQ-P14-01).
///
/// The dual-engine approach is recorded, not implemented: neither engine is
/// linked, and `register` records that no version of either is pinned. The
/// names are the roles the sources describe rather than product names, because
/// naming a product would read as a dependency choice nobody has made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum CadEngine {
    /// Exact `NURBS` boundary representation, for `STEP` interchange.
    ExactNurbsBrep,
    /// Polyhedral constructive solid geometry, for watertight booleans.
    PolyhedralCsg,
}

impl CadEngine {
    /// Both engines, in the order the sources list them.
    pub const BOTH: [Self; 2] = [Self::ExactNurbsBrep, Self::PolyhedralCsg];

    /// Returns the stable tag the wire form carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::ExactNurbsBrep => "exact-nurbs-brep",
            Self::PolyhedralCsg => "polyhedral-csg",
        }
    }

    /// Returns `true` when the engine is the one that reads `STEP` directly.
    #[must_use]
    pub const fn reads_step(self) -> bool {
        matches!(self, Self::ExactNurbsBrep)
    }
}

/// One validated geometry tolerance, in micrometres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tolerance(u32);

impl Tolerance {
    /// The scaffold's own tolerance: `0.001` millimetres, which is one
    /// micrometre.
    pub const SCAFFOLD: Self = Self(1);

    /// Parses `micrometres` into a tolerance.
    ///
    /// # Errors
    ///
    /// Returns [`HephaestusError::ToleranceOutOfRange`] outside
    /// [`MIN_TOLERANCE_MICROMETRES`]`..=`[`MAX_TOLERANCE_MICROMETRES`].
    pub fn new(micrometres: u32) -> Result<Self, HephaestusError> {
        if !(MIN_TOLERANCE_MICROMETRES..=MAX_TOLERANCE_MICROMETRES).contains(&micrometres) {
            return Err(HephaestusError::ToleranceOutOfRange {
                micrometres,
                min: MIN_TOLERANCE_MICROMETRES,
                max: MAX_TOLERANCE_MICROMETRES,
            });
        }
        Ok(Self(micrometres))
    }

    /// Returns the tolerance in micrometres.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// One validated mesh element count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MeshElementCount(usize);

impl MeshElementCount {
    /// Parses `elements` into an element count.
    ///
    /// # Errors
    ///
    /// Returns [`HephaestusError::MeshElementsOutOfRange`] past
    /// [`MAX_MESH_ELEMENTS`]. Zero is admissible: an empty mesh is a result, not
    /// a malformed number.
    pub fn new(elements: usize) -> Result<Self, HephaestusError> {
        if elements > MAX_MESH_ELEMENTS {
            return Err(HephaestusError::MeshElementsOutOfRange {
                elements,
                max: MAX_MESH_ELEMENTS,
            });
        }
        Ok(Self(elements))
    }

    /// Returns the element count.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

/// The element size range and refinement flag one mesh is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshConfig {
    min_micrometres: u32,
    max_micrometres: u32,
    boundary_layer_refinement: bool,
}

impl MeshConfig {
    /// Builds a mesh configuration.
    ///
    /// # Errors
    ///
    /// Returns [`HephaestusError::ElementSizeRangeInvalid`] when the smallest
    /// element size is zero or larger than the largest. The scaffold carries
    /// both as floats and checks neither.
    pub fn new(
        min_micrometres: u32,
        max_micrometres: u32,
        boundary_layer_refinement: bool,
    ) -> Result<Self, HephaestusError> {
        if min_micrometres == 0 || min_micrometres > max_micrometres {
            return Err(HephaestusError::ElementSizeRangeInvalid {
                min: min_micrometres,
                max: max_micrometres,
            });
        }
        Ok(Self {
            min_micrometres,
            max_micrometres,
            boundary_layer_refinement,
        })
    }

    /// Returns the smallest element size, in micrometres.
    #[must_use]
    pub const fn min_micrometres(&self) -> u32 {
        self.min_micrometres
    }

    /// Returns the largest element size, in micrometres.
    #[must_use]
    pub const fn max_micrometres(&self) -> u32 {
        self.max_micrometres
    }

    /// Returns whether boundary-layer refinement was asked for.
    #[must_use]
    pub const fn boundary_layer_refinement(&self) -> bool {
        self.boundary_layer_refinement
    }
}

/// What a bounded geometry evaluation concluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EvaluationOutcome {
    /// Every face was walked inside the iteration bound.
    Completed {
        /// How many iterations the walk took.
        iterations: usize,
    },
    /// The walk stopped at the bound with faces left, so nothing is concluded.
    StoppedAtIterationBound {
        /// The scalar bound that was hit.
        max: usize,
    },
}

impl EvaluationOutcome {
    /// Returns `true` only when the walk finished inside the bound.
    ///
    /// There is deliberately no `is_watertight`. The scaffold prints "topology
    /// verified watertight" after a loop with an empty body; this crate walks a
    /// count and can say only whether it reached the end of it.
    #[must_use]
    pub const fn is_complete(self) -> bool {
        matches!(self, Self::Completed { .. })
    }
}

/// The recorded evaluation over a geometry specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BrepEvaluator {
    engine: CadEngine,
    schema: StepSchema,
    tolerance: Tolerance,
}

impl BrepEvaluator {
    /// Builds an evaluator for `engine` at `tolerance`.
    #[must_use]
    pub const fn new(engine: CadEngine, tolerance: Tolerance) -> Self {
        Self {
            engine,
            schema: StepSchema::ADMITTED,
            tolerance,
        }
    }

    /// Returns the engine this evaluator was built for.
    #[must_use]
    pub const fn engine(&self) -> CadEngine {
        self.engine
    }

    /// Returns the `STEP` protocol this evaluator admits.
    #[must_use]
    pub const fn schema(&self) -> StepSchema {
        self.schema
    }

    /// Returns the tolerance this evaluator was built for.
    #[must_use]
    pub const fn tolerance(&self) -> Tolerance {
        self.tolerance
    }

    /// Walks `faces` topological faces of the named source, inside the bound.
    ///
    /// # Errors
    ///
    /// Returns [`HephaestusError::StepSourceMissing`] when no source is named.
    /// That is where the scaffold's `!step_path.exists()` refusal lands: this
    /// crate reads no filesystem, so "missing" means the request carried no
    /// source, and whether a named path resolves to a file is the loader's
    /// question and is not asked here.
    pub fn evaluate(
        &self,
        source: Option<StepPath>,
        faces: usize,
    ) -> Result<EvaluationOutcome, HephaestusError> {
        if source.is_none() {
            return Err(HephaestusError::StepSourceMissing);
        }
        let mut walked = 0usize;
        for _ in 0..MAX_GEOMETRY_ITERATIONS {
            if walked >= faces {
                break;
            }
            walked = walked.saturating_add(1);
        }
        if walked < faces {
            return Ok(EvaluationOutcome::StoppedAtIterationBound {
                max: MAX_GEOMETRY_ITERATIONS,
            });
        }
        Ok(EvaluationOutcome::Completed { iterations: walked })
    }

    /// Plans a mesh of `elements` elements under `config`.
    ///
    /// # Errors
    ///
    /// Returns [`HephaestusError::MeshElementsOutOfRange`] past
    /// [`MAX_MESH_ELEMENTS`]. Unlike the scaffold, the count is the caller's,
    /// so the bound can actually be crossed.
    pub fn mesh(&self, config: MeshConfig, elements: usize) -> Result<MeshPlan, HephaestusError> {
        Ok(MeshPlan {
            engine: self.engine,
            config,
            elements: MeshElementCount::new(elements)?,
        })
    }
}

/// A validated mesh request. Not a mesh: nothing here generates an element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshPlan {
    /// The engine the plan was made for.
    pub engine: CadEngine,
    /// The element size range and refinement flag the caller asked for.
    pub config: MeshConfig,
    /// The validated element count, inside [`MAX_MESH_ELEMENTS`].
    pub elements: MeshElementCount,
}
