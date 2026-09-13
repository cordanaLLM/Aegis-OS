// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The direct-DMA request: what is read, how much of it, and where it lands.
//!
//! REQ-P03-05 bounds one field: the block count is at least
//! [`MIN_BLOCK_COUNT`] and at most [`MAX_BLOCK_COUNT`]. The scaffold asserts
//! it at dispatch; [`BlockCount::new`] refuses it at construction, so a
//! dispatch cannot be reached with an out-of-range count and the boundary
//! cases are ordinary tests.
//!
//! # What this module does not do
//!
//! Nothing here issues a command, touches an `NVMe` queue or writes to GPU
//! memory. A [`TransferReceipt`] is arithmetic over validated integers: the
//! byte count the recorded requirement says such a transfer would move, and
//! the ring slot the tail advanced to.

use crate::error::VulcanError;
use crate::ring::RingIndex;

/// The smallest admissible block count.
pub const MIN_BLOCK_COUNT: u32 = 1;

/// The largest admissible block count (REQ-P03-05).
pub const MAX_BLOCK_COUNT: u32 = 8192;

/// The width of one `NVMe` logical block, in bytes.
pub const NVME_BLOCK_BYTES: u64 = 512;

/// A validated `NVMe` block count, within `1..=8192`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(into = "u32", try_from = "u32")]
pub struct BlockCount(u32);

impl BlockCount {
    /// The largest admissible count, as a value.
    pub const MAX: Self = Self(MAX_BLOCK_COUNT);

    /// The smallest admissible count, as a value.
    pub const MIN: Self = Self(MIN_BLOCK_COUNT);

    /// Validates a block count.
    ///
    /// # Errors
    ///
    /// Returns [`VulcanError::BlockCountOutOfRange`] for zero and for anything
    /// past [`MAX_BLOCK_COUNT`].
    pub const fn new(blocks: u32) -> Result<Self, VulcanError> {
        if blocks < MIN_BLOCK_COUNT || blocks > MAX_BLOCK_COUNT {
            return Err(VulcanError::BlockCountOutOfRange { blocks });
        }
        Ok(Self(blocks))
    }

    /// Returns the validated count.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Returns the bytes a transfer of this many blocks moves.
    ///
    /// The product cannot overflow: the count is bounded by
    /// [`MAX_BLOCK_COUNT`], so the widest transfer is 4 MiB.
    #[must_use]
    pub const fn transfer_bytes(self) -> u64 {
        (self.0 as u64).saturating_mul(NVME_BLOCK_BYTES)
    }
}

impl From<BlockCount> for u32 {
    fn from(value: BlockCount) -> Self {
        value.0
    }
}

impl TryFrom<u32> for BlockCount {
    type Error = VulcanError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// An `NVMe` logical block address: where a transfer starts on the source.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct Lba(u64);

impl Lba {
    /// Names a logical block address.
    ///
    /// Every 64-bit value is a representable LBA, so this cannot fail. The
    /// newtype exists so an LBA and a device address cannot be swapped at a
    /// call site.
    #[must_use]
    pub const fn new(block: u64) -> Self {
        Self(block)
    }

    /// Returns the logical block address.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A device-visible destination address for a peer transfer.
///
/// Named `VramAddress` after the scaffold's `vram_phys_addr`. It is the
/// address the DMA lands at on the far side of the link; binding it to a real
/// GPU allocation is M25 work over real hardware.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct VramAddress(u64);

impl VramAddress {
    /// Names a device-visible destination address.
    #[must_use]
    pub const fn new(address: u64) -> Self {
        Self(address)
    }

    /// Returns the destination address.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// One validated direct-DMA request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DmaRequest {
    /// Where the transfer starts on the `NVMe` source.
    pub lba: Lba,
    /// Where the transfer lands on the peer device.
    pub destination: VramAddress,
    /// How many logical blocks the transfer moves.
    pub blocks: BlockCount,
}

impl DmaRequest {
    /// Builds a request from validated parts.
    #[must_use]
    pub const fn new(lba: Lba, destination: VramAddress, blocks: BlockCount) -> Self {
        Self {
            lba,
            destination,
            blocks,
        }
    }

    /// Returns the bytes this request would move.
    #[must_use]
    pub const fn transfer_bytes(self) -> u64 {
        self.blocks.transfer_bytes()
    }
}

/// What a dispatched request accounted for.
///
/// It records arithmetic, not an effect: no data moved, because nothing in
/// this crate can move any.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransferReceipt {
    /// The bytes the recorded requirement says the request moves.
    pub bytes: u64,
    /// The ring slot the submission tail advanced to.
    pub ring_tail: RingIndex,
}
