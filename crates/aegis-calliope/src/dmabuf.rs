// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded DMA-BUF descriptor table and its stride arithmetic
//! (REQ-P08-01, REQ-P08-06).
//!
//! This is the scaffold's `register_dma_buf_stream` with the three things it
//! lacked: a bound on how many descriptors may exist, checked stride
//! arithmetic, and a pixel format that is read rather than asserted.
//!
//! # The recorded conflict in the scaffold's descriptor
//!
//! The scaffold fills one descriptor with `stride: width * 4` and
//! `fourcc: 0x34325641`, under the comment `// NV12 / ARGB fourcc`. Those
//! three statements cannot all hold:
//!
//! * `0x34325641` is the little-endian four-character code `AV24`. It is
//!   neither `NV12` (`0x3231564E`) nor `AR24` (`0x34325241`), the DRM code for
//!   `ARGB8888`, so the comment names two formats and the literal names a
//!   third;
//! * `width * 4` is the row stride of a packed 32-bit format such as
//!   `ARGB8888`. `NV12` is planar 8-bit luma, whose stride is `width`, so the
//!   same line cannot describe both formats.
//!
//! This module does not reconcile the two. [`PixelFormat`] admits the two
//! formats the comment names, each with its own bytes-per-pixel, and
//! [`SCAFFOLD_FOURCC`] is admitted by neither: a descriptor carrying it is
//! [`CalliopeError::UnknownPixelFormat`]. The conflict is recorded in
//! [`P08_RECORDED_CLAIMS`](crate::register::P08_RECORDED_CLAIMS).
//!
//! # What this module does not do
//!
//! **No DMA-BUF is allocated, exported or imported, and no file descriptor is
//! opened or held.** The scaffold's `fd: RawFd` field is deliberately absent:
//! a file descriptor this crate did not obtain from the kernel is a number
//! pretending to be a capability, and the scaffold's literal `42` is exactly
//! that. [`BufferHandle`] is an opaque table-local identifier and is
//! documented as one.

use core::fmt;

use crate::error::CalliopeError;

/// Scalar upper bound on the DMA-BUF descriptor table.
///
/// The scaffold's `MAX_DMA_BUFFERS`, labelled there as an explicit NASA JPL
/// P10-2 upper bound (export-026 `c2f1e433cd32`).
pub const MAX_DMA_BUFFERS: usize = 64;

/// The four-character code the scaffold writes into every descriptor.
///
/// `0x34325641` little-endian is `AV24`. See the module documentation: it
/// matches neither format the scaffold's own comment names.
pub const SCAFFOLD_FOURCC: FourCc = FourCc::new(0x3432_5641);

/// A DRM four-character pixel-format code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FourCc(u32);

impl FourCc {
    /// Names a format by its raw 32-bit code.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw 32-bit code.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Returns the four characters the code spells, least significant first.
    #[must_use]
    pub const fn tag_bytes(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }
}

impl fmt::Display for FourCc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.tag_bytes() {
            let shown = if byte.is_ascii_graphic() { byte } else { b'?' };
            f.write_str(core::str::from_utf8(&[shown]).unwrap_or("?"))?;
        }
        Ok(())
    }
}

/// The pixel formats this build admits for a shared video stream.
///
/// Exactly the two the scaffold's comment names, and nothing else. An
/// unadmitted code is refused rather than carried, because a stride computed
/// for the wrong format is a buffer overrun on the reader's side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PixelFormat {
    /// Packed 32-bit `ARGB8888`, DRM code `AR24`.
    Argb8888,
    /// Planar 8-bit luma `NV12`, DRM code `NV12`.
    Nv12,
}

impl PixelFormat {
    /// Both admitted formats.
    pub const ALL: [Self; 2] = [Self::Argb8888, Self::Nv12];

    /// Returns the DRM four-character code of this format.
    #[must_use]
    pub const fn fourcc(self) -> FourCc {
        match self {
            Self::Argb8888 => FourCc::new(0x3432_5241),
            Self::Nv12 => FourCc::new(0x3231_564E),
        }
    }

    /// Returns the bytes one pixel of the first plane costs.
    ///
    /// Four for the packed format and one for the planar luma plane; this is
    /// the whole reason a single `width * 4` cannot describe both.
    #[must_use]
    pub const fn bytes_per_pixel(self) -> u32 {
        match self {
            Self::Argb8888 => 4,
            Self::Nv12 => 1,
        }
    }

    /// Returns the stable tag the wire form carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Argb8888 => "argb8888",
            Self::Nv12 => "nv12",
        }
    }

    /// Returns the format a wire tag names, when it names one.
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|format| format.tag() == tag)
    }

    /// Returns the format a four-character code names, when it names one.
    ///
    /// # Errors
    ///
    /// Returns [`CalliopeError::UnknownPixelFormat`] for any other code,
    /// [`SCAFFOLD_FOURCC`] included.
    pub fn from_fourcc(fourcc: FourCc) -> Result<Self, CalliopeError> {
        Self::ALL
            .into_iter()
            .find(|format| format.fourcc() == fourcc)
            .ok_or(CalliopeError::UnknownPixelFormat { fourcc })
    }
}

/// Returns the row stride, in bytes, of `width` pixels in `format`.
///
/// # Errors
///
/// Returns [`CalliopeError::StrideOverflow`] when the product does not fit in
/// the 32-bit field that carries it. The scaffold computes `width * 4` with no
/// check, which for a packed format overflows above `u32::MAX / 4` and wraps
/// in release builds.
pub const fn stride_bytes(format: PixelFormat, width: u32) -> Result<u32, CalliopeError> {
    let bytes_per_pixel = format.bytes_per_pixel();
    match width.checked_mul(bytes_per_pixel) {
        Some(stride) => Ok(stride),
        None => Err(CalliopeError::StrideOverflow {
            width,
            bytes_per_pixel,
        }),
    }
}

/// A descriptor identifier.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct BufferId(u32);

impl BufferId {
    /// Names a descriptor by its raw identifier.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw identifier.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// An opaque, table-local handle.
///
/// **Not a file descriptor.** The scaffold stores `fd: RawFd` and fills it
/// with the literal `42`; this crate opens nothing, so it carries an index
/// into its own table and says what it is. Exchanging a real DMA-BUF file
/// descriptor between processes needs a transport, which is milestone M23.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct BufferHandle(u32);

impl BufferHandle {
    /// Names a handle by its raw value.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// The pixel dimensions of one shared frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameGeometry {
    /// The row width in pixels.
    pub width: u32,
    /// The frame height in rows.
    pub height: u32,
}

impl FrameGeometry {
    /// Names a geometry.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Returns the row stride of this geometry in `format`.
    ///
    /// # Errors
    ///
    /// Propagates [`stride_bytes`].
    pub const fn stride_bytes(self, format: PixelFormat) -> Result<u32, CalliopeError> {
        stride_bytes(format, self.width)
    }
}

/// One DMA-BUF descriptor row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaBufFrame {
    /// The descriptor identifier.
    pub buffer_id: BufferId,
    /// The opaque table-local handle.
    pub handle: BufferHandle,
    /// The frame dimensions.
    pub geometry: FrameGeometry,
    /// The pixel format the stride was computed for.
    pub format: PixelFormat,
    /// The row stride in bytes, checked against the format.
    pub stride_bytes: u32,
}

/// The bounded DMA-BUF descriptor table.
#[derive(Debug, Clone, Copy)]
pub struct DmaBufTable {
    frames: [Option<DmaBufFrame>; MAX_DMA_BUFFERS],
    count: usize,
}

impl Default for DmaBufTable {
    fn default() -> Self {
        Self::new()
    }
}

impl DmaBufTable {
    /// Builds an empty table.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            frames: [None; MAX_DMA_BUFFERS],
            count: 0,
        }
    }

    /// Admits one descriptor and returns its identifier.
    ///
    /// # Errors
    ///
    /// Returns [`CalliopeError::BufferTableFull`] at [`MAX_DMA_BUFFERS`] and
    /// [`CalliopeError::StrideOverflow`] when the stride would not fit.
    pub fn register(
        &mut self,
        geometry: FrameGeometry,
        format: PixelFormat,
    ) -> Result<BufferId, CalliopeError> {
        if self.count >= MAX_DMA_BUFFERS {
            return Err(CalliopeError::BufferTableFull {
                max: MAX_DMA_BUFFERS,
            });
        }
        let index = self.count;
        let raw = u32::try_from(index).unwrap_or(u32::MAX);
        let frame = DmaBufFrame {
            buffer_id: BufferId::new(raw),
            handle: BufferHandle::new(raw),
            geometry,
            format,
            stride_bytes: geometry.stride_bytes(format)?,
        };
        let cell = self
            .frames
            .get_mut(index)
            .ok_or(CalliopeError::BufferTableFull {
                max: MAX_DMA_BUFFERS,
            })?;
        *cell = Some(frame);
        self.count = self.count.saturating_add(1);
        Ok(frame.buffer_id)
    }

    /// Returns the row for `buffer_id`, when the table holds one.
    #[must_use]
    pub fn get(&self, buffer_id: BufferId) -> Option<DmaBufFrame> {
        self.frames
            .iter()
            .take(MAX_DMA_BUFFERS)
            .flatten()
            .find(|frame| frame.buffer_id == buffer_id)
            .copied()
    }

    /// Returns how many descriptors the table holds.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Returns `true` when the table holds no descriptor.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }
}
