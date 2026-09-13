// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The regression gate over the effects this crate does not have.
//!
//! The crate documentation says it loads nothing and runs nothing. That claim
//! is worth exactly as much as a check that can falsify it, so this file sweeps
//! the crate's own sources for a **named list** of identifiers that would be
//! needed to perform any of those effects, and fails if one appears outside a
//! comment.
//!
//! # What this is, and what it is not
//!
//! It is a regression gate over [`WATCHED`]: the twenty-two identifiers below
//! are the ones a `CAD` kernel, a mesher, a solver process, a cgroup or a file
//! would have to name, and adding one to the library without adding it here is
//! what this catches. It is **not** a proof that no effect of any kind is
//! reachable: an identifier that is not on the list is not checked, and a
//! dependency could in principle perform an effect this sweep cannot see. What
//! makes the second half narrow in practice is the dependency set, which
//! `manifest_hygiene.rs` pins closed, name by name.
//!
//! The filesystem half matters more here than in its neighbours, because the
//! scaffold this crate replaces **does** touch the filesystem: its geometry
//! evaluation calls `step_path.exists()`. That call is the one this crate
//! replaced with a typed refusal, so `std::path` and `exists()` are both on the
//! list. The bare spelling `Path::` is deliberately not, because it is a
//! substring of this crate's own `StepPath::parse`.

mod common;

use common::Fallible;

/// The identifiers a kernel, a mesher, a solver or a cgroup would name.
const WATCHED: [&str; 22] = [
    "std::fs",
    "std::process",
    "std::path",
    "PathBuf",
    "exists()",
    "Command::",
    "OpenOptions",
    "File::",
    "read_to_string",
    "create_dir",
    "truck",
    "opencascade",
    "manifold3d",
    "gmsh",
    "z3::",
    "z3_sys",
    "cgroups_rs",
    "sys/fs/cgroup",
    "libc",
    "dbus",
    "zbus",
    "SystemTime",
];

/// Three identifiers the sweep deliberately does **not** watch.
///
/// The solver tags and the slice name are data decision REQ-P14-03 requires the
/// crate to carry: a payload and a register that could not name the solver or
/// the slice would record nothing. A sweep that watched them would fail on the
/// very fields it is supposed to protect; what it watches instead is the
/// machinery that would start one -- `Command::`, `cgroups_rs`, `sys/fs/cgroup`.
const NOT_WATCHED: [&str; 3] = ["OpenFOAM", "CalculiX", "agent-solver.slice"];

// --- Positive -------------------------------------------------------------

/// Positive: the sweep reads the crate's sources, and reads several of them.
#[test]
fn the_sweep_reads_the_whole_crate() -> Fallible {
    let sources = common::rust_sources(&common::source_root())?;
    assert!(
        sources.len() >= 10,
        "the sweep read only {} sources; it is reading nothing, not finding nothing",
        sources.len()
    );
    assert!(sources.iter().any(|(name, _)| name == "lib.rs"));
    assert!(sources.iter().any(|(name, _)| name == "geometry.rs"));
    assert!(sources.iter().any(|(name, _)| name == "verification.rs"));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: no watched identifier appears in library code.
#[test]
fn no_watched_identifier_appears_in_library_code() -> Fallible {
    let found = common::scan_sources(&common::source_root(), &WATCHED)?;
    assert!(
        found.is_empty(),
        "effect identifiers in library code: {found:?}"
    );
    Ok(())
}

/// Negative: the sweep would notice one, so a pass means something.
///
/// The first plant is the scaffold's own line, which is the call this crate
/// replaced with a typed refusal.
#[test]
fn the_sweep_detects_a_planted_identifier() {
    let scaffold = "        if !step_path.exists() { return Err(\"missing\".into()); }";
    assert_eq!(
        common::scan_text("planted.rs", scaffold, &WATCHED),
        vec!["planted.rs:1: exists()".to_owned()]
    );
    let kernel = "    let solid = truck::modeling::builder::tsweep(&face, vector);";
    assert_eq!(
        common::scan_text("planted.rs", kernel, &WATCHED),
        vec!["planted.rs:1: truck".to_owned()]
    );
    let dispatch = "    Command::new(\"OpenFOAM\").spawn()?;";
    assert_eq!(
        common::scan_text("planted.rs", dispatch, &WATCHED),
        vec!["planted.rs:1: Command::".to_owned()]
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the watched list is exactly its recorded length and has no
/// duplicate, so the count in the documentation is the count that is swept.
#[test]
fn the_watched_list_is_its_recorded_length() {
    assert_eq!(WATCHED.len(), 22);
    let mut names = WATCHED.to_vec();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 22);
    assert!(names.iter().all(|needle| !needle.is_empty()));
}

/// Boundary: a comment naming a watched identifier is not a finding.
#[test]
fn a_comment_naming_an_identifier_is_not_a_finding() {
    let comment = "//! no truck kernel, no gmsh mesher, no z3:: solver, no Command:: dispatch";
    assert!(comment.trim().starts_with("//"));
    assert!(WATCHED.iter().any(|needle| comment.contains(needle)));
    assert!(common::scan_text("lib.rs", comment, &WATCHED).is_empty());
}

/// Boundary: the solver tags and the slice name are deliberately outside the
/// list, and the library does carry them -- so watching them would fail the
/// gate on the fields REQ-P14-03 requires.
#[test]
fn the_solver_names_are_deliberately_not_watched() -> Fallible {
    for tag in NOT_WATCHED {
        assert!(
            !WATCHED.contains(&tag),
            "{tag} is data this crate must carry, not an effect"
        );
    }
    let found = common::scan_sources(&common::source_root(), &NOT_WATCHED)?;
    assert!(
        !found.is_empty(),
        "the solver tags and the slice name must appear in library code as data"
    );
    assert!(WATCHED.contains(&"Command::"));
    assert!(WATCHED.contains(&"sys/fs/cgroup"));
    Ok(())
}
