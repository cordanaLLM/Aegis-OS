// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E07-2: the recorded decision, and the two upstream checks it asks for.
//!
//! Positive: D08 is recorded as settled on a pure Rust compositor, with both
//! rejected options still representable, and both dependency checks carry a
//! date, two reproducing commands and the version upstream reported. Negative:
//! **neither imported requirement is inherited**, neither crate is admitted at
//! this milestone, and a command crates.io would refuse does not pass the
//! command check. Boundary: the recorded claim register is exactly its
//! recorded length.
//!
//! # Why the command check is shaped the way it is
//!
//! The M07 verification found the recorded command reproduced nothing: crates.io
//! answers a request with no `User-Agent` with HTTP 403 and a zero-byte body,
//! and the `/api/v1/crates/<name>` endpoint carries no dependency data, so the
//! single recorded command could not have produced either the version or the
//! dependency counts. The check that let it through asserted only a
//! `curl -s https://crates.io/` prefix. [`reaches_crates_io`] replaces it, and
//! `a_header_less_command_is_not_a_reproducing_command` is the falsifier: the
//! exact string that was recorded is asserted to fail it.

mod common;

use aegis_compositor::{
    AdmittedBackend, COMPOSITOR_LIBRARY_CHECK, Citation, ClaimSource, ClaimStatus,
    CompositorImplementation, D08_COMPOSITOR_IMPLEMENTATION, DecisionRecord, DecisionState,
    M07_UPSTREAM_CHECKS, P04_RECORDED_CLAIMS, RecordedClaim, UpstreamCheck, ZENOH_CHECK,
};

// --- Positive -------------------------------------------------------------

/// Positive: D08 is recorded as settled on a pure Rust compositor.
#[test]
fn the_compositor_decision_is_recorded_as_settled() {
    let record: DecisionRecord = D08_COMPOSITOR_IMPLEMENTATION;
    assert_eq!(
        (record.id, record.dispute, record.applied_at, record.adr),
        ("D08", "DSP-23", "M07", "ADR-0001")
    );
    assert_eq!(record.state, DecisionState::Closed);
    assert_eq!(record.chosen, CompositorImplementation::PureRust);
    assert_eq!(record.touches, ["REQ-P04-01", "REQ-P04-04"]);
    assert!(record.question.contains("pure Rust"));
    assert!(record.rationale.contains("foreign-function boundary"));
    assert!(record.rationale.contains("no wlroots version is pinned"));
    assert_eq!(DecisionState::Closed.name(), "closed");
    assert_eq!(DecisionState::Unresolved.name(), "unresolved");
}

/// Positive: the decision cites the two sources that disagree.
#[test]
fn the_compositor_decision_cites_both_sources() {
    let sources: [Citation; 2] = D08_COMPOSITOR_IMPLEMENTATION.sources;
    assert_eq!(
        (sources[0].export, sources[0].sha256_prefix),
        ("export-013", "7f4c22813332")
    );
    assert_eq!(
        (sources[1].export, sources[1].sha256_prefix),
        ("export-027", "f19640d7a7da")
    );
}

/// Positive: both rejected options remain representable, so the register
/// states a choice between three readings rather than the only one it can
/// spell.
#[test]
fn both_rejected_options_remain_representable() {
    assert_eq!(CompositorImplementation::ALL.len(), 3);
    assert_eq!(
        D08_COMPOSITOR_IMPLEMENTATION.rejected,
        [
            CompositorImplementation::CWlrootsBinding,
            CompositorImplementation::HigherLevelFramework,
        ]
    );
    assert!(CompositorImplementation::CWlrootsBinding.adds_c_toolchain());
    assert!(!CompositorImplementation::PureRust.adds_c_toolchain());
    assert!(CompositorImplementation::HigherLevelFramework.rejected_by_blueprint());
    assert!(!CompositorImplementation::PureRust.rejected_by_blueprint());
    assert_eq!(
        CompositorImplementation::PureRust.name(),
        "pure-rust-compositor"
    );
    assert_eq!(
        CompositorImplementation::CWlrootsBinding.name(),
        "c-wlroots-binding"
    );
    assert_eq!(
        CompositorImplementation::HigherLevelFramework.name(),
        "higher-level-framework"
    );
}

/// Positive: the admitted backend has one variant, so nothing in this crate
/// can spell the superseded mandate.
#[test]
fn the_admitted_backend_has_one_variant() {
    assert_eq!(
        AdmittedBackend::SETTLED,
        AdmittedBackend::PureRustBackendAgnostic
    );
    assert_eq!(AdmittedBackend::SETTLED.tag(), "pure-rust-backend-agnostic");
    assert_eq!(
        AdmittedBackend::SETTLED.implementation(),
        CompositorImplementation::PureRust
    );
    assert_eq!(
        D08_COMPOSITOR_IMPLEMENTATION.chosen,
        AdmittedBackend::SETTLED.implementation()
    );
}

/// The `User-Agent` header crates.io requires of an API client.
///
/// Not a style preference: the same request without it is answered HTTP 403
/// with a zero-byte body, so a recorded command that omits it reproduces
/// nothing at all.
const REQUIRED_HEADER: &str = "-H 'User-Agent: ";

/// Returns `true` when `command` is one crates.io would actually answer.
///
/// A check over two named properties, not a proof that the command runs: it
/// requires the `User-Agent` header and a crates.io API URL. Whether the
/// endpoint carries the field the record quotes is asserted per check below,
/// because that differs between the two commands.
fn reaches_crates_io(command: &str) -> bool {
    command.starts_with("curl -sS ")
        && command.contains(REQUIRED_HEADER)
        && command.contains("https://crates.io/api/v1/crates/")
}

/// Returns `true` when `row`'s version command would reproduce both figures it
/// is recorded against: the highest stable version and its release date.
fn version_command_reproduces(row: UpstreamCheck) -> bool {
    reaches_crates_io(row.version_command)
        && row.version_command.contains(row.crate_name)
        && row.version_command.contains("max_stable_version")
        && row.version_command.contains(".created_at")
}

/// Returns `true` when `row`'s dependency command addresses the per-version
/// dependency endpoint for the version the record names.
///
/// Naming the version is the load-bearing half. The dependency endpoint is
/// per-version, and `/api/v1/crates/<name>` -- which is what this record used
/// to hold -- carries no dependency data at all, so a command that named only
/// the crate would be a command for the wrong endpoint.
fn dependency_command_reproduces(row: UpstreamCheck) -> bool {
    reaches_crates_io(row.dependency_command)
        && row.dependency_command.contains(row.crate_name)
        && row.dependency_command.contains(row.upstream_max_stable)
        && row.dependency_command.contains("/dependencies")
}

/// Positive: both upstream checks carry a date, two distinct commands and the
/// version upstream reported.
#[test]
fn both_upstream_checks_are_dated_and_commanded() {
    assert_eq!(M07_UPSTREAM_CHECKS.len(), 2);
    for check in M07_UPSTREAM_CHECKS {
        let row: UpstreamCheck = check;
        assert!(!row.crate_name.is_empty());
        assert_eq!(row.checked_on, "2026-09-13");
        assert_ne!(row.version_command, row.dependency_command);
        assert!(!row.upstream_max_stable.is_empty());
        assert!(!row.upstream_released_on.is_empty());
        assert!(!row.reason.is_empty());
    }
}

/// Positive: each recorded command is one crates.io would answer, addressing
/// the endpoint that carries the figure it is recorded against.
///
/// A check over named properties of the strings, not a proof that they run:
/// the gate has no network. What it stops is the defect the M07 verification
/// found, where the recorded command sent no `User-Agent` and addressed an
/// endpoint with no dependency data on it.
#[test]
fn both_recorded_commands_reproduce_their_figures() {
    for check in M07_UPSTREAM_CHECKS {
        let row: UpstreamCheck = check;
        assert!(version_command_reproduces(row), "{}", row.crate_name);
        assert!(dependency_command_reproduces(row), "{}", row.crate_name);
    }
}

/// Positive: the mesh check records the version current upstream reported, and
/// the requirement the imported manifest proposes, as two different things.
#[test]
fn the_mesh_check_records_both_versions() {
    let check: UpstreamCheck = ZENOH_CHECK;
    assert_eq!(check.crate_name, "zenoh");
    assert_eq!(check.proposed_requirement, "1.0");
    assert_eq!(check.upstream_max_stable, "1.10.1");
    assert_eq!(check.upstream_released_on, "2026-09-07");
    assert_ne!(check.proposed_requirement, check.upstream_max_stable);
    assert!(check.reason.contains("42 are required and 2 optional"));
    assert!(check.reason.contains("tokio"));
}

/// Positive: the compositor-library check ADR-0001 asks for was run, against
/// the candidate the blueprint names.
#[test]
fn the_compositor_library_check_was_run() {
    let check = COMPOSITOR_LIBRARY_CHECK;
    assert_eq!(check.crate_name, "smithay");
    assert_eq!(check.upstream_max_stable, "0.7.0");
    assert_eq!(check.upstream_released_on, "2025-06-24");
    assert!(check.proposed_requirement.contains("none"));
    assert!(check.admitted_at.contains("M12"));
    assert!(check.reason.contains("drm"));
    assert!(check.reason.contains("19 are required and 25 are"));
    assert!(check.reason.contains("default feature closure"));
}

/// Positive: every recorded claim is complete.
#[test]
fn every_recorded_claim_is_complete() {
    for claim in P04_RECORDED_CLAIMS {
        let row: RecordedClaim = claim;
        assert!(row.requirement.starts_with("REQ-P04-") || row.requirement.starts_with("DSP-"));
        assert!(!row.summary.is_empty());
        assert!(!row.settled_by.is_empty());
        let source: ClaimSource = row.source;
        assert!(source.export.starts_with("export-"));
        assert_eq!(source.sha256_prefix.len(), 12);
        assert!(source.sha256_prefix.chars().all(|c| c.is_ascii_hexdigit()));
    }
}

// --- Negative -------------------------------------------------------------

/// Negative: the command that was recorded before the M07 verification does
/// not pass the command check.
///
/// `curl -s https://crates.io/api/v1/crates/zenoh` is the exact string this
/// register used to hold. It sends no `User-Agent`, so crates.io answers it
/// 403 with a zero-byte body, and it addresses an endpoint with no dependency
/// data on it. Both recorded commands pass; that one does not, which is what
/// makes the positive test above worth its exit code.
#[test]
fn a_header_less_command_is_not_a_reproducing_command() {
    assert!(!reaches_crates_io(
        "curl -s https://crates.io/api/v1/crates/zenoh"
    ));
    assert!(!reaches_crates_io(
        "curl -sS https://crates.io/api/v1/crates/zenoh"
    ));
    assert!(!reaches_crates_io(
        "curl -sS -H 'User-Agent: aegis' https://example.invalid/zenoh"
    ));
    assert!(reaches_crates_io(ZENOH_CHECK.version_command));
    assert!(reaches_crates_io(
        COMPOSITOR_LIBRARY_CHECK.dependency_command
    ));
}

/// Negative: **neither imported requirement is inherited, and neither crate is
/// admitted at this milestone.**
///
/// The exit criterion says the export-006 pin is not inherited. This is that
/// statement in a form a test can falsify: a later edit that quietly adopts
/// `zenoh = "1.0"` has to flip a boolean here first.
#[test]
fn neither_dependency_is_inherited_or_admitted() {
    for check in M07_UPSTREAM_CHECKS {
        assert!(
            !check.inherits_proposal,
            "{} must not inherit a proposed requirement",
            check.crate_name
        );
        assert!(
            !check.admitted_here,
            "{} is not admitted at milestone M07",
            check.crate_name
        );
        assert!(!check.admitted_at.is_empty());
        assert_ne!(check.admitted_at, "M07");
    }
}

/// Negative: the deferred claims stay deferred, each with the standing this
/// milestone can defend.
#[test]
fn the_deferred_claims_stay_deferred() {
    let expected = [
        ("REQ-P04-02", ClaimStatus::DeferredTimingRequirement),
        ("REQ-P04-05", ClaimStatus::DeferredPrivilegedConfiguration),
        ("REQ-P04-06", ClaimStatus::DeferredSubsystem),
        ("DSP-21", ClaimStatus::UnresolvedSourceConflict),
        ("DSP-14", ClaimStatus::UnresolvedSourceConflict),
    ];
    for (requirement, status) in expected {
        let row = P04_RECORDED_CLAIMS
            .iter()
            .find(|claim| claim.requirement == requirement);
        assert_eq!(
            row.map(|claim| claim.status),
            Some(status),
            "{requirement} no longer carries the status this milestone recorded"
        );
    }
}

/// Negative: the mesh latency and throughput targets are recorded as deferred,
/// and the settlement says no figure is produced here.
#[test]
fn the_mesh_timing_target_produces_no_figure_here() {
    let row = P04_RECORDED_CLAIMS
        .iter()
        .find(|claim| claim.requirement == "REQ-P04-02");
    let settled_by = row.map(|claim| claim.settled_by).unwrap_or_default();
    assert!(settled_by.contains("PREEMPT_DYNAMIC"));
    assert!(settled_by.contains("No such figure is produced by this milestone"));
    assert!(settled_by.contains("MockedMesh"));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the claim register is exactly its recorded length, and the status
/// names are distinct.
#[test]
fn the_register_is_its_recorded_length() {
    assert_eq!(P04_RECORDED_CLAIMS.len(), 5);
    let mut requirements: Vec<&str> = P04_RECORDED_CLAIMS
        .iter()
        .map(|claim| claim.requirement)
        .collect();
    requirements.sort_unstable();
    requirements.dedup();
    assert_eq!(requirements.len(), 5);
    assert_eq!(ClaimStatus::DeferredSubsystem.name(), "deferred-subsystem");
    assert_eq!(
        ClaimStatus::DeferredTimingRequirement.name(),
        "deferred-timing-requirement"
    );
    assert_eq!(
        ClaimStatus::DeferredPrivilegedConfiguration.name(),
        "deferred-privileged-configuration"
    );
    assert_eq!(
        ClaimStatus::UnresolvedSourceConflict.name(),
        "unresolved-source-conflict"
    );
}
