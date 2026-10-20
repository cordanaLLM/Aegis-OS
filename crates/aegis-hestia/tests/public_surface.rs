// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The generalised public-surface sweep, carried over from
//! `aegis-janus-lifecycle`.
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
//! verification found them off it: a sweep that only understood
//! keyword-introduced items passed over a planted `pub untested_knob: u8` and
//! a planted `pub static UNTESTED_REGISTER`. This crate has more all-public
//! field types than that one -- every descriptor, every window and every
//! register record -- so the field arm carries more weight here, not less.
//!
//! What the rule does not claim: a name that appears is not thereby proved
//! correct. The triples that exercise these items live in this crate's other
//! test files. This sweep only guarantees that none of them is forgotten.
//!
//! This file excludes itself from the scanned test sources. A sweep that could
//! read its own expectation list would pass by naming the names it checks.

/// This file's name, excluded from the scanned test sources.
const SWEEP_FILE: &str = "public_surface.rs";

/// The smallest public surface the scanner may report before it is assumed to
/// be reading nothing rather than finding nothing.
///
/// The figure is measured, not guessed: 143 names are collected from this
/// crate's `src/`, of which [`FIELD_ARM_NAMES`] come from the public-field and
/// `pub static` arm of [`declared_name`] that the M15 verification found
/// missing. The floor sits a little below the total so that removing one item
/// on purpose does not fail the gate, while dropping the field arm -- which
/// would cost far more than that -- does. Removing public surface on purpose
/// means lowering the floor on purpose.
const MINIMUM_SURFACE: usize = 138;

/// How many of the collected names only the M15 arm reads.
///
/// Measured on 2026-09-13 by running the same walk twice: 143 names with
/// the arm and 117 without it. `the_field_arm_carries_the_floor` asserts
/// the difference, so the figure cannot drift out of agreement with the code
/// the way a number stated only in prose can. Every one of the 26 is a
/// public struct field; neither crate declares a `pub static` today, so that
/// half of the arm is coverage for a shape this `src/` does not yet use.
const FIELD_ARM_NAMES: usize = 26;

#[path = "../../surface_scan.rs"]
mod surface_scan;

// --- Positive -------------------------------------------------------------

/// Positive: hestia public surface; see [`surface_scan::assert_every_public_item_is_named`].
#[test]
fn every_public_item_is_named_by_an_integration_test() {
    surface_scan::assert_every_public_item_is_named(
        env!("CARGO_MANIFEST_DIR"),
        SWEEP_FILE,
        MINIMUM_SURFACE,
    );
}

// --- Negative -------------------------------------------------------------

/// Negative: hestia public surface; see [`surface_scan::assert_reads_public_surface_and_not_private_helpers`].
#[test]
fn the_scanner_reads_public_surface_and_not_private_helpers() {
    surface_scan::assert_reads_public_surface_and_not_private_helpers(
        env!("CARGO_MANIFEST_DIR"),
        &[
            "PgliteVectorStore",
            "QueryLimit",
            "DmaBufFd",
            "MAX_QUERY_LIMIT",
            "PGLITE_OPFS_BUFFER_MB",
            "NotInitialised",
            "D09_HESTIA_LOCATION",
            "P15_RECORDED_CLAIMS",
            "OverlayRegistration",
            "initialised",
            "correlation_id",
            "sha256_prefix",
            "rejected",
        ],
        &[
            "permitted",
            "peek_schema",
            "peek_correlation",
            "classify",
            "decode_text",
            "expecting",
            "visit_str",
            "check",
            "PeekSchema",
            "PeekCorrelation",
            "Parsed",
            "serialize_text",
        ],
    );
}

/// Negative: hestia fields and statics; see [`surface_scan::assert_reads_fields_and_statics`].
#[test]
fn the_scanner_reads_public_fields_and_statics_and_not_private_ones() {
    surface_scan::assert_reads_fields_and_statics(
        &[
            ("pub untested_knob: u8,", "untested_knob"),
            (
                "pub static UNTESTED_REGISTER: StoreState = StoreState::Uninitialised;",
                "UNTESTED_REGISTER",
            ),
            ("pub layer: ShellLayer,", "layer"),
            ("pub dma_buf: DmaBufFd,", "dma_buf"),
            ("pub buffer_mb: usize,", "buffer_mb"),
            ("pub const fn area(&self) -> u64 {", "area"),
        ],
        &[
            "active: Option<PipSurface>,",
            "pub(crate) fn helper(&self) -> u8 {",
            "pub use crate::store::QueryLimit;",
            "pub mod contracts;",
            "let pub_like = 1;",
            "",
        ],
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: hestia identifier matching; see [`surface_scan::assert_identifier_matcher_is_exact`].
#[test]
fn the_identifier_matcher_is_exact_at_both_ends() {
    surface_scan::assert_identifier_matcher_is_exact(
        &[
            ("HestiaError::NotInitialised", "NotInitialised"),
            ("QueryLimit", "QueryLimit"),
        ],
        &[
            ("MAX_QUERY_LIMIT", "QUERY_LIMIT"),
            ("buffer_mb_of", "buffer_mb"),
            ("outer_QueryLimit", "QueryLimit"),
            ("QueryLimit2", "QueryLimit"),
            ("", "QueryLimit"),
        ],
    );
}

/// Boundary: hestia field arm floor; see [`surface_scan::assert_field_arm_carries_the_floor`].
#[test]
fn the_field_arm_carries_the_floor() {
    surface_scan::assert_field_arm_carries_the_floor(
        env!("CARGO_MANIFEST_DIR"),
        FIELD_ARM_NAMES,
        MINIMUM_SURFACE,
    );
}
