// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Why a P03 value or a P03 dispatch was refused.
//!
//! Every refusal is a value, never a panic. The imported scaffold
//! (export-038 `fe2cb01cfd13`) states each of its rules as an `assert!`, so a
//! misaligned address, an unmapped window or an out-of-range block count
//! aborted the process and could only be covered by a test that expects a
//! panic. Here each of those rules is a constructor or a method returning
//! `Result`, which is what HISS-07 asks for and what makes the negative cases
//! ordinary tests.
//!
//! The mapping from scaffold assertion to refusal is fixed and swept by
//! `tests/scaffold_assertions.rs`:
//!
//! | Scaffold assertion | Refusal |
//! | :-- | :-- |
//! | BAR physical address is page aligned | [`VulcanError::BarMisaligned`] |
//! | cannot dispatch DMA without a mapped BAR | [`VulcanError::BarUnmapped`] |
//! | block count is within bounds | [`VulcanError::BlockCountOutOfRange`] |

use crate::bar::{BAR_ALIGNMENT_BYTES, MAX_BAR_BYTES};
use crate::device::MAX_VFIO_DEVICES;
use crate::dma::{MAX_BLOCK_COUNT, MIN_BLOCK_COUNT};

/// Reasons a P03 value or dispatch is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum VulcanError {
    /// The BAR physical address is not a multiple of the page alignment.
    ///
    /// REQ-P03-04, stated in the scaffold as an assertion.
    #[error("BAR physical address {address:#x} is not a multiple of {BAR_ALIGNMENT_BYTES}")]
    BarMisaligned {
        /// The address that was refused.
        address: u64,
    },
    /// The BAR window is empty or wider than the recorded bound.
    #[error("a BAR window of {bytes} bytes is outside 1..={MAX_BAR_BYTES}")]
    BarSizeOutOfRange {
        /// The width that was refused.
        bytes: u64,
    },
    /// The BAR window would run past the end of the address space.
    #[error("a BAR window at {address:#x} of {bytes} bytes runs past the address space")]
    BarWindowOverflow {
        /// The window base.
        address: u64,
        /// The window width.
        bytes: u64,
    },
    /// A DMA dispatch was attempted with no mapped BAR.
    ///
    /// The second scaffold assertion.
    #[error("no BAR is mapped; a direct DMA transfer cannot be dispatched")]
    BarUnmapped,
    /// The BAR was already mapped.
    ///
    /// Mapping twice is refused rather than silently repeated, so a driver
    /// cannot hold two readings of one window.
    #[error("the BAR is already mapped")]
    BarAlreadyMapped,
    /// The block count is outside the bounds the recorded requirement states.
    ///
    /// REQ-P03-05, the third scaffold assertion.
    #[error("a block count of {blocks} is outside {MIN_BLOCK_COUNT}..={MAX_BLOCK_COUNT}")]
    BlockCountOutOfRange {
        /// The count that was refused.
        blocks: u32,
    },
    /// The device table is full.
    #[error("the device table holds its bound of {MAX_VFIO_DEVICES} devices")]
    DeviceTableFull,
    /// A second device claimed an `IOMMU` group another device already holds.
    ///
    /// REQ-P03-06: the isolation model is one `VFIO` group per protection
    /// domain, so two devices sharing a group is a modelling error rather
    /// than a configuration to accept.
    #[error("IOMMU group {group} is already held by another device")]
    GroupAlreadyHeld {
        /// The group that was claimed twice.
        group: u32,
    },
}
