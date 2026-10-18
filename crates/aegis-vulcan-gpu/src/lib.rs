// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P03 `aegis-vulcan-gpu`: GPU host slices on the reference profile
//! (milestone M25, slice 1).
//!
//! This crate implements the host inspection half of the M25 milestone, measuring
//! GPU topology and capabilities through read-only sysfs inspection and unprivileged
//! `/dev` device node stat calls.
//!
//! The record half opens, binds, unbinds and writes no hardware device. The
//! [`dmabuf`] half (E25-2) opens `/dev/udmabuf` and the render nodes to share
//! one buffer between drivers; it binds and unbinds nothing.
//!
//! # Unsafe Code
//!
//! This crate is the workspace's one unsafe home (ledger
//! `aegis-os-m25-unsafe-home`). Its manifest restates the workspace lints with
//! `unsafe_code = "deny"` in place of `forbid`, so `unsafe` compiles only inside
//! a function that carries `#[allow(unsafe_code)]`, and every `unsafe` block
//! carries a `// SAFETY:` proof. `tools/verify_unsafe_home.py` holds that, holds
//! every other member at `forbid`, and holds the dated category C exception in
//! `.standards.yaml`.

pub mod dmabuf;
pub mod drm_sched;
pub mod error;
pub mod gpus;
pub mod heap;
pub mod reader;
pub mod rebar;
pub mod record;
pub mod render_nodes;
pub mod vfio;

pub use crate::dmabuf::{
    ShareBounds, ShareRefusal, ShareRun, check_modifier, read_share_bounds, run_share,
};
pub use crate::drm_sched::{DrmSchedRecord, MAX_HOLDERS, inspect_holders, is_provider};
pub use crate::error::GpuError;
pub use crate::gpus::{
    GpuRecord, MAX_CARD_ENTRIES, MAX_DRM_ENTRIES, MAX_PCI_DEVICES, discover_gpus,
    read_candidate_input,
};
pub use crate::heap::{DmaHeapRecord, inspect_dma_heap};
pub use crate::reader::{MAX_ATTR_BYTES, read_bounded};
pub use crate::rebar::{BarResizeRecord, MAX_BARS, inspect_device_rebar, parse_resize_bitmask};
pub use crate::record::{GPU_RECORD_SCHEMA, GpuRecordDocument, RecordRun, run_record};
pub use crate::render_nodes::{
    MAX_MODESET_BYTES, MAX_RENDER_NODES, RenderNodeInfo, inspect_render_nodes,
};
pub use crate::vfio::{
    MapBarRun, VfioCandidateInput, VfioDecision, choose_bar_to_map, evaluate_candidate,
    format_bar_region, format_region_flags, run_map_bar,
};
