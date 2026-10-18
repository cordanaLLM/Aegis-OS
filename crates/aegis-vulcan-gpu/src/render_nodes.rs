// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Inspection of DRM render nodes under `/sys/class/drm/renderD*`.

use std::path::Path;

use crate::error::GpuError;

/// Maximum number of `/sys/class/drm` entries scanned.
///
/// This bound is a loop cap against unbounded filesystem iteration, not an expected size.
pub const MAX_RENDER_NODES: usize = 256;

/// Maximum length in bytes of a modeset parameter read.
pub const MAX_MODESET_BYTES: usize = 64;

/// Information discovered for one DRM render node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderNodeInfo {
    /// Render node identifier (e.g. "renderD128").
    pub node: String,
    /// Bound PCI device address (e.g. "0000:f6:00.0").
    pub address: String,
    /// Bound kernel driver name (e.g. "nvidia").
    pub driver: String,
    /// Formatted modeset parameter status string.
    pub modeset_status: String,
}

impl RenderNodeInfo {
    /// Formats the single-line summary matching the gate log shape.
    #[must_use]
    pub fn format_line(&self) -> String {
        format!(
            "     {}: {} at {}; {}",
            self.node, self.driver, self.address, self.modeset_status
        )
    }
}

/// Reads the modeset parameter for a driver module, if present.
fn inspect_modeset(sysfs: &Path, driver: &str) -> String {
    let candidate = if driver == "nvidia" {
        "nvidia_drm"
    } else {
        driver
    };
    let path = sysfs
        .join("module")
        .join(candidate)
        .join("parameters")
        .join("modeset");
    match crate::reader::read_bounded(&path, MAX_MODESET_BYTES) {
        Ok(text) => {
            let trimmed = text.trim();
            format!("{candidate} modeset '{trimmed}'")
        }
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            format!("{candidate} modeset not readable without privilege")
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            format!("{candidate} modeset not present")
        }
        Err(err) => format!("{candidate} modeset error: {err}"),
    }
}

/// Discovers and inspects all DRM render nodes under `<sysfs>/class/drm/renderD*`.
///
/// # Errors
///
/// Returns [`GpuError`] if the DRM class directory cannot be read or symlinks are broken.
pub fn inspect_render_nodes(sysfs: &Path) -> Result<Vec<RenderNodeInfo>, GpuError> {
    let drm_dir = sysfs.join("class").join("drm");
    let entries = match std::fs::read_dir(&drm_dir) {
        Ok(iter) => iter,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(GpuError::io(drm_dir, err)),
    };

    let mut nodes = Vec::new();
    let mut count = 0usize;
    for entry_res in entries {
        if count >= MAX_RENDER_NODES {
            return Err(GpuError::bound(&drm_dir, MAX_RENDER_NODES));
        }
        count = count.saturating_add(1);
        let entry = entry_res.map_err(|err| GpuError::io(&drm_dir, err))?;
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        if !name.starts_with("renderD") {
            continue;
        }

        let node_path = entry.path();
        let info = parse_render_node(sysfs, &node_path, &name)?;
        nodes.push(info);
    }

    nodes.sort_by(|a, b| a.node.cmp(&b.node));
    Ok(nodes)
}

/// Parses a single render node directory.
fn parse_render_node(
    sysfs: &Path,
    node_path: &Path,
    name: &str,
) -> Result<RenderNodeInfo, GpuError> {
    let device_link = node_path.join("device");
    let dev_target = std::fs::read_link(&device_link).map_err(|e| GpuError::io(&device_link, e))?;
    let address = dev_target
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .ok_or_else(|| GpuError::malformed(&device_link, "device link has no target name"))?;

    let driver_link = device_link.join("driver");
    let driver = match std::fs::read_link(&driver_link) {
        Ok(drv_target) => drv_target.file_name().map_or_else(
            || "unknown".to_owned(),
            |s| s.to_string_lossy().into_owned(),
        ),
        Err(_) => "none".to_owned(),
    };

    let modeset_status = inspect_modeset(sysfs, &driver);
    Ok(RenderNodeInfo {
        node: name.to_owned(),
        address,
        driver,
        modeset_status,
    })
}
