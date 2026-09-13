// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fixtures shared by the integration tests.
//!
//! Everything here reaches `aegis-fabrica-defs` through its public API only,
//! and nothing here uses `unwrap` or `expect`: a test that cannot build its
//! own input returns the failure instead of panicking through a lint the
//! workspace denies.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// What every test in this suite returns.
pub type Fallible = Result<(), Box<dyn std::error::Error>>;

/// Scalar bound on the reviewed definitions a test may read.
pub const MAX_REVIEWED: usize = 32;

/// The repository root, two levels above this crate.
#[must_use]
pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Reads one reviewed definition from `build/`, returning its name and text.
///
/// # Errors
///
/// Returns the I/O error when the reviewed file cannot be read, so a renamed
/// or deleted definition fails the suite instead of silently skipping it.
pub fn reviewed(relative: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    let path = repository_root().join("build").join(relative);
    let text = std::fs::read_to_string(&path)?;
    let name = path.file_name().map_or_else(
        || relative.to_owned(),
        |raw| raw.to_string_lossy().into_owned(),
    );
    Ok((name, text))
}

/// Reads every reviewed file in `build/<directory>` with `extension`.
///
/// Both shipped sets are swept by directory rather than named file by file,
/// so a definition added later is covered by the suite the moment it lands
/// instead of shipping untested until someone remembers to name it.
///
/// # Errors
///
/// Returns the I/O error when the directory cannot be listed or read.
fn reviewed_directory(
    directory: &str,
    extension: &str,
) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let base = repository_root().join("build").join(directory);
    let mut names: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&base)? {
        let path = entry?.path();
        if path.extension().is_none_or(|ext| ext != extension) {
            continue;
        }
        names.push(
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
        );
    }
    names.sort();
    names.truncate(MAX_REVIEWED);
    let mut out: Vec<(String, String)> = Vec::new();
    for name in names {
        let text = std::fs::read_to_string(base.join(&name))?;
        out.push((name, text));
    }
    Ok(out)
}

/// Reads every reviewed repart drop-in, sorted by file name.
///
/// # Errors
///
/// Returns the I/O error when `build/repart.d` cannot be listed or read.
pub fn reviewed_repart_set() -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    reviewed_directory("repart.d", "conf")
}

/// Reads every reviewed sysupdate transfer, sorted by file name.
///
/// The mirror of [`reviewed_repart_set`]. `build/sysupdate.d` holds one
/// transfer today and its header records two more as M11 work; sweeping the
/// directory means those arrive covered rather than unnamed and untested.
///
/// # Errors
///
/// Returns the I/O error when `build/sysupdate.d` cannot be listed or read.
pub fn reviewed_transfer_set() -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    reviewed_directory("sysupdate.d", "transfer")
}

/// Reads one reviewed payload from `build/`, returning its text.
///
/// # Errors
///
/// Returns the I/O error when the reviewed payload cannot be read, so a
/// renamed or deleted payload fails the suite instead of silently skipping it.
pub fn reviewed_payload(relative: &str) -> Result<String, Box<dyn std::error::Error>> {
    Ok(std::fs::read_to_string(
        repository_root().join("build").join(relative),
    )?)
}

/// Reads the recorded reference profile document.
///
/// The tests that use it are checking a schema against a measured machine, so
/// a missing profile is a failure rather than a skipped case.
///
/// # Errors
///
/// Returns the I/O error when `planning/hardware-profile.json` cannot be read.
pub fn reference_profile_text() -> Result<String, Box<dyn std::error::Error>> {
    Ok(std::fs::read_to_string(
        repository_root()
            .join("planning")
            .join("hardware-profile.json"),
    )?)
}
