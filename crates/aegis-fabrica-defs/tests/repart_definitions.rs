// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Triples for the repart drop-ins, including the cases systemd 261 was
//! observed on: a missing `Type=`, an inverted size pair, an equal size pair
//! and the `Subsystem=` key systemd ignores while exiting 0.

mod common;

use aegis_fabrica_defs::{
    DefinitionError, ESP_MAX_BYTES, ESP_MIN_BYTES, ESP_TYPE, Finding, KNOWN_KEYS, MAX_DEFINITIONS,
    MAX_FINDINGS, PARTITION_SECTION, ROOT_TYPE, RepartDefinition, VAR_TYPE, check_repart_set,
};

use common::Fallible;

const ESP: &str = "[Partition]\nType=esp\nFormat=vfat\nLabel=ESP\n\
SizeMinBytes=512M\nSizeMaxBytes=1G\nCopyFiles=/boot:/\nPriority=100\n";
const ROOT_A: &str = "[Partition]\nType=root\nFormat=erofs\nLabel=root-a\n\
Verity=data\nVerityMatchKey=root\nMinimize=yes\nPriority=90\nWeight=1000\n";
const ROOT_B: &str = "[Partition]\nType=root\nLabel=root-b\nPriority=90\nWeight=1000\n";
const VERITY: &str = "[Partition]\nType=root-verity\nVerity=hash\n\
VerityMatchKey=root\nMinimize=yes\nPriority=90\n";
const VAR: &str = "[Partition]\nType=var\nFormat=btrfs\nLabel=var\n\
Encrypt=tpm2\nSizeMinBytes=10G\nWeight=2000\n";

/// Parses the five-drop-in set the milestone ships in `build/repart.d`.
fn reviewed_set() -> Result<Vec<RepartDefinition>, DefinitionError> {
    [
        ("00-esp.conf", ESP),
        ("10-root-a.conf", ROOT_A),
        ("10-root-b.conf", ROOT_B),
        ("11-root-verity.conf", VERITY),
        ("20-var.conf", VAR),
    ]
    .into_iter()
    .map(|(name, text)| RepartDefinition::parse(name, text))
    .collect()
}

// --- Positive -------------------------------------------------------------

/// Positive: the ESP drop-in yields every field the set checks read.
#[test]
fn the_esp_drop_in_is_read_field_by_field() -> Fallible {
    let esp = RepartDefinition::parse("00-esp.conf", ESP)?;
    assert_eq!(esp.partition_type(), ESP_TYPE);
    assert_eq!(esp.label(), Some("ESP"));
    assert_eq!(esp.size_min(), Some(ESP_MIN_BYTES));
    assert_eq!(esp.size_max(), Some(ESP_MAX_BYTES));
    assert_eq!(esp.verity(), None);
    assert_eq!(esp.verity_match_key(), None);
    assert!(!esp.is_tpm2_encrypted());
    assert_eq!(esp.unit().value(PARTITION_SECTION, "Format"), Some("vfat"));
    assert!(esp.findings().is_empty());
    Ok(())
}

/// Positive: the reviewed set meets every requirement the parser checks.
#[test]
fn the_reviewed_set_reports_no_findings() -> Fallible {
    let set = reviewed_set()?;
    assert_eq!(set.len(), 5);
    assert_eq!(check_repart_set(&set), Vec::new());
    let roots = set
        .iter()
        .filter(|row| row.partition_type() == ROOT_TYPE)
        .count();
    assert_eq!(roots, 2);
    let var = set.iter().find(|row| row.partition_type() == VAR_TYPE);
    assert!(var.is_some_and(RepartDefinition::is_tpm2_encrypted));
    Ok(())
}

/// Positive: a drop-in renders back to a form that parses to the same values.
#[test]
fn a_drop_in_round_trips_through_the_parser() -> Fallible {
    for (name, text) in [("00-esp.conf", ESP), ("20-var.conf", VAR)] {
        let once = RepartDefinition::parse(name, text)?;
        let twice = RepartDefinition::parse(name, &once.render())?;
        assert_eq!(twice, once);
        assert_eq!(twice.render(), once.render());
    }
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: systemd 261 exits 1 with `Type= not defined, refusing.`
#[test]
fn a_drop_in_without_a_type_is_refused() {
    assert_eq!(
        RepartDefinition::parse("50-no-type.conf", "[Partition]\nLabel=no-type\n").err(),
        Some(DefinitionError::MissingKey {
            name: "50-no-type.conf".to_owned(),
            section: PARTITION_SECTION.to_owned(),
            key: "Type",
        })
    );
}

/// Negative: the key systemd ignores while still exiting 0 (REQ-CI-01).
#[test]
fn the_key_systemd_only_warns_about_is_refused_here() {
    let text = "[Partition]\nType=esp\nSubsystem=block\n";
    assert_eq!(
        RepartDefinition::parse("00-esp.conf", text).err(),
        Some(DefinitionError::UnknownKey {
            name: "00-esp.conf".to_owned(),
            line: 3,
            section: PARTITION_SECTION.to_owned(),
            key: "Subsystem".to_owned(),
        })
    );
    let imported = "[Partition]\nType=var\nBtrfsSubvolumes=@var @var-log\n";
    assert!(matches!(
        RepartDefinition::parse("20-var.conf", imported).err(),
        Some(DefinitionError::UnknownKey { ref key, .. }) if key == "BtrfsSubvolumes"
    ));
}

/// Negative: an unreadable size, and a section repart does not define.
#[test]
fn a_malformed_size_or_foreign_section_is_refused() {
    assert!(matches!(
        RepartDefinition::parse("x.conf", "[Partition]\nType=esp\nSizeMinBytes=512 M\n").err(),
        Some(DefinitionError::MalformedValue { line: 3, .. })
    ));
    assert!(matches!(
        RepartDefinition::parse("x.conf", "[Transfer]\nType=esp\n").err(),
        Some(DefinitionError::UnknownSection { ref section, .. }) if section == "Transfer"
    ));
}

/// Negative: a verity data partition with no hash partner is reported.
#[test]
fn a_verity_data_partition_without_its_hash_is_reported() -> Fallible {
    let set = vec![
        RepartDefinition::parse("10-root-a.conf", ROOT_A)?,
        RepartDefinition::parse("10-root-b.conf", ROOT_B)?,
    ];
    let findings = check_repart_set(&set);
    assert!(findings.contains(&Finding::MissingVerityHash {
        match_key: "root".to_owned(),
    }));
    let orphan = vec![RepartDefinition::parse("11-root-verity.conf", VERITY)?];
    assert!(
        check_repart_set(&orphan).contains(&Finding::MissingVerityData {
            match_key: "root".to_owned(),
        })
    );
    Ok(())
}

/// Negative: an unencrypted `/var` is reported against REQ-P02-03.
#[test]
fn an_unencrypted_var_partition_is_reported() -> Fallible {
    let text = "[Partition]\nType=var\nFormat=btrfs\nLabel=var\nSizeMinBytes=10G\n";
    let var = RepartDefinition::parse("20-var.conf", text)?;
    assert_eq!(var.findings(), vec![Finding::VarNotEncrypted]);
    assert!(!var.is_tpm2_encrypted());
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: min equal to max is accepted, min above max is refused.
#[test]
fn the_size_pair_is_accepted_at_equality_and_refused_when_inverted() -> Fallible {
    let equal = "[Partition]\nType=esp\nSizeMinBytes=512M\nSizeMaxBytes=512M\n";
    let parsed = RepartDefinition::parse("00-esp.conf", equal)?;
    assert_eq!(parsed.size_min(), parsed.size_max());
    assert!(parsed.findings().is_empty());

    let inverted = "[Partition]\nType=esp\nSizeMinBytes=1G\nSizeMaxBytes=512M\n";
    assert_eq!(
        RepartDefinition::parse("00-esp.conf", inverted).err(),
        Some(DefinitionError::InvertedSize {
            name: "00-esp.conf".to_owned(),
            minimum: ESP_MAX_BYTES,
            maximum: ESP_MIN_BYTES,
        })
    );
    Ok(())
}

/// Boundary: 511M is below the ESP minimum, 512M is not; 1G is the maximum.
#[test]
fn the_esp_size_bound_is_exact_at_511m_512m_and_1g() -> Fallible {
    let below =
        RepartDefinition::parse("00-esp.conf", "[Partition]\nType=esp\nSizeMinBytes=511M\n")?;
    assert_eq!(
        below.findings(),
        vec![Finding::EspBelowMinimum { bytes: 535_822_336 }]
    );
    let at = RepartDefinition::parse("00-esp.conf", "[Partition]\nType=esp\nSizeMinBytes=512M\n")?;
    assert!(at.findings().is_empty());
    let above = RepartDefinition::parse("00-esp.conf", "[Partition]\nType=esp\nSizeMaxBytes=2G\n")?;
    assert_eq!(
        above.findings(),
        vec![Finding::EspAboveMaximum {
            bytes: 2_147_483_648
        }]
    );
    let unbounded = RepartDefinition::parse("00-esp.conf", "[Partition]\nType=esp\n")?;
    assert_eq!(unbounded.findings(), vec![Finding::EspUnbounded]);
    Ok(())
}

/// Boundary: one root slot is not A/B, two is, and three is not either.
#[test]
fn the_ab_slot_count_is_exact() -> Fallible {
    let one = vec![RepartDefinition::parse("10-root-a.conf", ROOT_B)?];
    assert!(check_repart_set(&one).contains(&Finding::MissingAlternateRootSlot { slots: 1 }));
    let two = reviewed_set()?;
    assert!(
        !check_repart_set(&two)
            .iter()
            .any(|finding| matches!(finding, Finding::MissingAlternateRootSlot { .. }))
    );
    let mut three = two;
    three.push(RepartDefinition::parse("10-root-c.conf", ROOT_B)?);
    assert!(check_repart_set(&three).contains(&Finding::MissingAlternateRootSlot { slots: 3 }));
    assert!(three.len() <= MAX_DEFINITIONS);
    Ok(())
}

/// Boundary: the known-key list is sorted, bounded and holds the shipped keys.
#[test]
fn the_known_key_list_covers_the_shipped_keys_and_not_the_ignored_ones() {
    let mut sorted = KNOWN_KEYS.to_vec();
    sorted.sort_unstable();
    assert_eq!(sorted, KNOWN_KEYS.to_vec());
    for key in [
        "Type",
        "Format",
        "Label",
        "SizeMinBytes",
        "SizeMaxBytes",
        "CopyFiles",
        "Priority",
        "Weight",
        "Verity",
        "VerityMatchKey",
        "Minimize",
        "Encrypt",
        "Subvolumes",
    ] {
        assert!(KNOWN_KEYS.contains(&key), "{key} must be a known key");
    }
    for key in ["Subsystem", "BtrfsSubvolumes", "Partitions"] {
        assert!(!KNOWN_KEYS.contains(&key), "{key} must not be a known key");
    }
}

/// Boundary: a set at the definition bound reports at most [`MAX_FINDINGS`]
/// findings, so a pathological input cannot grow the report without limit.
#[test]
fn a_report_never_grows_past_the_finding_bound() -> Fallible {
    let unbounded_esp = "[Partition]\nType=esp\n";
    let mut set: Vec<RepartDefinition> = Vec::new();
    for index in 0..MAX_DEFINITIONS {
        set.push(RepartDefinition::parse(
            &format!("{index:02}-esp.conf"),
            unbounded_esp,
        )?);
    }
    let findings = check_repart_set(&set);
    assert_eq!(findings.len(), MAX_FINDINGS);
    assert!(
        findings
            .iter()
            .all(|finding| *finding == Finding::EspUnbounded)
    );
    Ok(())
}
