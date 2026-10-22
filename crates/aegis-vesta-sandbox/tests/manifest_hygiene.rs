// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The crate's membership, its admitted dependencies and its own closure
//! (D78).
//!
//! `socket2` is the one crate M21 added to the lock: its `SockAddr::vsock` is
//! the safe constructor of an `AF_VSOCK` address, where `rustix` 1.1.5 offers
//! none and the workspace forbids `unsafe`. Since 2026-10-16 the code that
//! uses it is `saeptum-microvm` (cut unit CUT-1632-04), declared once in
//! `[workspace.dependencies]` at a git revision, and socket2 arrives through
//! it. `aegis-vesta`'s own closure stays free of it and of `rustix`: the table
//! and the payloads still start nothing.

#[path = "../../dependency_closure.rs"]
mod dependency_closure;

#[path = "../../manifest_hygiene.rs"]
mod manifest_hygiene;

use manifest_hygiene::read;

/// This crate's package name, the start of its closure.
const PACKAGE: &str = "aegis-vesta-sandbox";

/// Crates this closure must not resolve: the async runtimes, the monitor
/// bindings and the engines no M21 step uses.
const FORBIDDEN: [&str; 9] = [
    "tokio",
    "nix",
    "vsock",
    "tokio-vsock",
    "kvm-ioctls",
    "firecracker",
    "wasmtime",
    "wasmer",
    "vm-memory",
];

// --- Positive -------------------------------------------------------------

/// Positive: the crate is a written-out member with a lock entry and two
/// binaries, the host run and the guest init.
#[test]
fn the_crate_is_a_locked_member_with_two_binaries() {
    assert!(read("Cargo.toml").contains("\"crates/aegis-vesta-sandbox\""));
    assert!(read("Cargo.lock").contains("name = \"aegis-vesta-sandbox\""));
    let manifest = read("crates/aegis-vesta-sandbox/Cargo.toml");
    assert_eq!(manifest.matches("[[bin]]").count(), 2);
    assert!(manifest.contains("name = \"aegis-vesta-sandbox\""));
    assert!(manifest.contains("name = \"aegis-vesta-guest\""));
    assert!(manifest.contains("[lints]\nworkspace = true"));
}

/// Positive: the microVM half is `saeptum-microvm`, declared once at a git
/// revision, and socket2 reaches this closure only through it, still locked at
/// the admitted version.
#[test]
fn saeptum_microvm_is_pinned_and_carries_socket2() {
    let root = read("Cargo.toml");
    assert!(
        root.contains("saeptum-microvm = { git = \"https://github.com/plurano/saeptum\", rev = \"")
    );
    assert!(!root.contains("socket2 ="));
    let manifest = read("crates/aegis-vesta-sandbox/Cargo.toml");
    assert!(manifest.contains("saeptum-microvm.workspace = true"));
    assert!(!manifest.contains("socket2"));
    let lock = read("Cargo.lock");
    assert!(lock.contains("name = \"socket2\"\nversion = \"0.6.5\""));
    assert!(lock.contains("name = \"saeptum-microvm\"\nversion = \"0.1.0\"\nsource = \"git+https://github.com/plurano/saeptum?rev="));
}

// --- Negative -------------------------------------------------------------

/// Negative (D78): this closure resolves no async runtime, monitor binding or
/// engine.
#[test]
fn the_closure_resolves_no_runtime_or_binding() -> Result<(), String> {
    let names = dependency_closure::assert_closure_forbids(PACKAGE, &FORBIDDEN)?;
    assert!(names.contains("socket2"));
    Ok(())
}

/// Negative: `aegis-vesta` itself still resolves neither socket crate.
#[test]
fn the_table_crate_stays_free_of_sockets() -> Result<(), String> {
    let names = dependency_closure::own_closure("aegis-vesta")?;
    let found = dependency_closure::hits(&names, &["socket2", "rustix"]);
    assert!(found.is_empty(), "aegis-vesta resolves {found:?}");
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the lists are their recorded lengths.
#[test]
fn the_lists_are_their_recorded_lengths() {
    assert_eq!(FORBIDDEN.len(), 9);
    let manifest = read("crates/aegis-vesta-sandbox/Cargo.toml");
    assert!(!manifest.contains("[build-dependencies]"));
    assert!(manifest.contains("publish = false"));
}
