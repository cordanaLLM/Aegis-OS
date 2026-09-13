// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Locality enforcement: which logical CPUs a critical thread may run on
//! (REQ-P07-02).
//!
//! The P07 report requires `isolcpus` to pin the performance-critical threads
//! -- the Wayland compositor first among them -- to physical cores, as its
//! Locality Enforcement rule P-002 (export-016 `cfa58b5b23b8`). What that
//! needs from user space is a set of logical CPU indices, and this module is
//! that set and nothing more.
//!
//! # What this module does not do
//!
//! **No affinity is set and no kernel parameter is written.** There is no
//! `sched_setaffinity`, no `cpuset` controller write and no boot-parameter
//! edit. [`CoreMask`] is a 64-bit word; whether a machine has the cores it
//! names is not checked here, because checking would mean reading
//! `/sys/devices/system/cpu`, which this crate does not do. The reference
//! machine's own topology is recorded once, in
//! `planning/hardware-profile.json`, and a mask is not evidence about it.

use crate::error::LictorError;

/// The number of logical CPUs one mask can name.
///
/// A mask is one 64-bit word, so the width is 64. A machine with more logical
/// CPUs than that needs a wider mask, which is a change to this type rather
/// than a run-time condition, and the refusal below is what makes that visible
/// instead of silent.
pub const CORE_MASK_WIDTH: u32 = 64;

/// A set of logical CPU indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CoreMask {
    bits: u64,
}

impl CoreMask {
    /// Builds an empty mask.
    #[must_use]
    pub const fn new() -> Self {
        Self { bits: 0 }
    }

    /// Returns the mask with `core` added.
    ///
    /// # Errors
    ///
    /// Returns [`LictorError::CoreOutOfRange`] at or above
    /// [`CORE_MASK_WIDTH`], so an index the mask cannot hold is refused rather
    /// than shifted out.
    pub const fn with(self, core: u32) -> Result<Self, LictorError> {
        if core >= CORE_MASK_WIDTH {
            return Err(LictorError::CoreOutOfRange {
                core,
                width: CORE_MASK_WIDTH,
            });
        }
        Ok(Self {
            bits: self.bits | (1u64 << core),
        })
    }

    /// Returns `true` when the mask names `core`.
    ///
    /// An index the mask cannot hold is not in it, which is why this returns a
    /// `bool` rather than a `Result`: asking whether core 64 is in a 64-wide
    /// mask has an answer.
    #[must_use]
    pub const fn contains(self, core: u32) -> bool {
        if core >= CORE_MASK_WIDTH {
            return false;
        }
        self.bits & (1u64 << core) != 0
    }

    /// Returns how many logical CPUs the mask names.
    #[must_use]
    pub const fn count(self) -> u32 {
        self.bits.count_ones()
    }

    /// Returns `true` when the mask names no logical CPU.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    /// Returns the raw bits.
    #[must_use]
    pub const fn bits(self) -> u64 {
        self.bits
    }
}
