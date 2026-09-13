// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! "systemd and sysupdate calls are stubbed", checked rather than promised.
//!
//! Milestone M15's third exit criterion says nothing may invoke a real
//! `systemd-sysupdate`, touch a device or reboot. A reviewer can read the
//! source and believe it; this file makes the next change to the source have
//! to pass the same reading.
//!
//! The rule is crude and therefore checkable: outside comments, the crate's
//! own `src/` may not name any of the [`HOST_REACH`] identifiers -- twenty
//! recorded ways to start a process, open a file or device, read the host
//! clock, consult the environment, reach a socket or a thread, pull a file in
//! at compile time, or step outside safe Rust. Every value that would come
//! from the host arrives as data instead, through
//! [`SysupdatePort`](aegis_janus_lifecycle::SysupdatePort).
//!
//! What the rule does not claim, and this is the important half: the list is
//! an enumeration, not a quantifier. It is *not* "every identifier through
//! which the host could be reached" -- no list of identifiers can be that, and
//! claiming it would be false the moment someone reached the host through a
//! name nobody wrote down. The M15 verification made exactly that point by
//! planting `option_env!`, `std::path::Path::new(..).exists()`,
//! `std::os::unix::net::UnixStream::connect(..)`, `std::io::Write::flush` and
//! `std::thread::current()` past a fourteen-token list that passed; those five
//! are now on the list, which says nothing about the sixth. What the sweep is,
//! then, is a regression gate over the reaches this crate is known to have had
//! or been shown to be open to, plus a structural argument -- no `unsafe`, no
//! `libc`, no binary, no build script and four pinned dependencies -- for why
//! the remaining surface is small. An absent identifier is not proof that a
//! future real implementation will be correct, only that this crate is not
//! one, and the sweep cannot see through a dependency, so the manifest is
//! checked too.

mod common;

use std::path::{Path, PathBuf};

use common::Fallible;

/// Scalar bound on directories visited and entries read per directory.
const WALK_BOUND: usize = 64;

/// Scalar bound on the lines read from any one source file.
const LINE_BOUND: usize = 4096;

/// The source files this crate is known to have, so a sweep that reads
/// nothing cannot pass as a sweep that found nothing.
const MINIMUM_SOURCES: usize = 11;

/// The twenty identifiers this sweep is a regression gate over.
///
/// Each is a recorded way to start a process, open a file or device, read the
/// host clock, consult the environment, reach a socket, a path or a thread,
/// pull a file in at compile time, or step outside safe Rust. It is a list and
/// not a quantifier: an identifier that is not on it is not swept for.
///
/// The last six were added by the M15 verification, which planted a function
/// that reached the host through all of them and watched the previous
/// fourteen-token list pass. `option_env!` is listed separately from `env!`
/// because the token match is exact at both ends, so `env!` does not find
/// `option_env!`; the same holds for `include!` against `include_str!`.
/// `std::path` rather than `std::path::Path` so that `PathBuf` is caught too,
/// and `std::os` rather than `std::os::unix` so that no target-specific module
/// slips past.
const HOST_REACH: [&str; 20] = [
    "std::process",
    "Command",
    "std::fs",
    "File",
    "OpenOptions",
    "SystemTime",
    "Instant",
    "std::net",
    "std::env",
    "std::io",
    "std::os",
    "std::path",
    "std::thread",
    "env!",
    "option_env!",
    "include!",
    "include_str!",
    "include_bytes!",
    "unsafe",
    "libc",
];

/// The reach the M15 verification planted in `src/machine.rs`, verbatim.
///
/// The sweep passed over it when the list held fourteen tokens. It is kept
/// here as the input the widened list has to catch, so that dropping one of
/// the six added tokens fails a test that says why the token is there rather
/// than silently shrinking the gate.
const PLANTED_REACH: &str = "\
fn host_peek() -> bool {
    let home = option_env!(\"HOME\").unwrap_or(\"/\");
    let seen = std::path::Path::new(\"/etc/os-release\").exists();
    let connected = std::os::unix::net::UnixStream::connect(\"/run/systemd/private\").is_ok();
    let _ = std::io::Write::flush(&mut std::io::stdout());
    let pid = std::thread::current().id();
    !home.is_empty() && seen && !connected && format!(\"{pid:?}\").len() > 1
}";

/// The dependencies the crate is allowed to declare.
const ALLOWED_DEPENDENCIES: [&str; 4] = ["base16ct", "serde", "serde_json", "thiserror"];

/// The package fields inherited from the workspace: version, edition,
/// rust-version, license, repository and authors.
const INHERITED_PACKAGE_FIELDS: usize = 6;

/// Returns `true` when `c` may appear inside a Rust identifier.
fn is_identifier_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Returns `true` when `text` names `token` as a whole identifier.
///
/// A trailing `!` or `:` is part of the token, so `env!` is distinguished from
/// a local named `env` and `std::fs` from a field called `fs`.
fn names_token(text: &str, token: &str) -> bool {
    text.match_indices(token).any(|(at, _)| {
        let before = text.get(..at).and_then(|head| head.chars().next_back());
        let after = text
            .get(at.saturating_add(token.len())..)
            .and_then(|tail| tail.chars().next());
        let head_clean = !before.is_some_and(is_identifier_char);
        let tail_clean = !after.is_some_and(is_identifier_char);
        head_clean && tail_clean
    })
}

/// Returns `body` without its comment lines, bounded by [`LINE_BOUND`].
fn without_comments(body: &str) -> String {
    let mut code = String::new();
    for line in body.lines().take(LINE_BOUND) {
        if line.trim_start().starts_with("//") {
            continue;
        }
        code.push_str(line);
        code.push('\n');
    }
    code
}

/// Returns the `.rs` files under `root`, as (file name, code) pairs.
///
/// Comment lines are removed: this crate documents the real calls a future
/// implementation would make, and naming one in prose is the point of the
/// documentation rather than a violation of it.
fn code_under(root: &Path) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    for _ in 0..WALK_BOUND {
        let Some(directory) = stack.pop() else { break };
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten().take(WALK_BOUND) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            let name = path
                .file_name()
                .map(|raw| raw.to_string_lossy().into_owned())
                .unwrap_or_default();
            let body = std::fs::read_to_string(&path).unwrap_or_default();
            out.push((name, without_comments(&body)));
        }
    }
    out
}

/// Returns this crate's own source files.
fn crate_sources() -> Vec<(String, String)> {
    code_under(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"))
}

// --- Positive -------------------------------------------------------------

/// Positive: no module in `src/` names anything that reaches the host.
#[test]
fn no_module_reaches_the_host() {
    let sources = crate_sources();
    assert!(
        sources.len() >= MINIMUM_SOURCES,
        "the sweep read {} source files, fewer than the {MINIMUM_SOURCES} this crate has; \
         it is reading nothing, not finding nothing",
        sources.len()
    );
    let mut found: Vec<String> = Vec::new();
    for (name, code) in &sources {
        for token in HOST_REACH {
            if names_token(code, token) {
                found.push(format!("{name} names {token}"));
            }
        }
    }
    assert!(
        found.is_empty(),
        "the lifecycle model reaches the host: {found:?}"
    );
}

/// Positive: the manifest declares four pinned workspace dependencies, no
/// binary, and no build script.
#[test]
fn the_manifest_declares_only_pinned_workspace_dependencies() -> Fallible {
    let manifest =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))?;
    for dependency in ALLOWED_DEPENDENCIES {
        assert!(
            manifest.contains(&format!("{dependency}.workspace = true")),
            "{dependency} must be taken from the workspace pin"
        );
    }
    let declared = manifest.matches(".workspace = true").count();
    assert_eq!(
        declared,
        ALLOWED_DEPENDENCIES
            .len()
            .saturating_add(INHERITED_PACKAGE_FIELDS),
        "the manifest declares a workspace entry this test does not know about"
    );
    assert!(
        !manifest.contains("[[bin]]"),
        "the crate declares no binary"
    );
    assert!(
        !manifest.contains("build ="),
        "the crate has no build script"
    );
    assert!(manifest.contains("publish = false"));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: the sweep would catch a planted reach, and does not fire on a
/// name that merely contains one.
#[test]
fn the_sweep_catches_a_planted_reach_and_not_a_lookalike() {
    assert!(names_token(
        "let output = Command::new(\"systemctl\");",
        "Command"
    ));
    assert!(names_token("use std::process::exit;", "std::process"));
    assert!(names_token("let now = SystemTime::now();", "SystemTime"));
    assert!(names_token(
        "let root = env!(\"CARGO_MANIFEST_DIR\");",
        "env!"
    ));
    assert!(!names_token("let commanded = 1;", "Command"));
    assert!(!names_token("struct FileName;", "File"));
    assert!(!names_token("let environment = 1;", "env!"));
    assert!(!names_token("", "Command"));
}

/// Negative: the reach the M15 verification planted is caught, token by token.
///
/// Each assertion names one of the six identifiers the fourteen-token list did
/// not have, so a future edit that drops one fails here with the reason
/// attached rather than quietly narrowing the sweep.
#[test]
fn the_sweep_catches_the_reach_the_verification_planted() {
    for token in [
        "option_env!",
        "std::path",
        "std::os",
        "std::io",
        "std::thread",
    ] {
        assert!(
            names_token(PLANTED_REACH, token),
            "the planted reach names {token} and the sweep must see it"
        );
    }
    let caught: Vec<&str> = HOST_REACH
        .into_iter()
        .filter(|token| names_token(PLANTED_REACH, token))
        .collect();
    assert_eq!(
        caught.len(),
        5,
        "the planted reach is caught through exactly its five identifiers, not by accident: {caught:?}"
    );
}

/// Negative: the exact match at both ends is why three of the added tokens
/// have to be listed separately rather than being found by a shorter one.
///
/// `env!` does not find `option_env!`, `include_str!` does not find
/// `include!`, and `std::net` does not find `std::os::unix::net`. That is the
/// precise mechanism by which the planted reach walked past the earlier list.
#[test]
fn a_shorter_token_does_not_find_a_longer_identifier() {
    assert!(!names_token("let home = option_env!(\"HOME\");", "env!"));
    assert!(names_token(
        "let home = option_env!(\"HOME\");",
        "option_env!"
    ));
    assert!(!names_token("include_str!(\"fixture.json\")", "include!"));
    assert!(names_token("include!(\"generated.rs\");", "include!"));
    assert!(!names_token(
        "std::os::unix::net::UnixStream::connect(p)",
        "std::net"
    ));
    assert!(names_token(
        "std::os::unix::net::UnixStream::connect(p)",
        "std::os"
    ));
    assert!(names_token(
        "let p = std::path::PathBuf::new();",
        "std::path"
    ));
    assert!(!names_token("let paths = 1;", "std::path"));
}

/// Negative: the comment stripping does not hide code, only prose.
///
/// The crate's documentation names the real calls on purpose. If the stripping
/// were wrong in the other direction -- removing code -- the sweep would pass
/// by reading nothing, so the sweep asserts it still reads the code it should.
#[test]
fn stripping_comments_does_not_hide_code() -> Fallible {
    let sources = crate_sources();
    let machine = sources
        .iter()
        .find(|(name, _)| name == "machine.rs")
        .ok_or("machine.rs is a source file of this crate")?;
    assert!(machine.1.contains("pub fn step"));
    assert!(machine.1.contains("fn plan_bless"));
    assert!(
        !machine.1.contains("systemd-bless-boot"),
        "the prose that names the real command is a comment and is stripped"
    );
    let ports = sources
        .iter()
        .find(|(name, _)| name == "ports.rs")
        .ok_or("ports.rs is a source file of this crate")?;
    assert!(ports.1.contains("pub trait SysupdatePort"));
    assert!(!ports.1.contains("systemd-sysupdate"));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: every module the crate declares is swept, none is skipped.
#[test]
fn every_declared_module_is_swept() {
    let sources = crate_sources();
    let names: Vec<&str> = sources.iter().map(|(name, _)| name.as_str()).collect();
    for module in [
        "lib.rs",
        "candidate.rs",
        "clock.rs",
        "decision.rs",
        "error.rs",
        "machine.rs",
        "ports.rs",
        "slot.rs",
        "state.rs",
        "trace.rs",
        "verity.rs",
    ] {
        assert!(names.contains(&module), "the sweep skipped {module}");
    }
    assert_eq!(names.len(), MINIMUM_SOURCES);
}
