// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Discovery and inspection of PCI GPU devices under `/sys/bus/pci/devices`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::GpuError;

/// Maximum number of PCI devices scanned.
///
/// This bound is a loop cap against unbounded filesystem iteration, not an expected size.
pub const MAX_PCI_DEVICES: usize = 1024;

/// Maximum number of `/sys/class/drm` entries scanned.
///
/// This bound is a loop cap against unbounded filesystem iteration, not an expected size.
pub const MAX_DRM_ENTRIES: usize = 256;

/// Maximum number of DRM card directories or connectors per GPU.
pub const MAX_CARD_ENTRIES: usize = 32;

/// Serializable record of a single GPU device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuRecord {
    /// PCI slot address (e.g. "0000:03:00.0").
    pub address: String,
    /// PCI vendor ID string (e.g. "0x8086").
    pub vendor: String,
    /// PCI device ID string (e.g. "0x56a5").
    pub device: String,
    /// PCI class code string (e.g. "0x030000").
    pub class: String,
    /// Kernel driver currently bound (e.g. "xe"), or "none".
    pub driver: String,
    /// Configured driver override, or "(null)".
    pub driver_override: String,
    /// Boot VGA flag ("0" or "1").
    pub boot_vga: String,
    /// IOMMU group number as string.
    pub iommu_group: String,
    /// All device addresses sharing the same IOMMU group.
    pub group_members: Vec<String>,
    /// DRM entries associated with the device (cards, control nodes, render nodes).
    pub drm: Vec<String>,
    /// Display connectors and their connection statuses.
    pub connectors: BTreeMap<String, String>,
}

/// Reads a trimmed line from a file under `dir`, returning a default if missing.
fn read_line_or_default(dir: &Path, file_name: &str, default: &str) -> String {
    let path = dir.join(file_name);
    match crate::reader::read_bounded(&path, crate::reader::MAX_ATTR_BYTES) {
        Ok(s) => s.trim().to_owned(),
        Err(_) => default.to_owned(),
    }
}

/// Discovers all GPU PCI devices under `<sysfs>/bus/pci/devices`.
///
/// GPUs have PCI class `0x0300xx` or `0x0380xx`.
///
/// # Errors
///
/// Returns [`GpuError`] if the PCI devices directory cannot be read.
pub fn discover_gpus(sysfs: &Path) -> Result<BTreeMap<String, GpuRecord>, GpuError> {
    let pci_dir = sysfs.join("bus").join("pci").join("devices");
    let entries = match std::fs::read_dir(&pci_dir) {
        Ok(iter) => iter,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(err) => return Err(GpuError::io(pci_dir, err)),
    };

    let mut gpus = BTreeMap::new();
    let mut count = 0usize;
    for entry_res in entries {
        if count >= MAX_PCI_DEVICES {
            return Err(GpuError::bound(&pci_dir, MAX_PCI_DEVICES));
        }
        count = count.saturating_add(1);
        let entry = entry_res.map_err(|err| GpuError::io(&pci_dir, err))?;
        let addr = entry.file_name().to_string_lossy().into_owned();
        let dev_dir = entry.path();

        let class = read_line_or_default(&dev_dir, "class", "");
        let class_lower = class.to_lowercase();
        if !class_lower.starts_with("0x0300") && !class_lower.starts_with("0x0380") {
            continue;
        }

        let record = parse_gpu_record(sysfs, &dev_dir, &addr, &class)?;
        gpus.insert(addr, record);
    }
    Ok(gpus)
}

/// Reads the VFIO candidate input facts for a specific PCI device from sysfs.
///
/// # Errors
///
/// Returns [`GpuError`] if the device directory does not exist or sysfs attributes cannot be read.
pub fn read_candidate_input(
    sysfs: &Path,
    device: &str,
) -> Result<crate::vfio::VfioCandidateInput, GpuError> {
    let dev_dir = sysfs.join("bus").join("pci").join("devices").join(device);
    if !dev_dir.is_dir() {
        return Err(GpuError::io(
            dev_dir,
            std::io::Error::new(std::io::ErrorKind::NotFound, "PCI device not found"),
        ));
    }

    let driver = match std::fs::read_link(dev_dir.join("driver")) {
        Ok(target) => target
            .file_name()
            .map_or_else(|| "none".to_owned(), |s| s.to_string_lossy().into_owned()),
        Err(_) => "none".to_owned(),
    };

    let boot_vga_str = read_line_or_default(&dev_dir, "boot_vga", "0");
    let boot_vga = boot_vga_str == "1" || boot_vga_str.eq_ignore_ascii_case("true");

    let (iommu_group, group_members) = parse_iommu_group(&dev_dir, device)?;
    let drm = parse_drm_nodes(&dev_dir)?;
    let connectors = discover_connectors(sysfs, &drm)?;

    Ok(crate::vfio::VfioCandidateInput {
        address: device.to_owned(),
        driver,
        iommu_group,
        group_members,
        connectors,
        boot_vga,
    })
}

fn parse_gpu_record(
    sysfs: &Path,
    dev_dir: &Path,
    addr: &str,
    class: &str,
) -> Result<GpuRecord, GpuError> {
    let vendor = read_line_or_default(dev_dir, "vendor", "0x0000");
    let device = read_line_or_default(dev_dir, "device", "0x0000");
    let driver_override = read_line_or_default(dev_dir, "driver_override", "(null)");
    let boot_vga = read_line_or_default(dev_dir, "boot_vga", "0");

    let driver = match std::fs::read_link(dev_dir.join("driver")) {
        Ok(target) => target
            .file_name()
            .map_or_else(|| "none".to_owned(), |s| s.to_string_lossy().into_owned()),
        Err(_) => "none".to_owned(),
    };

    let (iommu_group, group_members) = parse_iommu_group(dev_dir, addr)?;
    let drm = parse_drm_nodes(dev_dir)?;
    let connectors = discover_connectors(sysfs, &drm)?;

    Ok(GpuRecord {
        address: addr.to_owned(),
        vendor,
        device,
        class: class.to_owned(),
        driver,
        driver_override,
        boot_vga,
        iommu_group,
        group_members,
        drm,
        connectors,
    })
}

fn parse_iommu_group(dev_dir: &Path, addr: &str) -> Result<(String, Vec<String>), GpuError> {
    let group_link = dev_dir.join("iommu_group");
    let Ok(target) = std::fs::read_link(&group_link) else {
        return Ok((String::new(), vec![addr.to_owned()]));
    };

    let group_id = target
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    let devices_dir = group_link.join("devices");
    let mut members = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&devices_dir) {
        let mut count = 0usize;
        for entry_res in entries {
            if count >= MAX_PCI_DEVICES {
                return Err(GpuError::bound(&devices_dir, MAX_PCI_DEVICES));
            }
            count = count.saturating_add(1);
            let entry = entry_res.map_err(|err| GpuError::io(&devices_dir, err))?;
            members.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    if members.is_empty() {
        members.push(addr.to_owned());
    }
    members.sort();
    Ok((group_id, members))
}

fn parse_drm_nodes(dev_dir: &Path) -> Result<Vec<String>, GpuError> {
    let drm_dir = dev_dir.join("drm");
    let mut nodes = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&drm_dir) {
        let mut count = 0usize;
        for entry_res in entries {
            if count >= MAX_CARD_ENTRIES {
                return Err(GpuError::bound(&drm_dir, MAX_CARD_ENTRIES));
            }
            count = count.saturating_add(1);
            let entry = entry_res.map_err(|err| GpuError::io(&drm_dir, err))?;
            nodes.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    nodes.sort();
    Ok(nodes)
}

fn discover_connectors(
    sysfs: &Path,
    drm_nodes: &[String],
) -> Result<BTreeMap<String, String>, GpuError> {
    let mut connectors = BTreeMap::new();
    let class_drm = sysfs.join("class").join("drm");

    // Look for cards (e.g. "card2")
    for node in drm_nodes {
        if !node.starts_with("card") || node.contains('-') {
            continue;
        }
        let prefix = format!("{node}-");
        let Ok(entries) = std::fs::read_dir(&class_drm) else {
            continue;
        };
        let mut count = 0usize;
        for entry_res in entries {
            if count >= MAX_DRM_ENTRIES {
                return Err(GpuError::bound(&class_drm, MAX_DRM_ENTRIES));
            }
            count = count.saturating_add(1);
            let entry = entry_res.map_err(|err| GpuError::io(&class_drm, err))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(&prefix) {
                let status = read_line_or_default(&entry.path(), "status", "unknown");
                connectors.insert(name, status);
            }
        }
    }
    Ok(connectors)
}
