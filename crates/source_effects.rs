// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! D116: shared source effect scanner for integration tests.
//!
//! One file, included by path into the integration tests and test common modules
//! (`#[path = "../../source_effects.rs"] mod source_effects;`), extending D78 and D116
//! precedents to source sweeps. It is not a crate and has no manifest: a test helper
//! that were a workspace member would enter every dependency closure.

#![allow(dead_code)]

use std::path::Path;

#[path = "surface_sources.rs"]
mod surface_sources;

/// Scalar bound on directories visited and entries read per directory.
pub const WALK_BOUND: usize = 64;

/// Scalar bound on the lines read from any one source file.
pub const LINE_BOUND: usize = 4096;

/// Returns the `.rs` files under `root`, as (file name, body) pairs.
///
/// Delegates to [`surface_sources::rust_sources`] (D116) to follow path-included sources.
///
/// # Errors
///
/// Returns the first read failure, so a sweep cannot pass by reading nothing.
pub fn rust_sources(root: &Path) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let out = surface_sources::rust_sources(root);
    if out.is_empty() {
        return Err(format!("no Rust source was read under {}", root.display()).into());
    }
    Ok(out)
}

/// Returns the `"<file>:<line>: <identifier>"` hits `text` carries.
pub fn hits(name: &str, number: usize, text: &str, watched: &[&str]) -> Vec<String> {
    watched
        .iter()
        .filter(|needle| text.contains(**needle))
        .map(|needle| format!("{name}:{}: {needle}", number.saturating_add(1)))
        .collect()
}

/// Returns the hits one body of text carries, skipping comment lines.
///
/// Comment lines are skipped whole: the crate documentation names the very
/// identifiers a sweep watches for, precisely in order to say it does not use
/// them, so a sweep that read comments would fail on its own documentation.
///
/// This is public so a watched list can be exercised against planted text.
/// A list of constructs is a gate only if each entry is one the scan reports,
/// and that is checkable without writing a file.
pub fn scan_text(name: &str, body: &str, watched: &[&str]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for (number, line) in body.lines().take(LINE_BOUND).enumerate() {
        let text = line.trim();
        if text.starts_with("//") {
            continue;
        }
        found.extend(hits(name, number, text, watched));
    }
    found
}

/// Scans sources under `root` for `watched` identifiers, optionally filtering outside literals.
pub fn scan_sources_mode(
    root: &Path,
    watched: &[&str],
    outside_literals: bool,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut found: Vec<String> = Vec::new();
    for (name, body) in rust_sources(root)? {
        let mut inside = false;
        for (number, line) in body.lines().take(LINE_BOUND).enumerate() {
            let text = line.trim();
            if !inside && text.starts_with("//") {
                continue;
            }
            let code = if outside_literals {
                code_only(text, &mut inside)
            } else {
                text.to_string()
            };
            found.extend(
                watched
                    .iter()
                    .filter(|needle| code.contains(**needle))
                    .map(|needle| format!("{name}:{}: {needle}", number.saturating_add(1))),
            );
        }
    }
    Ok(found)
}

/// Returns every watched identifier the sources under `root` carry.
///
/// # Errors
///
/// Propagates [`rust_sources`], so a sweep cannot pass by reading nothing.
pub fn scan_sources(
    root: &Path,
    watched: &[&str],
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    scan_sources_mode(root, watched, false)
}

/// Returns `line` with every double-quoted run removed, carrying `inside`
/// across lines.
///
/// The state has to be carried because this crate's recorded claims are
/// multi-line string literals joined with a trailing backslash: the second
/// line of one carries no opening quote, and a per-line rule reads it as code.
/// That was the first thing this sweep got wrong.
///
/// The rule is otherwise deliberately crude: it splits on the double-quote
/// character and drops every other piece. An escaped quote inside a literal
/// would be split wrongly, which can only ever make the sweep stricter than
/// intended, never looser.
pub fn code_only(line: &str, inside: &mut bool) -> String {
    let mut out = String::new();
    for (index, piece) in line.split('"').enumerate() {
        if index > 0 {
            *inside = !*inside;
        }
        if !*inside {
            out.push_str(piece);
            out.push(' ');
        }
    }
    out
}

/// Returns `line` with its literals removed, starting outside one.
pub fn outside_string_literals(line: &str) -> String {
    let mut inside = false;
    code_only(line, &mut inside)
}

/// Returns the `"<file>:<line>: <name>"` hits `root` carries outside literals.
///
/// # Errors
///
/// Propagates [`rust_sources`].
pub fn scan_outside_literals(
    root: &Path,
    watched: &[&str],
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    scan_sources_mode(root, watched, true)
}

/// The path prefix a standard-library reach has to name.
///
/// Watched on its own rather than as a list entry, because it is a structural
/// property and not one more identifier somebody remembered.
pub const STANDARD_LIBRARY_PATH: &str = "std::";

/// Returns a crate's `src/` directory from its manifest directory.
#[must_use]
pub fn crate_source_root(manifest_dir: &str) -> std::path::PathBuf {
    Path::new(manifest_dir).join("src")
}

/// Positive assertion: the sweep reads the whole crate and several files.
///
/// Asserts that at least `min_sources` files are read, and that every file in
/// `expected_files` was visited, ensuring the sweep cannot pass by reading nothing.
pub fn assert_sweep_reads_sources(
    src: &Path,
    min_sources: usize,
    expected_files: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    let sources = rust_sources(src)?;
    assert!(
        sources.len() >= min_sources,
        "the sweep read only {} sources; it is reading nothing, not finding nothing",
        sources.len()
    );
    for expected in expected_files {
        assert!(
            sources.iter().any(|(name, _)| name == *expected),
            "the sweep did not read {expected}"
        );
    }
    Ok(())
}

/// Negative assertion: no watched code identifier appears in library code.
///
/// Asserts that no watched code identifier appears in non-comment source lines.
pub fn assert_no_watched_code(
    src: &Path,
    watched: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    let found = scan_sources(src, watched)?;
    assert!(
        found.is_empty(),
        "effect identifiers in library code: {found:?}"
    );
    Ok(())
}

/// Negative assertion: no watched path appears outside a string literal.
///
/// Asserts that no watched path appears outside string literals.
pub fn assert_no_watched_paths(
    src: &Path,
    watched: &[&str],
    kind: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let found = scan_outside_literals(src, watched)?;
    assert!(
        found.is_empty(),
        "recorded {kind} used as code rather than as data: {found:?}"
    );
    Ok(())
}

/// Negative assertion: no line of library code names `std::` at all.
///
/// This is a different check from the two lists above rather than a stronger
/// one: not "none of these named identifiers appears" but "no line under
/// `src/` names the standard library". Every `use` under `src/` is `core::`,
/// `crate::`, `serde::` or a workspace crate, so a reach that has to spell a
/// `std::` path is caught whether or not anybody wrote its identifier down.
///
/// It is a check over the text of `src/`, not a proof that no effect is
/// reachable. The standard macro prelude is the hole it cannot see:
/// `println!`, `eprintln!`, `print!`, `eprint!` and `dbg!` write to a file
/// descriptor without naming `std::` and appear on no watched list. Those are
/// denied by `print_stdout`, `print_stderr` and `dbg_macro` in
/// `[workspace.lints.clippy]`, which is a third mechanism and is not this one.
/// A dependency can still perform an effect none of the three sees, and a
/// dependency that re-exported a standard-library type would hide the path.
pub fn assert_no_standard_library(src: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let found = scan_sources(src, &[STANDARD_LIBRARY_PATH])?;
    assert!(
        found.is_empty(),
        "library code naming the standard library: {found:?}"
    );
    Ok(())
}

/// Negative assertion: the structural sweep catches reaches the named lists miss.
///
/// Both planted lines are real effects, and neither carries an identifier on
/// either list. That is the point of holding the structural property as well
/// as the enumeration.
pub fn assert_structural_sweep_detects_reach(watched_code: &[&str]) {
    for planted in [
        "    let reachable = std::net::UdpSocket::bind(\"127.0.0.1:0\").is_ok();",
        "    let leaked = std::env::var(\"AEGIS_PLANT\").is_ok();",
    ] {
        assert!(
            scan_text("planted.rs", planted, watched_code).is_empty(),
            "the named list is not what catches {planted}"
        );
        let structural = scan_text("planted.rs", planted, &[STANDARD_LIBRARY_PATH]);
        assert!(
            !structural.is_empty(),
            "the structural sweep missed {planted}"
        );
    }
}

/// Boundary assertion: watched lists are their recorded lengths, with no duplicate or overlap.
///
/// Both lists are exactly their recorded lengths, with no duplicate
/// and no overlap, so the counts in the documentation are the counts swept.
pub fn assert_watched_lists_distinct(
    watched_code: &[&str],
    watched_paths: &[&str],
    expected_code_len: usize,
    expected_paths_len: usize,
) {
    assert_eq!(watched_code.len(), expected_code_len);
    assert_eq!(watched_paths.len(), expected_paths_len);

    let mut all: Vec<&str> = watched_code.to_vec();
    all.extend_from_slice(watched_paths);
    let expected_total = expected_code_len.saturating_add(expected_paths_len);
    assert_eq!(all.len(), expected_total);
    all.sort_unstable();
    let before = all.len();
    all.dedup();
    assert_eq!(before, all.len(), "an identifier is watched twice");
    assert!(all.iter().all(|needle| !needle.is_empty()));
}

/// Boundary assertion: a comment naming a watched identifier is not a finding.
pub fn assert_comment_naming_identifier_is_not_finding(comment: &str, watched: &[&str]) {
    assert!(comment.trim().starts_with("//"));
    assert!(watched.iter().any(|needle| comment.contains(needle)));
    assert_eq!(
        scan_text("doc.rs", comment, watched),
        [] as [std::string::String; 0]
    );
}
