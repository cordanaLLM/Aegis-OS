// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The two recorded decisions, the recorded claims, and the three credential
//! probes.
//!
//! The claim this file exists to hold is the one that is easiest to lose:
//! **the reference profile has a TPM2 and no FIDO2 authenticator**, so the two
//! halves of P11's credential story have different statuses and neither is a
//! stub that could be read as coverage.

mod common;

use aegis_ludus::{
    Citation, ClaimSource, ClaimStatus, CredentialProbe, D12_STEAMWORKS_EXCLUSION,
    D48_PRESENCE_INTEGRATION, DecisionState, FIDO2_AUTHENTICATOR, P11_RECORDED_CLAIMS,
    PresenceDecision, PresenceDisposition, REFERENCE_PROFILE_PROBES, RecordedClaim, SdkDecision,
    SteamworksDisposition, TPM2_DEVICE, TPM2_MAJOR_VERSION, recorded_probe,
};

use common::Fallible;

// --- Positive -------------------------------------------------------------

/// Positive: D12 is recorded as settled on the exclusion.
#[test]
fn d12_is_recorded_as_settled() {
    let decision: SdkDecision = D12_STEAMWORKS_EXCLUSION;
    assert_eq!(decision.id, "D12");
    assert_eq!(decision.state, DecisionState::Closed);
    assert_eq!(decision.state.name(), "closed");
    assert_eq!(decision.chosen, SteamworksDisposition::ExcludedFromImage);
    assert_eq!(decision.chosen.name(), "excluded-from-image");
    assert!(!decision.chosen.admits_proprietary_sdk());
    assert_eq!(decision.applied_at, "M08");
    assert_eq!(decision.dispute, "DSP-26");
}

/// Positive: the rejected options stay representable, and the record says what
/// it can and cannot check.
#[test]
fn d12_keeps_its_rejected_options_and_states_its_scope() {
    let decision: SdkDecision = D12_STEAMWORKS_EXCLUSION;
    assert_eq!(
        decision.rejected,
        [
            SteamworksDisposition::OptionalFeatureBehindBuildFlag,
            SteamworksDisposition::ProceedAsDrafted,
        ]
    );
    assert!(
        decision
            .rejected
            .iter()
            .all(|option| option.admits_proprietary_sdk())
    );
    assert_eq!(SteamworksDisposition::ALL.len(), 3);
    assert_eq!(decision.touches, ["REQ-P11-01", "REQ-P11-05"]);
    assert_eq!(
        decision.adr,
        "docs/adr/0002-exclude-steamworks-from-image.md"
    );
    assert!(decision.question.contains("open-source"));
    assert!(decision.scope.contains("the image is not"));
}

/// Positive: the sources D12 rests on are cited by export and digest prefix.
#[test]
fn d12_cites_both_sources() {
    let sources: [Citation; 2] = D12_STEAMWORKS_EXCLUSION.sources;
    assert_eq!(sources.len(), 2);
    let exports: Vec<&str> = sources.iter().map(|row| row.export).collect();
    assert_eq!(exports, vec!["export-019", "export-004"]);
    for source in sources {
        assert_eq!(source.sha256_prefix.len(), 12);
        assert!(source.sha256_prefix.chars().all(|c| c.is_ascii_hexdigit()));
    }
}

/// Positive: the three probes are recorded with their commands and readings.
#[test]
fn the_three_probes_are_recorded() -> Fallible {
    assert_eq!(REFERENCE_PROFILE_PROBES.len(), 3);
    for probe in REFERENCE_PROFILE_PROBES {
        assert_eq!(probe.probed_on, "2026-09-13");
        assert!(!probe.command.is_empty());
        assert!(!probe.observed.is_empty());
    }
    let Some(device) = recorded_probe(TPM2_DEVICE) else {
        return Err("the TPM2 device probe must be recorded".into());
    };
    assert_eq!(device.command, "ls /sys/class/tpm/");
    assert_eq!(device.observed, "tpm0");
    assert!(device.present);
    assert!(!device.is_procurement_dependency());
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: the reference profile has no FIDO2 authenticator, and the register
/// says so rather than stubbing it.
///
/// This is exit criterion 5. The probe is `present: false`, its recorded claim
/// is a [`ClaimStatus::ProcurementDependency`] rather than a deferred
/// requirement, and nothing in the crate's public surface offers a FIDO2 type
/// that could be mistaken for one.
#[test]
fn the_profile_has_no_fido2_authenticator() -> Fallible {
    let Some(probe) = recorded_probe(FIDO2_AUTHENTICATOR) else {
        return Err("the FIDO2 probe must be recorded".into());
    };
    assert!(!probe.present);
    assert!(probe.is_procurement_dependency());
    assert_eq!(
        probe.command,
        "lsusb | grep -iE 'yubi|fido|solo|token|nitro'"
    );
    assert!(probe.observed.contains("no output"));
    assert!(probe.observed.contains("exited 1"));

    let Some(claim) = claim_for("P11-HW-FIDO2") else {
        return Err("the FIDO2 claim must be recorded".into());
    };
    assert_eq!(claim.status, ClaimStatus::ProcurementDependency);
    assert_eq!(claim.status.name(), "procurement-dependency");
    assert!(claim.settled_by.contains("buying one"));
    Ok(())
}

/// Negative: D48 is recorded and is **not** settled, so nothing was typed for
/// the presence edge at this milestone.
#[test]
fn d48_is_recorded_and_unresolved() {
    let decision: PresenceDecision = D48_PRESENCE_INTEGRATION;
    assert_eq!(decision.id, "D48");
    assert_eq!(decision.state, DecisionState::Unresolved);
    assert_eq!(decision.state.name(), "unresolved");
    assert!(!decision.is_settled());
    assert_eq!(decision.recorded_at, "M08");
    assert_eq!(decision.edge, "DISPATCH_RICH_PRESENCE");
    assert!(decision.carried.starts_with("nothing"));
    assert!(decision.settled_by.contains("named library"));
}

/// Negative: both readings of D48 stay representable, so the register states a
/// choice rather than the only option it can spell.
#[test]
fn d48_keeps_both_readings_representable() {
    let decision: PresenceDecision = D48_PRESENCE_INTEGRATION;
    assert_eq!(decision.readings, PresenceDisposition::BOTH);
    assert_eq!(
        decision.readings,
        [
            PresenceDisposition::ExcludedAsProprietaryAndNetworkDependent,
            PresenceDisposition::KeptAsLocalIpcToUserInstalledClient,
        ]
    );
    assert_eq!(
        decision.readings.map(PresenceDisposition::name),
        ["excluded-as-proprietary", "kept-as-local-ipc"]
    );
}

/// Negative: the recorded claims are recorded and none is promoted.
///
/// The sweep is over every row, so a later edit that quietly promotes one fails
/// here rather than passing because the test named the other four.
#[test]
fn no_recorded_claim_is_discharged() {
    assert_eq!(P11_RECORDED_CLAIMS.len(), 5);
    for claim in P11_RECORDED_CLAIMS {
        assert!(matches!(
            claim.status,
            ClaimStatus::DeferredHardwareRequirement
                | ClaimStatus::ProcurementDependency
                | ClaimStatus::SupersededByDecision
        ));
    }
    let deferred = P11_RECORDED_CLAIMS
        .iter()
        .filter(|claim| claim.status == ClaimStatus::DeferredHardwareRequirement)
        .count();
    assert_eq!(deferred, 3);
    assert_eq!(
        ClaimStatus::SupersededByDecision.name(),
        "superseded-by-decision"
    );
    assert_eq!(
        ClaimStatus::DeferredHardwareRequirement.name(),
        "deferred-hardware-requirement"
    );
}

/// Negative: every recorded claim carries a summary, a settlement and a source
/// cited by export identifier and digest prefix.
#[test]
fn every_recorded_claim_carries_its_source() {
    for claim in P11_RECORDED_CLAIMS {
        assert!(!claim.summary.is_empty());
        assert!(!claim.settled_by.is_empty());
        let source: ClaimSource = claim.source;
        assert_eq!(source.sha256_prefix.len(), 12);
        assert!(source.export.starts_with("export-"));
        assert!(source.sha256_prefix.chars().all(|c| c.is_ascii_hexdigit()));
    }
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the TPM2 half is present and unbound, which is a different status
/// from the FIDO2 half being absent.
///
/// Both probes and both statuses are asserted together, because the thing that
/// would be lost is the difference between them and not either one alone.
#[test]
fn the_two_credentials_are_recorded_apart() -> Fallible {
    let Some(version) = recorded_probe(TPM2_MAJOR_VERSION) else {
        return Err("the TPM2 version probe must be recorded".into());
    };
    assert_eq!(version.command, "cat /sys/class/tpm/tpm0/tpm_version_major");
    assert_eq!(version.observed, "2");
    assert!(version.present);

    let present: Vec<&str> = REFERENCE_PROFILE_PROBES
        .iter()
        .filter(|probe| probe.present)
        .map(|probe| probe.capability)
        .collect();
    let absent: Vec<&str> = REFERENCE_PROFILE_PROBES
        .iter()
        .filter(|probe| !probe.present)
        .map(|probe| probe.capability)
        .collect();
    assert_eq!(present, vec![TPM2_DEVICE, TPM2_MAJOR_VERSION]);
    assert_eq!(absent, vec![FIDO2_AUTHENTICATOR]);

    let Some(sealing) = claim_for("REQ-P11-04") else {
        return Err("the TPM2 sealing claim must be recorded".into());
    };
    assert_eq!(sealing.status, ClaimStatus::DeferredHardwareRequirement);
    assert_ne!(sealing.status, ClaimStatus::ProcurementDependency);
    Ok(())
}

/// Boundary: an unrecorded capability has no probe, so the register cannot be
/// read as covering something it never looked for.
#[test]
fn an_unrecorded_capability_has_no_probe() {
    assert!(recorded_probe("secure-enclave").is_none());
    assert!(recorded_probe("").is_none());
    assert!(recorded_probe("tpm2").is_none());
    let probe: Option<CredentialProbe> = recorded_probe(TPM2_DEVICE);
    assert!(probe.is_some());
}

/// Returns the recorded claim for `requirement`, if the register holds one.
fn claim_for(requirement: &str) -> Option<RecordedClaim> {
    P11_RECORDED_CLAIMS
        .into_iter()
        .take(P11_RECORDED_CLAIMS.len())
        .find(|claim| claim.requirement == requirement)
}
