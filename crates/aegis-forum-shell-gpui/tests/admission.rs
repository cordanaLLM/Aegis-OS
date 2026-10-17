// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E28-1: gpui's admission (D102). The pin is one rev of zed-industries/zed,
//! declared once in `[workspace.dependencies]` and locked at that rev; a
//! branch or a tag in its place fails. Every package in this member's closure
//! (D78, all platforms, all dependency kinds) carries a licence expression
//! recorded below, and one that is not recorded fails. The member is written
//! out, uses the workspace lints and holds no `unsafe` token.

#[path = "../../dependency_closure.rs"]
mod dependency_closure;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use dependency_closure::{closure, metadata, plant, set, synthetic};

/// This member's package name.
const PACKAGE: &str = "aegis-forum-shell-gpui";

/// The zed-industries/zed commit gpui is pinned to (D102), read on 2026-10-17.
const ZED_REV: &str = "322cd509dcccaee1f6bc44725d270598ae8171ad";

/// The zed crates admitted from that commit.
const ZED_CRATES: [&str; 2] = ["gpui", "gpui_linux"];

/// The licence expressions recorded for the closure, exactly as each crate's
/// manifest spells them. A new expression is a maintainer decision first
/// (M28 exit criterion 1); a non-permissive one is named in
/// `docs/roadmap/toolchain-admission.md`.
const RECORDED_LICENCES: &[&str] = &[
    "(MIT OR Apache-2.0) AND NCSA",
    "(MIT OR Apache-2.0) AND Unicode-3.0",
    "0BSD OR MIT OR Apache-2.0",
    "Apache-2.0",
    "Apache-2.0 OR BSL-1.0",
    "Apache-2.0 OR GPL-2.0-only",
    "Apache-2.0 OR MIT",
    "Apache-2.0 OR MIT OR Zlib",
    "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT",
    "Apache-2.0/MIT",
    "BSD-2-Clause",
    "BSD-2-Clause OR Apache-2.0 OR MIT",
    "BSD-3-Clause",
    "BSD-3-Clause OR Apache-2.0",
    "CC0-1.0 OR Apache-2.0",
    "EUPL-1.2",
    "ISC",
    "MIT",
    "MIT / Apache-2.0",
    "MIT OR Apache-2.0",
    "MIT OR Apache-2.0 OR LGPL-2.1-or-later",
    "MIT OR Apache-2.0 OR Zlib",
    "MIT OR Zlib OR Apache-2.0",
    "MIT/Apache-2.0",
    "MPL-2.0",
    "Unicode-3.0",
    "Unlicense OR MIT",
    "Unlicense/MIT",
    "Zlib",
    "Zlib OR Apache-2.0 OR MIT",
];

type Fallible = Result<(), Box<dyn std::error::Error>>;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap_or_default()
}

/// Why `line`, a `[workspace.dependencies]` declaration of `name`, is not an
/// admitted pin: it must name the zed repository and `rev = ZED_REV`, and
/// neither a branch nor a tag.
fn pin_fault(line: &str, name: &str) -> Option<&'static str> {
    if !line.starts_with(&format!(
        "{name} = {{ git = \"https://github.com/zed-industries/zed\""
    )) {
        return Some("not the zed git source");
    }
    if line.contains("branch =") || line.contains("tag =") {
        return Some("a branch or a tag in place of a rev");
    }
    if !line.contains(&format!("rev = \"{ZED_REV}\"")) {
        return Some("not the admitted rev");
    }
    None
}

/// The lock-file source line every zed crate carries.
fn pinned_source() -> String {
    format!("source = \"git+https://github.com/zed-industries/zed?rev={ZED_REV}#{ZED_REV}\"")
}

/// The source line of each `Cargo.lock` entry named `name` (the line after
/// its version line), in lock order.
fn locked_sources<'l>(lock: &'l str, name: &str) -> Vec<&'l str> {
    let header = format!("name = \"{name}\"");
    let lines: Vec<&str> = lock.lines().collect();
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| **line == header)
        .filter_map(|(at, _)| lines.get(at.saturating_add(2)).copied())
        .collect()
}

/// Every declaration of `name` in `manifest`, at the start of a line.
fn declarations<'m>(manifest: &'m str, name: &str) -> Vec<&'m str> {
    let prefix = format!("{name} = ");
    manifest
        .lines()
        .filter(|line| line.starts_with(&prefix))
        .collect()
}

/// Every licence expression a package named in `names` carries, by name; a
/// package with no `license` field maps to the empty string.
fn licences(metadata: &Value, names: &BTreeSet<String>) -> BTreeMap<String, BTreeSet<String>> {
    let mut found: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    for package in packages {
        let Some(name) = package.get("name").and_then(Value::as_str) else {
            continue;
        };
        if names.contains(name) {
            let licence = package.get("license").and_then(Value::as_str).unwrap_or("");
            found
                .entry(licence.to_owned())
                .or_default()
                .insert(name.to_owned());
        }
    }
    found
}

/// The expressions in `found` that are not recorded, with who carries them.
fn unrecorded(found: &BTreeMap<String, BTreeSet<String>>) -> Vec<String> {
    found
        .iter()
        .filter(|(licence, _)| !RECORDED_LICENCES.contains(&licence.as_str()))
        .map(|(licence, names)| format!("{licence:?}: {names:?}"))
        .collect()
}

// --- Positive -------------------------------------------------------------

/// Positive: a written-out member with the workspace lints, unpublished.
#[test]
fn the_member_is_written_out_with_workspace_lints() {
    assert!(read("Cargo.toml").contains("    \"crates/aegis-forum-shell-gpui\",\n"));
    assert!(read("Cargo.lock").contains("name = \"aegis-forum-shell-gpui\""));
    let manifest = read("crates/aegis-forum-shell-gpui/Cargo.toml");
    assert!(manifest.contains("[lints]\nworkspace = true"));
    assert!(manifest.contains("publish = false"));
    assert!(manifest.contains("aegis-forum-shell = { path = \"../aegis-forum-shell\" }"));
}

/// Positive: each zed crate is declared once, at the admitted rev, used by
/// reference, and locked from that rev.
#[test]
fn the_zed_crates_are_pinned_to_one_rev_and_locked() {
    let manifest = read("Cargo.toml");
    let member = read("crates/aegis-forum-shell-gpui/Cargo.toml");
    let lock = read("Cargo.lock");
    for name in ZED_CRATES {
        let lines = declarations(&manifest, name);
        assert_eq!(lines.len(), 1, "{name}: {lines:?}");
        assert_eq!(lines.first().and_then(|line| pin_fault(line, name)), None);
        assert!(
            member.contains(&format!("{name}.workspace = true")),
            "{name}"
        );
        assert_eq!(locked_sources(&lock, name), vec![pinned_source()], "{name}");
    }
}

/// Positive: every licence in this member's closure is recorded.
#[test]
fn every_licence_in_the_closure_is_recorded() -> Fallible {
    let metadata = metadata()?;
    let names = closure(&metadata, PACKAGE)?;
    assert!(names.contains("gpui") && names.contains("accesskit_unix"));
    let missing = unrecorded(&licences(&metadata, &names));
    assert!(missing.is_empty(), "{missing:#?}");
    Ok(())
}

/// Positive: the member's sources hold no `unsafe` token.
#[test]
fn the_sources_hold_no_unsafe_token() -> Fallible {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for entry in std::fs::read_dir(dir)?.take(64) {
        let path = entry?.path();
        let text = std::fs::read_to_string(&path)?;
        assert!(!text.contains("unsafe"), "{}", path.display());
    }
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a branch, a tag, another rev or another source is refused.
#[test]
fn a_branch_a_tag_or_another_rev_is_refused() {
    let base = "gpui = { git = \"https://github.com/zed-industries/zed\"";
    assert_eq!(
        pin_fault(&format!("{base}, branch = \"main\" }}"), "gpui"),
        Some("a branch or a tag in place of a rev")
    );
    assert_eq!(
        pin_fault(&format!("{base}, tag = \"v0.200.0\" }}"), "gpui"),
        Some("a branch or a tag in place of a rev")
    );
    assert_eq!(
        pin_fault(&format!("{base}, rev = \"{}\" }}", "0".repeat(40)), "gpui"),
        Some("not the admitted rev")
    );
    assert_eq!(
        pin_fault(&format!("{base} }}"), "gpui"),
        Some("not the admitted rev")
    );
    assert_eq!(
        pin_fault("gpui = { version = \"0.2.2\" }", "gpui"),
        Some("not the zed git source")
    );
}

/// Negative: a package whose licence is not recorded, or that has none, is
/// named.
#[test]
fn an_unrecorded_licence_is_named() -> Fallible {
    let mut doc = synthetic(PACKAGE, &["permissive"]);
    plant(&mut doc, PACKAGE, "copyleft", None)?;
    plant(&mut doc, PACKAGE, "silent", None)?;
    if let Some(packages) = doc.get_mut("packages").and_then(Value::as_array_mut) {
        for package in packages.iter_mut() {
            let licence = match package.get("name").and_then(Value::as_str) {
                Some("permissive") => "MIT",
                Some("copyleft") => "GPL-3.0-only",
                _ => continue,
            };
            set(package, "license", Value::from(licence));
        }
    }
    let names = closure(&doc, PACKAGE)?;
    let missing = unrecorded(&licences(&doc, &names));
    assert_eq!(
        missing,
        vec![
            "\"\": {\"aegis-forum-shell-gpui\", \"silent\"}".to_owned(),
            "\"GPL-3.0-only\": {\"copyleft\"}".to_owned(),
        ]
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a second declaration of a zed crate is caught, a rev that
/// differs from the admitted one in its last character is refused, and a lock
/// entry at that near rev is told apart from the pinned one.
#[test]
fn a_second_declaration_and_a_near_rev_are_caught() {
    let entry = format!(
        "name = \"gpui\"\nversion = \"0.2.2\"\n{}\n",
        pinned_source()
    );
    assert_eq!(locked_sources(&entry, "gpui"), vec![pinned_source()]);
    let near_entry = entry.replacen(&format!("#{ZED_REV}"), "#0", 1);
    assert_ne!(locked_sources(&near_entry, "gpui"), vec![pinned_source()]);
    assert_eq!(locked_sources(&entry, "gpu"), [] as [&str; 0]);
    let line = format!(
        "gpui = {{ git = \"https://github.com/zed-industries/zed\", rev = \"{ZED_REV}\" }}"
    );
    assert_eq!(declarations(&format!("{line}\n{line}\n"), "gpui").len(), 2);
    let near = format!("{}0", &ZED_REV[..39]);
    assert_ne!(near, ZED_REV);
    assert_eq!(
        pin_fault(&line.replace(ZED_REV, &near), "gpui"),
        Some("not the admitted rev")
    );
}
