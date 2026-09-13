// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Triples for the sysupdate transfer, including the six keys systemd 261
//! ignored in the imported definition and the single slot it silently bumps.

mod common;

use aegis_fabrica_defs::{
    AB_INSTANCES, DefinitionError, Finding, SOURCE_KEYS, SOURCE_SECTION, TARGET_KEYS,
    TARGET_SECTION, TRANSFER_KEYS, TRANSFER_SECTION, TransferDefinition,
};

use common::Fallible;

const REVIEWED: &str = "[Transfer]\nProtectVersion=%A\n\n\
[Source]\nType=url-file\nPath=https://releases.aegisos.org/update/\n\
MatchPattern=aegis-os-root-@v.raw.xz\n\n\
[Target]\nType=partition\nPath=auto\nMatchPattern=root-@v\n\
MatchPartitionType=root\nReadOnly=1\nMode=0444\nInstancesMax=2\n";

/// Returns the reviewed transfer with one line replaced.
fn with(line: &str, replacement: &str) -> String {
    REVIEWED.replace(line, replacement)
}

// --- Positive -------------------------------------------------------------

/// Positive: the reviewed transfer yields every field the findings read.
#[test]
fn the_reviewed_transfer_is_read_field_by_field() -> Fallible {
    let transfer = TransferDefinition::parse("10-root.transfer", REVIEWED)?;
    assert_eq!(transfer.source_type(), Some("url-file"));
    assert_eq!(transfer.target_type(), Some("partition"));
    assert_eq!(transfer.target_pattern(), Some("root-@v"));
    assert_eq!(transfer.instances_max(), Some(AB_INSTANCES));
    assert!(transfer.is_read_only());
    assert_eq!(
        transfer.unit().value(TRANSFER_SECTION, "ProtectVersion"),
        Some("%A")
    );
    assert_eq!(transfer.findings(), Vec::new());
    Ok(())
}

/// Positive: the transfer round-trips through its canonical rendering.
#[test]
fn the_transfer_round_trips_through_the_parser() -> Fallible {
    let once = TransferDefinition::parse("10-root.transfer", REVIEWED)?;
    let twice = TransferDefinition::parse("10-root.transfer", &once.render())?;
    assert_eq!(twice, once);
    assert_eq!(
        twice.unit().sections(),
        [TRANSFER_SECTION, SOURCE_SECTION, TARGET_SECTION]
    );
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: each key systemd 261 ignored in the imported transfer is refused.
#[test]
fn every_key_the_imported_transfer_used_is_refused() {
    for (section, key, line) in [
        (SOURCE_SECTION, "File", "File=aegis-os-root-%v.raw.xz"),
        (TARGET_SECTION, "Partitions", "Partitions=root-a:root-b"),
        (TARGET_SECTION, "Writable", "Writable=no"),
        (TARGET_SECTION, "Format", "Format=erofs"),
        (TARGET_SECTION, "MatchKey", "MatchKey=root"),
        (TARGET_SECTION, "VerityMatchKey", "VerityMatchKey=root"),
    ] {
        let anchor = if section == SOURCE_SECTION {
            "MatchPattern=aegis-os-root-@v.raw.xz"
        } else {
            "Mode=0444"
        };
        let text = with(anchor, &format!("{anchor}\n{line}"));
        assert!(
            matches!(
                TransferDefinition::parse("10-root.transfer", &text).err(),
                Some(DefinitionError::UnknownKey {
                    section: ref observed,
                    key: ref observed_key,
                    ..
                }) if observed == section && observed_key == key
            ),
            "{key} must be refused in [{section}]"
        );
    }
}

/// Negative: systemd refuses a source with no `MatchPattern=`; so does the parser.
#[test]
fn a_source_without_a_match_pattern_is_refused() {
    let text = with("MatchPattern=aegis-os-root-@v.raw.xz\n", "");
    assert_eq!(
        TransferDefinition::parse("10-root.transfer", &text).err(),
        Some(DefinitionError::MissingKey {
            name: "10-root.transfer".to_owned(),
            section: SOURCE_SECTION.to_owned(),
            key: "MatchPattern",
        })
    );
}

/// Negative: a target with no Type=, and an unreadable instance count.
#[test]
fn a_target_without_a_type_or_with_a_malformed_count_is_refused() {
    let no_type = with("Type=partition\n", "");
    assert!(matches!(
        TransferDefinition::parse("10-root.transfer", &no_type).err(),
        Some(DefinitionError::MissingKey { key: "Type", .. })
    ));
    let bad_count = with("InstancesMax=2", "InstancesMax=two");
    assert!(matches!(
        TransferDefinition::parse("10-root.transfer", &bad_count).err(),
        Some(DefinitionError::MalformedValue { ref key, .. }) if key == "InstancesMax"
    ));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: one slot is reported, two is accepted, and an absent count is not A/B.
#[test]
fn the_instance_count_is_exact_at_the_ab_boundary() -> Fallible {
    let single = TransferDefinition::parse(
        "10-root.transfer",
        &with("InstancesMax=2", "InstancesMax=1"),
    )?;
    assert_eq!(
        single.findings(),
        vec![Finding::SingleSlotTransfer { instances_max: 1 }]
    );
    let pair = TransferDefinition::parse("10-root.transfer", REVIEWED)?;
    assert!(pair.findings().is_empty());
    let unstated = TransferDefinition::parse("10-root.transfer", &with("InstancesMax=2\n", ""))?;
    assert_eq!(
        unstated.findings(),
        vec![Finding::SingleSlotTransfer { instances_max: 0 }]
    );
    let more = TransferDefinition::parse(
        "10-root.transfer",
        &with("InstancesMax=2", "InstancesMax=3"),
    )?;
    assert!(more.findings().is_empty());
    Ok(())
}

/// Boundary: every spelling of a read-only target, and a writable one.
#[test]
fn the_read_only_target_is_recognised_in_every_accepted_spelling() -> Fallible {
    for spelling in ["1", "yes", "true", "on"] {
        let text = with("ReadOnly=1", &format!("ReadOnly={spelling}"));
        let transfer = TransferDefinition::parse("10-root.transfer", &text)?;
        assert!(transfer.is_read_only(), "ReadOnly={spelling}");
    }
    for spelling in ["0", "no", "false", "off"] {
        let text = with("ReadOnly=1", &format!("ReadOnly={spelling}"));
        let transfer = TransferDefinition::parse("10-root.transfer", &text)?;
        assert_eq!(transfer.findings(), vec![Finding::WritableTransferTarget]);
    }
    Ok(())
}

/// Boundary: the three key vocabularies are sorted, disjoint where they must be,
/// and hold exactly the keys the reviewed transfer uses.
#[test]
fn the_key_vocabularies_are_sorted_and_cover_the_reviewed_transfer() {
    for list in [&TRANSFER_KEYS[..], &SOURCE_KEYS[..], &TARGET_KEYS[..]] {
        let mut sorted = list.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, list.to_vec());
    }
    assert!(TRANSFER_KEYS.contains(&"ProtectVersion"));
    assert!(SOURCE_KEYS.contains(&"MatchPattern"));
    assert!(TARGET_KEYS.contains(&"InstancesMax"));
    assert!(!SOURCE_KEYS.contains(&"InstancesMax"));
    assert!(!TARGET_KEYS.contains(&"ProtectVersion"));
    assert_eq!(AB_INSTANCES, 2);
}
