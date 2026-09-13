// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fixtures shared by the integration tests.
//!
//! Everything here reaches `aegis-vulcan` through its public API only, and
//! nothing here uses `unwrap` or `expect`: a test that cannot build its own
//! input returns the failure instead of panicking through a lint the workspace
//! denies.

#![allow(dead_code)]

use aegis_vulcan::{
    BarWindow, BlockCount, CorrelationId, DmaBufExport, DmaRequest, DrmDriver, EdgeId, IommuGroup,
    Lba, MediaIngestDescriptor, MediaIngestVersion, ModesetState, RenderNode, UserSpacePcieDriver,
    VfioDeviceConfig, VramAddress, WeightStreamDescriptor, WeightStreamVersion,
};

/// What every test in this suite returns.
pub type Fallible = Result<(), Box<dyn std::error::Error>>;

/// The page-aligned BAR base the fixtures use (the scaffold's own value).
pub const ALIGNED_BAR: u64 = 0xf720_0000;

/// The same base plus one byte: aligned to nothing.
pub const MISALIGNED_BAR: u64 = 0xf720_0001;

/// The BAR window width the fixtures use (the scaffold's own value).
pub const BAR_BYTES: u64 = 16384;

/// The correlation identifier the fixtures thread through every payload.
pub const CORRELATION: &str = "m17-fixture-0001";

/// Returns the fixture BAR window.
///
/// # Errors
///
/// Returns the refusal, so a fixture that stops being a valid window fails the
/// suite instead of being quietly replaced.
pub fn bar() -> Result<BarWindow, Box<dyn std::error::Error>> {
    Ok(BarWindow::parse(ALIGNED_BAR, BAR_BYTES)?)
}

/// Returns the fixture device: the scaffold's group, vendor and device.
///
/// # Errors
///
/// Propagates [`bar`].
pub fn device() -> Result<VfioDeviceConfig, Box<dyn std::error::Error>> {
    Ok(VfioDeviceConfig::new(
        IommuGroup::new(12),
        0x144d,
        0xa808,
        bar()?,
    ))
}

/// Returns a driver that has already mapped its window.
///
/// # Errors
///
/// Propagates [`device`] and the mapping refusal.
pub fn mapped() -> Result<UserSpacePcieDriver, Box<dyn std::error::Error>> {
    let mut driver = UserSpacePcieDriver::new(device()?);
    driver.map_bar()?;
    Ok(driver)
}

/// Returns a request of `blocks` blocks at the fixture addresses.
///
/// # Errors
///
/// Propagates [`BlockCount::new`].
pub fn request(blocks: u32) -> Result<DmaRequest, Box<dyn std::error::Error>> {
    Ok(DmaRequest::new(
        Lba::new(0x1000),
        VramAddress::new(0xe000_0000),
        BlockCount::new(blocks)?,
    ))
}

/// Returns the fixture correlation identifier.
///
/// # Errors
///
/// Propagates [`CorrelationId::parse`].
pub fn correlation() -> Result<CorrelationId, Box<dyn std::error::Error>> {
    Ok(CorrelationId::parse(CORRELATION)?)
}

/// Returns the export path the reference profile reports for the Intel card.
///
/// # Errors
///
/// Propagates [`RenderNode::new`].
pub fn intel_export() -> Result<DmaBufExport, Box<dyn std::error::Error>> {
    Ok(DmaBufExport::new(
        RenderNode::new(129)?,
        DrmDriver::I915,
        ModesetState::DriverDefault,
    ))
}

/// Returns a weight-stream descriptor of `blocks` blocks.
///
/// # Errors
///
/// Propagates the field constructors.
pub fn weight_stream(blocks: u32) -> Result<WeightStreamDescriptor, Box<dyn std::error::Error>> {
    Ok(WeightStreamDescriptor {
        schema: WeightStreamVersion::V1,
        edge: EdgeId::GpudirectWeightStreaming,
        correlation_id: correlation()?,
        group: IommuGroup::new(12),
        nvme_lba: Lba::new(0x1000),
        destination: VramAddress::new(0xe000_0000),
        blocks: BlockCount::new(blocks)?,
        export: intel_export()?,
    })
}

/// Returns a media-ingest descriptor of `blocks` blocks.
///
/// # Errors
///
/// Propagates the field constructors.
pub fn media_ingest(blocks: u32) -> Result<MediaIngestDescriptor, Box<dyn std::error::Error>> {
    Ok(MediaIngestDescriptor {
        schema: MediaIngestVersion::V1,
        edge: EdgeId::ZeroCopyMediaIngest,
        correlation_id: correlation()?,
        bar: bar()?,
        blocks: BlockCount::new(blocks)?,
        export: intel_export()?,
    })
}

/// Scalar bound on directories visited and entries read per directory.
pub const WALK_BOUND: usize = 64;

/// Returns the `.rs` files under `root`, as (file name, body) pairs.
///
/// The walk is iterative and doubly bounded: at most [`WALK_BOUND`]
/// directories and at most [`WALK_BOUND`] entries per directory.
///
/// # Errors
///
/// Returns the first read failure, so a sweep cannot pass by reading nothing.
pub fn rust_sources(
    root: &std::path::Path,
) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = vec![root.to_path_buf()];
    for _ in 0..WALK_BOUND {
        let Some(directory) = stack.pop() else { break };
        for entry in std::fs::read_dir(&directory)?.take(WALK_BOUND) {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            let name = path
                .file_name()
                .map(|raw| raw.to_string_lossy().into_owned())
                .unwrap_or_default();
            out.push((name, std::fs::read_to_string(&path)?));
        }
    }
    if out.is_empty() {
        return Err(format!("no Rust source was read under {}", root.display()).into());
    }
    Ok(out)
}

/// Scalar bound on the lines read from any one source file.
pub const LINE_BOUND: usize = 4096;

/// Returns the `"<file>:<line>: <identifier>"` hits `text` carries.
fn hits(name: &str, number: usize, text: &str, watched: &[&str]) -> Vec<String> {
    watched
        .iter()
        .filter(|needle| text.contains(**needle))
        .map(|needle| format!("{name}:{}: {needle}", number.saturating_add(1)))
        .collect()
}

/// Returns the hits one body of text carries, skipping comment lines.
///
/// Comment lines are skipped whole: the crate documentation names the very
/// identifiers a sweep watches for, precisely in order to say it does not use
/// them, so a sweep that read comments would fail on its own documentation.
///
/// This is public so a watched list can be exercised against planted text.
/// A list of constructs is a gate only if each entry is one the scan reports,
/// and that is checkable without writing a file.
pub fn scan_text(name: &str, body: &str, watched: &[&str]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for (number, line) in body.lines().take(LINE_BOUND).enumerate() {
        let text = line.trim();
        if text.starts_with("//") {
            continue;
        }
        found.extend(hits(name, number, text, watched));
    }
    found
}

/// Returns every watched identifier the sources under `root` carry.
///
/// # Errors
///
/// Propagates [`rust_sources`], so a sweep cannot pass by reading nothing.
pub fn scan_sources(
    root: &std::path::Path,
    watched: &[&str],
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut found: Vec<String> = Vec::new();
    for (name, body) in rust_sources(root)? {
        found.extend(scan_text(&name, &body, watched));
    }
    Ok(found)
}
