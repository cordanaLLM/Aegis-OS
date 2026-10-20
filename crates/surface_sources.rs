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
pub const INCLUDE_BOUND: usize = 32;

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
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let Some((canonical, name, body)) = read_source_file(&path, visited) else {
                continue;
            };
            if let Some(parent) = canonical.parent() {
                scan_includes(&body, parent, queue);
            }
            out.push((name, body));
        }
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
