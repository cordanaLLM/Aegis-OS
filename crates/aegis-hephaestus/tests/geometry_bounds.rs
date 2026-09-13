// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E08-2: the bounded geometry and meshing loops.
//!
//! Positive: a mesh under the bound is accepted. Negative: a request naming no
//! `STEP` source errors -- whether a named path resolves to a file is the
//! loader's question and is not asked here. Boundary: 500,000 elements are
//! accepted and 500,001 rejected, and the iteration bound is held.
//!
//! The scaffold holds neither bound against anything: its evaluation loop has
//! an empty body, and its meshing check compares the constant `120_000` with
//! its own limit, so the comparison can only ever succeed. Every case below
//! feeds a caller-supplied number, so both bounds can actually be crossed.

mod common;

use aegis_hephaestus::{
    BrepEvaluator, CadEngine, EvaluationOutcome, HephaestusError, IdError, MAX_GEOMETRY_ITERATIONS,
    MAX_MESH_ELEMENTS, MAX_STEP_PATH_LEN, MAX_TOLERANCE_MICROMETRES, MIN_TOLERANCE_MICROMETRES,
    MeshConfig, MeshElementCount, MeshPlan, StepPath, StepSchema, Tolerance,
};

use common::{Fallible, SCAFFOLD_ELEMENTS, evaluator, mesh_config, step_source};

// --- Positive -------------------------------------------------------------

/// Positive: a mesh under the bound is accepted, and the plan carries what it
/// was asked for.
#[test]
fn a_mesh_under_the_bound_is_accepted() -> Fallible {
    let engine = evaluator();
    let config = mesh_config()?;
    let plan: MeshPlan = engine.mesh(config, SCAFFOLD_ELEMENTS)?;
    assert_eq!(plan.elements.get(), SCAFFOLD_ELEMENTS);
    assert_eq!(plan.engine, CadEngine::ExactNurbsBrep);
    assert_eq!(plan.config.min_micrometres(), 100);
    assert_eq!(plan.config.max_micrometres(), 2_000);
    assert!(plan.config.boundary_layer_refinement());
    assert_eq!(engine.mesh(config, 0)?.elements.get(), 0);
    Ok(())
}

/// Positive: a bounded evaluation over a named source completes.
#[test]
fn a_bounded_evaluation_completes() -> Fallible {
    let engine = evaluator();
    let outcome = engine.evaluate(Some(step_source()?), 120)?;
    assert_eq!(outcome, EvaluationOutcome::Completed { iterations: 120 });
    assert!(outcome.is_complete());
    assert_eq!(
        engine.evaluate(Some(step_source()?), 0)?,
        EvaluationOutcome::Completed { iterations: 0 }
    );
    Ok(())
}

/// Positive: the evaluator reports what it was built for.
#[test]
fn the_evaluator_reports_its_configuration() -> Fallible {
    let engine = evaluator();
    assert_eq!(engine.engine(), CadEngine::ExactNurbsBrep);
    assert_eq!(engine.schema(), StepSchema::Ap242);
    assert_eq!(engine.schema().tag(), "AP242");
    assert_eq!(engine.tolerance(), Tolerance::SCAFFOLD);
    assert_eq!(engine.tolerance().get(), MIN_TOLERANCE_MICROMETRES);
    let polyhedral = BrepEvaluator::new(CadEngine::PolyhedralCsg, Tolerance::new(500)?);
    assert_eq!(polyhedral.engine(), CadEngine::PolyhedralCsg);
    assert_eq!(polyhedral.tolerance().get(), 500);
    Ok(())
}

/// Positive: both recorded engines are named, and only one reads the protocol.
#[test]
fn both_engines_are_recorded_and_one_reads_step() {
    assert_eq!(
        CadEngine::BOTH.map(CadEngine::tag),
        ["exact-nurbs-brep", "polyhedral-csg"]
    );
    assert!(CadEngine::ExactNurbsBrep.reads_step());
    assert!(!CadEngine::PolyhedralCsg.reads_step());
}

/// Positive: a bounded source path parses and reads back whole.
#[test]
fn a_step_source_path_parses() -> Fallible {
    let source = step_source()?;
    assert_eq!(source.to_string(), common::STEP_SOURCE);
    assert_eq!(source.len(), common::STEP_SOURCE.len());
    assert!(!source.is_empty());
    assert_eq!(source.as_bytes(), common::STEP_SOURCE.as_bytes());
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a request naming no `STEP` source errors.
///
/// This is the epic's negative case, in the terms the epic now states it. The
/// scaffold asks the filesystem; this crate reads no filesystem, so the case it
/// can answer is a request that named no source at all, and whether a
/// well-formed name resolves to a file on some machine is the loader's question
/// and is not asked here.
#[test]
fn a_request_naming_no_step_source_errors() {
    let engine = evaluator();
    assert_eq!(
        engine.evaluate(None, 120),
        Err(HephaestusError::StepSourceMissing)
    );
    assert_eq!(
        engine.evaluate(None, 0),
        Err(HephaestusError::StepSourceMissing)
    );
}

/// Negative: a malformed source path is refused whole, never truncated.
#[test]
fn a_malformed_step_path_is_refused() {
    assert_eq!(StepPath::parse(""), Err(IdError::Empty));
    let long = "a".repeat(MAX_STEP_PATH_LEN + 1);
    assert_eq!(
        StepPath::parse(&long),
        Err(IdError::TooLong {
            max: MAX_STEP_PATH_LEN,
            actual: MAX_STEP_PATH_LEN + 1,
        })
    );
    assert_eq!(StepPath::parse("bracket step"), Err(IdError::Charset));
    assert_eq!(StepPath::parse("bracket\u{0}.step"), Err(IdError::Charset));
}

/// Negative: a path refusal converts into the crate's error type.
///
/// No library path produces this variant: [`StepPath::parse`] returns
/// [`IdError`] and the evaluator never parses a path itself. It exists so a
/// caller that builds a path and then evaluates it can carry both refusals
/// through one `?`, and this case is what keeps it from being surface nothing
/// exercises.
#[test]
fn a_path_refusal_converts_into_the_crate_error() {
    assert_eq!(
        HephaestusError::from(IdError::Empty),
        HephaestusError::Identifier(IdError::Empty)
    );
    let refusal = StepPath::parse("").map_err(HephaestusError::from);
    assert_eq!(
        refusal.map(|_| ()),
        Err(HephaestusError::Identifier(IdError::Empty))
    );
}

/// Negative: an empty or inverted element-size range is refused, and a
/// tolerance outside its range with it.
#[test]
fn an_invalid_mesh_configuration_is_refused() {
    assert_eq!(
        MeshConfig::new(0, 2_000, false),
        Err(HephaestusError::ElementSizeRangeInvalid { min: 0, max: 2_000 })
    );
    assert_eq!(
        MeshConfig::new(3_000, 2_000, false),
        Err(HephaestusError::ElementSizeRangeInvalid {
            min: 3_000,
            max: 2_000,
        })
    );
    assert_eq!(
        Tolerance::new(0),
        Err(HephaestusError::ToleranceOutOfRange {
            micrometres: 0,
            min: MIN_TOLERANCE_MICROMETRES,
            max: MAX_TOLERANCE_MICROMETRES,
        })
    );
    assert!(Tolerance::new(MAX_TOLERANCE_MICROMETRES + 1).is_err());
}

// --- Boundary -------------------------------------------------------------

/// Boundary: 500,000 elements are accepted and 500,001 rejected.
#[test]
fn the_mesh_bound_holds_at_both_ends() -> Fallible {
    let engine = evaluator();
    let config = mesh_config()?;
    let at_bound = engine.mesh(config, MAX_MESH_ELEMENTS)?;
    assert_eq!(at_bound.elements.get(), MAX_MESH_ELEMENTS);
    assert_eq!(MAX_MESH_ELEMENTS, 500_000);

    assert_eq!(
        engine.mesh(config, MAX_MESH_ELEMENTS + 1).map(|_| ()),
        Err(HephaestusError::MeshElementsOutOfRange {
            elements: MAX_MESH_ELEMENTS + 1,
            max: MAX_MESH_ELEMENTS,
        })
    );
    assert_eq!(
        MeshElementCount::new(MAX_MESH_ELEMENTS)?.get(),
        MAX_MESH_ELEMENTS
    );
    assert!(MeshElementCount::new(MAX_MESH_ELEMENTS + 1).is_err());
    Ok(())
}

/// Boundary: the iteration bound is held, so a walk that would run past it
/// stops and says so rather than reporting a completion it did not reach.
#[test]
fn the_iteration_bound_is_held() -> Fallible {
    let engine = evaluator();
    assert_eq!(MAX_GEOMETRY_ITERATIONS, 1_000);

    let at_bound = engine.evaluate(Some(step_source()?), MAX_GEOMETRY_ITERATIONS)?;
    assert_eq!(
        at_bound,
        EvaluationOutcome::Completed {
            iterations: MAX_GEOMETRY_ITERATIONS,
        }
    );
    assert!(at_bound.is_complete());

    for faces in [
        MAX_GEOMETRY_ITERATIONS + 1,
        MAX_GEOMETRY_ITERATIONS * 10,
        usize::MAX,
    ] {
        let over = engine.evaluate(Some(step_source()?), faces)?;
        assert_eq!(
            over,
            EvaluationOutcome::StoppedAtIterationBound {
                max: MAX_GEOMETRY_ITERATIONS,
            }
        );
        assert!(!over.is_complete());
    }
    Ok(())
}

/// Boundary: a source path of exactly the byte bound is accepted and the next
/// byte is refused.
#[test]
fn a_step_path_at_the_byte_bound_is_accepted() -> Fallible {
    assert_eq!(MAX_STEP_PATH_LEN, 128);
    let at_bound = "b".repeat(MAX_STEP_PATH_LEN);
    let parsed = StepPath::parse(&at_bound)?;
    assert_eq!(parsed.len(), MAX_STEP_PATH_LEN);
    assert!(StepPath::parse(&"b".repeat(MAX_STEP_PATH_LEN + 1)).is_err());
    Ok(())
}

/// Boundary: the tolerance range is admissible at both ends and refused past
/// either.
#[test]
fn the_tolerance_range_holds_at_both_ends() -> Fallible {
    assert_eq!(MAX_TOLERANCE_MICROMETRES, 1_000);
    assert_eq!(
        Tolerance::new(MIN_TOLERANCE_MICROMETRES)?.get(),
        MIN_TOLERANCE_MICROMETRES
    );
    assert_eq!(
        Tolerance::new(MAX_TOLERANCE_MICROMETRES)?.get(),
        MAX_TOLERANCE_MICROMETRES
    );
    assert!(Tolerance::new(MIN_TOLERANCE_MICROMETRES - 1).is_err());
    assert!(Tolerance::new(MAX_TOLERANCE_MICROMETRES + 1).is_err());
    assert!(
        Tolerance::new(MIN_TOLERANCE_MICROMETRES)? < Tolerance::new(MAX_TOLERANCE_MICROMETRES)?
    );
    Ok(())
}
