// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The generalised public-surface sweep, carried over from `aegis-fabrica-defs`.
//!
//! HISS-15 asks for positive, negative and boundary coverage of every public
//! interface. Reviewing that by hand caught individual holes and then let new
//! ones in, so the sweep is mechanised here instead of repeated by eye.
//!
//! The rule is deliberately crude and therefore checkable: every name the
//! crate's `src/` declares as public API -- a type, an enum variant, a method,
//! a trait item, an associated or free constant, a public struct field, a
//! public static -- must appear as a whole identifier in at least one
//! integration test. A public item with no test reference is either untested
//! surface or surface that should not exist; both are findings, and this file
//! turns either into a failed gate.
//!
//! Public struct fields and statics are on that list because the M15
//! verification found them off it. `TraceHeader`, `TraceLine`,
//! `DecisionRecord` and `Citation` are all-public-field structs -- the whole
//! rendered form of the trace M24 will diff against -- and a planted
//! `pub untested_knob: u8` and `pub static UNTESTED_REGISTER` both passed a
//! sweep that only understood keyword-introduced items.
//!
//! What the rule does not claim: a name that appears is not thereby proved
//! correct. The triples that exercise these items live in this crate's other
//! test files: `lifecycle_happy_path.rs`, `lifecycle_refusals.rs`,
//! `watchdog_boundary.rs`, `consolidation_decision.rs`, `transition_trace.rs`,
//! `stubbed_ports.rs`, `stubbed_effects.rs`, `bounded_values.rs` and
//! `allocation_bounds.rs`. This sweep only guarantees that none of them is
//! forgotten.
//!
//! This file excludes itself from the scanned test sources. A sweep that could
//! read its own expectation list would pass by naming the names it checks.

/// This file's name, excluded from the scanned test sources.
const SWEEP_FILE: &str = "public_surface.rs";

/// The smallest public surface the scanner may report before it is assumed to
/// be reading nothing rather than finding nothing.
///
/// The floor is deliberately above 173, which is what the keyword arms of
/// [`declared_name`] collect on their own: the public fields and statics the
/// M15 verification found unswept are 13 names more, so dropping that arm
/// again fails this bound rather than quietly narrowing the sweep. Removing
/// public surface on purpose means lowering the floor on purpose.
const MINIMUM_SURFACE: usize = 180;

#[path = "../../surface_scan.rs"]
mod surface_scan;

// --- Positive -------------------------------------------------------------

/// Positive: janus-lifecycle public surface; see [`surface_scan::assert_every_public_item_is_named`].
#[test]
fn every_public_item_is_named_by_an_integration_test() {
    surface_scan::assert_every_public_item_is_named(
        env!("CARGO_MANIFEST_DIR"),
        SWEEP_FILE,
        MINIMUM_SURFACE,
    );
}

// --- Negative -------------------------------------------------------------

/// Negative: janus-lifecycle public surface; see [`surface_scan::assert_reads_public_surface_and_not_private_helpers`].
#[test]
fn the_scanner_reads_public_surface_and_not_private_helpers() {
    surface_scan::assert_reads_public_surface_and_not_private_helpers(
        env!("CARGO_MANIFEST_DIR"),
        &[
            "Machine",
            "Trace",
            "Transition",
            "TRACE_SCHEMA",
            "MAX_TRANSITIONS",
            "VerityMismatch",
            "D13_CONSOLIDATION",
            "SysupdatePort",
            "StubClock",
            "expected_root_hash",
            "transitions",
            "sha256_prefix",
            "reconciliation",
        ],
        &[
            "plan_idle",
            "plan_declared",
            "plan_bless",
            "plan_pipeline",
            "unexpected",
            "permitted",
            "read_header",
            "candidate_of",
            "append",
            "Effect",
            "Plan",
            "BLANK",
            "measured",
            "guard",
            "effect",
            "next",
        ],
    );
}

/// Negative: janus-lifecycle fields and statics; see [`surface_scan::assert_reads_fields_and_statics`].
///
/// These assertions are on [`surface_scan::declared_name`] directly rather than on the
/// collected surface, because the crate declares no `pub static` of its own
/// and a field it does not declare cannot be scanned for. The two inputs the
/// M15 verification planted -- `pub untested_knob: u8` on `TraceHeader` and
/// `pub static UNTESTED_REGISTER` in `decision.rs`, both of which the sweep
/// passed over -- are the first two cases here.
#[test]
fn the_scanner_reads_public_fields_and_statics_and_not_private_ones() {
    surface_scan::assert_reads_fields_and_statics(
        &[
            ("pub untested_knob: u8,", "untested_knob"),
            (
                "pub static UNTESTED_REGISTER: DecisionState = DecisionState::Unresolved;",
                "UNTESTED_REGISTER",
            ),
            ("pub schema: String,", "schema"),
            ("pub sources: [Citation; 2],", "sources"),
            ("pub slot: Option<Slot>,", "slot"),
            ("pub const fn seq(self) -> u32 {", "seq"),
        ],
        &[
            "records: [Transition; MAX_TRANSITIONS],",
            "pub(crate) fn helper(&self) -> u8 {",
            "pub use crate::candidate::Candidate;",
            "pub mod candidate;",
            "let pub_like = 1;",
            "",
        ],
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: janus-lifecycle identifier matching; see [`surface_scan::assert_identifier_matcher_is_exact`].
#[test]
fn the_identifier_matcher_is_exact_at_both_ends() {
    surface_scan::assert_identifier_matcher_is_exact(
        &[
            ("LifecycleError::VerityMismatch { .. }", "VerityMismatch"),
            ("Machine", "Machine"),
        ],
        &[
            ("MAX_TRANSITIONS", "TRANSITIONS"),
            ("parse_hex_of", "parse_hex"),
            ("outer_Machine", "Machine"),
            ("Machine2", "Machine"),
            ("", "Machine"),
        ],
    );
}
