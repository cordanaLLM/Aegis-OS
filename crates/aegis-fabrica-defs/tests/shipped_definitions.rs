// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The reviewed files in `build/`, read by the parser rather than by hand.
//!
//! `tools/verify_systemd_definitions.py` runs the same files through the
//! host's systemd. These tests are the Rust half of the same claim, and they
//! run on a machine with no systemd at all, so a checkout that cannot execute
//! the gate still learns whether the shipped definitions parse.

mod common;

use aegis_fabrica_defs::{
    ESP_MAX_BYTES, ESP_MIN_BYTES, ESP_TYPE, Finding, ROOT_TYPE, RepartDefinition,
    TransferDefinition, VAR_TYPE, check_repart_set,
};

use common::{Fallible, reviewed_repart_set, reviewed_transfer_set};

/// Substrings that must never appear in a reviewed file's header comments.
///
/// `/home/` catches an absolute developer path. The other two catch the
/// relative spellings of the private source bundle, which an absolute-path
/// check on its own would let through.
const PRIVATE_MARKERS: [&str; 3] = ["/home/", ".workingdir", "notebook-prepared"];

// --- Positive -------------------------------------------------------------

/// Positive: every shipped repart drop-in parses and the set has no findings.
#[test]
fn the_shipped_repart_set_parses_and_meets_every_requirement() -> Fallible {
    let sources = reviewed_repart_set()?;
    assert_eq!(sources.len(), 5);
    let mut set: Vec<RepartDefinition> = Vec::new();
    for (name, text) in &sources {
        set.push(RepartDefinition::parse(name, text)?);
    }
    assert_eq!(check_repart_set(&set), Vec::new());
    Ok(())
}

/// Positive: every shipped transfer parses and reports no findings.
///
/// The transfer side is swept by directory, the way the repart side already
/// was. `build/sysupdate.d` holds one transfer today and its own header
/// records the dm-verity hash and the UKI transfers as M11 work; a test that
/// named one file would let those two ship untested by the Rust half.
#[test]
fn every_shipped_transfer_parses_and_reports_no_findings() -> Fallible {
    let sources = reviewed_transfer_set()?;
    assert!(!sources.is_empty(), "build/sysupdate.d ships no transfer");
    for (name, text) in &sources {
        let transfer = TransferDefinition::parse(name, text)?;
        assert_eq!(transfer.findings(), Vec::new(), "{name}");
    }
    Ok(())
}

/// Positive: the shipped root transfer meets the A/B requirement.
#[test]
fn the_shipped_transfer_parses_and_is_a_read_only_ab_target() -> Fallible {
    let sources = reviewed_transfer_set()?;
    let found = sources
        .iter()
        .find(|(name, _)| name.as_str() == "10-root.transfer");
    let Some((name, text)) = found else {
        return Err("build/sysupdate.d must ship 10-root.transfer".into());
    };
    let transfer = TransferDefinition::parse(name, text)?;
    assert_eq!(transfer.source_type(), Some("url-file"));
    assert_eq!(transfer.target_type(), Some("partition"));
    assert!(transfer.is_read_only());
    assert_eq!(transfer.findings(), Vec::new());
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: no shipped drop-in carries a key systemd would ignore.
///
/// The check is the parser's own refusal: if a rejected key came back into a
/// reviewed file, `parse` would fail before this assertion is reached. The
/// assertion here is the complementary one, that each file still declares the
/// keys its requirement depends on.
#[test]
fn each_shipped_drop_in_still_declares_the_keys_its_requirement_needs() -> Fallible {
    let sources = reviewed_repart_set()?;
    for (name, text) in &sources {
        let definition = RepartDefinition::parse(name, text)?;
        match definition.partition_type() {
            ESP_TYPE => {
                assert_eq!(definition.size_min(), Some(ESP_MIN_BYTES), "{name}");
                assert_eq!(definition.size_max(), Some(ESP_MAX_BYTES), "{name}");
            }
            VAR_TYPE => assert!(definition.is_tpm2_encrypted(), "{name}"),
            ROOT_TYPE => assert!(definition.label().is_some(), "{name}"),
            other => assert_eq!(other, "root-verity", "{name}"),
        }
    }
    Ok(())
}

/// Negative: the two root slots carry distinct labels, so sysupdate can tell
/// them apart; a set that labelled both the same would list one instance.
#[test]
fn the_two_root_slots_carry_distinct_labels() -> Fallible {
    let sources = reviewed_repart_set()?;
    let mut labels: Vec<String> = Vec::new();
    for (name, text) in &sources {
        let definition = RepartDefinition::parse(name, text)?;
        if definition.partition_type() != ROOT_TYPE {
            continue;
        }
        let label = definition.label().unwrap_or_default().to_owned();
        assert!(!labels.contains(&label), "duplicate root label {label}");
        labels.push(label);
    }
    labels.sort();
    assert_eq!(labels, ["root-a", "root-b"]);
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: removing either root slot from the shipped set makes it not A/B,
/// which is what keeps the A/B check honest on the real files.
#[test]
fn dropping_a_root_slot_from_the_shipped_set_is_reported() -> Fallible {
    let sources = reviewed_repart_set()?;
    let mut set: Vec<RepartDefinition> = Vec::new();
    for (name, text) in &sources {
        set.push(RepartDefinition::parse(name, text)?);
    }
    let position = set.iter().position(|row| row.label() == Some("root-b"));
    let Some(position) = position else {
        return Err("the shipped set must declare a root-b slot".into());
    };
    set.remove(position);
    let findings = check_repart_set(&set);
    assert!(findings.contains(&Finding::MissingAlternateRootSlot { slots: 1 }));
    Ok(())
}

/// Boundary: each reviewed file cites the private source it was derived from.
///
/// The citation is an export id and a 64-character digest, never a path.
/// Both shipped sets are swept, so a definition added later is held to the
/// same citation rule without being named here.
#[test]
fn every_reviewed_file_cites_an_export_id_and_a_digest() -> Fallible {
    let mut sources = reviewed_repart_set()?;
    sources.extend(reviewed_transfer_set()?);
    for (name, text) in &sources {
        let comments: String = text
            .lines()
            .take(64)
            .filter(|line| line.starts_with('#'))
            .collect::<Vec<&str>>()
            .join("\n");
        assert!(comments.contains("export-0"), "{name} cites no export id");
        let digests = comments
            .split_whitespace()
            .filter(|word| word.len() == 64 && word.chars().all(|c| c.is_ascii_hexdigit()))
            .count();
        assert!(digests >= 1, "{name} cites no sha256 digest");
        for marker in PRIVATE_MARKERS {
            assert!(
                !comments.contains(marker),
                "{name} names the private path {marker}"
            );
        }
    }
    Ok(())
}
