// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Inspection of `drm_sched` holders under `/sys/module/gpu_sched/holders`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::GpuError;

/// Maximum number of holder modules scanned.
pub const MAX_HOLDERS: usize = 32;

/// Serializable record of DRM scheduler state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrmSchedRecord {
    /// Modules that hold a reference to `gpu_sched`.
    pub gpu_sched_holders: Vec<String>,
    /// Driver bound to the Intel GPU (vendor 0x8086), if present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc_driver: Option<String>,
    /// Device PCI addresses driven by the `xe` driver.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub xe_bound_devices: Vec<String>,
}

/// Discovers `gpu_sched` holder modules under `<sysfs>/module/gpu_sched/holders`.
///
/// # Errors
///
/// Returns [`GpuError`] if the holders directory cannot be read or is malformed.
pub fn inspect_holders(sysfs: &Path) -> Result<Vec<String>, GpuError> {
    let holders_dir = sysfs.join("module").join("gpu_sched").join("holders");
    let entries = match std::fs::read_dir(&holders_dir) {
        Ok(iter) => iter,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(GpuError::io(holders_dir, err)),
    };

    let mut holders = Vec::new();
    for entry_res in entries.take(MAX_HOLDERS) {
        let entry = entry_res.map_err(|err| GpuError::io(&holders_dir, err))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        holders.push(name);
    }
    holders.sort();
    Ok(holders)
}

/// Evaluates whether a driver is a `drm_sched` provider.
#[must_use]
pub fn is_provider(driver: &str, holders: &[String]) -> bool {
    holders.iter().any(|h| h == driver)
}

/// Formats the detail line for a GPU device's `drm_sched` status.
#[must_use]
pub fn format_gpu_sched_line(driver: &str, node: &str, is_intel: bool, provider: bool) -> String {
    let intel_suffix = if is_intel { ", the Intel card" } else { "" };
    if provider {
        format!("     {driver} ({node}{intel_suffix}) is a drm_sched provider: a holder")
    } else {
        format!("     {driver} ({node}{intel_suffix}) is not: no holder entry")
    }
}
