// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The kernel requirement schema (epic E18-2), as a payload.
//!
//! Everything here is about the shape of the payload and the fragment it
//! renders. What the measured reference profile does and does not satisfy is
//! the subject of `reference_profile.rs`.

mod common;

use aegis_fabrica_defs::field::{ConfigSymbol, KernelRelease, RequirementId};
use aegis_fabrica_defs::kernel::{
    Architecture, ArtifactExpectation, ConfigState, FeatureRequirement, KERNEL_REQUIREMENT_TAG,
    KernelAbi, KernelRequirementVersion, MAX_ARCHITECTURES, MAX_FEATURES, ProbeSource,
    RequiredState,
};
use aegis_fabrica_defs::payload::MAX_PAYLOAD_BYTES;
use aegis_fabrica_defs::{KernelError, KernelRequirement};

use common::{Fallible, reviewed_payload};

/// The reviewed product kernel requirement, as this repository ships it.
fn shipped() -> Result<KernelRequirement, Box<dyn std::error::Error>> {
    Ok(KernelRequirement::decode(&reviewed_payload(
        "kernel-requirement.json",
    )?)?)
}

/// A requirement payload with one field replaced, for the refusal cases.
fn altered(field: &str, value: &str) -> Result<String, Box<dyn std::error::Error>> {
    let text = reviewed_payload("kernel-requirement.json")?;
    let mut document: serde_json::Value = serde_json::from_str(&text)?;
    let replacement: serde_json::Value = serde_json::from_str(value)?;
    document
        .as_object_mut()
        .ok_or("the reviewed requirement is not a JSON object")?
        .insert(field.to_owned(), replacement);
    Ok(serde_json::to_string(&document)?)
}

/// One feature row, built from parts, for the cases that need a small payload.
fn row(
    symbol: &str,
    state: RequiredState,
    probe: ProbeSource,
) -> Result<FeatureRequirement, Box<dyn std::error::Error>> {
    Ok(FeatureRequirement {
        symbol: ConfigSymbol::try_from(symbol.to_owned())?,
        state,
        probe,
        required_by: RequirementId::try_from("REQ-P07-01".to_owned())?,
    })
}

/// The number of fragment lines that are assignments rather than comments.
fn assignment_lines(fragment: &str) -> usize {
    fragment
        .lines()
        .filter(|line| !line.starts_with('#'))
        .count()
}

/// The command each probe kind is expected to render, written out by hand.
///
/// Written out rather than derived, so the assertion is against the commands a
/// reviewer actually runs and not against the crate's own rendering.
fn expected_command(feature: &FeatureRequirement) -> String {
    match feature.probe {
        ProbeSource::KernelConfig => format!("zgrep {} /proc/config.gz", feature.symbol),
        ProbeSource::LsmList => "cat /sys/kernel/security/lsm".to_owned(),
        ProbeSource::BtfVmlinux => "ls /sys/kernel/btf/vmlinux".to_owned(),
        ProbeSource::Powercap => "ls /sys/class/powercap".to_owned(),
        ProbeSource::IommuGroups => "ls /sys/kernel/iommu_groups".to_owned(),
    }
}

// --- Positive -------------------------------------------------------------

/// Positive: the reviewed requirement lists what P06, P07 and P13 need.
///
/// The assertion is deliberately about the payload's rows rather than about
/// named constants in this crate: the schema carries no symbol list of its
/// own, so the only place `CONFIG_BPF_LSM` appears is the reviewed JSON.
#[test]
fn the_reviewed_requirement_lists_the_features_p06_p07_and_p13_need() -> Fallible {
    let requirement = shipped()?;
    assert_eq!(requirement.schema, KernelRequirementVersion::V1);
    assert_eq!(KernelRequirement::SCHEMA_TAG, KERNEL_REQUIREMENT_TAG);
    assert_eq!(requirement.architectures, vec![Architecture::X86_64]);
    assert_eq!(requirement.abi.minimum_release.as_str(), "6.12");
    assert_eq!(
        requirement
            .abi
            .target_release
            .as_ref()
            .map(KernelRelease::as_str),
        Some("7.3")
    );
    let symbols: Vec<&str> = requirement
        .features
        .iter()
        .map(|feature| feature.symbol.as_str())
        .collect();
    for expected in [
        "CONFIG_PREEMPT_RT",
        "CONFIG_HZ_1000",
        "CONFIG_SCHED_CLASS_EXT",
        "CONFIG_BPF_LSM",
        "CONFIG_DEBUG_INFO_BTF",
        "CONFIG_POWERCAP",
        "CONFIG_INTEL_RAPL",
        "CONFIG_IOMMU_API",
        "CONFIG_VFIO",
        "CONFIG_KVM",
    ] {
        assert!(symbols.contains(&expected), "{expected} is not required");
    }
    Ok(())
}

/// Positive: every feature row carries the command that checks it.
///
/// A kernel-configuration row names its own symbol in the command; a runtime
/// row names the interface it reads. Both are the exact commands recorded in
/// `docs/build/product-input.md`.
#[test]
fn every_feature_row_renders_its_own_probe_command() -> Fallible {
    let requirement = shipped()?;
    for feature in &requirement.features {
        let command = feature.probe_command();
        assert!(
            command.contains(feature.probe.path()),
            "{command} does not name {}",
            feature.probe.path()
        );
        assert_eq!(command, expected_command(feature));
    }
    Ok(())
}

/// Positive: the payload renders as a Kconfig fragment a kernel build applies.
///
/// This is the form milestone M26 consumes: one assignment per required
/// feature, each preceded by the recorded requirement it comes from, and no
/// line that is not a comment or an assignment.
#[test]
fn the_payload_renders_as_a_kconfig_fragment() -> Fallible {
    let requirement = shipped()?;
    let fragment = requirement.config_fragment()?;
    assert!(fragment.contains("CONFIG_PREEMPT_RT=y"));
    assert!(fragment.contains("CONFIG_HZ_1000=y"));
    assert!(fragment.contains("CONFIG_SCHED_CLASS_EXT=y"));
    assert!(fragment.contains("CONFIG_BPF_LSM=y"));
    assert!(fragment.contains("CONFIG_DEBUG_INFO_BTF=y"));
    assert!(fragment.contains("CONFIG_VFIO=m"));
    assert!(fragment.contains(KERNEL_REQUIREMENT_TAG));
    assert_eq!(assignment_lines(&fragment), requirement.features.len());
    Ok(())
}

/// Positive: a fragment carries nothing but comments and assignments.
///
/// A line that was neither would make the fragment unusable as it stands,
/// which is the whole claim milestone M26 relies on.
#[test]
fn a_fragment_carries_nothing_but_comments_and_assignments() -> Fallible {
    let fragment = shipped()?.config_fragment()?;
    for line in fragment.lines() {
        let shaped = line.starts_with('#') || line.contains('=');
        assert!(
            shaped,
            "fragment line {line:?} is neither comment nor assignment"
        );
    }
    Ok(())
}

/// Positive: a disabled feature renders as the Kconfig "not set" comment.
#[test]
fn a_disabled_feature_renders_as_the_kconfig_not_set_comment() -> Fallible {
    let mut requirement = shipped()?;
    requirement.features = vec![row(
        "CONFIG_PREEMPT_RT",
        RequiredState::Absent,
        ProbeSource::KernelConfig,
    )?];
    let fragment = requirement.config_fragment()?;
    assert!(fragment.contains("# CONFIG_PREEMPT_RT is not set"));
    assert!(!fragment.contains("CONFIG_PREEMPT_RT=y"));
    Ok(())
}

/// Positive: a validated requirement re-encodes and decodes back to itself,
/// carrying an artifact digest and signature when it has them.
#[test]
fn a_requirement_with_an_artifact_expectation_round_trips() -> Fallible {
    let mut requirement = shipped()?;
    requirement.artifact = Some(ArtifactExpectation {
        digest: "a".repeat(64).try_into()?,
        signature: Some("beef".to_owned().try_into()?),
    });
    let encoded = requirement.encode()?;
    assert_eq!(KernelRequirement::decode(&encoded)?, requirement);
    assert!(encoded.contains("\"artifact\""));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: an architecture outside the accepted set does not decode.
#[test]
fn an_unknown_architecture_is_refused() -> Fallible {
    let refused = KernelRequirement::decode(&altered("architectures", "[\"riscv64\"]")?);
    assert!(
        matches!(refused, Err(KernelError::Malformed { .. })),
        "{refused:?}"
    );
    let misspelt = KernelRequirement::decode(&altered("architectures", "[\"x86_64\"]")?);
    assert!(
        matches!(misspelt, Err(KernelError::Malformed { .. })),
        "{misspelt:?}"
    );
    Ok(())
}

/// Negative: a payload naming another contract version is told so by name.
#[test]
fn a_payload_naming_another_contract_version_is_refused_as_an_unknown_version() -> Fallible {
    let other = altered("schema", "\"aegis.p01-nucleus.kernel-requirement.v2\"")?;
    let refused = KernelRequirement::decode(&other);
    let Err(KernelError::UnknownVersion { declared, expected }) = refused else {
        return Err(format!("expected an unknown-version refusal, got {refused:?}").into());
    };
    assert_eq!(declared, "aegis.p01-nucleus.kernel-requirement.v2");
    assert_eq!(expected, KERNEL_REQUIREMENT_TAG);
    Ok(())
}

/// Negative: an artifact expectation that promises a signature over nothing
/// is not representable, so a payload carrying one does not decode.
#[test]
fn a_signature_without_a_digest_is_refused() -> Fallible {
    let refused = KernelRequirement::decode(&altered("artifact", "{\"signature\": \"beef\"}")?);
    assert!(
        matches!(refused, Err(KernelError::Malformed { .. })),
        "{refused:?}"
    );
    Ok(())
}

/// Negative: the same symbol required twice is refused.
///
/// Two rows for one symbol would render two fragment lines, and the later one
/// would silently win in the built configuration.
#[test]
fn a_repeated_symbol_is_refused() -> Fallible {
    let mut requirement = shipped()?;
    let first = requirement
        .features
        .first()
        .ok_or("the reviewed requirement lists no feature")?
        .clone();
    requirement.features.push(first.clone());
    let refused = requirement.validate();
    let Err(KernelError::DuplicateSymbol { symbol }) = refused else {
        return Err(format!("expected a duplicate refusal, got {refused:?}").into());
    };
    assert_eq!(symbol, first.symbol);
    Ok(())
}

/// Negative: a target release older than the minimum is refused.
#[test]
fn a_target_release_older_than_the_minimum_is_refused() -> Fallible {
    let inverted = altered(
        "abi",
        "{\"minimum-release\": \"7.3\", \"target-release\": \"6.12\"}",
    )?;
    let refused = KernelRequirement::decode(&inverted);
    let Err(KernelError::InvertedRelease { minimum, target }) = refused else {
        return Err(format!("expected an inverted-release refusal, got {refused:?}").into());
    };
    assert_eq!(minimum.as_str(), "7.3");
    assert_eq!(target.as_str(), "6.12");
    Ok(())
}

/// Negative: an unknown field is refused rather than ignored.
#[test]
fn an_unknown_field_is_refused_rather_than_ignored() -> Fallible {
    let refused = KernelRequirement::decode(&altered("patches", "[]")?);
    assert!(
        matches!(refused, Err(KernelError::Malformed { .. })),
        "{refused:?}"
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: an empty requirement list is refused explicitly.
///
/// Not read as "no requirements": a consumer receiving an empty list cannot
/// tell an unwritten payload from a deliberate one, and would build whatever
/// it liked. The same applies to an empty architecture list.
#[test]
fn an_empty_requirement_list_is_refused_explicitly() -> Fallible {
    let refused = KernelRequirement::decode(&altered("features", "[]")?);
    assert!(
        matches!(refused, Err(KernelError::NoFeatures)),
        "{refused:?}"
    );
    let no_arch = KernelRequirement::decode(&altered("architectures", "[]")?);
    assert!(
        matches!(no_arch, Err(KernelError::NoArchitectures)),
        "{no_arch:?}"
    );
    Ok(())
}

/// Boundary: the feature and architecture bounds are exact.
#[test]
fn the_feature_and_architecture_bounds_are_exact() -> Fallible {
    let mut requirement = shipped()?;
    let mut features: Vec<FeatureRequirement> = Vec::new();
    for index in 0..MAX_FEATURES {
        features.push(row(
            &format!("CONFIG_BOUND_{index}"),
            RequiredState::Present,
            ProbeSource::KernelConfig,
        )?);
    }
    requirement.features = features;
    requirement.validate()?;
    requirement.features.push(row(
        "CONFIG_ONE_TOO_MANY",
        RequiredState::Present,
        ProbeSource::KernelConfig,
    )?);
    let over = requirement.validate();
    assert!(
        matches!(
            over,
            Err(KernelError::TooMany {
                what: "features",
                bound: MAX_FEATURES
            })
        ),
        "{over:?}"
    );

    let mut arches = shipped()?;
    arches.architectures = vec![Architecture::Arm64; MAX_ARCHITECTURES.saturating_add(1)];
    let too_many = arches.validate();
    assert!(
        matches!(
            too_many,
            Err(KernelError::TooMany {
                what: "architectures",
                ..
            })
        ),
        "{too_many:?}"
    );
    Ok(())
}

/// Boundary: a fragment is rendered whole at the bound and refused past it.
///
/// Every field of the payload is public, so a caller can assemble a
/// requirement without going through `decode`, which is the path that
/// validates. At [`MAX_FEATURES`] the fragment carries one assignment per row;
/// one row past it the renderer refuses instead of emitting a fragment that is
/// short by exactly the rows it dropped. Milestone M26 applies this fragment to
/// a base configuration, so a silently incomplete one is the failure that
/// matters.
#[test]
fn a_fragment_is_rendered_whole_at_the_bound_and_refused_past_it() -> Fallible {
    let mut requirement = shipped()?;
    let mut features: Vec<FeatureRequirement> = Vec::new();
    for index in 0..MAX_FEATURES {
        features.push(row(
            &format!("CONFIG_BOUND_{index}"),
            RequiredState::BuiltIn,
            ProbeSource::KernelConfig,
        )?);
    }
    requirement.features = features;
    let fragment = requirement.config_fragment()?;
    assert_eq!(assignment_lines(&fragment), MAX_FEATURES);

    requirement.features.push(row(
        "CONFIG_ONE_TOO_MANY",
        RequiredState::BuiltIn,
        ProbeSource::KernelConfig,
    )?);
    let refused = requirement.config_fragment();
    assert!(
        matches!(
            refused,
            Err(KernelError::TooMany {
                what: "features",
                bound: MAX_FEATURES
            })
        ),
        "{refused:?}"
    );
    Ok(())
}

/// Boundary: each required state is satisfied by exactly the states it names.
///
/// `Present` is the only one that admits two, and an unrecorded symbol
/// satisfies none of them -- including `Absent`, because an unrecorded symbol
/// is an unanswered question rather than a measured absence.
#[test]
fn each_required_state_is_satisfied_by_exactly_the_states_it_names() {
    let observed = [
        ConfigState::BuiltIn,
        ConfigState::Module,
        ConfigState::NotSet,
    ];
    let expected = [
        (RequiredState::BuiltIn, [true, false, false]),
        (RequiredState::Module, [false, true, false]),
        (RequiredState::Present, [true, true, false]),
        (RequiredState::Absent, [false, false, true]),
    ];
    for (required, admits) in expected {
        for (index, state) in observed.iter().enumerate() {
            let satisfied = required.satisfied_by(Some(*state));
            assert_eq!(
                satisfied,
                admits.get(index).copied().unwrap_or(false),
                "{required:?} against {}",
                state.tag()
            );
        }
        assert!(!required.satisfied_by(None), "{required:?} against nothing");
    }
    assert_eq!(ConfigState::parse("y"), Some(ConfigState::BuiltIn));
    assert_eq!(ConfigState::parse("m"), Some(ConfigState::Module));
    assert_eq!(ConfigState::parse("not set"), Some(ConfigState::NotSet));
    assert_eq!(ConfigState::parse("maybe"), None);
}

/// Boundary: each required state renders exactly one Kconfig fragment line.
///
/// `Present` is the one state with a choice to make: a fragment states one
/// assignment, so "either way" renders as the stronger of the two.
#[test]
fn each_required_state_renders_one_kconfig_line() -> Fallible {
    let symbol = ConfigSymbol::try_from("CONFIG_VFIO".to_owned())?;
    assert_eq!(
        RequiredState::BuiltIn.fragment_line(&symbol),
        "CONFIG_VFIO=y"
    );
    assert_eq!(
        RequiredState::Module.fragment_line(&symbol),
        "CONFIG_VFIO=m"
    );
    assert_eq!(
        RequiredState::Present.fragment_line(&symbol),
        "CONFIG_VFIO=y"
    );
    assert_eq!(
        RequiredState::Absent.fragment_line(&symbol),
        "# CONFIG_VFIO is not set"
    );
    Ok(())
}

/// Boundary: a payload one byte past the scalar bound is refused unparsed.
#[test]
fn a_payload_past_the_byte_bound_is_refused_before_it_is_parsed() {
    let oversized = "x".repeat(MAX_PAYLOAD_BYTES.saturating_add(1));
    let refused = KernelRequirement::decode(&oversized);
    assert!(
        matches!(
            refused,
            Err(KernelError::TooLong {
                bound: MAX_PAYLOAD_BYTES,
                ..
            })
        ),
        "{refused:?}"
    );
}

/// Boundary: an ABI block with neither optional half still decodes.
///
/// The minimum release is the only mandatory bound; a requirement written
/// before any artifact exists has no target and no module ABI to demand.
#[test]
fn an_abi_with_only_a_minimum_release_decodes() -> Fallible {
    let text = altered("abi", "{\"minimum-release\": \"6.12\"}")?;
    let requirement = KernelRequirement::decode(&text)?;
    assert_eq!(
        requirement.abi,
        KernelAbi {
            minimum_release: KernelRelease::try_from("6.12".to_owned())?,
            target_release: None,
            module_abi: None,
        }
    );
    Ok(())
}
