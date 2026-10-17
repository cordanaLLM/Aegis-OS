// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Inspection of `PCIe` Resizable BAR (`ReBAR`) support and allocation.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::GpuError;

/// Maximum number of Base Address Registers (BARs) on a standard `PCIe` device.
pub const MAX_BARS: usize = 6;

/// Maximum number of lines read from a device's `resource` file.
pub const MAX_RESOURCE_LINES: usize = 64;

/// One Resizable BAR measurement record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BarResizeRecord {
    /// Zero-based BAR index (0..5).
    pub bar: usize,
    /// Currently mapped BAR size in bytes.
    pub current_bytes: u64,
    /// Sorted list of supported BAR sizes in bytes.
    pub admitted_bytes: Vec<u64>,
    /// Whether current size equals the maximum admitted size.
    pub at_largest: bool,
}

impl BarResizeRecord {
    /// Formats the single-line summary for gate logging.
    #[must_use]
    pub fn format_line(&self, address: &str) -> String {
        let current_mib = self.current_bytes / 1024 / 1024;
        let min_mib = self.admitted_bytes.first().copied().unwrap_or(0) / 1024 / 1024;
        let max_mib = self.admitted_bytes.last().copied().unwrap_or(0) / 1024 / 1024;
        let at_largest_str = if self.at_largest { "True" } else { "False" };
        format!(
            "     {address} BAR{}: {current_mib} MiB now; admits {min_mib} MiB to {max_mib} MiB; at its largest: {at_largest_str}",
            self.bar
        )
    }
}

/// Parses the current size of BAR `bar_idx` from the device's `resource` file.
fn parse_current_bar_size(dev_dir: &Path, bar_idx: usize) -> Result<u64, GpuError> {
    let res_path = dev_dir.join("resource");
    let content =
        crate::reader::read_bounded(&res_path, crate::reader::MAX_ATTR_BYTES).map_err(|e| {
            if e.kind() == std::io::ErrorKind::InvalidData {
                GpuError::malformed(&res_path, e.to_string())
            } else {
                GpuError::io(&res_path, e)
            }
        })?;
    let lines: Vec<&str> = content.lines().take(MAX_RESOURCE_LINES).collect();
    let line = lines
        .get(bar_idx)
        .ok_or_else(|| GpuError::malformed(&res_path, format!("missing line for BAR {bar_idx}")))?;

    let mut parts = line.split_whitespace();
    let start_str = parts
        .next()
        .ok_or_else(|| GpuError::malformed(&res_path, "missing start address in resource entry"))?;
    let end_str = parts
        .next()
        .ok_or_else(|| GpuError::malformed(&res_path, "missing end address in resource entry"))?;

    let start = parse_hex_u64(&res_path, start_str)?;
    let end = parse_hex_u64(&res_path, end_str)?;

    if start == 0 && end == 0 {
        return Ok(0);
    }
    if end < start {
        return Err(GpuError::malformed(
            &res_path,
            format!("end address {end:#x} is less than start {start:#x}"),
        ));
    }
    // end - start + 1
    let len = end.saturating_sub(start).saturating_add(1);
    Ok(len)
}

fn parse_hex_u64(path: &Path, s: &str) -> Result<u64, GpuError> {
    let stripped = s.strip_prefix("0x").unwrap_or(s);
    u64::from_str_radix(stripped, 16)
        .map_err(|e| GpuError::malformed(path, format!("invalid hex number '{s}': {e}")))
}

/// Decodes the bitmask in `resource{N}_resize` into admitted byte sizes.
///
/// # Errors
///
/// Returns [`GpuError`] if the bitmask is not a valid hex string or is zero.
pub fn parse_resize_bitmask(path: &Path, content: &str) -> Result<Vec<u64>, GpuError> {
    let trimmed = content.trim();
    let mask = parse_hex_u64(path, trimmed)?;
    if mask == 0 {
        return Err(GpuError::malformed(path, "resize bitmask is zero"));
    }

    let mut admitted = Vec::new();
    for k in 0u32..44u32 {
        if (mask & (1u64 << k)) != 0 {
            // bit k => 2^(k + 20) bytes
            let shift = k.saturating_add(20);
            if shift < 64 {
                admitted.push(1u64 << shift);
            }
        }
    }
    admitted.sort_unstable();
    Ok(admitted)
}

/// Inspects Resizable BAR state for one PCI device.
///
/// # Errors
///
/// Returns [`GpuError`] if a resize file or resource file is malformed.
pub fn inspect_device_rebar(dev_dir: &Path) -> Result<Vec<BarResizeRecord>, GpuError> {
    let mut records = Vec::new();
    for bar in 0..MAX_BARS {
        let resize_path = dev_dir.join(format!("resource{bar}_resize"));
        if !resize_path.is_file() {
            continue;
        }

        let content = crate::reader::read_bounded(&resize_path, crate::reader::MAX_ATTR_BYTES)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::InvalidData {
                    GpuError::malformed(&resize_path, e.to_string())
                } else {
                    GpuError::io(&resize_path, e)
                }
            })?;
        let admitted_bytes = parse_resize_bitmask(&resize_path, &content)?;
        let current_bytes = parse_current_bar_size(dev_dir, bar)?;
        let at_largest = admitted_bytes
            .last()
            .is_some_and(|&max| current_bytes == max);

        records.push(BarResizeRecord {
            bar,
            current_bytes,
            admitted_bytes,
            at_largest,
        });
    }
    Ok(records)
}
