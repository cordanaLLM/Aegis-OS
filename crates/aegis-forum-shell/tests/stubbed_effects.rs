// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! M16's scope criterion, as a sweep: the crate paints nothing, creates no
//! window or surface, opens no real socket and contacts no D-Bus daemon.
//!
//! The sweep reads every line of code under `src/` -- comments excluded,
//! because the documentation names the very things the crate does not do --
//! and fails on any identifier that would be needed to do one of them. It is
//! a regression gate over an enumeration, not a proof over every such
//! identifier; the dependency closure test holds the other half, that no
//! crate able to do them is linked at all. The D32 endpoint appears only as
//! the string constant `endpoint.rs` records, and nothing opens it.

use std::path::Path;

/// Identifiers that would open a socket, reach a bus, start a process or a
/// thread, read the clock, touch the file system, or paint.
const EFFECTS: [&str; 24] = [
    "UnixStream",
    "UnixListener",
    "UnixDatagram",
    "TcpStream",
    "TcpListener",
    "UdpSocket",
    "zbus",
    "dbus",
    "atspi",
    "accesskit_unix",
    "gpui",
    "winit",
    "wayland",
    "tokio",
    "Command::new",
    "thread::spawn",
    "thread::sleep",
    "SystemTime",
    "Instant",
    "std::fs",
    "File::",
    "OpenOptions",
    "unsafe",
    "COMPOSITOR_ENDPOINT)",
];

/// Every `*.rs` file under `dir`, found with an explicit stack.
fn sources(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    for _ in 0..256 {
        let Some(next) = stack.pop() else { break };
        let Ok(entries) = std::fs::read_dir(&next) else {
            continue;
        };
        for entry in entries.flatten().take(256) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// The `"<file>:<line>: <identifier>"` hits in the code lines of `root`.
fn hits(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for path in sources(root) {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for (number, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for effect in EFFECTS.iter().filter(|effect| line.contains(**effect)) {
                out.push(format!(
                    "{}:{}: {effect}",
                    path.display(),
                    number.saturating_add(1)
                ));
            }
        }
    }
    out
}

/// Positive: the crate's sources name no effect.
#[test]
fn the_sources_name_no_effect() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let files = sources(&root);
    assert!(
        files.len() >= 16,
        "the sweep found only {} files",
        files.len()
    );
    assert_eq!(hits(&root), Vec::<String>::new());
}

/// Negative: the sweep finds an effect when one is planted in code, and not
/// when the same word sits in a comment.
#[test]
fn the_sweep_finds_a_planted_effect() -> Result<(), Box<dyn std::error::Error>> {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("forum-shell-sweep");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("planted.rs"),
        "// UnixStream in a comment\nlet s = UnixStream::pair();\n",
    )?;
    let found = hits(&dir);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found
            .first()
            .is_some_and(|hit| hit.ends_with(":2: UnixStream"))
    );
    Ok(())
}
