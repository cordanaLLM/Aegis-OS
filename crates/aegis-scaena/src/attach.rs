// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! What the attach step checks before a received fd goes anywhere near the
//! compositor.
//!
//! In order: the fd is a DMA-BUF, read from its filesystem's magic with
//! `fstatfs` (`DMA_BUF_MAGIC`, `0x444d4142`, `include/uapi/linux/magic.h`), so
//! a memfd or any other file is refused; the descriptor lists as many planes
//! as its format has; each plane's pitch holds one of its rows; and each plane
//! ends inside the object, whose size is read with `lseek(SEEK_END)`, the way
//! the DMA-BUF documentation gives it. [`check`] runs all four and returns a
//! [`Checked`] frame, the only form the attach path takes; on a refusal the fd
//! is dropped, which closes it.
//!
//! None of this maps the fd: Aegis code never reads or writes the buffer, it
//! hands the descriptor to the compositor. The format and modifier pair is
//! checked against what the compositor advertised for the surface by
//! [`FormatTable`], client-side, so the refusal does not rest on one
//! compositor's conformance.

use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use rustix::fs::{SeekFrom, fstat, fstatfs, seek};

use crate::descriptor::{DecodedFrame, FourCc};
use crate::error::{AttachError, errno};

/// `DMA_BUF_MAGIC` from `include/uapi/linux/magic.h`: the filesystem magic of
/// every DMA-BUF fd.
pub const DMA_BUF_MAGIC: u64 = 0x444d_4142;

/// A file's identity: the device and inode `fstat` reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileIdentity {
    /// `st_dev`.
    pub dev: u64,
    /// `st_ino`.
    pub ino: u64,
}

/// The identity of the file behind `fd`.
///
/// # Errors
///
/// Returns [`AttachError::Io`] when `fstat` fails.
pub fn identity(fd: BorrowedFd<'_>) -> Result<FileIdentity, AttachError> {
    let stat = fstat(fd).map_err(|error| AttachError::Io {
        errno: errno(error),
    })?;
    Ok(FileIdentity {
        dev: stat.st_dev,
        ino: stat.st_ino,
    })
}

/// The magic of the filesystem behind `fd`, as `fstatfs` reports it.
///
/// # Errors
///
/// Returns [`AttachError::Io`] when `fstatfs` fails.
pub fn filesystem_magic(fd: BorrowedFd<'_>) -> Result<u64, AttachError> {
    let statfs = fstatfs(fd).map_err(|error| AttachError::Io {
        errno: errno(error),
    })?;
    let magic = i128::from(statfs.f_type);
    u64::try_from(magic).map_err(|_| AttachError::NotDmaBuf { magic: u64::MAX })
}

/// Refuses an fd whose filesystem is not the DMA-BUF one.
///
/// # Errors
///
/// Returns [`AttachError::NotDmaBuf`] naming the magic found.
pub fn require_dma_buf(fd: BorrowedFd<'_>) -> Result<(), AttachError> {
    let magic = filesystem_magic(fd)?;
    if magic == DMA_BUF_MAGIC {
        Ok(())
    } else {
        Err(AttachError::NotDmaBuf { magic })
    }
}

/// The object's size, read with `lseek(SEEK_END)`; the offset is put back to
/// the start afterwards.
///
/// # Errors
///
/// Returns [`AttachError::Io`] when either seek fails.
pub fn object_size(fd: BorrowedFd<'_>) -> Result<u64, AttachError> {
    let size = seek(fd, SeekFrom::End(0)).map_err(|error| AttachError::Io {
        errno: errno(error),
    })?;
    seek(fd, SeekFrom::Start(0)).map_err(|error| AttachError::Io {
        errno: errno(error),
    })?;
    Ok(size)
}

/// One plane of a known format: how many rows it has and how many bytes one
/// of its rows holds at least, for a frame of the given size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaneShape {
    /// Rows in the plane.
    pub rows: u64,
    /// The fewest bytes one row occupies.
    pub row_bytes: u64,
}

/// The plane shapes of `fourcc` at `width` x `height`, or `None` for a format
/// whose layout this crate does not know. NV12 is a full-size luma plane and
/// a half-height plane of interleaved chroma pairs; XRGB8888 and ARGB8888 are
/// one plane of four bytes per pixel.
#[must_use]
pub fn plane_shapes(fourcc: FourCc, width: u32, height: u32) -> Option<([PlaneShape; 2], usize)> {
    let (width, height) = (u64::from(width), u64::from(height));
    let empty = PlaneShape {
        rows: 0,
        row_bytes: 0,
    };
    if fourcc == FourCc::NV12 {
        let luma = PlaneShape {
            rows: height,
            row_bytes: width,
        };
        let chroma = PlaneShape {
            rows: height.div_ceil(2),
            row_bytes: width.div_ceil(2).checked_mul(2)?,
        };
        return Some(([luma, chroma], 2));
    }
    if fourcc == FourCc::XRGB8888 || fourcc == FourCc::ARGB8888 {
        let packed = PlaneShape {
            rows: height,
            row_bytes: width.checked_mul(4)?,
        };
        return Some(([packed, empty], 1));
    }
    None
}

/// Checks every plane of `frame` against its format and an object of `size`
/// bytes: the plane count, each pitch, and that each plane ends at or before
/// the end of the object.
///
/// # Errors
///
/// Returns [`AttachError::UnsupportedFormat`], [`AttachError::PlaneLayout`],
/// [`AttachError::PitchTooSmall`] or [`AttachError::PlaneOutOfBounds`].
pub fn check_layout(frame: &DecodedFrame, size: u64) -> Result<(), AttachError> {
    let (shapes, expected) = plane_shapes(frame.fourcc, frame.width, frame.height).ok_or(
        AttachError::UnsupportedFormat {
            fourcc: frame.fourcc,
        },
    )?;
    let listed = frame.planes.len();
    if listed != expected {
        return Err(AttachError::PlaneLayout { expected, listed });
    }
    for (plane, (layout, shape)) in frame.planes.as_slice().iter().zip(shapes).enumerate() {
        let pitch = u64::from(layout.pitch);
        if pitch < shape.row_bytes {
            return Err(AttachError::PitchTooSmall { plane });
        }
        let end = pitch
            .checked_mul(shape.rows)
            .and_then(|bytes| bytes.checked_add(u64::from(layout.offset)))
            .unwrap_or(u64::MAX);
        if end > size {
            return Err(AttachError::PlaneOutOfBounds { plane, end, size });
        }
    }
    Ok(())
}

/// A frame whose fd passed every check: the only form the attach path takes.
#[derive(Debug)]
pub struct Checked {
    frame: DecodedFrame,
    fd: OwnedFd,
    identity: FileIdentity,
    size: u64,
}

impl Checked {
    /// The descriptor.
    #[must_use]
    pub const fn frame(&self) -> &DecodedFrame {
        &self.frame
    }

    /// The DMA-BUF.
    #[must_use]
    pub fn fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    /// The received file's identity, to compare with the exported one.
    #[must_use]
    pub const fn identity(&self) -> FileIdentity {
        self.identity
    }

    /// The object's size in bytes.
    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    /// Gives the fd up, for the call that hands it to the compositor.
    #[must_use]
    pub fn into_fd(self) -> OwnedFd {
        self.fd
    }
}

/// Runs every attach check on a received frame.
///
/// # Errors
///
/// Returns the first refusal; the fd is closed with it.
pub fn check(frame: DecodedFrame, fd: OwnedFd) -> Result<Checked, AttachError> {
    require_dma_buf(fd.as_fd())?;
    let size = object_size(fd.as_fd())?;
    check_layout(&frame, size)?;
    let identity = identity(fd.as_fd())?;
    Ok(Checked {
        frame,
        fd,
        identity,
        size,
    })
}

/// The most format and modifier pairs one surface's feedback may advertise.
pub const MAX_FORMAT_PAIRS: usize = 1_024;

/// The format and modifier pairs the compositor advertised for one surface,
/// filled once from its `zwp_linux_dmabuf_feedback_v1` tranches.
#[derive(Debug, Clone, Default)]
pub struct FormatTable {
    pairs: Vec<(u32, u64)>,
}

impl FormatTable {
    /// An empty table, its storage reserved once so filling it does not
    /// reallocate.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pairs: Vec::with_capacity(MAX_FORMAT_PAIRS),
        }
    }

    /// Records one advertised pair; a repeat is kept once.
    ///
    /// # Errors
    ///
    /// Returns [`AttachError::TableFull`] past [`MAX_FORMAT_PAIRS`].
    pub fn advertise(&mut self, fourcc: FourCc, modifier: u64) -> Result<(), AttachError> {
        let pair = (fourcc.code(), modifier);
        if self.pairs.contains(&pair) {
            return Ok(());
        }
        if self.pairs.len() >= MAX_FORMAT_PAIRS {
            return Err(AttachError::TableFull);
        }
        self.pairs.push(pair);
        Ok(())
    }

    /// How many distinct pairs were advertised.
    #[must_use]
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    /// Whether nothing was advertised.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// Refuses a pair the compositor did not advertise, client-side.
    ///
    /// # Errors
    ///
    /// Returns [`AttachError::NotAdvertised`].
    pub fn admit(&self, fourcc: FourCc, modifier: u64) -> Result<(), AttachError> {
        if self.pairs.contains(&(fourcc.code(), modifier)) {
            Ok(())
        } else {
            Err(AttachError::NotAdvertised { fourcc, modifier })
        }
    }
}
