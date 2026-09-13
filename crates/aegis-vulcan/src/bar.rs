// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The `PCIe` Base Address Register window, validated on construction.
//!
//! REQ-P03-04 is one rule: the BAR physical address mapped into user space is
//! a multiple of [`BAR_ALIGNMENT_BYTES`]. The scaffold states it as an
//! assertion inside the mapping call, which means an unaligned address is a
//! process abort. Here it is a constructor refusal, so an address that is not
//! page aligned never becomes a [`BarAddress`] at all and the negative case is
//! an ordinary test.
//!
//! # What is bounded, and what deliberately is not
//!
//! The width of the window is bounded by [`MAX_BAR_BYTES`], which is above the
//! largest window the reference profile reports (a 32 GiB resizable BAR1), and
//! the window is refused if base plus width would leave the address space.
//! The width is **not** required to be a multiple of the page alignment: the
//! recorded requirement is about the address, and inventing a second rule here
//! would make the type refuse windows the requirement admits.
//!
//! # What this module does not do
//!
//! Nothing here opens a `VFIO` container, maps anything, or touches
//! `/sys/bus/pci`. A [`BarWindow`] is a validated pair of integers.

use crate::error::VulcanError;

/// The page alignment a mapped BAR physical address must satisfy.
///
/// The value is the scaffold's `BAR_ALIGNMENT_BYTES` (export-038
/// `fe2cb01cfd13`), and it is the x86-64 base page size.
pub const BAR_ALIGNMENT_BYTES: u64 = 4096;

/// Scalar upper bound, in bytes, on one BAR window.
///
/// 64 GiB. The reference profile recorded in `planning/hardware-profile.json`
/// reports a 32 GiB resizable BAR1 on the discrete card, so the bound leaves
/// one doubling of headroom and still refuses a window that could only come
/// from a decoding mistake.
pub const MAX_BAR_BYTES: u64 = 64 * 1024 * 1024 * 1024;

/// A page-aligned BAR physical address.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(into = "u64", try_from = "u64")]
pub struct BarAddress(u64);

impl BarAddress {
    /// Validates a BAR physical address.
    ///
    /// # Errors
    ///
    /// Returns [`VulcanError::BarMisaligned`] when `address` is not a multiple
    /// of [`BAR_ALIGNMENT_BYTES`]. The address is refused, never rounded.
    pub const fn new(address: u64) -> Result<Self, VulcanError> {
        if address % BAR_ALIGNMENT_BYTES != 0 {
            return Err(VulcanError::BarMisaligned { address });
        }
        Ok(Self(address))
    }

    /// Returns the validated address.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl From<BarAddress> for u64 {
    fn from(value: BarAddress) -> Self {
        value.0
    }
}

impl TryFrom<u64> for BarAddress {
    type Error = VulcanError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// The width of a BAR window, in bytes.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(into = "u64", try_from = "u64")]
pub struct BarSize(u64);

impl BarSize {
    /// Validates a BAR window width.
    ///
    /// # Errors
    ///
    /// Returns [`VulcanError::BarSizeOutOfRange`] for zero or for a width past
    /// [`MAX_BAR_BYTES`].
    pub const fn new(bytes: u64) -> Result<Self, VulcanError> {
        if bytes == 0 || bytes > MAX_BAR_BYTES {
            return Err(VulcanError::BarSizeOutOfRange { bytes });
        }
        Ok(Self(bytes))
    }

    /// Returns the validated width in bytes.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl From<BarSize> for u64 {
    fn from(value: BarSize) -> Self {
        value.0
    }
}

impl TryFrom<u64> for BarSize {
    type Error = VulcanError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// A validated BAR window: a page-aligned base and a bounded width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct BarWindow {
    /// The page-aligned base address of the window.
    pub address: BarAddress,
    /// The width of the window in bytes.
    pub size: BarSize,
}

impl BarWindow {
    /// Builds a window from a validated base and width.
    ///
    /// # Errors
    ///
    /// Returns [`VulcanError::BarWindowOverflow`] when the window would run
    /// past the end of the 64-bit address space.
    pub const fn new(address: BarAddress, size: BarSize) -> Result<Self, VulcanError> {
        if address.get().checked_add(size.get()).is_none() {
            return Err(VulcanError::BarWindowOverflow {
                address: address.get(),
                bytes: size.get(),
            });
        }
        Ok(Self { address, size })
    }

    /// Validates a window given as raw integers.
    ///
    /// # Errors
    ///
    /// Propagates [`BarAddress::new`], [`BarSize::new`] and [`Self::new`].
    pub const fn parse(address: u64, bytes: u64) -> Result<Self, VulcanError> {
        let address = match BarAddress::new(address) {
            Ok(address) => address,
            Err(error) => return Err(error),
        };
        let size = match BarSize::new(bytes) {
            Ok(size) => size,
            Err(error) => return Err(error),
        };
        Self::new(address, size)
    }

    /// Returns the first address past the window.
    ///
    /// The addition cannot overflow: [`Self::new`] refused the window if it
    /// could.
    #[must_use]
    pub const fn end(self) -> u64 {
        self.address.get().saturating_add(self.size.get())
    }

    /// Checks the invariants a decoded window could otherwise arrive without.
    ///
    /// Both fields are public, so a window can be built by struct literal or
    /// decoded field by field. Each field validates itself, but the
    /// relationship between them does not, which is what this restates.
    ///
    /// # Errors
    ///
    /// Returns [`VulcanError::BarWindowOverflow`] for a window that leaves the
    /// address space.
    pub const fn validate(&self) -> Result<(), VulcanError> {
        match Self::new(self.address, self.size) {
            Ok(_) => Ok(()),
            Err(error) => Err(error),
        }
    }
}
