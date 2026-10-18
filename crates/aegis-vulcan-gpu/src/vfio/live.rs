// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Live VFIO BAR mapping execution (milestone M25, epic E25-1).
//!
//! Validates candidate admission via sysfs, opens VFIO container and group
//! device nodes, binds the group to container, queries BAR regions 0..=5,
//! maps the first eligible BAR as read-only MMIO, reads the first 4 KiB
//! with volatile 32-bit reads, and reports the SHA-256 digest.

use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::Path;

use crate::gpus::read_candidate_input;
use crate::vfio::sys::{
    self, VFIO_REGION_INFO_FLAG_CAPS, VFIO_REGION_INFO_FLAG_MMAP, VFIO_REGION_INFO_FLAG_READ,
    VFIO_REGION_INFO_FLAG_WRITE, VfioRegionInfo,
};
use crate::vfio::{VfioCandidateInput, VfioDecision, evaluate_candidate};

/// Outcome of a `map-bar` execution.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MapBarRun {
    /// Formatted output lines in execution order.
    pub lines: Vec<String>,
    /// Whether any gating step failed.
    pub failed: bool,
}

impl MapBarRun {
    /// Appends a passing outcome line.
    pub fn pass(&mut self, mut line: String) {
        line.insert_str(0, "PASS: ");
        self.lines.push(line);
    }

    /// Appends a failing outcome line and marks the run failed.
    pub fn fail(&mut self, mut line: String) {
        line.insert_str(0, "FAIL: ");
        self.lines.push(line);
        self.failed = true;
    }
}

/// Formats the region access flags as a comma-separated string (`r,w,mmap`).
#[must_use]
pub fn format_region_flags(flags: u32) -> String {
    let mut parts = Vec::new();
    if flags & VFIO_REGION_INFO_FLAG_READ != 0 {
        parts.push("r");
    }
    if flags & VFIO_REGION_INFO_FLAG_WRITE != 0 {
        parts.push("w");
    }
    if flags & VFIO_REGION_INFO_FLAG_MMAP != 0 {
        parts.push("mmap");
    }
    if flags & VFIO_REGION_INFO_FLAG_CAPS != 0 {
        parts.push("caps");
    }
    parts.join(",")
}

/// Formats a BAR region info line if size is non-zero.
#[must_use]
pub fn format_bar_region(index: usize, info: &VfioRegionInfo) -> Option<String> {
    if info.size == 0 {
        return None;
    }
    let flags_str = format_region_flags(info.flags);
    Some(format!(
        "BAR{index} size={} flags={flags_str} offset=0x{:x}",
        info.size, info.offset
    ))
}

/// `IORESOURCE_PREFETCH` in the flags column of sysfs `resource`.
const IORESOURCE_PREFETCH: u64 = 0x2000;

/// Upper bound on the sysfs `resource` file: 17 lines of three 18-byte fields.
const MAX_RESOURCE_BYTES: usize = 1024;

/// Marks which of BARs 0..=5 the sysfs `resource` text declares prefetchable.
///
/// A line that does not hold three hex fields counts as not prefetchable.
#[must_use]
pub fn parse_prefetchable_bars(resource: &str) -> [bool; 6] {
    let mut bars = [false; 6];
    for (slot, line) in bars.iter_mut().zip(resource.lines()) {
        let flags = line
            .split_whitespace()
            .nth(2)
            .and_then(|f| u64::from_str_radix(f.trim_start_matches("0x"), 16).ok());
        *slot = flags.is_some_and(|f| f & IORESOURCE_PREFETCH != 0);
    }
    bars
}

/// Finds the first prefetchable BAR that has the MMAP flag and size >= 4096 bytes.
///
/// Register BARs are never chosen: a read of device registers can have side
/// effects, a read of prefetchable memory has none.
#[must_use]
pub fn choose_bar_to_map(regions: &[VfioRegionInfo], prefetchable: &[bool; 6]) -> Option<usize> {
    regions
        .iter()
        .zip(prefetchable)
        .position(|(r, &pf)| pf && (r.flags & VFIO_REGION_INFO_FLAG_MMAP) != 0 && r.size >= 4096)
}

/// Reads the prefetchable BAR marks for `device_addr` from sysfs.
fn read_prefetchable_bars(sysfs: &Path, device_addr: &str) -> Result<[bool; 6], String> {
    let path = sysfs
        .join("bus")
        .join("pci")
        .join("devices")
        .join(device_addr)
        .join("resource");
    let text = crate::reader::read_bounded(&path, MAX_RESOURCE_BYTES)
        .map_err(|err| format!("sysfs: {}: {err}", path.display()))?;
    Ok(parse_prefetchable_bars(&text))
}

/// Validates that a candidate device is admitted and bound to `vfio-pci`.
fn validate_candidate(candidate: &VfioCandidateInput) -> Result<(), String> {
    let mut reasons = Vec::new();
    if candidate.driver != "vfio-pci" {
        reasons.push(format!(
            "{} driver is {}, expected vfio-pci",
            candidate.address, candidate.driver
        ));
    }
    if let VfioDecision::Refused(cand_reasons) = evaluate_candidate(candidate) {
        reasons.extend(cand_reasons);
    }
    if reasons.is_empty() {
        Ok(())
    } else {
        let joined = reasons.join("; ");
        Err(format!("candidate: refused: {joined}"))
    }
}

/// Opens the VFIO container and group device nodes.
fn open_vfio_nodes(dev: &Path, group_id: &str) -> Result<(OwnedFd, OwnedFd), String> {
    let container_path = dev.join("vfio").join("vfio");
    let container = rustix::fs::open(
        &container_path,
        rustix::fs::OFlags::RDWR | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|err| format!("open {}: {err}", container_path.display()))?;

    let group_path = dev.join("vfio").join(group_id);
    let group = rustix::fs::open(
        &group_path,
        rustix::fs::OFlags::RDWR | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|err| format!("open {}: {err}", group_path.display()))?;

    Ok((container, group))
}

/// Verifies API version, extensions, group viability, sets container, and retrieves device fd.
fn setup_vfio_device(
    container: BorrowedFd<'_>,
    group: BorrowedFd<'_>,
    group_id: &str,
    device_addr: &str,
) -> Result<OwnedFd, String> {
    let version =
        sys::vfio_get_api_version(container).map_err(|err| format!("get api version: {err}"))?;
    if version != sys::VFIO_API_VERSION {
        return Err(format!("get api version: unsupported version {version}"));
    }

    let ext = sys::vfio_check_extension(container, sys::VFIO_TYPE1v2_IOMMU)
        .map_err(|err| format!("check extension: {err}"))?;
    if ext == 0 {
        return Err("check extension: VFIO_TYPE1v2_IOMMU extension not supported".to_owned());
    }

    let status =
        sys::vfio_group_get_status(group).map_err(|err| format!("group get status: {err}"))?;
    if (status.flags & sys::VFIO_GROUP_FLAGS_VIABLE) == 0 {
        return Err(format!("group get status: group {group_id} not viable"));
    }

    sys::vfio_group_set_container(group, container)
        .map_err(|err| format!("group set container: {err}"))?;

    sys::vfio_set_iommu(container, sys::VFIO_TYPE1v2_IOMMU)
        .map_err(|err| format!("set iommu: {err}"))?;

    let device_fd = sys::vfio_group_get_device_fd(group, device_addr)
        .map_err(|err| format!("group get device fd: {err}"))?;

    Ok(device_fd)
}

/// Queries BAR regions 0..=5 and records lines for non-zero sized regions.
fn query_bar_regions(
    device: BorrowedFd<'_>,
    lines: &mut Vec<String>,
) -> Result<Vec<VfioRegionInfo>, String> {
    let mut regions = Vec::with_capacity(6);
    for idx in 0..=5u32 {
        let info = sys::vfio_device_get_region_info(device, idx)
            .map_err(|err| format!("device get region info for BAR{idx}: {err}"))?;
        if let Some(line) = format_bar_region(idx as usize, &info) {
            lines.push(line);
        }
        regions.push(info);
    }
    Ok(regions)
}

/// Maps the first eligible BAR, reads 4 KiB via volatile 32-bit MMIO reads, and prints digest.
fn map_and_verify_bar(
    device: BorrowedFd<'_>,
    regions: &[VfioRegionInfo],
    prefetchable: &[bool; 6],
    lines: &mut Vec<String>,
) -> Result<usize, String> {
    let bar_idx = choose_bar_to_map(regions, prefetchable).ok_or_else(|| {
        "mmap: no prefetchable BAR with MMAP flag and size >= 4096; register BARs are not read"
            .to_owned()
    })?;
    let region = regions
        .get(bar_idx)
        .ok_or_else(|| format!("internal error: region index {bar_idx} not found"))?;

    let mapping =
        sys::mmap_bar_mmio(device, region.offset, 4096).map_err(|err| format!("mmap: {err}"))?;
    let (dword0, buffer) = mapping
        .read_4k_mmio()
        .map_err(|err| format!("mmio read: {err}"))?;
    let digest = crate::dmabuf::sha256_hex(&buffer);
    lines.push(format!(
        "MAPPED BAR{bar_idx} 4096 bytes sha256={digest} dword0=0x{dword0:08x}"
    ));
    mapping.unmap().map_err(|err| format!("munmap: {err}"))?;
    Ok(bar_idx)
}

/// Queries the BARs, reads which are prefetchable from sysfs and maps the chosen one.
fn query_and_map(
    sysfs: &Path,
    device_addr: &str,
    device: BorrowedFd<'_>,
    lines: &mut Vec<String>,
) -> Result<usize, String> {
    let regions = query_bar_regions(device, lines)?;
    let prefetchable = read_prefetchable_bars(sysfs, device_addr)?;
    map_and_verify_bar(device, &regions, &prefetchable, lines)
}

/// Executes the full `map-bar` sequence against a device under `sysfs` and `dev`.
#[must_use]
pub fn run_map_bar(sysfs: &Path, dev: &Path, device_addr: &str) -> MapBarRun {
    let mut run = MapBarRun::default();

    let candidate = match read_candidate_input(sysfs, device_addr) {
        Ok(c) => c,
        Err(err) => {
            run.fail(format!("sysfs: {err}"));
            return run;
        }
    };

    if let Err(why) = validate_candidate(&candidate) {
        run.fail(why);
        return run;
    }

    let (container, group) = match open_vfio_nodes(dev, &candidate.iommu_group) {
        Ok(nodes) => nodes,
        Err(why) => {
            run.fail(why);
            return run;
        }
    };

    let device_fd = match setup_vfio_device(
        container.as_fd(),
        group.as_fd(),
        &candidate.iommu_group,
        device_addr,
    ) {
        Ok(fd) => fd,
        Err(why) => {
            run.fail(why);
            return run;
        }
    };

    let bar_idx = match query_and_map(sysfs, device_addr, device_fd.as_fd(), &mut run.lines) {
        Ok(idx) => idx,
        Err(why) => {
            run.fail(why);
            return run;
        }
    };

    run.pass(format!(
        "BAR{bar_idx} of {} mapped through VFIO group {}",
        candidate.address, candidate.iommu_group
    ));
    run
}
