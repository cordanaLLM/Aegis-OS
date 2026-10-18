// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! DMA-BUF sharing between DRM drivers on the reference profile (M25, E25-2).
//!
//! The buffer is a sealed `memfd` that `/dev/udmabuf` turns into a dma-buf,
//! which needs no privilege and no GPU allocator library. Each render node
//! imports it with `DRM_IOCTL_PRIME_FD_TO_HANDLE` and exports it again with
//! `DRM_IOCTL_PRIME_HANDLE_TO_FD`; the re-export is the same dma-buf when the
//! descriptors name one inode, which is how a second driver's import of the
//! first driver's export is shown. The bytes are read back through a mapping
//! of the dma-buf between `DMA_BUF_IOCTL_SYNC` start and end. That read is a
//! CPU read: no GPU reads the buffer here, and no line claims one.
//!
//! The checks in this file run before any syscall: an allocation outside the
//! page-aligned bounds the udmabuf module sets, and a declared modifier the
//! export does not carry, are refused without opening a device.

pub mod live;
pub mod sys;

use std::fmt;
use std::path::Path;

use sha2::Digest as _;

use crate::error::GpuError;
use crate::reader::{MAX_ATTR_BYTES, read_bounded};

pub use crate::dmabuf::live::{MAX_IMPORTERS, SHARE_DEADLINE, ShareRun, run_share};

/// `DRM_FORMAT_MOD_LINEAR`: the layout a udmabuf export carries.
pub const DRM_FORMAT_MOD_LINEAR: u64 = 0;

/// `I915_FORMAT_MOD_X_TILED`, the tiled modifier the negative case declares.
pub const I915_FORMAT_MOD_X_TILED: u64 = 0x0100_0000_0000_0001;

/// The sysfs parameter, under the sysfs root, that caps one udmabuf in MiB.
pub const UDMABUF_LIMIT_PARAM: &str = "module/udmabuf/parameters/size_limit_mb";

/// Bytes in one MiB, the unit of `size_limit_mb`.
const MIB: u64 = 1 << 20;

/// A request this slice refuses before it opens any device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShareRefusal {
    /// A zero-byte allocation.
    ZeroSize,
    /// A size that is not a multiple of the page size.
    NotPageAligned {
        /// The requested size in bytes.
        size: u64,
        /// The page size in bytes.
        page: u64,
    },
    /// A size above the udmabuf module's limit.
    OverLimit {
        /// The requested size in bytes.
        size: u64,
        /// The limit in bytes.
        limit: u64,
    },
    /// A declared modifier that differs from the export's.
    ModifierMismatch {
        /// The modifier the export carries.
        exported: u64,
        /// The modifier the importer declared.
        declared: u64,
    },
}

impl fmt::Display for ShareRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroSize => write!(f, "a 0-byte allocation is refused"),
            Self::NotPageAligned { size, page } => {
                write!(f, "{size} bytes is not a multiple of the {page}-byte page")
            }
            Self::OverLimit { size, limit } => {
                write!(
                    f,
                    "{size} bytes is above the udmabuf limit of {limit} bytes"
                )
            }
            Self::ModifierMismatch { exported, declared } => write!(
                f,
                "declared modifier {declared:#018x} differs from the export's {exported:#018x}"
            ),
        }
    }
}

/// The page size and the per-buffer limit that bound one allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShareBounds {
    /// The page size in bytes, the smallest allocation.
    pub page: u64,
    /// The udmabuf limit in bytes, the largest allocation.
    pub limit: u64,
}

impl ShareBounds {
    /// Refuses a size outside `[page, limit]` or not a multiple of `page`.
    ///
    /// # Errors
    ///
    /// The [`ShareRefusal`] that names the bound the size breaks.
    pub fn check(&self, size: u64) -> Result<(), ShareRefusal> {
        if size == 0 {
            return Err(ShareRefusal::ZeroSize);
        }
        if self.page == 0 || size.checked_rem(self.page) != Some(0) {
            return Err(ShareRefusal::NotPageAligned {
                size,
                page: self.page,
            });
        }
        if size > self.limit {
            return Err(ShareRefusal::OverLimit {
                size,
                limit: self.limit,
            });
        }
        Ok(())
    }
}

/// Reads the udmabuf limit under `sysfs` and pairs it with `page`.
///
/// # Errors
///
/// [`GpuError::Io`] when the parameter cannot be read (the module is not
/// loaded), [`GpuError::Malformed`] when it is no whole number of MiB, or one
/// whose byte count does not fit in 64 bits or is smaller than one page.
pub fn read_share_bounds(sysfs: &Path, page: u64) -> Result<ShareBounds, GpuError> {
    let path = sysfs.join(UDMABUF_LIMIT_PARAM);
    let text = read_bounded(&path, MAX_ATTR_BYTES).map_err(|err| GpuError::io(&path, err))?;
    let mib: u64 = text
        .trim()
        .parse()
        .map_err(|_| GpuError::malformed(&path, format!("not a MiB count: {:?}", text.trim())))?;
    let limit = mib
        .checked_mul(MIB)
        .ok_or_else(|| GpuError::malformed(&path, format!("{mib} MiB overflows 64 bits")))?;
    if page == 0 || limit < page {
        return Err(GpuError::malformed(
            &path,
            format!("a limit of {limit} bytes is below one {page}-byte page"),
        ));
    }
    Ok(ShareBounds { page, limit })
}

/// Refuses an import whose declared modifier is not the export's.
///
/// # Errors
///
/// [`ShareRefusal::ModifierMismatch`] when the two differ.
pub fn check_modifier(exported: u64, declared: u64) -> Result<(), ShareRefusal> {
    if exported == declared {
        Ok(())
    } else {
        Err(ShareRefusal::ModifierMismatch { exported, declared })
    }
}

/// The deterministic byte pattern written into a buffer of `len` bytes.
///
/// Byte `i` is the top byte of `i` times the 32-bit golden-ratio constant, so
/// neighbouring pages differ and a page read from the wrong offset mismatches.
#[must_use]
pub fn pattern(len: usize) -> Vec<u8> {
    (0..len)
        .map(|index| {
            let mixed = u64::try_from(index)
                .unwrap_or(u64::MAX)
                .wrapping_mul(0x9E37_79B9)
                .to_be_bytes();
            mixed.get(4).copied().unwrap_or(0)
        })
        .collect()
}

/// The lowercase hex sha256 of `bytes`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = sha2::Sha256::digest(bytes);
    digest
        .iter()
        .fold(String::with_capacity(64), |mut text, byte| {
            let _ = fmt::Write::write_fmt(&mut text, format_args!("{byte:02x}"));
            text
        })
}
