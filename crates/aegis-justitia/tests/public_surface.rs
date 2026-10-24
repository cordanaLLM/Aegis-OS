// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The generalised public-surface sweep.
//!
//! HISS-15 asks for positive, negative and boundary coverage of every public
//! interface. Reviewing that by hand caught individual holes and then let new
//! ones in, so the sweep is mechanised here instead of repeated by eye.
//!
//! The rule is deliberately crude and therefore checkable: every name the
//! crate's `src/` declares as public API -- a type, an enum variant, a method,
//! a trait item, an associated or free constant -- must appear as a whole
//! identifier in at least one integration test. A public item with no test
//! reference is either untested surface or surface that should not exist; both
//! are findings, and this file turns either into a failed gate.
//!
//! What the rule does not claim: a name that appears is not thereby proved
//! correct. The triples that exercise these items live in the other test files,
//! `public_surface_triples.rs` included. This sweep only guarantees that none
//! of them is forgotten.
//!
//! This file excludes itself from the scanned test sources. A sweep that could
//! read its own expectation list would pass by naming the names it checks.

/// This file's name, excluded from the scanned test sources.
const SWEEP_FILE: &str = "public_surface.rs";

/// The smallest public surface the scanner may report before it is assumed to
/// be reading nothing rather than finding nothing.
const MINIMUM_SURFACE: usize = 100;

#[path = "../../surface_scan.rs"]
mod surface_scan;

// --- Positive -------------------------------------------------------------

/// Positive: justitia public surface; see [`surface_scan::assert_every_keyword_declared_item_is_named`].
#[test]
fn every_public_item_is_named_by_an_integration_test() {
    surface_scan::assert_every_keyword_declared_item_is_named(
        env!("CARGO_MANIFEST_DIR"),
        SWEEP_FILE,
        MINIMUM_SURFACE,
    );
}

// --- Negative -------------------------------------------------------------

/// Negative: justitia public surface; see [`surface_scan::assert_reads_keyword_surface_and_not_private_helpers`].
#[test]
fn the_scanner_reads_public_surface_and_not_private_helpers() {
    surface_scan::assert_reads_keyword_surface_and_not_private_helpers(
        env!("CARGO_MANIFEST_DIR"),
        &[
            "Chain",
            "ChainRecord",
            "Timestamped",
            "SignedAuditRecord",
            "DecisionRequest",
            "ActionProposal",
            "DecisionRecord",
            "DecisionState",
            "SchemaId",
            "Correlation",
        ],
        &[
            "verify_link",
            "validate_correlation",
            "validate_hex",
            "check_edge",
            "extract_pld",
        ],
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: justitia identifier matching; see [`surface_scan::assert_identifier_matcher_is_exact`].
#[test]
fn the_identifier_matcher_is_exact_at_both_ends() {
    surface_scan::assert_identifier_matcher_is_exact(
        &[
            ("InterceptorOutcome::Block { reason }", "Block"),
            ("Block", "Block"),
        ],
        &[
            ("BlockReason::RegistryFull", "Block"),
            ("InterceptorOutcome", "Outcome"),
            ("outer_Block", "Block"),
            ("Block2", "Block"),
            ("", "Block"),
        ],
    );
}
