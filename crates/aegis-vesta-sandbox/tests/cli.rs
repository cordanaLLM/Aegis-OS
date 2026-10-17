// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The built `aegis-vesta-sandbox` and `aegis-vesta-guest` binaries: their
//! arguments, their output and the exit codes they reach without `/dev/kvm`.
//!
//! The sandbox run here names a Firecracker that does not exist, so the
//! microVM start fails inside the run directory this test owns and no guest
//! boots. The guest refuses to run as anything but process 1. A booted
//! microVM stays the hand-run `make verify-workstation`.

mod common;

#[path = "../../binary_transcript.rs"]
mod binary_transcript;

use std::path::PathBuf;

use binary_transcript::Transcript;

use common::Fallible;

/// The host binary Cargo built for this test run.
const SANDBOX: &str = env!("CARGO_BIN_EXE_aegis-vesta-sandbox");

/// The guest binary Cargo built for this test run.
const GUEST: &str = env!("CARGO_BIN_EXE_aegis-vesta-guest");

/// The usage line the host binary prints on a refused command line.
const USAGE: &str = "usage: aegis-vesta-sandbox run <firecracker> <kernel> <initramfs> <run-dir>";

/// The line the guest prints outside a microVM.
const NOT_INIT: &str = "aegis-vesta-guest: not process 1; it runs only as a microVM's init\n";

/// The six cases a sandbox run prints, in order.
const CASES: [&str; 6] = [
    "sandbox/over-limit-memory-refused",
    "sandbox/boot-and-round-trip",
    "sandbox/vsock-transport",
    "sandbox/sixty-fourth-accepted",
    "sandbox/sixty-fifth-refused",
    "sandbox/teardown",
];

/// Runs the host binary with `args`.
fn sandbox(args: &[&str]) -> Result<Transcript, String> {
    binary_transcript::run(SANDBOX, args, &[])
}

/// Asserts that `args` is refused as a usage error before any case runs.
fn refused_as_usage(args: &[&str]) -> Fallible {
    let run = sandbox(args)?;
    assert_eq!(run.code, Some(64), "{run:#?}");
    assert_eq!(run.stdout, format!("{USAGE}\n"), "{run:#?}");
    Ok(())
}

/// Runs the host binary against a Firecracker path that does not exist,
/// from its scratch directory. `run_dir` is relative to it, since the run
/// directory sets the socket path's length; the other three paths are
/// absolute, since the start runs in the microVM's own directory. The kernel
/// and initramfs exist as empty files, so a launch that started one of them
/// in Firecracker's place would fail with another error than a missing file.
fn run_without_firecracker(run_dir: &str) -> Result<(Transcript, PathBuf), String> {
    let dir = binary_transcript::scratch("sandbox-run")?;
    let runs = dir.join(run_dir);
    std::fs::create_dir(&runs).map_err(|error| error.to_string())?;
    for file in ["vmlinux", "initramfs"] {
        std::fs::write(dir.join(file), b"").map_err(|error| error.to_string())?;
    }
    let [firecracker, kernel, initrd] = ["no-firecracker", "vmlinux", "initramfs"]
        .map(|file| dir.join(file).to_string_lossy().into_owned());
    let args = ["run", &firecracker, &kernel, &initrd, run_dir];
    let transcript = binary_transcript::run_in(&dir, SANDBOX, &args, &[])?;
    Ok((transcript, runs))
}

// --- Positive -------------------------------------------------------------

/// Positive: four paths after `run` are parsed into a launch and the run
/// reaches all six cases; the admission case needs no microVM and passes,
/// and the start uses the Firecracker path the command line named.
#[test]
fn a_full_command_line_reaches_the_cases() -> Fallible {
    let (run, _) = run_without_firecracker("run")?;
    assert!(
        run.stdout
            .contains("PASS sandbox/over-limit-memory-refused"),
        "{run:#?}"
    );
    for case in CASES {
        assert!(run.stdout.contains(case), "{case}: {run:#?}");
    }
    assert!(
        run.stdout.contains("starting firecracker: No such file"),
        "{run:#?}"
    );
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a Firecracker that cannot start fails the boot case and every
/// case that waits on it, and the run exits 1.
#[test]
fn a_missing_firecracker_fails_the_run() -> Fallible {
    let (run, _) = run_without_firecracker("run")?;
    assert_eq!(run.code, Some(1), "{run:#?}");
    assert!(
        run.stdout.contains("FAIL sandbox/boot-and-round-trip"),
        "{run:#?}"
    );
    assert!(run.last_line().starts_with("RESULT fail: "), "{run:#?}");
    Ok(())
}

/// Negative: an unknown mode is a usage error.
#[test]
fn an_unknown_mode_is_a_usage_error() -> Fallible {
    refused_as_usage(&["boot", "a", "b", "c", "d"])
}

/// Negative: no arguments at all is a usage error.
#[test]
fn no_arguments_is_a_usage_error() -> Fallible {
    refused_as_usage(&[])
}

/// Negative: the guest outside a microVM refuses to run and restarts nothing.
#[test]
fn the_guest_refuses_outside_a_microvm() -> Fallible {
    let run = binary_transcript::run(GUEST, &[], &[])?;
    assert_eq!(run.code, Some(64), "{run:#?}");
    assert_eq!(run.stderr, NOT_INIT, "{run:#?}");
    assert!(run.stdout.is_empty(), "{run:#?}");
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: one path short of the four the run takes.
#[test]
fn one_path_short_is_a_usage_error() -> Fallible {
    refused_as_usage(&["run", "a", "b", "c"])
}

/// Boundary: one argument past the five the binary takes.
#[test]
fn one_argument_past_is_a_usage_error() -> Fallible {
    refused_as_usage(&["run", "a", "b", "c", "d", "e"])
}

/// Boundary: a run directory whose socket path passes the 107 bytes `AF_UNIX`
/// allows fails the boot case before any microVM directory is made.
#[test]
fn an_over_long_socket_path_is_refused() -> Fallible {
    let (run, run_dir) = run_without_firecracker(&"r".repeat(100))?;
    assert_eq!(run.code, Some(1), "{run:#?}");
    assert!(run.stdout.contains("AF_UNIX allows 107"), "{run:#?}");
    assert!(!run_dir.join("vm-1").exists(), "{run:#?}");
    Ok(())
}

/// Boundary: arguments do not make the guest act as init.
#[test]
fn the_guest_ignores_its_arguments() -> Fallible {
    let run = binary_transcript::run(GUEST, &["serve"], &[])?;
    assert_eq!(run.code, Some(64), "{run:#?}");
    assert_eq!(run.stderr, NOT_INIT, "{run:#?}");
    Ok(())
}
