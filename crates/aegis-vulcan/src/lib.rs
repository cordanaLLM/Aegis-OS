// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P03 `aegis-vulcan`: the validation slice of the direct-DMA engine.
//!
//! The imported P03 scaffold (export-038 `fe2cb01cfd13`) is a daemon that
//! prints what it would do, guarded by three `assert!` calls. Milestone M17
//! extracts the part that can be checked without hardware -- the arithmetic
//! and the bounds -- and restates each assertion as a refusal, which is what
//! HISS-07 asks for: a misaligned address, an unmapped window or an
//! out-of-range block count is a `Result`, not a process abort, so the
//! negative cases are ordinary tests.
//!
//! # The four things a reviewer should look at
//!
//! * [`BarAddress`] holds REQ-P03-04. The page-alignment rule is a
//!   constructor, so an unaligned address never becomes one.
//! * [`BlockCount`] holds REQ-P03-05 at `1..=8192`, and [`RingIndex`] owns the
//!   modulus of the submission ring, so the scaffold's `(tail + 1) %
//!   NVME_RING_SIZE` cannot be written without it.
//! * [`contracts`] carries the two versioned descriptors P03 produces:
//!   [`WeightStreamDescriptor`] for P09 Minerva and [`MediaIngestDescriptor`]
//!   for P15 Hestia. Both carry a [`DmaBufExport`], which is the field M25
//!   fills in with a real render node.
//! * [`mandate`] records the numbers this crate cannot measure -- the 20 W
//!   power budget, the six protection boundaries and the 10 microsecond
//!   latency budget -- as a register, not as an enforcement.
//!
//! # Design invariants (see `AGENTS.md`)
//!
//! * no recursion: every function here is a flat check or a field access, and
//!   no function in the crate calls itself directly or through another;
//! * every loop carries a scalar upper bound: [`MAX_VFIO_DEVICES`],
//!   [`NVME_RING_SIZE`], [`MAX_CORRELATION_LEN`] and
//!   [`MAX_CONTRACT_PAYLOAD_BYTES`];
//! * every value on a decision path is `Copy`, so no such path allocates;
//!   `tests/allocation_bounds.rs` is the falsifier, and the one place that is
//!   deliberately not claimed -- a JSON string carrying an escape, which
//!   `serde_json` unescapes into a heap scratch buffer before any field of
//!   ours sees it -- is tested rather than denied;
//! * no `unwrap`, `expect`, `panic!`, slice indexing or unchecked arithmetic,
//!   and no `unsafe` (forbidden at the workspace root).
//!
//! # What this crate does not do
//!
//! It maps nothing and transfers nothing. There is no `mmap`, no `memmap2`,
//! no `VFIO` container, no `IOMMU` domain, no `NVMe` queue, no `CUDA` call, no
//! peer-to-peer transfer, no `DMA-BUF` file descriptor and no render node opened
//! anywhere in it. "Mapped" is a state in a struct and a transfer is
//! arithmetic. `tests/stubbed_effects.rs` sweeps the crate's own sources for a
//! recorded list of identifiers that would be needed to do any of it and fails
//! if one appears -- a regression gate over an enumeration, not a proof over
//! every such identifier.
//!
//! A pass of this crate's tests is evidence about the validation arithmetic.
//! It closes no hardware gate: the reference profile records P03's
//! peer-to-peer requirement as **missing**, and `IOMMU`, `VFIO`, `NVMe` and GPU
//! peer-to-peer evidence remain milestone M25 work.
//!
//! # Example
//!
//! ```
//! use aegis_vulcan::{
//!     BarWindow, BlockCount, DmaRequest, IommuGroup, Lba, UserSpacePcieDriver,
//!     VfioDeviceConfig, VramAddress,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let bar = BarWindow::parse(0xf720_0000, 16384)?;
//! let device = VfioDeviceConfig::new(IommuGroup::new(12), 0x144d, 0xa808, bar);
//! let mut driver = UserSpacePcieDriver::new(device);
//!
//! driver.map_bar()?;
//! let request = DmaRequest::new(Lba::new(0x1000), VramAddress::new(0xe000_0000), BlockCount::new(64)?);
//! let receipt = driver.execute_direct_dma(request)?;
//!
//! assert_eq!(receipt.bytes, 64 * 512);
//! assert_eq!(receipt.ring_tail.get(), 1);
//! assert!(BlockCount::new(8193).is_err());
//! # Ok(())
//! # }
//! ```

pub mod bar;
pub mod contracts;
pub mod device;
pub mod dma;
pub mod driver;
pub mod error;
pub mod id;
pub mod mandate;
pub mod ring;

pub use crate::bar::{BAR_ALIGNMENT_BYTES, BarAddress, BarSize, BarWindow, MAX_BAR_BYTES};
pub use crate::contracts::dmabuf::{
    DmaBufError, DmaBufExport, DrmDriver, ModesetState, REFERENCE_PROFILE_EXPORTS,
    RENDER_NODE_MINOR_BASE, RENDER_NODE_MINOR_MAX, RENDER_NODE_PREFIX, RenderNode,
};
pub use crate::contracts::graph::EdgeId;
pub use crate::contracts::media_ingest::{MediaIngestDescriptor, MediaIngestVersion};
pub use crate::contracts::weight_stream::{WeightStreamDescriptor, WeightStreamVersion};
pub use crate::contracts::{
    ContractError, Correlation, MAX_CONTRACT_PAYLOAD_BYTES, PayloadBuffer, SchemaId,
};
pub use crate::device::{DeviceTable, IommuGroup, MAX_VFIO_DEVICES, VfioDeviceConfig};
pub use crate::dma::{
    BlockCount, DmaRequest, Lba, MAX_BLOCK_COUNT, MIN_BLOCK_COUNT, NVME_BLOCK_BYTES,
    TransferReceipt, VramAddress,
};
pub use crate::driver::{MapState, UserSpacePcieDriver};
pub use crate::error::VulcanError;
pub use crate::id::{CorrelationId, IdError, MAX_CORRELATION_LEN};
pub use crate::mandate::{Citation, EngineeringMandate, P03_MANDATE, ProtectionBoundary};
pub use crate::ring::{NVME_RING_SIZE, RingIndex};
