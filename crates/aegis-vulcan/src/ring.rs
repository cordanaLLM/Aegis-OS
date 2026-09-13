// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The lock-less `NVMe` submission-ring index.
//!
//! The scaffold advances its tail with `ring_tail = (ring_tail + 1) %
//! NVME_RING_SIZE` on a bare `usize`, which is correct only for as long as
//! every writer remembers the modulus. [`RingIndex`] carries the modulus in
//! the type: the only way to move an index is [`RingIndex::advance`], and the
//! only way to build one out of an integer is [`RingIndex::new`], which
//! refuses anything at or past [`NVME_RING_SIZE`].
//!
//! Wrapping is the behaviour, not an edge case: advancing the last slot
//! returns [`RingIndex::ZERO`]. `tests/ring_index.rs` holds that boundary.

/// The number of slots in the `NVMe` submission ring.
///
/// The scaffold's `NVME_RING_SIZE` (export-038 `fe2cb01cfd13`).
pub const NVME_RING_SIZE: usize = 1024;

/// An index into the `NVMe` submission ring, always below [`NVME_RING_SIZE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RingIndex(usize);

impl RingIndex {
    /// The first slot of the ring, and the index a fresh driver starts at.
    pub const ZERO: Self = Self(0);

    /// The last slot of the ring: the one [`Self::advance`] wraps from.
    pub const LAST: Self = Self(NVME_RING_SIZE.saturating_sub(1));

    /// Builds an index, refusing one at or past [`NVME_RING_SIZE`].
    ///
    /// Returns `None` rather than folding the value, so a caller that has
    /// computed an out-of-ring index learns about it instead of writing to a
    /// slot it did not mean.
    #[must_use]
    pub const fn new(slot: usize) -> Option<Self> {
        if slot >= NVME_RING_SIZE {
            return None;
        }
        Some(Self(slot))
    }

    /// Returns the slot this index names.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }

    /// Returns the next slot, wrapping at [`NVME_RING_SIZE`].
    ///
    /// This is the scaffold's tail update, with the modulus owned by the type.
    #[must_use]
    pub const fn advance(self) -> Self {
        let next = self.0.saturating_add(1);
        if next >= NVME_RING_SIZE {
            return Self::ZERO;
        }
        Self(next)
    }

    /// Returns how many advances separate `self` from `other`, forwards.
    ///
    /// The ring is circular, so the distance from the last slot to the first
    /// is one, not a negative number.
    #[must_use]
    pub const fn distance_to(self, other: Self) -> usize {
        if other.0 >= self.0 {
            return other.0.saturating_sub(self.0);
        }
        NVME_RING_SIZE
            .saturating_sub(self.0)
            .saturating_add(other.0)
    }
}
