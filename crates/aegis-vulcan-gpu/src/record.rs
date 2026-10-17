// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Coordination and generation of the M25 GPU record document and gate log output.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::drm_sched::{DrmSchedRecord, format_gpu_sched_line, inspect_holders, is_provider};
use crate::error::GpuError;
use crate::gpus::{GpuRecord, discover_gpus};
use crate::heap::{DmaHeapRecord, inspect_dma_heap};
use crate::rebar::{BarResizeRecord, inspect_device_rebar};
use crate::render_nodes::{RenderNodeInfo, inspect_render_nodes};
use crate::vfio::{VfioCandidateInput, evaluate_candidate};

/// Schema identifier for the GPU record document.
pub const GPU_RECORD_SCHEMA: &str = "aegis.m25.gpu-record.v1";

/// The complete M25 GPU record document serialized to JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuRecordDocument {
    /// Schema tag ("aegis.m25.gpu-record.v1").
    pub schema: String,
    /// DRM scheduler holder and provider measurements.
    pub drm_sched: DrmSchedRecord,
    /// Resizable BAR configuration keyed by PCI device address.
    pub rebar: BTreeMap<String, Vec<BarResizeRecord>>,
    /// Status and access permissions of DMA-BUF system heap.
    pub dma_heap: DmaHeapRecord,
    /// Detailed discovery facts for all GPU devices keyed by address.
    pub gpus: BTreeMap<String, GpuRecord>,
}

/// The result of evaluating all GPU record gates.
#[derive(Debug, Clone)]
pub struct RecordRun {
    /// Human-readable log lines matching gate.log format.
    pub lines: Vec<String>,
    /// The structured document.
    pub document: GpuRecordDocument,
    /// Number of failed gating checks.
    pub failed: usize,
}

impl RecordRun {
    /// Returns true if all gating checks passed.
    #[must_use]
    pub fn is_pass(&self) -> bool {
        self.failed == 0
    }
}

/// Gathers GPU host facts and builds the record run and document.
///
/// # Errors
///
/// Returns [`GpuError`] if a reading fails or is malformed.
pub fn run_record(sysfs: &Path, dev: &Path) -> Result<RecordRun, GpuError> {
    let mut lines = Vec::new();

    // 1. Render nodes
    let render_nodes = inspect_render_nodes(sysfs)?;
    lines.push("PASS gpu/render-nodes".to_owned());
    for node in &render_nodes {
        lines.push(node.format_line());
    }

    // 2. Discover GPUs
    let gpus = discover_gpus(sysfs)?;

    // 3. DRM scheduler providers
    let holders = inspect_holders(sysfs)?;
    let (arc_driver, xe_bound_devices) =
        record_drm_sched(&holders, &gpus, &render_nodes, &mut lines);

    // 4. ReBAR
    let rebar = record_rebar(sysfs, &gpus, &mut lines)?;

    // 5. DMA heap (recorded, never gating)
    let dma_heap = inspect_dma_heap(dev);
    lines.push(dma_heap.format_line());

    // 6. VFIO candidate selection
    record_vfio(&gpus, &mut lines);

    let document = GpuRecordDocument {
        schema: GPU_RECORD_SCHEMA.to_owned(),
        drm_sched: DrmSchedRecord {
            gpu_sched_holders: holders,
            arc_driver,
            xe_bound_devices,
        },
        rebar,
        dma_heap,
        gpus,
    };

    Ok(RecordRun {
        lines,
        document,
        failed: 0,
    })
}

fn record_drm_sched(
    holders: &[String],
    gpus: &BTreeMap<String, GpuRecord>,
    render_nodes: &[RenderNodeInfo],
    lines: &mut Vec<String>,
) -> (Option<String>, Vec<String>) {
    let arc_driver = gpus
        .values()
        .find(|g| g.vendor.eq_ignore_ascii_case("0x8086"))
        .map(|g| g.driver.clone());
    let xe_bound_devices: Vec<String> = gpus
        .values()
        .filter(|g| g.driver == "xe")
        .map(|g| g.address.clone())
        .collect();

    lines.push("PASS drm-sched/measured".to_owned());
    lines.push(format!(
        "     /sys/module/gpu_sched/holders: {}",
        holders.join(", ")
    ));
    for gpu in gpus.values() {
        let is_intel = gpu.vendor.eq_ignore_ascii_case("0x8086");
        let provider = is_provider(&gpu.driver, holders);
        let node_str = find_render_node(render_nodes, &gpu.address).unwrap_or(gpu.address.as_str());
        lines.push(format_gpu_sched_line(
            &gpu.driver,
            node_str,
            is_intel,
            provider,
        ));
    }
    (arc_driver, xe_bound_devices)
}

fn record_rebar(
    sysfs: &Path,
    gpus: &BTreeMap<String, GpuRecord>,
    lines: &mut Vec<String>,
) -> Result<BTreeMap<String, Vec<BarResizeRecord>>, GpuError> {
    let mut rebar = BTreeMap::new();
    lines.push("PASS rebar/measured".to_owned());
    for addr in gpus.keys() {
        let dev_dir = sysfs.join("bus").join("pci").join("devices").join(addr);
        let records = inspect_device_rebar(&dev_dir)?;
        for r in &records {
            lines.push(r.format_line(addr));
        }
        if !records.is_empty() {
            rebar.insert(addr.clone(), records);
        }
    }
    Ok(rebar)
}

fn record_vfio(gpus: &BTreeMap<String, GpuRecord>, lines: &mut Vec<String>) {
    lines.push("PASS vfio/candidate-selection".to_owned());
    for gpu in gpus.values() {
        let candidate = VfioCandidateInput {
            address: gpu.address.clone(),
            driver: gpu.driver.clone(),
            iommu_group: gpu.iommu_group.clone(),
            group_members: gpu.group_members.clone(),
            connectors: gpu.connectors.clone(),
            boot_vga: gpu.boot_vga == "1" || gpu.boot_vga.eq_ignore_ascii_case("true"),
        };
        let decision = evaluate_candidate(&candidate);
        lines.push(decision.format_line(&gpu.address, &gpu.driver));
    }
}

fn find_render_node<'a>(nodes: &'a [RenderNodeInfo], address: &str) -> Option<&'a str> {
    nodes
        .iter()
        .find(|n| n.address == address)
        .map(|n| n.node.as_str())
}
