// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P03 `aegis-vulcan-gpu`: GPU host slices on the reference profile
//! (milestone M25, slice 1).
//!
//! This crate implements the host inspection half of the M25 milestone, measuring
//! GPU topology and capabilities through read-only sysfs inspection and unprivileged
//! `/dev` device node stat calls.
//!
//! It opens, binds, unbinds and writes no hardware device.
//!
//! # Unsafe Code
//!
//! This crate contains zero unsafe code and inherits the workspace `forbid` on unsafe code.

pub mod drm_sched;
pub mod error;
pub mod gpus;
pub mod heap;
pub mod reader;
pub mod rebar;
pub mod record;
pub mod render_nodes;
pub mod vfio;

pub use crate::drm_sched::{DrmSchedRecord, MAX_HOLDERS, inspect_holders, is_provider};
pub use crate::error::GpuError;
pub use crate::gpus::{
    GpuRecord, MAX_CARD_ENTRIES, MAX_DRM_ENTRIES, MAX_PCI_DEVICES, discover_gpus,
};
pub use crate::heap::{DmaHeapRecord, inspect_dma_heap};
pub use crate::reader::{MAX_ATTR_BYTES, read_bounded};
pub use crate::rebar::{BarResizeRecord, MAX_BARS, inspect_device_rebar, parse_resize_bitmask};
pub use crate::record::{GPU_RECORD_SCHEMA, GpuRecordDocument, RecordRun, run_record};
pub use crate::render_nodes::{
    MAX_MODESET_BYTES, MAX_RENDER_NODES, RenderNodeInfo, inspect_render_nodes,
};
pub use crate::vfio::{VfioCandidateInput, VfioDecision, evaluate_candidate};
