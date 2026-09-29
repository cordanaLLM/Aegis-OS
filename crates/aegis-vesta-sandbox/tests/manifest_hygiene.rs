// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The crate's membership, its admitted dependencies and its own closure
//! (D78).
//!
//! `socket2` is the one crate M21 adds to the lock: its `SockAddr::vsock` is
//! the safe constructor of an `AF_VSOCK` address, where `rustix` 1.1.5 offers
//! none and the workspace forbids `unsafe`. It is declared once in
//! `[workspace.dependencies]`. `aegis-vesta`'s own closure stays free of it and
//! of `rustix`: the table and the payloads still start nothing.

#[path = "../../dependency_closure.rs"]
mod dependency_closure;

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

/// Reads a file relative to the workspace root.
fn read(name: &str) -> String {
    dependency_closure::workspace_root()
        .and_then(|root| std::fs::read_to_string(root.join(name)).ok())
        .unwrap_or_default()
}

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

/// Positive: socket2 is declared once, at the pinned version, with `all`.
#[test]
fn socket2_is_declared_once_and_locked() {
    let root = read("Cargo.toml");
    assert!(root.contains(
        "socket2 = { version = \"0.6.5\", default-features = false, features = [\"all\"] }"
    ));
    assert!(read("crates/aegis-vesta-sandbox/Cargo.toml").contains("socket2.workspace = true"));
    let lock = read("Cargo.lock");
    assert!(lock.contains("name = \"socket2\"\nversion = \"0.6.5\""));
}

// --- Negative -------------------------------------------------------------

/// Negative (D78): this closure resolves no async runtime, monitor binding or
/// engine.
#[test]
fn the_closure_resolves_no_runtime_or_binding() -> Result<(), String> {
    let names = dependency_closure::own_closure(PACKAGE)?;
    let found = dependency_closure::hits(&names, &FORBIDDEN);
    assert!(
        found.is_empty(),
        "the closure of {PACKAGE} resolves {found:?}"
    );
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
