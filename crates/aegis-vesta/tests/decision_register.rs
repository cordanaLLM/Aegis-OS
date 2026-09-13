// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E06-3: decision D06, the monitor register and the deferred claims.
//!
//! Positive: D06 is recorded closed, names what was chosen, and cites its
//! sources by export identifier and digest prefix. Negative: the rejected
//! options stay representable but are not the chosen one, and the option the
//! language-boundary rule bars is marked as barred. Boundary: the register's
//! arity is exactly what the decision has, so an option cannot be dropped
//! without failing this file.
//!
//! Nothing here runs a runtime or a monitor. The register is static data.

mod common;

use aegis_vesta::{
    AdmittedRuntime, Citation, ClaimSource, ClaimStatus, D06_WASM_RUNTIME, DecisionRecord,
    DecisionState, P10_RECORDED_CLAIMS, RecordedClaim, VMM_PROBES, VmmIdentity, VmmProbe,
    WasmRuntimeChoice,
};

use common::Fallible;

// --- Positive -------------------------------------------------------------

/// Positive: D06 is recorded, closed, and names the Rust-native runtime.
#[test]
fn d06_is_recorded_as_closed_on_the_rust_native_runtime() {
    let record: DecisionRecord = D06_WASM_RUNTIME;
    assert_eq!(record.id, "D06");
    assert_eq!(record.state, DecisionState::Closed);
    assert_eq!(record.state.name(), "closed");
    assert_eq!(record.chosen, WasmRuntimeChoice::RustNativeRuntime);
    assert_eq!(record.chosen.name(), "rust-native-runtime");
}

/// Positive: D06 records where it is applied, what it touches and why.
#[test]
fn d06_records_where_it_applies_and_why() {
    let record = D06_WASM_RUNTIME;
    assert_eq!(record.applied_at, "M06");
    assert_eq!(record.dispute, "DSP-25");
    assert!(record.question.contains("Wasm capsule runtime"));
    assert!(record.rationale.contains("Go library"));
    assert_eq!(record.touches, ["REQ-P10-08", "REQ-P10-06", "REQ-WS-01"]);
}

/// Positive: the decision cites both sides of the dispute it closes.
///
/// The two digest literals are pinned here so that editing one is a visible
/// test change, not so that they are verified here: comparing a constant with
/// a copy of itself proves nothing. What binds `public:` prefixes to the file
/// they name is `verify_crate_citations()` in `tools/verify_preparation.py`,
/// which hashes `docs/integration/stack.md` and compares the first 12
/// characters; it runs inside `make verify-all`.
#[test]
fn d06_cites_both_sides_of_its_dispute() {
    let [runtime_source, boundary_rule]: [Citation; 2] = D06_WASM_RUNTIME.sources;
    assert_eq!(runtime_source.export, "export-037");
    assert_eq!(runtime_source.sha256_prefix, "ce490c88081f");
    assert_eq!(boundary_rule.export, "public:docs/integration/stack.md");
    assert_eq!(boundary_rule.sha256_prefix, "8d40bdc6886c");
    for citation in D06_WASM_RUNTIME.sources {
        assert_eq!(citation.sha256_prefix.len(), 12);
        assert!(
            citation
                .sha256_prefix
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        );
    }
}

/// Positive: the admitted runtime instantiates the chosen option.
#[test]
fn the_admitted_runtime_instantiates_the_chosen_option() {
    assert_eq!(
        AdmittedRuntime::SETTLED,
        AdmittedRuntime::RustNativeComponentModel
    );
    assert_eq!(AdmittedRuntime::SETTLED.choice(), D06_WASM_RUNTIME.chosen);
    assert_eq!(
        AdmittedRuntime::SETTLED.tag(),
        "rust-native-component-model"
    );
}

/// Positive: both monitor probes are recorded with the command that produced
/// them, and Firecracker is the one D58 chose.
#[test]
fn the_monitor_probes_record_their_commands() {
    assert_eq!(VMM_PROBES.len(), 2);
    assert_eq!(VmmIdentity::CHOSEN, VmmIdentity::Firecracker);
    for probe in VMM_PROBES {
        let row: VmmProbe = probe;
        assert!(row.command.contains("--version"));
        assert!(!row.observed.is_empty());
        assert!(row.package.contains(row.vmm.pinned_version()));
        assert_eq!(row.probed_on, "2026-09-13");
        assert!(
            row.observed.contains(row.vmm.pinned_version()),
            "the probe must report the version D58 pins"
        );
    }
}

/// Positive: both monitors have a distinct recorded tag and pinned version.
#[test]
fn both_monitors_have_a_distinct_tag() {
    assert_eq!(VmmIdentity::BOTH.len(), 2);
    assert_eq!(VmmIdentity::Firecracker.tag(), "firecracker");
    assert_eq!(VmmIdentity::QemuMicrovm.tag(), "qemu-microvm");
    assert_eq!(VmmIdentity::Firecracker.pinned_version(), "1.17.0");
    assert_eq!(VmmIdentity::QemuMicrovm.pinned_version(), "11.1.1");
}

// --- Negative -------------------------------------------------------------

/// Negative: the rejected options stay representable, and the one the
/// language-boundary rule bars is marked as barred.
#[test]
fn the_rejected_options_stay_representable_and_marked() {
    assert_eq!(
        D06_WASM_RUNTIME.rejected,
        [
            WasmRuntimeChoice::OutOfProcessAdapter,
            WasmRuntimeChoice::DirectGoDependency
        ]
    );
    for rejected in D06_WASM_RUNTIME.rejected {
        assert_ne!(rejected, D06_WASM_RUNTIME.chosen);
    }
    assert_eq!(DecisionState::Unresolved.name(), "unresolved");
}

/// Negative: the language-boundary rule bars exactly one option, and the
/// register says which and why.
#[test]
fn the_language_boundary_rule_bars_exactly_one_option() {
    assert!(WasmRuntimeChoice::DirectGoDependency.adds_go_dependency());
    assert!(!WasmRuntimeChoice::DirectGoDependency.permitted_by_language_boundary());
    assert!(WasmRuntimeChoice::OutOfProcessAdapter.needs_adapter());
    assert!(WasmRuntimeChoice::OutOfProcessAdapter.permitted_by_language_boundary());
    assert!(!WasmRuntimeChoice::RustNativeRuntime.needs_adapter());
    assert!(!WasmRuntimeChoice::RustNativeRuntime.adds_go_dependency());
    assert!(WasmRuntimeChoice::RustNativeRuntime.permitted_by_language_boundary());
}

/// Negative: no recorded claim is marked as discharged, because none is.
#[test]
fn no_recorded_claim_is_discharged() {
    for claim in P10_RECORDED_CLAIMS {
        let row: RecordedClaim = claim;
        assert!(!row.settled_by.is_empty());
        assert!(row.requirement.starts_with("REQ-"));
        assert!(matches!(
            row.status,
            ClaimStatus::UnmeasuredTarget
                | ClaimStatus::DeferredHardwareRequirement
                | ClaimStatus::UnresolvedSourceConflict
        ));
        let source: ClaimSource = row.source;
        assert_eq!(source.sha256_prefix.len(), 12);
        assert!(source.export.starts_with("export-"));
        assert!(!row.summary.is_empty());
    }
}

/// Negative: the two hardware-backed requirements stay deferred to M21, which
/// is what epic E06-3 asks this milestone to record rather than deliver.
#[test]
fn venus_and_vsock_pricing_stay_deferred() -> Fallible {
    for requirement in ["REQ-P10-02", "REQ-P10-03"] {
        let claim = P10_RECORDED_CLAIMS
            .into_iter()
            .find(|row| row.requirement == requirement)
            .ok_or("the register must carry the deferred requirement")?;
        assert_eq!(claim.status, ClaimStatus::DeferredHardwareRequirement);
        assert_eq!(claim.status.name(), "deferred-hardware-requirement");
        assert!(
            claim.settled_by.contains("M21"),
            "{requirement} is settled by the hardware-backed milestone"
        );
    }
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the register's arity is exactly what the decision has, so an
/// option cannot be quietly dropped.
#[test]
fn the_register_arity_is_exact() {
    assert_eq!(WasmRuntimeChoice::ALL.len(), 3);
    assert_eq!(D06_WASM_RUNTIME.rejected.len(), 2);
    let mut named: Vec<&str> = WasmRuntimeChoice::ALL
        .iter()
        .map(|row| row.name())
        .collect();
    named.sort_unstable();
    named.dedup();
    assert_eq!(named.len(), 3, "every option has a distinct recorded name");
    assert_eq!(P10_RECORDED_CLAIMS.len(), 5);
    let mut requirements: Vec<&str> = P10_RECORDED_CLAIMS
        .iter()
        .map(|row| row.requirement)
        .collect();
    requirements.sort_unstable();
    requirements.dedup();
    assert_eq!(requirements.len(), 5, "no requirement is recorded twice");
}

/// Boundary: exactly one monitor passes a device through, which is why the
/// accelerator refusal exists at all.
#[test]
fn exactly_one_monitor_passes_a_device_through() {
    let passthrough: Vec<VmmIdentity> = VmmIdentity::BOTH
        .into_iter()
        .filter(|vmm| vmm.supports_pci_passthrough())
        .collect();
    assert_eq!(passthrough, vec![VmmIdentity::QemuMicrovm]);
    assert!(!VmmIdentity::CHOSEN.supports_pci_passthrough());
}
