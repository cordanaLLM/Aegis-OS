// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! D116: shared source walker for public-surface sweeps.
//!
//! One file, included by path into the public-surface sweep of every crate
//! (`#[path = "../../surface_sources.rs"]`), extending the D78 and D116 precedents
//! to surface scanners. It is not a crate and has no manifest: a workspace
//! member would enter every dependency closure.
//!
//! The walker traverses a source directory up to scalar bounds and additionally
//! follows `#[path = "..."]` attributes to collect external shared sources such as
//! `crates/contract_scaffold.rs` that are path-included into the crate's modules.
//! Targets are resolved relative to the enclosing file's directory, canonicalised,
//! deduplicated, bounded by a scalar include limit, and read without panicking.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// Scalar bound on directories visited and entries read per directory.
const WALK_BOUND: usize = 64;

/// Scalar bound on path-included source files followed.
const INCLUDE_BOUND: usize = 32;

/// Scalar bound on lines scanned in any one source file.
const LINE_BOUND: usize = 4096;

/// Extracts the path from a `#[path = "..."]` or `#![path = "..."]` attribute on `line`.
fn extract_path_attr(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.starts_with("//") {
        return None;
    }
    let after_open = if let Some(idx) = trimmed.find("#[") {
        trimmed.get(idx.saturating_add(2)..)?
    } else {
        let idx = trimmed.find("#![")?;
        trimmed.get(idx.saturating_add(3)..)?
    };
    let rest = after_open.trim_start().strip_prefix("path")?.trim_start();
    let rest = rest.strip_prefix('=')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    rest.get(..end)
}

/// Scans lines of `body` for `#[path = "..."]` attributes, adding targets to `queue`.
fn scan_includes(body: &str, parent: &Path, queue: &mut Vec<PathBuf>) {
    for line in body.lines().take(LINE_BOUND) {
        if queue.len() >= INCLUDE_BOUND {
            break;
        }
        let Some(rel) = extract_path_attr(line) else {
            continue;
        };
        queue.push(parent.join(rel));
    }
}

/// Reads one canonical source file into memory if not already visited.
fn read_source_file(path: &Path, visited: &mut Vec<PathBuf>) -> Option<(PathBuf, String, String)> {
    let canonical = std::fs::canonicalize(path).ok()?;
    if visited.contains(&canonical) {
        return None;
    }
    visited.push(canonical.clone());
    let body = std::fs::read_to_string(&canonical).ok()?;
    let name = match canonical.file_name() {
        Some(raw) => raw.to_string_lossy().into_owned(),
        None => String::new(),
    };
    Some((canonical, name, body))
}

/// Reads directory entries up to bounds, queueing subdirectories and path includes.
fn walk_dir_entries(
    directory: &Path,
    stack: &mut Vec<PathBuf>,
    visited: &mut Vec<PathBuf>,
    queue: &mut Vec<PathBuf>,
    out: &mut Vec<(String, String)>,
) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten().take(WALK_BOUND) {
        let path = entry.path();
        if path.is_dir() {
            stack.push(path);
            continue;
        }
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let Some((canonical, name, body)) = read_source_file(&path, visited) else {
            continue;
        };
        if let Some(parent) = canonical.parent() {
            scan_includes(&body, parent, queue);
        }
        out.push((name, body));
    }
}

/// Processes queued path includes up to scalar bounds, following chained includes.
fn process_includes(
    queue: &mut Vec<PathBuf>,
    visited: &mut Vec<PathBuf>,
    out: &mut Vec<(String, String)>,
) {
    let mut chained = Vec::new();
    for _ in 0..INCLUDE_BOUND {
        if queue.is_empty() {
            break;
        }
        let target = queue.remove(0);
        let Some((canonical, name, body)) = read_source_file(&target, visited) else {
            continue;
        };
        if let Some(parent) = canonical.parent() {
            scan_includes(&body, parent, &mut chained);
        }
        out.push((name, body));
        for next in chained.drain(..) {
            if queue.len() < INCLUDE_BOUND {
                queue.push(next);
            }
        }
    }
}

/// Returns the `.rs` files under `root` and path-included sources, as (file name, body) pairs.
///
/// The walk is iterative and doubly bounded: at most [`WALK_BOUND`] directories
/// and at most [`WALK_BOUND`] entries per directory. Any `#[path = "..."]` attributes
/// are followed up to [`INCLUDE_BOUND`] targets, resolved relative to the enclosing
/// file's directory, canonicalised, deduplicated, and read without panicking.
pub fn rust_sources(root: &Path) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    let mut visited: Vec<PathBuf> = Vec::new();
    let mut include_queue: Vec<PathBuf> = Vec::new();

    for _ in 0..WALK_BOUND {
        let Some(directory) = stack.pop() else { break };
        walk_dir_entries(
            &directory,
            &mut stack,
            &mut visited,
            &mut include_queue,
            &mut out,
        );
    }

    process_includes(&mut include_queue, &mut visited, &mut out);

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TempFixture {
        path: PathBuf,
    }

    impl TempFixture {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let count = COUNTER.fetch_add(1, Ordering::Relaxed);
            let pid = std::process::id();
            let name = format!("aegis_surface_sources_test_{label}_{pid}_{count}");
            let path = std::env::temp_dir().join(name);
            let _ = std::fs::remove_dir_all(&path);
            let _ = std::fs::create_dir_all(&path);
            Self { path }
        }

        /// Creates and returns the fixture's `src/` and `shared/` directories.
        fn src_and_shared(&self) -> (PathBuf, PathBuf) {
            let src = self.path.join("src");
            let shared = self.path.join("shared");
            assert!(std::fs::create_dir_all(&src).is_ok());
            assert!(std::fs::create_dir_all(&shared).is_ok());
            (src, shared)
        }
    }

    impl Drop for TempFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn positive_path_included_source_returned() {
        let fixture = TempFixture::new("positive");
        let (src, shared) = fixture.src_and_shared();

        let lib_path = src.join("lib.rs");
        let external_path = shared.join("external.rs");
        assert!(
            std::fs::write(
                &lib_path,
                "#[path = \"../shared/external.rs\"]\nmod external;\n"
            )
            .is_ok()
        );
        assert!(std::fs::write(&external_path, "pub fn unique_external_symbol() {}\n").is_ok());

        let sources = rust_sources(&src);
        let found = sources
            .iter()
            .any(|(name, body)| name == "external.rs" && body.contains("unique_external_symbol"));
        assert!(found, "path-included source was not collected");
    }

    #[test]
    fn negative_missing_include_target_skipped_no_panic() {
        let fixture = TempFixture::new("negative");
        let src = fixture.path.join("src");
        assert!(std::fs::create_dir_all(&src).is_ok());

        let lib_path = src.join("lib.rs");
        assert!(
            std::fs::write(
                &lib_path,
                "#[path = \"../missing_file.rs\"]\nmod missing;\n"
            )
            .is_ok()
        );

        let sources = rust_sources(&src);
        assert_eq!(sources.len(), 1);
        let first_name = match sources.first() {
            Some((name, _)) => name.as_str(),
            None => "",
        };
        assert_eq!(first_name, "lib.rs");
    }

    #[test]
    fn boundary_include_chain_hits_bound_and_stops() {
        let fixture = TempFixture::new("chain_bound");
        let dir = fixture.path.join("chain");
        assert!(std::fs::create_dir_all(&dir).is_ok());

        let chain_len = INCLUDE_BOUND.saturating_add(10);
        for i in 0..chain_len {
            let file_path = dir.join(format!("file_{i}.rs"));
            let next_i = i.saturating_add(1);
            let content = format!("#[path = \"file_{next_i}.rs\"]\nmod next;\n");
            assert!(std::fs::write(&file_path, content).is_ok());
        }
        let last_path = dir.join(format!("file_{chain_len}.rs"));
        assert!(std::fs::write(&last_path, "pub fn final_chain_target() {}\n").is_ok());

        let root_dir = fixture.path.join("root");
        assert!(std::fs::create_dir_all(&root_dir).is_ok());
        let root_file = root_dir.join("lib.rs");
        assert!(
            std::fs::write(&root_file, "#[path = \"../chain/file_0.rs\"]\nmod chain;\n").is_ok()
        );

        let sources = rust_sources(&root_dir);
        let bound_limit = INCLUDE_BOUND.saturating_add(1);
        assert!(
            sources.len() <= bound_limit,
            "sources length {} exceeded bound {}",
            sources.len(),
            bound_limit
        );
        let reached_final = sources
            .iter()
            .any(|(name, _)| name == &format!("file_{chain_len}.rs"));
        assert!(
            !reached_final,
            "include chain did not stop at INCLUDE_BOUND"
        );
    }

    #[test]
    fn boundary_duplicate_include_read_once() {
        let fixture = TempFixture::new("dedupe");
        let (src, shared) = fixture.src_and_shared();

        let a_path = src.join("a.rs");
        let b_path = src.join("b.rs");
        let shared_path = shared.join("dup.rs");
        assert!(std::fs::write(&a_path, "#[path = \"../shared/dup.rs\"]\nmod dup;\n").is_ok());
        assert!(std::fs::write(&b_path, "#[path = \"../shared/dup.rs\"]\nmod dup;\n").is_ok());
        assert!(std::fs::write(&shared_path, "pub fn dup_symbol() {}\n").is_ok());

        let sources = rust_sources(&src);
        let dup_count = sources.iter().filter(|(name, _)| name == "dup.rs").count();
        assert_eq!(
            dup_count, 1,
            "duplicate include target was not deduplicated"
        );
    }
}
