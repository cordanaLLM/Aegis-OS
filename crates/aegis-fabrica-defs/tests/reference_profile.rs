// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The kernel requirement schema against the measured reference profile.
//!
//! `planning/hardware-profile.json` is a record of one machine, read back with
//! the commands each row names. These tests check the schema against that
//! record rather than against an invented one, which is what makes the
//! negative case here falsifiable: the reference kernel reports
//! `# CONFIG_PREEMPT_RT is not set` with `CONFIG_PREEMPT_DYNAMIC=y`, so a
//! requirement demanding a realtime kernel must be refused by it.
//!
//! Scope: a pass is development evidence on one workstation. It qualifies no
//! hardware and closes no image, boot, hardware or release gate.

mod common;

use aegis_fabrica_defs::field::ConfigSymbol;
use aegis_fabrica_defs::kernel::{
    Architecture, ConfigState, MAX_PROFILE_BYTES, MAX_UNMET, ProbeSource, RequiredState,
};
use aegis_fabrica_defs::{KernelError, KernelRequirement, ReferenceProfile, Unmet};

use common::{Fallible, reference_profile_text, reviewed_payload};

/// The measured reference profile, as this repository records it.
fn profile() -> Result<ReferenceProfile, Box<dyn std::error::Error>> {
    Ok(ReferenceProfile::from_profile_json(
        &reference_profile_text()?,
    )?)
}

/// One reviewed requirement payload, decoded.
fn payload(name: &str) -> Result<KernelRequirement, Box<dyn std::error::Error>> {
    Ok(KernelRequirement::decode(&reviewed_payload(name)?)?)
}

/// Builds a one-row requirement from the reviewed reference payload.
fn only(
    symbol: &str,
    state: RequiredState,
) -> Result<KernelRequirement, Box<dyn std::error::Error>> {
    let mut requirement = payload("kernel-requirement.reference.json")?;
    let wanted = ConfigSymbol::try_from(symbol.to_owned())?;
    let mut kept: Vec<_> = requirement
        .features
        .iter()
        .filter(|feature| feature.symbol == wanted)
        .cloned()
        .collect();
    for feature in &mut kept {
        feature.state = state;
    }
    if kept.is_empty() {
        return Err(format!("the reference payload carries no {symbol} row").into());
    }
    requirement.features = kept;
    requirement.abi.module_abi = None;
    requirement.validate()?;
    Ok(requirement)
}

// --- Positive -------------------------------------------------------------

/// Positive: the reviewed reference requirement is satisfied by the profile.
///
/// This is exit criterion 6. Every row of `build/kernel-requirement.reference`
/// is met by the measured profile: architecture, kernel release, module ABI,
/// every Kconfig symbol state and every runtime capability.
#[test]
fn the_reference_requirement_is_satisfied_by_the_measured_profile() -> Fallible {
    let profile = profile()?;
    let requirement = payload("kernel-requirement.reference.json")?;
    assert_eq!(profile.architecture(), Architecture::X86_64);
    assert_eq!(profile.release().as_str(), "7.2.4-1-cachyos");
    let unmet = requirement.unmet(&profile);
    assert_eq!(unmet, Vec::new(), "{unmet:?}");
    Ok(())
}

/// Positive: each asserted feature is traceable to the command that observed it.
///
/// The schema's probe path and the profile's own recorded evidence command
/// must name the same interface. A row whose probe drifted away from the
/// command the profile was measured with fails here rather than passing on a
/// plausible-looking string.
#[test]
fn each_asserted_feature_is_traceable_to_its_probe_command() -> Fallible {
    let profile = profile()?;
    let requirement = payload("kernel-requirement.reference.json")?;
    let mut runtime_rows = 0_usize;
    for feature in &requirement.features {
        let command = feature.probe_command();
        assert!(command.contains(feature.probe.path()), "{command}");
        let Some(capability) = feature.probe.capability() else {
            assert_eq!(feature.probe, ProbeSource::KernelConfig);
            assert!(command.starts_with("zgrep "), "{command}");
            continue;
        };
        runtime_rows = runtime_rows.saturating_add(1);
        assert_eq!(
            profile.capability(capability),
            Some(true),
            "profile capability {capability} is not present"
        );
        let evidence = profile
            .capability_evidence(capability)
            .ok_or("the profile records no evidence command for a cited capability")?;
        assert!(
            evidence.contains(feature.probe.path()),
            "profile evidence {evidence:?} does not name {}",
            feature.probe.path()
        );
    }
    assert!(
        runtime_rows >= 4,
        "only {runtime_rows} runtime probes cited"
    );
    Ok(())
}

/// Positive: the profile reports the states the reviewed payloads rely on.
///
/// Read back on 2026-09-13 with the profile's own `kernel.evidence_command`,
/// `zgrep -E '^(# )?(CONFIG_...)' /proc/config.gz`; the values below are what
/// that command printed, transcribed into `planning/hardware-profile.json`.
/// The `(# )?` alternation is the part that matters for the first row: a
/// symbol that is off is spelled `# CONFIG_X is not set`, so a pattern
/// anchored on `CONFIG_` alone prints nothing for it and a reader cannot tell
/// "not set" from "symbol absent".
#[test]
fn the_profile_reports_the_measured_symbol_states() -> Fallible {
    let profile = profile()?;
    let expected = [
        ("CONFIG_PREEMPT_RT", ConfigState::NotSet),
        ("CONFIG_PREEMPT_DYNAMIC", ConfigState::BuiltIn),
        ("CONFIG_HZ_1000", ConfigState::BuiltIn),
        ("CONFIG_SCHED_CLASS_EXT", ConfigState::BuiltIn),
        ("CONFIG_BPF_LSM", ConfigState::BuiltIn),
        ("CONFIG_DEBUG_INFO_BTF", ConfigState::BuiltIn),
        ("CONFIG_POWERCAP", ConfigState::BuiltIn),
        ("CONFIG_INTEL_RAPL", ConfigState::Module),
        ("CONFIG_IOMMU_API", ConfigState::BuiltIn),
        ("CONFIG_VFIO", ConfigState::Module),
        ("CONFIG_KVM", ConfigState::Module),
    ];
    for (name, state) in expected {
        let symbol = ConfigSymbol::try_from(name.to_owned())?;
        assert_eq!(profile.state_of(&symbol), Some(state), "{name}");
    }
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: the product requirement is refused by this workstation.
///
/// This is exit criterion 7, and it is the case the reference profile makes
/// genuinely falsifiable. `zgrep CONFIG_PREEMPT_RT /proc/config.gz` on the
/// reference host prints `# CONFIG_PREEMPT_RT is not set`, and
/// `zgrep CONFIG_PREEMPT_DYNAMIC /proc/config.gz` prints
/// `CONFIG_PREEMPT_DYNAMIC=y`. The workstation kernel is therefore not a
/// kernel the product requirement admits, and the schema says so with the one
/// symbol that is wrong rather than refusing the whole payload.
#[test]
fn the_product_requirement_is_refused_by_the_non_realtime_reference_kernel() -> Fallible {
    let profile = profile()?;
    let requirement = payload("kernel-requirement.json")?;
    let unmet = requirement.unmet(&profile);
    let preempt_rt = ConfigSymbol::try_from("CONFIG_PREEMPT_RT".to_owned())?;
    assert_eq!(
        unmet,
        vec![Unmet::StateMismatch {
            symbol: preempt_rt,
            required: RequiredState::BuiltIn,
            observed: ConfigState::NotSet,
        }],
        "the only unmet row must be the realtime one: {unmet:?}"
    );
    Ok(())
}

/// Negative: a symbol the profile records nothing about fails closed.
#[test]
fn a_symbol_the_profile_does_not_record_is_unobserved_rather_than_assumed() -> Fallible {
    let profile = profile()?;
    let mut requirement = payload("kernel-requirement.reference.json")?;
    let unrecorded = ConfigSymbol::try_from("CONFIG_NOT_IN_THE_PROFILE".to_owned())?;
    let mut row = requirement
        .features
        .first()
        .ok_or("the reference payload lists no feature")?
        .clone();
    row.symbol = unrecorded.clone();
    row.state = RequiredState::Absent;
    requirement.features = vec![row];
    requirement.abi.module_abi = None;
    let unmet = requirement.unmet(&profile);
    assert_eq!(
        unmet,
        vec![Unmet::Unobserved {
            symbol: unrecorded,
            probe: ProbeSource::KernelConfig,
        }],
        "{unmet:?}"
    );
    Ok(())
}

/// Negative: a requirement for another architecture or a newer kernel is refused.
#[test]
fn another_architecture_or_a_newer_minimum_is_refused() -> Fallible {
    let profile = profile()?;
    let mut requirement = only("CONFIG_HZ_1000", RequiredState::BuiltIn)?;
    requirement.architectures = vec![Architecture::Arm64];
    requirement.abi.minimum_release = "9.0".to_owned().try_into()?;
    let unmet = requirement.unmet(&profile);
    assert!(
        unmet.contains(&Unmet::ArchitectureNotAccepted {
            observed: Architecture::X86_64
        }),
        "{unmet:?}"
    );
    assert!(
        unmet
            .iter()
            .any(|row| matches!(row, Unmet::ReleaseBelowMinimum { .. })),
        "{unmet:?}"
    );
    assert!(unmet.len() <= MAX_UNMET);
    Ok(())
}

/// Negative: a module ABI other than the profile's is refused.
#[test]
fn a_module_abi_other_than_the_profiles_is_refused() -> Fallible {
    let profile = profile()?;
    let mut requirement = only("CONFIG_HZ_1000", RequiredState::BuiltIn)?;
    requirement.abi.module_abi = Some("7.3.0-aegis1".to_owned().try_into()?);
    let unmet = requirement.unmet(&profile);
    assert!(
        unmet
            .iter()
            .any(|row| matches!(row, Unmet::AbiMismatch { .. })),
        "{unmet:?}"
    );
    Ok(())
}

/// Negative: a profile document that is not the recorded shape is refused.
#[test]
fn a_profile_that_is_not_the_recorded_shape_is_refused() {
    let refused = ReferenceProfile::from_profile_json("{}");
    assert!(
        matches!(refused, Err(KernelError::UnreadableProfile { .. })),
        "{refused:?}"
    );
    let unreadable_state = ReferenceProfile::from_profile_json(
        "{\"cpu\": {\"architecture\": \"x86-64\"}, \"capabilities\": {}, \
         \"kernel\": {\"release\": \"7.2\", \"config\": {\"CONFIG_HZ\": \"1000\"}}}",
    );
    assert!(
        matches!(unreadable_state, Err(KernelError::UnreadableProfile { .. })),
        "a numeric Kconfig value must be refused, got {unreadable_state:?}"
    );
    let oversized = ReferenceProfile::from_profile_json(&"x".repeat(MAX_PROFILE_BYTES + 1));
    assert!(
        matches!(oversized, Err(KernelError::UnreadableProfile { .. })),
        "{oversized:?}"
    );
}

/// Negative: a runtime probe whose capability is absent is reported as such.
///
/// The reference profile records every capability the reviewed payloads read
/// as present, so this case uses a synthetic profile document -- the same
/// shape, with `btf` recorded absent. That keeps the two halves of a runtime
/// claim distinguishable: the symbol can be compiled in while the interface
/// the probe reads is unavailable.
#[test]
fn a_runtime_probe_with_an_absent_capability_is_reported() -> Fallible {
    let synthetic = "{\"cpu\": {\"architecture\": \"x86-64\"}, \
         \"kernel\": {\"release\": \"7.2.4-1-cachyos\", \
         \"config\": {\"CONFIG_DEBUG_INFO_BTF\": \"y\"}}, \
         \"capabilities\": {\"btf\": {\"present\": false, \
         \"evidence_command\": \"ls /sys/kernel/btf/vmlinux\"}}}";
    let profile = ReferenceProfile::from_profile_json(synthetic)?;
    assert_eq!(profile.capability("btf"), Some(false));
    let mut requirement = payload("kernel-requirement.reference.json")?;
    let btf = ConfigSymbol::try_from("CONFIG_DEBUG_INFO_BTF".to_owned())?;
    requirement.features.retain(|feature| feature.symbol == btf);
    requirement.abi.module_abi = None;
    requirement.validate()?;
    let unmet = requirement.unmet(&profile);
    assert_eq!(
        unmet,
        vec![Unmet::CapabilityAbsent {
            probe: ProbeSource::BtfVmlinux,
            capability: "btf",
        }],
        "{unmet:?}"
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a requirement for `CONFIG_HZ_1000` alone is accepted.
///
/// This is exit criterion 8, and it is what proves the schema discriminates
/// per feature rather than per kernel flavour. The same profile that refuses
/// the product requirement -- because `# CONFIG_PREEMPT_RT is not set` --
/// satisfies a requirement for the timer frequency, because
/// `zgrep CONFIG_HZ_1000 /proc/config.gz` prints `CONFIG_HZ_1000=y` on the
/// same kernel.
#[test]
fn a_requirement_for_the_timer_frequency_alone_is_accepted() -> Fallible {
    let profile = profile()?;
    let timer = only("CONFIG_HZ_1000", RequiredState::BuiltIn)?;
    assert_eq!(timer.features.len(), 1);
    assert_eq!(timer.unmet(&profile), Vec::new());

    let realtime = only("CONFIG_HZ_1000", RequiredState::BuiltIn)?;
    let product = payload("kernel-requirement.json")?;
    assert!(
        !product.unmet(&profile).is_empty(),
        "the product requirement must still fail on the same profile"
    );
    assert_eq!(realtime.unmet(&profile), Vec::new());
    Ok(())
}

/// Boundary: the realtime row alone is the whole difference.
///
/// Taking the product requirement and relaxing only `CONFIG_PREEMPT_RT` to
/// `Absent` -- the state the profile actually records -- makes the same
/// payload pass. Nothing else about the workstation is in the way.
#[test]
fn relaxing_only_the_realtime_row_makes_the_product_requirement_pass() -> Fallible {
    let profile = profile()?;
    let mut requirement = payload("kernel-requirement.json")?;
    let preempt_rt = ConfigSymbol::try_from("CONFIG_PREEMPT_RT".to_owned())?;
    for feature in &mut requirement.features {
        if feature.symbol == preempt_rt {
            feature.state = RequiredState::Absent;
        }
    }
    requirement.validate()?;
    let unmet = requirement.unmet(&profile);
    assert_eq!(unmet, Vec::new(), "{unmet:?}");
    Ok(())
}

/// Boundary: a capability the profile records as absent is reported.
///
/// The reference profile records `realtime_kernel` as absent. A probe that
/// read it would be refused for the capability rather than for the symbol, so
/// the two halves of a runtime claim stay distinguishable.
#[test]
fn an_absent_capability_is_reported_apart_from_the_symbol_state() -> Fallible {
    let profile = profile()?;
    assert_eq!(profile.capability("realtime_kernel"), Some(false));
    assert_eq!(profile.capability("no_such_capability"), None);
    assert_eq!(profile.capability_evidence("no_such_capability"), None);
    let evidence = profile
        .capability_evidence("bpf_lsm")
        .ok_or("the profile records no evidence command for bpf_lsm")?;
    assert!(evidence.contains(ProbeSource::LsmList.path()), "{evidence}");
    Ok(())
}
