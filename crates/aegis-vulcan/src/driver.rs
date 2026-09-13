// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The user-space `PCIe` driver model: map once, then dispatch.
//!
//! This is the scaffold's `UserSpacePcieDriver` with its two assertions turned
//! into refusals. Mapping is a state change rather than a side effect:
//! [`UserSpacePcieDriver::map_bar`] moves the driver from unmapped to mapped
//! and refuses a second call, and
//! [`UserSpacePcieDriver::execute_direct_dma`] refuses to dispatch while the
//! driver is unmapped. The block-count rule is already held by
//! [`BlockCount`](crate::dma::BlockCount), so a dispatch cannot be reached
//! with a count the requirement refuses.
//!
//! # What this module does not do
//!
//! It maps nothing. There is no `mmap`, no `memmap2`, no file descriptor, no
//! `/dev/vfio`, no `CUDA` call and no peer-to-peer transfer anywhere in this
//! crate. "Mapped" is a boolean in a struct; a dispatch returns the byte
//! arithmetic the recorded requirement states and advances a ring index.
//! `tests/stubbed_effects.rs` is the regression gate that keeps it that way.

use crate::bar::BarWindow;
use crate::device::VfioDeviceConfig;
use crate::dma::{DmaRequest, TransferReceipt};
use crate::error::VulcanError;
use crate::ring::RingIndex;

/// Whether the driver has mapped its BAR window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MapState {
    /// No window is mapped; a dispatch is refused.
    Unmapped,
    /// The BAR 0 window is mapped; a dispatch is admitted.
    Mapped,
}

impl MapState {
    /// Returns the stable name this state is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Unmapped => "unmapped",
            Self::Mapped => "mapped",
        }
    }
}

/// The P03 user-space `PCIe` driver, as a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserSpacePcieDriver {
    config: VfioDeviceConfig,
    state: MapState,
    ring_head: RingIndex,
    ring_tail: RingIndex,
}

impl UserSpacePcieDriver {
    /// Builds an unmapped driver over `config`.
    #[must_use]
    pub const fn new(config: VfioDeviceConfig) -> Self {
        Self {
            config,
            state: MapState::Unmapped,
            ring_head: RingIndex::ZERO,
            ring_tail: RingIndex::ZERO,
        }
    }

    /// Returns the device configuration the driver was built over.
    #[must_use]
    pub const fn config(&self) -> VfioDeviceConfig {
        self.config
    }

    /// Returns whether a window is mapped.
    #[must_use]
    pub const fn state(&self) -> MapState {
        self.state
    }

    /// Returns the submission-ring head.
    #[must_use]
    pub const fn ring_head(&self) -> RingIndex {
        self.ring_head
    }

    /// Returns the submission-ring tail.
    #[must_use]
    pub const fn ring_tail(&self) -> RingIndex {
        self.ring_tail
    }

    /// Admits the BAR 0 window and returns it.
    ///
    /// The address was already refused if it was not page aligned, because
    /// [`BarAddress`](crate::bar::BarAddress) is the only way to spell one.
    ///
    /// # Errors
    ///
    /// Returns [`VulcanError::BarAlreadyMapped`] on a second call, and
    /// propagates [`VfioDeviceConfig::validate`].
    pub fn map_bar(&mut self) -> Result<BarWindow, VulcanError> {
        if matches!(self.state, MapState::Mapped) {
            return Err(VulcanError::BarAlreadyMapped);
        }
        self.config.validate()?;
        self.state = MapState::Mapped;
        Ok(self.config.bar0)
    }

    /// Accounts for one direct-DMA request and advances the submission tail.
    ///
    /// # Errors
    ///
    /// Returns [`VulcanError::BarUnmapped`] while no window is mapped.
    pub fn execute_direct_dma(
        &mut self,
        request: DmaRequest,
    ) -> Result<TransferReceipt, VulcanError> {
        if matches!(self.state, MapState::Unmapped) {
            return Err(VulcanError::BarUnmapped);
        }
        self.ring_tail = self.ring_tail.advance();
        Ok(TransferReceipt {
            bytes: request.transfer_bytes(),
            ring_tail: self.ring_tail,
        })
    }

    /// Marks one completed submission by advancing the head.
    ///
    /// # Errors
    ///
    /// Returns [`VulcanError::BarUnmapped`] while no window is mapped, and
    /// leaves the head where it is when it has caught the tail.
    pub fn complete(&mut self) -> Result<RingIndex, VulcanError> {
        if matches!(self.state, MapState::Unmapped) {
            return Err(VulcanError::BarUnmapped);
        }
        if self.ring_head != self.ring_tail {
            self.ring_head = self.ring_head.advance();
        }
        Ok(self.ring_head)
    }

    /// Returns how many submissions are outstanding.
    #[must_use]
    pub const fn outstanding(&self) -> usize {
        self.ring_head.distance_to(self.ring_tail)
    }
}
