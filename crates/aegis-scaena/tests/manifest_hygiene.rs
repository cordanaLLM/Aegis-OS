// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! M27 criteria 1 to 3 and E27-3: the crate's membership, its admitted
//! dependencies, its own dependency closure (D78) and the workspace
//! `rust-version` those dependencies set.
//!
//! The closure is read through `crates/dependency_closure.rs`, the one
//! definition of the D78 rule every re-scoped sweep includes. P17's own sweep
//! carries D02's hash exclusion and the async-runtime exclusion into the new
//! crate, and refuses the engines and transports ADR-0003 keeps out of the
//! first slice.

#[path = "../../dependency_closure.rs"]
mod dependency_closure;

use core::time::Duration;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

use dependency_closure::{
    Invocation, METADATA_ARGS, closure, hits, metadata, own_closure, plant, set, synthetic,
};

/// This crate's package name, the component name (P17).
const PACKAGE: &str = "aegis-scaena";

/// The crates P17's closure must not resolve (M27 criterion 3).
const FORBIDDEN: [&str; 16] = [
    "servo",
    "webrender",
    "mozjs",
    "mozjs_sys",
    "wgpu",
    "wgpu-hal",
    "smithay",
    "wayland-server",
    "tokio",
    "zbus",
    "zenoh",
    "iceoryx2",
    "md5",
    "md-5",
    "blake3",
    "blake2",
];

/// The git revision D80 pins cros-libva at.
const CROS_LIBVA_REV: &str = "59384456ac2ae78c0c3e5515f41ef1efd9b802cf";

/// The workspace root, two directories above this crate's manifest.
fn workspace_root() -> Option<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    Some(manifest.parent()?.parent()?.to_path_buf())
}

/// Reads a workspace file, or the empty string.
fn read(name: &str) -> String {
    workspace_root()
        .and_then(|root| std::fs::read_to_string(root.join(name)).ok())
        .unwrap_or_default()
}

/// The crate directories that hold a manifest.
fn crate_directories() -> Vec<String> {
    let Some(root) = workspace_root() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(root.join("crates")) else {
        return Vec::new();
    };
    let mut found: Vec<String> = entries
        .flatten()
        .take(64)
        .filter(|entry| entry.path().join("Cargo.toml").is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    found.sort_unstable();
    found
}

/// Checks that `manifest` writes out exactly the crate directories that have
/// a manifest, and names the ones it misses or adds.
fn membership(manifest: &str, directories: &[String]) -> Result<(), String> {
    let members: BTreeSet<String> = manifest
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("\"crates/")?.strip_suffix("\","))
        .map(str::to_owned)
        .collect();
    let expected: BTreeSet<String> = directories.iter().cloned().collect();
    if manifest.contains("crates/*") {
        return Err("the member list is globbed".to_owned());
    }
    let missing: Vec<&String> = expected.difference(&members).collect();
    let extra: Vec<&String> = members.difference(&expected).collect();
    if missing.is_empty() && extra.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "members missing {missing:?}, members without a crate {extra:?}"
        ))
    }
}

/// A `rust-version`: major, minor and patch.
type RustVersion = (u64, u64, u64);

/// Parses `1.87` or `1.85.0` into comparable numbers.
fn rust_version(text: &str) -> Option<RustVersion> {
    let mut parts = text.split('.').map(str::parse::<u64>);
    let major = parts.next()?.ok()?;
    let minor = parts.next()?.ok()?;
    let patch = parts.next().unwrap_or(Ok(0)).ok()?;
    Some((major, minor, patch))
}

/// The highest `rust_version` any package outside the workspace declares,
/// with the packages that declare it.
fn highest_declared(metadata: &Value) -> Result<(RustVersion, Vec<String>), String> {
    let members: BTreeSet<&str> = metadata
        .get("workspace_members")
        .and_then(Value::as_array)
        .ok_or("no members")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let declared: Vec<(RustVersion, String)> = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or("no packages")?
        .iter()
        .filter(|package| {
            !members.contains(package.get("id").and_then(Value::as_str).unwrap_or(""))
        })
        .filter_map(|package| {
            let version = rust_version(package.get("rust_version")?.as_str()?)?;
            Some((version, package.get("name")?.as_str()?.to_owned()))
        })
        .collect();
    let highest = declared
        .iter()
        .map(|(version, _)| *version)
        .max()
        .ok_or("none declared")?;
    let names = declared
        .into_iter()
        .filter(|(version, _)| *version == highest)
        .map(|(_, name)| name)
        .collect();
    Ok((highest, names))
}

/// The workspace's own `rust-version`, from the root manifest's text.
fn workspace_rust_version(manifest: &str) -> Option<RustVersion> {
    let line = manifest
        .lines()
        .find(|line| line.starts_with("rust-version = \""))?;
    rust_version(line.strip_prefix("rust-version = \"")?.strip_suffix('"')?)
}

/// The Rust sources of this crate, found with an explicit stack.
fn crate_sources() -> Vec<PathBuf> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut stack = vec![base.join("src"), base.join("tests")];
    let mut sources = vec![base.join("build.rs")];
    for _ in 0..256 {
        let Some(directory) = stack.pop() else { break };
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for path in entries.flatten().take(256).map(|entry| entry.path()) {
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                sources.push(path);
            }
        }
    }
    sources.retain(|path| path.is_file());
    sources
}

/// Whether `line` holds `word` as a whole identifier.
fn holds_word(line: &str, word: &str) -> bool {
    let identifier = |c: char| c.is_ascii_alphanumeric() || c == '_';
    line.match_indices(word).any(|(at, _)| {
        let before = line.get(..at).and_then(|head| head.chars().next_back());
        let after = line
            .get(at.saturating_add(word.len())..)
            .and_then(|tail| tail.chars().next());
        !before.is_some_and(identifier) && !after.is_some_and(identifier)
    })
}

// --- Positive -------------------------------------------------------------

/// Positive (criterion 2): a written-out member with a lock entry and the
/// workspace lints, so the forbid lint on unchecked code applies; the
/// toolchain pin is unchanged.
#[test]
fn the_crate_is_a_written_out_member_with_a_lock_entry_and_workspace_lints() {
    let root = read("Cargo.toml");
    assert!(root.contains("    \"crates/aegis-scaena\",\n"));
    assert!(root.contains("unsafe_code = \"forbid\""));
    assert!(read("Cargo.lock").contains("name = \"aegis-scaena\""));
    let manifest = read("crates/aegis-scaena/Cargo.toml");
    assert!(manifest.contains("name = \"aegis-scaena\""));
    assert!(manifest.contains("[lints]\nworkspace = true"));
    assert!(!manifest.contains("[lints.rust]") && !manifest.contains("[lints.clippy]"));
    assert!(manifest.contains("publish = false"));
    assert!(read("rust-toolchain.toml").contains("channel = \"1.98.1\""));
}

/// Positive (criterion 1): the three D80 crates are declared once, in the
/// workspace table, used by reference here, and locked at the admitted
/// versions, cros-libva from its git revision.
#[test]
fn the_admitted_crates_are_declared_once_and_locked() {
    let root = read("Cargo.toml");
    for declaration in [
        format!(
            "cros-libva = {{ git = \"https://github.com/chromeos/cros-libva\", rev = \"{CROS_LIBVA_REV}\" }}"
        ),
        "smithay-client-toolkit = { version = \"0.21.1\", default-features = false }".to_owned(),
        "rustix = { version = \"1.1.5\", features = [\"net\", \"fs\", \"event\"] }".to_owned(),
    ] {
        assert!(root.contains(&declaration), "{declaration}");
    }
    let manifest = read("crates/aegis-scaena/Cargo.toml");
    for name in ["cros-libva", "rustix", "smithay-client-toolkit"] {
        assert_eq!(root.matches(&format!("\n{name} = ")).count(), 1, "{name}");
        assert!(
            manifest.contains(&format!("{name}.workspace = true")),
            "{name}"
        );
    }
    let lock = read("Cargo.lock");
    for entry in [
        format!(
            "name = \"cros-libva\"\nversion = \"0.0.13\"\nsource = \"git+https://github.com/chromeos/cros-libva?rev={CROS_LIBVA_REV}#{CROS_LIBVA_REV}\""
        ),
        "name = \"smithay-client-toolkit\"\nversion = \"0.21.1\"".to_owned(),
        "name = \"rustix\"\nversion = \"1.1.5\"".to_owned(),
        "name = \"bindgen\"\nversion = \"0.70.1\"".to_owned(),
    ] {
        assert!(lock.contains(&entry), "{entry}");
    }
}

/// Positive (criterion 1): `cargo tree --locked -p aegis-scaena` shows
/// cros-libva's git source at the pinned revision.
#[test]
fn cargo_tree_shows_the_git_source_at_the_pinned_revision() -> Result<(), String> {
    let root = workspace_root().ok_or("no workspace root")?;
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let out = std::env::temp_dir().join(format!("aegis-scaena-tree-{}.txt", std::process::id()));
    let file = std::fs::File::create(&out).map_err(|error| error.to_string())?;
    let mut child = Command::new(cargo)
        .args([
            "tree",
            "--locked",
            "--offline",
            "-p",
            PACKAGE,
            "--depth",
            "1",
        ])
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(file)
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| error.to_string())?;
    let mut status = None;
    for _ in 0..4_800 {
        status = child.try_wait().map_err(|error| error.to_string())?;
        if status.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    if status.is_none() {
        let _ = child.kill().and_then(|()| child.wait());
        return Err("cargo tree missed its 120 s deadline".to_owned());
    }
    let tree = std::fs::read_to_string(&out).map_err(|error| error.to_string())?;
    let _ = std::fs::remove_file(&out);
    let expected = format!(
        "cros-libva v0.0.13 (https://github.com/chromeos/cros-libva?rev={CROS_LIBVA_REV}#{})",
        CROS_LIBVA_REV.get(..8).unwrap_or_default()
    );
    assert!(tree.contains(&expected), "{expected} not in:\n{tree}");
    Ok(())
}

/// Positive (criterion 3): the real closure holds the admitted crates and
/// none of the sixteen P17 refuses.
#[test]
fn the_closure_holds_the_admitted_crates_and_nothing_forbidden() -> Result<(), String> {
    let names = own_closure(PACKAGE)?;
    for admitted in [
        PACKAGE,
        "cros-libva",
        "smithay-client-toolkit",
        "rustix",
        "wayland-client",
    ] {
        assert!(names.contains(admitted), "{admitted} is not in the closure");
    }
    let found = hits(&names, &FORBIDDEN);
    assert!(
        found.is_empty(),
        "the closure of {PACKAGE} resolves {found:?}"
    );
    Ok(())
}

/// Positive (criterion 3): the closure is read with exactly the invocation D78
/// records -- `cargo metadata --format-version 1 --locked --offline
/// --all-features` -- and without `--filter-platform`, both in the shared
/// constant and in the invocation every sweep runs.
#[test]
fn the_closure_is_read_with_the_invocation_d78_records() -> Result<(), String> {
    let recorded = [
        "metadata",
        "--format-version",
        "1",
        "--locked",
        "--offline",
        "--all-features",
    ];
    assert_eq!(METADATA_ARGS, recorded);
    let invocation = Invocation::recorded()?;
    assert_eq!(invocation.args, recorded);
    assert!(
        invocation
            .args
            .iter()
            .all(|arg| !arg.starts_with("--filter-platform")),
        "{:?}",
        invocation.args
    );
    Ok(())
}

/// Positive (criterion 9): no engine or high-rate transport ADR-0003 keeps
/// out of the first slice -- Servo, webrender, `SpiderMonkey`, wgpu, iceoryx2 --
/// has any entry in `Cargo.lock`, names matched exactly.
#[test]
fn no_engine_or_transport_crate_is_locked_anywhere() {
    let lock = read("Cargo.lock");
    let locked: BTreeSet<&str> = lock
        .lines()
        .filter_map(|line| line.strip_prefix("name = \"")?.strip_suffix('"'))
        .collect();
    assert!(locked.contains(PACKAGE) && locked.contains("cros-libva"));
    let engines = [
        "servo",
        "webrender",
        "mozjs",
        "mozjs_sys",
        "wgpu",
        "wgpu-hal",
        "iceoryx2",
    ];
    let found: Vec<&str> = engines
        .into_iter()
        .filter(|name| locked.contains(name))
        .collect();
    assert!(found.is_empty(), "Cargo.lock resolves {found:?}");
}

/// Positive (criterion 2): the workspace `rust-version` equals the highest
/// `rust-version` any resolved package declares, read from `cargo metadata`.
#[test]
fn the_workspace_rust_version_is_the_highest_the_graph_declares() -> Result<(), String> {
    let (highest, names) = highest_declared(&metadata()?)?;
    let declared = workspace_rust_version(&read("Cargo.toml")).ok_or("no rust-version")?;
    assert_eq!(declared, highest, "declared by {names:?}");
    assert!(names.iter().any(|name| name == "accesskit"), "{names:?}");
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative (criterion 2): no source of this crate holds the keyword the
/// workspace's forbid lint refuses, outside comment lines.
#[test]
fn no_source_holds_the_forbidden_keyword() {
    let keyword = concat!("un", "safe");
    let sources = crate_sources();
    assert!(sources.len() >= 8, "{sources:?}");
    for path in &sources {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        for (number, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            assert!(
                !holds_word(line, keyword),
                "{}:{}",
                path.display(),
                number.saturating_add(1)
            );
        }
    }
    assert!(
        holds_word(&format!("{keyword} {{"), keyword),
        "the scan finds the word"
    );
    assert!(
        !holds_word(&format!("{keyword}_code"), keyword),
        "and only the word"
    );
}

/// Negative (M27 criterion 5): Aegis code never maps a file. No source of
/// this crate names a mapping call or crate outside comment lines, and the
/// rustix features the workspace admits leave out `mm`, so the received
/// DMA-BUF can only be handed on; the compositor's format table is read with
/// `pread`.
#[test]
fn no_source_maps_a_file() {
    let words = [
        concat!("mm", "ap"),
        concat!("mem", "map2"),
        concat!("Mm", "ap"),
        concat!("MmapOp", "tions"),
    ];
    for path in &crate_sources() {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        for (number, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for word in words {
                assert!(
                    !holds_word(line, word),
                    "{}:{} names {word}",
                    path.display(),
                    number.saturating_add(1)
                );
            }
        }
    }
    let root = read("Cargo.toml");
    let rustix = root
        .lines()
        .find(|line| line.starts_with("rustix = "))
        .unwrap_or_default();
    assert!(!rustix.contains("\"mm\""), "{rustix}");
}

/// Positive (M27 criterion 9): the layer surface asks for no keyboard focus,
/// once, and asks for nothing else.
#[test]
fn the_layer_surface_takes_no_keyboard_focus() {
    let present = read("crates/aegis-scaena/src/present.rs");
    assert_eq!(present.matches("set_keyboard_interactivity(").count(), 1);
    assert!(present.contains("set_keyboard_interactivity(KeyboardInteractivity::None)"));
}

/// Negative (E27-3): a forbidden crate planted in this closure, as a dev or
/// a build dependency, fails the sweep.
#[test]
fn a_planted_forbidden_crate_fails_the_sweep() -> Result<(), String> {
    let mut planted = metadata()?;
    plant(&mut planted, PACKAGE, "tokio", Some("dev"))?;
    plant(&mut planted, PACKAGE, "wgpu", Some("build"))?;
    let found = hits(&closure(&planted, PACKAGE)?, &FORBIDDEN);
    assert_eq!(found, vec!["wgpu", "tokio"]);
    Ok(())
}

/// Negative (E27-3): a failing, timed-out, unspawnable or unparsable
/// `cargo metadata` fails closed, and the non-zero exit names the cure.
#[test]
fn a_failing_or_timed_out_metadata_run_fails_closed() -> Result<(), String> {
    let recorded = Invocation::recorded()?;
    let mut failing = recorded.clone();
    failing.args.extend([
        "--manifest-path".to_owned(),
        "/nonexistent/Cargo.toml".to_owned(),
    ]);
    let error = failing.run().err().ok_or("a failing run passed")?;
    assert!(error.contains("cargo fetch --locked"), "{error}");
    let timed_out = Invocation {
        deadline: Duration::ZERO,
        ..recorded.clone()
    };
    let error = timed_out.run().err().ok_or("a timed-out run passed")?;
    assert!(error.contains("deadline"), "{error}");
    let unspawnable = Invocation {
        program: "/nonexistent/cargo".into(),
        ..recorded.clone()
    };
    let error = unspawnable.run().err().ok_or("an unspawnable run passed")?;
    assert!(error.contains("could not be spawned"), "{error}");
    let unparsable = Invocation {
        args: vec!["--version".to_owned()],
        ..recorded
    };
    let error = unparsable.run().err().ok_or("an unparsable run passed")?;
    assert!(error.contains("not JSON"), "{error}");
    Ok(())
}

/// Negative (E27-3): a node-less document fails closed: a null resolve, no
/// start node, or two.
#[test]
fn a_node_less_metadata_document_fails_closed() {
    let mut null = synthetic(PACKAGE, &[]);
    set(&mut null, "resolve", Value::Null);
    assert!(closure(&null, PACKAGE).is_err());
    let mut none = synthetic(PACKAGE, &[]);
    set(&mut none, "workspace_members", serde_json::json!([]));
    assert!(closure(&none, PACKAGE).is_err());
    let mut two = synthetic(PACKAGE, &[]);
    set(
        &mut two,
        "workspace_members",
        serde_json::json!(["member", "member"]),
    );
    assert!(closure(&two, PACKAGE).is_err());
}

/// Negative (E27-3): a manifest that omits aegis-scaena from the written-out
/// member list fails the membership check, which the real manifest passes.
#[test]
fn a_manifest_that_omits_this_crate_fails_the_membership_check() -> Result<(), String> {
    let root = read("Cargo.toml");
    let directories = crate_directories();
    membership(&root, &directories)?;
    let omitted = root.replace("    \"crates/aegis-scaena\",\n", "");
    assert_ne!(omitted, root);
    let error = membership(&omitted, &directories)
        .err()
        .ok_or("the omission passed")?;
    assert!(error.contains("aegis-scaena"), "{error}");
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary (E27-3): a `rust-version` one release above the graph's highest
/// is not equal to it, and neither is one below.
#[test]
fn the_rust_version_is_not_one_release_more() -> Result<(), String> {
    let (highest, _) = highest_declared(&metadata()?)?;
    let (major, minor, patch) = highest;
    for text in [
        format!("rust-version = \"{major}.{}\"", minor.saturating_add(1)),
        format!("rust-version = \"{major}.{}\"", minor.saturating_sub(1)),
    ] {
        assert_ne!(workspace_rust_version(&text), Some(highest), "{text}");
    }
    let exact = format!("rust-version = \"{major}.{minor}.{patch}\"");
    assert_eq!(workspace_rust_version(&exact), Some(highest));
    Ok(())
}

/// Boundary (E27-3): names match exactly -- wayland-client is admitted while
/// wayland-server is refused, tokio-macros is not tokio, and both MD5
/// spellings are refused.
#[test]
fn names_match_exactly() -> Result<(), String> {
    let admitted = synthetic(
        PACKAGE,
        &["wayland-client", "tokio-macros", "blake3-sys", "md5-asm"],
    );
    assert!(hits(&closure(&admitted, PACKAGE)?, &FORBIDDEN).is_empty());
    let refused = synthetic(PACKAGE, &["wayland-server", "md-5", "md5"]);
    assert_eq!(
        hits(&closure(&refused, PACKAGE)?, &FORBIDDEN),
        vec!["wayland-server", "md5", "md-5"]
    );
    Ok(())
}
