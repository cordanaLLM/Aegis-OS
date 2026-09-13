// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The P03 mandate register: what is recorded, and that it stays recorded.
//!
//! REQ-P03-01, REQ-P03-02 and REQ-P03-06 state numbers and boundaries this
//! crate cannot measure. The register writes them down; this file holds the
//! register against the requirement identifiers, so a later edit cannot
//! quietly promote a recorded number into an enforced one, or drop one of the
//! six boundaries.

mod common;

use aegis_vulcan::{Citation, EngineeringMandate, P03_MANDATE, ProtectionBoundary};

use common::Fallible;

// --- Positive -------------------------------------------------------------

/// Positive: the register names the recorded numbers and their requirements.
#[test]
fn the_register_names_the_recorded_mandate() {
    let mandate: EngineeringMandate = P03_MANDATE;
    assert_eq!(mandate.power_budget_watts, 20);
    assert_eq!(mandate.latency_budget_micros, 10);
    assert_eq!(mandate.records, ["REQ-P03-01", "REQ-P03-02", "REQ-P03-06"]);
    assert!(mandate.isolation_model.contains("IOMMU"));
}

/// Positive: every source is cited by export identifier and digest prefix.
#[test]
fn every_source_is_cited_by_identifier_and_prefix() {
    for source in P03_MANDATE.sources {
        let citation: Citation = source;
        assert!(citation.export.starts_with("export-"));
        assert_eq!(citation.sha256_prefix.len(), 12);
        assert!(
            citation
                .sha256_prefix
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        );
    }
}

// --- Negative -------------------------------------------------------------

/// Negative: the register says, in its own text, that it enforces nothing.
///
/// This is the claim that keeps the numbers honest. If a later change makes
/// the crate measure a watt or time a transfer, the sentence stops being true
/// and this case is where that has to be dealt with.
#[test]
fn the_register_states_that_it_enforces_nothing() {
    let enforcement = P03_MANDATE.enforcement;
    assert!(enforcement.contains("recorded only"));
    assert!(enforcement.contains("measures no power"));
    assert!(enforcement.contains("M25"));
}

/// Negative: no crate source names a power or latency measurement interface.
///
/// A regression gate over a named list of six identifiers, not a proof that
/// the crate can never measure anything.
#[test]
fn no_source_reaches_a_measurement_interface() -> Fallible {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let watched = [
        "powercap",
        "energy_uj",
        "intel-rapl",
        "Instant",
        "SystemTime",
        "hwmon",
    ];
    assert_eq!(watched.len(), 6);
    let found = common::scan_sources(&root, &watched)?;
    assert!(found.is_empty(), "measurement interfaces named: {found:?}");
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: exactly six boundaries are recorded, and all are distinct.
#[test]
fn exactly_six_protection_boundaries_are_recorded() {
    assert_eq!(ProtectionBoundary::ALL.len(), 6);
    assert_eq!(P03_MANDATE.boundaries, ProtectionBoundary::ALL);

    let mut names: Vec<&'static str> = ProtectionBoundary::ALL
        .iter()
        .map(|boundary| boundary.name())
        .collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 6);
    assert_eq!(ProtectionBoundary::Scope.name(), "scope");
    assert_eq!(ProtectionBoundary::Budget.name(), "budget");
    assert_eq!(ProtectionBoundary::SensedState.name(), "sensed-state");
    assert_eq!(
        ProtectionBoundary::CausalOperation.name(),
        "causal-operation"
    );
    assert_eq!(ProtectionBoundary::Topology.name(), "topology");
    assert_eq!(ProtectionBoundary::Reversibility.name(), "reversibility");
}
