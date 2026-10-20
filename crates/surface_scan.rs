// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! D116: shared scanner core for public-surface sweeps.
//!
//! One file, included by path into the public-surface sweep of every crate
//! (`#[path = "../../surface_scan.rs"] mod surface_scan;`), extending D78 and D116 precedents
//! to surface scanners. It is not a crate and has no manifest: a workspace
//! member would enter every dependency closure.

#![allow(dead_code)]

use std::path::Path;

#[path = "surface_sources.rs"]
mod surface_sources;
pub use surface_sources::rust_sources;

/// Scalar bound on lines scanned in any one source file.
pub const LINE_BOUND: usize = 4096;

/// Which enclosing block makes an unmarked item public API.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Block {
    /// Not inside a public enum or trait.
    Outside,
    /// Inside a `pub enum`: its direct children are variants.
    Enum,
    /// Inside a `pub trait`: its direct children are trait items.
    Trait,
}

/// Returns the identifier that starts `text`, if it starts with one.
pub fn leading_identifier(text: &str) -> Option<String> {
    let name: String = text
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    let first = name.chars().next()?;
    if first.is_ascii_digit() {
        return None;
    }
    Some(name)
}

/// Returns the public struct field `text` declares, once `pub ` is stripped.
///
/// A field is the one public declaration that introduces its name without a
/// keyword: `pub schema: String,`. The colon after the identifier is what
/// distinguishes it, because no keyword-introduced item may carry one there,
/// and `::` is excluded so that a `pub use crate::..` path is not read as a
/// field. `pub(crate)` never reaches here: it has no space after `pub`.
///
/// The M15 verification found this arm missing. `TraceHeader`, `TraceLine`,
/// `DecisionRecord` and `Citation` are all-public-field structs, so the whole
/// rendered form of a trace sat outside the sweep, and a planted
/// `pub untested_knob: u8` passed.
pub fn field_name(after_pub: &str) -> Option<String> {
    let name = leading_identifier(after_pub)?;
    let tail = after_pub.get(name.len()..)?.trim_start();
    if tail.starts_with("::") || !tail.starts_with(':') {
        return None;
    }
    Some(name)
}

/// Returns the name a `pub` declaration on `text` introduces, optionally including fields and statics.
pub fn parse_declared_name(text: &str, include_fields_and_statics: bool) -> Option<String> {
    let after_pub = text.strip_prefix("pub ")?;
    let rest = after_pub
        .strip_prefix("const fn ")
        .or_else(|| after_pub.strip_prefix("unsafe fn "))
        .or_else(|| after_pub.strip_prefix("fn "))
        .or_else(|| after_pub.strip_prefix("struct "))
        .or_else(|| after_pub.strip_prefix("enum "))
        .or_else(|| after_pub.strip_prefix("trait "))
        .or_else(|| after_pub.strip_prefix("type "))
        .or_else(|| after_pub.strip_prefix("const "));
    let rest = if include_fields_and_statics {
        rest.or_else(|| after_pub.strip_prefix("static "))
    } else {
        rest
    };
    match rest {
        Some(rest) => leading_identifier(rest.trim_start()),
        None => {
            if include_fields_and_statics {
                field_name(after_pub)
            } else {
                None
            }
        }
    }
}

/// Returns the name a `pub` declaration on `text` introduces.
///
/// `pub use` and `pub mod` are deliberately not names of their own: a
/// re-export names an item declared elsewhere, and a module is a namespace
/// rather than an interface. Everything else a `pub` line can introduce is
/// here: an item behind its keyword, or a struct field behind none.
pub fn declared_name(text: &str) -> Option<String> {
    parse_declared_name(text, true)
}

/// Returns the name a keyword-only sweep would have read on `text`.
///
/// This is [`declared_name`] without the two shapes the M15 verification found
/// missing: the keyword-free public field, and `pub static`. It exists so the
/// contribution of that arm can be measured against the floor rather than
/// asserted in prose.
pub fn keyword_only_name(text: &str) -> Option<String> {
    let name = declared_name(text)?;
    let after_pub = text.strip_prefix("pub ")?;
    if after_pub.starts_with("static ") || field_name(after_pub).is_some() {
        return None;
    }
    Some(name)
}

/// Returns the keyword-introduced declaration name without struct fields or statics.
pub fn keyword_declared_name(text: &str) -> Option<String> {
    parse_declared_name(text, false)
}

/// Returns the variant `text` declares, when it declares one.
pub fn variant_name(text: &str) -> Option<String> {
    let name = leading_identifier(text)?;
    if !name.chars().next()?.is_ascii_uppercase() {
        return None;
    }
    let tail = text.get(name.len()..)?.trim_start();
    let opens_payload = tail.starts_with('{') || tail.starts_with('(');
    if tail.is_empty() || tail.starts_with(',') || opens_payload {
        return Some(name);
    }
    None
}

/// Returns the trait item `text` declares, when it declares one.
pub fn trait_item_name(text: &str) -> Option<String> {
    if let Some(rest) = text.strip_prefix("fn ") {
        return leading_identifier(rest);
    }
    leading_identifier(text.strip_prefix("const ")?)
}

/// Returns the name of a direct child of a public enum or trait.
pub fn member_name(block: Block, text: &str) -> Option<String> {
    match block {
        Block::Outside => None,
        Block::Enum => variant_name(text),
        Block::Trait => trait_item_name(text),
    }
}

/// Returns the kind of block `text` opens, if it opens one.
pub fn block_opener(text: &str) -> Option<Block> {
    if text.starts_with("pub enum ") {
        return Some(Block::Enum);
    }
    if text.starts_with("pub trait ") {
        return Some(Block::Trait);
    }
    None
}

/// Returns the net brace nesting `line` introduces.
pub fn brace_delta(line: &str) -> i32 {
    let opens = i32::try_from(line.matches('{').count()).unwrap_or(0);
    let closes = i32::try_from(line.matches('}').count()).unwrap_or(0);
    opens.saturating_sub(closes)
}

/// Collects public names from one source file, line by line.
///
/// `reader` is how a line is turned into the name it declares. It is a
/// parameter so the same walk can be run with the M15 arm and without it, and
/// the difference read off as that arm's contribution.
pub struct Scanner {
    depth: i32,
    block: Block,
    block_depth: i32,
    reader: fn(&str) -> Option<String>,
    pub names: Vec<String>,
}

impl Scanner {
    /// Builds an empty scanner that reads declarations with `reader`.
    pub fn new(reader: fn(&str) -> Option<String>) -> Self {
        Self {
            depth: 0,
            block: Block::Outside,
            block_depth: 0,
            reader,
            names: Vec::new(),
        }
    }

    /// Scans one file, resetting the nesting state first.
    pub fn file(&mut self, body: &str) {
        self.depth = 0;
        self.block = Block::Outside;
        self.block_depth = 0;
        for line in body.lines().take(LINE_BOUND) {
            self.line(line);
        }
    }

    /// Records what one line declares and tracks the nesting it changes.
    ///
    /// Comment lines are skipped whole, braces included: the crate documents
    /// its own encodings, and a brace inside prose is not nesting.
    pub fn line(&mut self, line: &str) {
        let text = line.trim();
        if text.starts_with("//") {
            return;
        }
        self.collect(text);
        if let Some(kind) = block_opener(text) {
            self.block = kind;
            self.block_depth = self.depth;
        }
        self.close(brace_delta(line));
    }

    /// Records whatever `text` declares at the current nesting.
    pub fn collect(&mut self, text: &str) {
        if let Some(name) = (self.reader)(text) {
            self.names.push(name);
        }
        if self.depth != self.block_depth.saturating_add(1) {
            return;
        }
        if let Some(name) = member_name(self.block, text) {
            self.names.push(name);
        }
    }

    /// Advances the nesting depth and leaves the block when it closes.
    pub fn close(&mut self, delta: i32) {
        let next = self.depth.saturating_add(delta);
        if next <= self.block_depth {
            self.block = Block::Outside;
        }
        self.depth = next;
    }
}

/// Returns the distinct names `reader` finds in `src`.
pub fn collected_names(src: &Path, reader: fn(&str) -> Option<String>) -> Vec<String> {
    let mut scanner = Scanner::new(reader);
    for (_, body) in rust_sources(src) {
        scanner.file(&body);
    }
    scanner.names.sort_unstable();
    scanner.names.dedup();
    scanner.names
}

/// Returns the code of every integration test but `sweep_file`, comments removed.
///
/// Comments are removed so that naming an item in a doc comment does not count
/// as exercising it.
pub fn test_sources(tests: &Path, sweep_file: &str) -> String {
    let mut out = String::new();
    for (name, body) in rust_sources(tests) {
        if name == sweep_file {
            continue;
        }
        for line in body.lines().take(LINE_BOUND) {
            if line.trim_start().starts_with("//") {
                continue;
            }
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Returns `true` when `c` may appear inside a Rust identifier.
pub fn is_identifier_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Returns `true` when `text` contains `name` as a whole identifier.
pub fn contains_identifier(text: &str, name: &str) -> bool {
    text.match_indices(name).any(|(at, _)| {
        let before = text.get(..at).and_then(|head| head.chars().next_back());
        let after = text
            .get(at.saturating_add(name.len())..)
            .and_then(|tail| tail.chars().next());
        !before.is_some_and(is_identifier_char) && !after.is_some_and(is_identifier_char)
    })
}

/// Returns the distinct names `reader` finds in the crate's `src/`.
pub fn crate_collected_names(
    manifest_dir: &str,
    reader: fn(&str) -> Option<String>,
) -> Vec<String> {
    let src = Path::new(manifest_dir).join("src");
    collected_names(&src, reader)
}

/// Returns every public name the crate's `src/` declares.
pub fn crate_public_names(manifest_dir: &str) -> Vec<String> {
    crate_collected_names(manifest_dir, declared_name)
}

/// Returns the public names a keyword-only sweep would have collected.
pub fn crate_keyword_only_names(manifest_dir: &str) -> Vec<String> {
    crate_collected_names(manifest_dir, keyword_only_name)
}

/// Returns the code of every integration test but `sweep_file`, comments removed.
pub fn crate_test_sources(manifest_dir: &str, sweep_file: &str) -> String {
    let tests = Path::new(manifest_dir).join("tests");
    test_sources(&tests, sweep_file)
}

/// Positive assertion: every public item declared in `src/` is named by an integration test.
fn assert_every_item_is_named_with_names(
    names: &[String],
    manifest_dir: &str,
    sweep_file: &str,
    minimum_surface: usize,
) {
    assert!(
        names.len() >= minimum_surface,
        "the scanner reported only {} public names, fewer than the {minimum_surface} \
         this crate is known to export; it is reading nothing, not finding nothing",
        names.len()
    );
    let sources = crate_test_sources(manifest_dir, sweep_file);
    assert!(
        !sources.is_empty(),
        "no integration test source was read at all"
    );
    let missing: Vec<&str> = names
        .iter()
        .map(String::as_str)
        .filter(|name| !contains_identifier(&sources, name))
        .collect();
    assert!(
        missing.is_empty(),
        "public items with no integration-test reference: {missing:?}"
    );
}

/// Positive assertion: every public item the crate declares is named by a test.
///
/// This is the sweep itself. A failure lists the items, so closing it is
/// mechanical: give the item a triple, or delete the item.
pub fn assert_every_public_item_is_named(
    manifest_dir: &str,
    sweep_file: &str,
    minimum_surface: usize,
) {
    let names = crate_public_names(manifest_dir);
    assert_every_item_is_named_with_names(&names, manifest_dir, sweep_file, minimum_surface);
}

/// Positive assertion for crates using `keyword_declared_name`.
///
/// This is the sweep itself. A failure lists the items, so closing it is
/// mechanical: give the item a triple, or delete the item.
pub fn assert_every_keyword_declared_item_is_named(
    manifest_dir: &str,
    sweep_file: &str,
    minimum_surface: usize,
) {
    let names = crate_collected_names(manifest_dir, keyword_declared_name);
    assert_every_item_is_named_with_names(&names, manifest_dir, sweep_file, minimum_surface);
}

fn assert_reads_surface_and_not_private_helpers_with_names(
    names: &[String],
    expected_public: &[&str],
    private_helpers: &[&str],
) {
    for expected in expected_public {
        assert!(
            names.iter().any(|name| name == expected),
            "the scanner missed the public item {expected}"
        );
    }
    for private in private_helpers {
        assert!(
            !names.iter().any(|name| name == private),
            "the scanner collected the private helper {private}"
        );
    }
}

/// Negative assertion: the scanner distinguishes public surface from private helpers,
/// so the sweep cannot pass by collecting nothing or by collecting everything.
pub fn assert_reads_public_surface_and_not_private_helpers(
    manifest_dir: &str,
    expected_public: &[&str],
    private_helpers: &[&str],
) {
    let names = crate_public_names(manifest_dir);
    assert_reads_surface_and_not_private_helpers_with_names(
        &names,
        expected_public,
        private_helpers,
    );
}

/// Negative assertion for keyword-declared surface: the scanner distinguishes
/// public surface from private helpers, so the sweep cannot pass by collecting
/// nothing or by collecting everything.
pub fn assert_reads_keyword_surface_and_not_private_helpers(
    manifest_dir: &str,
    expected_public: &[&str],
    private_helpers: &[&str],
) {
    let names = crate_collected_names(manifest_dir, keyword_declared_name);
    assert_reads_surface_and_not_private_helpers_with_names(
        &names,
        expected_public,
        private_helpers,
    );
}

/// Negative assertion: scanner reads public fields and statics and not private ones.
///
/// A public struct field and a `pub static` are declarations, and
/// the scanner reads them as such without reading a private field, a
/// `pub(crate)` item, a `pub use` or a `pub mod`.
///
/// The two inputs the M15 verification planted -- `pub untested_knob: u8` and
/// `pub static UNTESTED_REGISTER`, both of which a keyword-only sweep passed
/// over -- are the first two cases here.
pub fn assert_reads_fields_and_statics(declarations: &[(&str, &str)], ignored: &[&str]) {
    for (line, expected) in declarations {
        assert_eq!(
            declared_name(line).as_deref(),
            Some(*expected),
            "the scanner did not read {line:?} as declaring {expected}"
        );
    }
    for ignored_line in ignored {
        assert_eq!(
            declared_name(ignored_line),
            None,
            "the scanner read a declaration out of {ignored_line:?}"
        );
    }
}

/// Boundary assertion: identifier matching is exact at both ends, so a name is not
/// counted as referenced because a longer name contains it.
pub fn assert_identifier_matcher_is_exact(
    matching: &[(&str, &str)],
    non_matching: &[(&str, &str)],
) {
    for (text, id) in matching {
        assert!(contains_identifier(text, id));
    }
    for (text, id) in non_matching {
        assert!(!contains_identifier(text, id));
    }
}

/// Boundary assertion: field-and-static arm carries the floor.
///
/// The field-and-static arm is what carries the floor above what a
/// keyword-only sweep collects, and its contribution is measured here.
///
/// The previous form of this test filtered the collected names by whether
/// `pub {name}: u8,` reads as a declaration, which is true of every collected
/// name while the arm exists. It therefore measured nothing and would have
/// passed at any contribution, zero included. This form runs the same walk
/// twice, once with the arm and once without, and reads the difference.
pub fn assert_field_arm_carries_the_floor(
    manifest_dir: &str,
    field_arm_names: usize,
    minimum_surface: usize,
) {
    let all = crate_public_names(manifest_dir);
    let keyword_only = crate_keyword_only_names(manifest_dir);
    let contribution = all.len().saturating_sub(keyword_only.len());
    assert_eq!(
        contribution, field_arm_names,
        "the field-and-static arm contributes {contribution} names, not {field_arm_names}"
    );
    assert!(
        all.len() >= minimum_surface,
        "the collected surface must stay at or above the measured floor"
    );
    assert!(
        keyword_only.len() < minimum_surface,
        "a keyword-only sweep still collects {} names, which clears the floor, \
         so dropping the arm would not fail the gate",
        keyword_only.len()
    );
}

#[cfg(test)]
mod surface_sources_tests {
    use super::surface_sources::*;
    use std::path::PathBuf;
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
