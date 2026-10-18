// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The DMA-BUF syscalls, the one module of the unsafe home that holds `unsafe`.
//!
//! Every function here is safe to call. Each wraps one ioctl or one mapping,
//! and its `// SAFETY:` proof names why the kernel reads or writes nothing
//! outside the value Rust owns. The structs and opcodes are the UAPI of
//! `linux/udmabuf.h`, `drm/drm.h` and `linux/dma-buf.h`; `tests/dmabuf_tests.rs`
//! pins each opcode against the number those headers produce.

use core::ffi::c_void;
use core::ptr::NonNull;
use std::os::fd::{AsFd, BorrowedFd, FromRawFd, OwnedFd};

use rustix::ioctl::{self, Ioctl, IoctlOutput, Opcode, Setter, Updater, opcode};
use rustix::mm::{MapFlags, ProtFlags};

/// `struct udmabuf_create` from `linux/udmabuf.h`.
#[repr(C)]
struct UdmabufCreate {
    memfd: u32,
    flags: u32,
    offset: u64,
    size: u64,
}

/// `struct drm_prime_handle` from `drm/drm.h`.
#[repr(C)]
struct DrmPrimeHandle {
    handle: u32,
    flags: u32,
    fd: i32,
}

/// `struct drm_gem_close` from `drm/drm.h`.
#[repr(C)]
struct DrmGemClose {
    handle: u32,
    pad: u32,
}

/// `struct dma_buf_sync` from `linux/dma-buf.h`.
#[repr(C)]
struct DmaBufSync {
    flags: u64,
}

/// `UDMABUF_CREATE`: `_IOW('u', 0x42, struct udmabuf_create)`.
pub const UDMABUF_CREATE: Opcode = opcode::write::<UdmabufCreate>(b'u', 0x42);
/// `DRM_IOCTL_PRIME_HANDLE_TO_FD`: `DRM_IOWR(0x2d, struct drm_prime_handle)`.
pub const DRM_IOCTL_PRIME_HANDLE_TO_FD: Opcode = opcode::read_write::<DrmPrimeHandle>(b'd', 0x2d);
/// `DRM_IOCTL_PRIME_FD_TO_HANDLE`: `DRM_IOWR(0x2e, struct drm_prime_handle)`.
pub const DRM_IOCTL_PRIME_FD_TO_HANDLE: Opcode = opcode::read_write::<DrmPrimeHandle>(b'd', 0x2e);
/// `DRM_IOCTL_GEM_CLOSE`: `DRM_IOW(0x09, struct drm_gem_close)`.
pub const DRM_IOCTL_GEM_CLOSE: Opcode = opcode::write::<DrmGemClose>(b'd', 0x09);
/// `DMA_BUF_IOCTL_SYNC`: `_IOW('b', 0, struct dma_buf_sync)`.
pub const DMA_BUF_IOCTL_SYNC: Opcode = opcode::write::<DmaBufSync>(b'b', 0);

/// The PRIME import, reading `fd` and writing `handle`.
type FdToHandle<'a> = Updater<'a, DRM_IOCTL_PRIME_FD_TO_HANDLE, DrmPrimeHandle>;
/// The PRIME export, reading `handle` and `flags` and writing `fd`.
type HandleToFd<'a> = Updater<'a, DRM_IOCTL_PRIME_HANDLE_TO_FD, DrmPrimeHandle>;

/// `UDMABUF_FLAGS_CLOEXEC`.
const UDMABUF_FLAGS_CLOEXEC: u32 = 0x01;
/// `DRM_CLOEXEC | DRM_RDWR`, which `drm/drm.h` defines as `O_CLOEXEC | O_RDWR`.
const PRIME_FLAGS: u32 = 0o2_000_000 | 0o2;
/// `DMA_BUF_SYNC_READ`.
const SYNC_READ: u64 = 1;
/// `DMA_BUF_SYNC_END`; `DMA_BUF_SYNC_START` is zero.
const SYNC_END: u64 = 4;

/// `UDMABUF_CREATE`, whose return value is the new dma-buf descriptor.
struct CreateUdmabuf(UdmabufCreate);

// SAFETY: the opcode is `UDMABUF_CREATE` and the pointer is the
// `udmabuf_create` the kernel expects for it; `_IOW` means the kernel only
// reads that struct, so `IS_MUTATING` is false; a successful call returns a
// new descriptor that nothing else owns.
#[allow(unsafe_code)]
unsafe impl Ioctl for CreateUdmabuf {
    type Output = OwnedFd;

    const IS_MUTATING: bool = false;

    fn opcode(&self) -> Opcode {
        UDMABUF_CREATE
    }

    fn as_ptr(&mut self) -> *mut c_void {
        (&raw mut self.0).cast::<c_void>()
    }

    #[allow(unsafe_code)]
    unsafe fn output_from_ptr(out: IoctlOutput, _: *mut c_void) -> rustix::io::Result<OwnedFd> {
        // SAFETY: rustix calls this only after a successful ioctl, whose
        // return value for UDMABUF_CREATE is a fresh descriptor (udmabuf.c,
        // `dma_buf_fd`) that this `OwnedFd` becomes the only owner of.
        Ok(unsafe { OwnedFd::from_raw_fd(out) })
    }
}

/// Turns the sealed `memfd` range `[0, size)` into a dma-buf through `/dev/udmabuf`.
///
/// # Errors
///
/// The kernel's error: `EINVAL` for a size that is not page aligned or is
/// larger than the memfd, `EMFILE`, `ENOMEM`, or a size over the module's
/// `size_limit_mb`.
#[allow(unsafe_code)]
pub fn udmabuf_create(
    udmabuf: BorrowedFd<'_>,
    memfd: BorrowedFd<'_>,
    size: u64,
) -> rustix::io::Result<OwnedFd> {
    let raw = std::os::fd::AsRawFd::as_raw_fd(&memfd);
    let memfd = u32::try_from(raw).map_err(|_| rustix::io::Errno::BADF)?;
    let request = CreateUdmabuf(UdmabufCreate {
        memfd,
        flags: UDMABUF_FLAGS_CLOEXEC,
        offset: 0,
        size,
    });
    // SAFETY: `CreateUdmabuf` implements the UDMABUF_CREATE contract above,
    // and `udmabuf` is a live descriptor borrowed for the whole call.
    unsafe { ioctl::ioctl(udmabuf, request) }
}

/// Imports `dmabuf` into the DRM device `render`, returning its GEM handle.
///
/// # Errors
///
/// The kernel's error, `EOPNOTSUPP` or `EINVAL` from a driver that refuses
/// the import among them.
#[allow(unsafe_code)]
pub fn prime_fd_to_handle(
    render: BorrowedFd<'_>,
    dmabuf: BorrowedFd<'_>,
) -> rustix::io::Result<u32> {
    let fd = std::os::fd::AsRawFd::as_raw_fd(&dmabuf);
    let mut value = DrmPrimeHandle {
        handle: 0,
        flags: 0,
        fd,
    };
    // SAFETY: the opcode is DRM_IOCTL_PRIME_FD_TO_HANDLE, `drm_prime_handle`
    // is the struct it reads and writes, and `value` outlives the call, which
    // borrows it mutably; the kernel reads `fd` and writes `handle` only.
    let request = unsafe { FdToHandle::new(&mut value) };
    // SAFETY: `request` holds the proof above, and `render` is a live
    // descriptor borrowed for the whole call.
    unsafe { ioctl::ioctl(render, request) }?;
    Ok(value.handle)
}

/// Exports the GEM `handle` of `render` as a dma-buf descriptor.
///
/// # Errors
///
/// The kernel's error, `ENOENT` for an unknown handle among them.
#[allow(unsafe_code)]
pub fn prime_handle_to_fd(render: BorrowedFd<'_>, handle: u32) -> rustix::io::Result<OwnedFd> {
    let mut value = DrmPrimeHandle {
        handle,
        flags: PRIME_FLAGS,
        fd: -1,
    };
    // SAFETY: the opcode is DRM_IOCTL_PRIME_HANDLE_TO_FD, `drm_prime_handle`
    // is the struct it reads and writes, and `value` outlives the call; the
    // kernel reads `handle` and `flags` and writes `fd` only.
    let request = unsafe { HandleToFd::new(&mut value) };
    // SAFETY: `request` holds the proof above, and `render` is a live
    // descriptor borrowed for the whole call.
    unsafe { ioctl::ioctl(render, request) }?;
    if value.fd < 0 {
        return Err(rustix::io::Errno::BADF);
    }
    // SAFETY: a successful HANDLE_TO_FD stores in `fd` a descriptor it
    // installed for this process (`drm_prime_handle_to_fd_ioctl`); this
    // `OwnedFd` becomes its only owner.
    Ok(unsafe { OwnedFd::from_raw_fd(value.fd) })
}

/// Releases the GEM `handle` of `render`.
///
/// # Errors
///
/// The kernel's error, `EINVAL` for an unknown handle among them.
#[allow(unsafe_code)]
pub fn gem_close(render: BorrowedFd<'_>, handle: u32) -> rustix::io::Result<()> {
    let value = DrmGemClose { handle, pad: 0 };
    // SAFETY: the opcode is DRM_IOCTL_GEM_CLOSE and `drm_gem_close` is the
    // struct it reads; `_IOW` means the kernel writes nothing back.
    let request = unsafe { Setter::<DRM_IOCTL_GEM_CLOSE, DrmGemClose>::new(value) };
    // SAFETY: `request` holds the proof above, and `render` is a live
    // descriptor borrowed for the whole call.
    unsafe { ioctl::ioctl(render, request) }
}

/// Brackets CPU reads of `dmabuf`: `end` false starts the access, true ends it.
///
/// # Errors
///
/// The kernel's error, `EINVAL` for a descriptor that is no dma-buf among them.
#[allow(unsafe_code)]
pub fn dma_buf_sync_read(dmabuf: BorrowedFd<'_>, end: bool) -> rustix::io::Result<()> {
    let flags = if end { SYNC_READ | SYNC_END } else { SYNC_READ };
    // SAFETY: the opcode is DMA_BUF_IOCTL_SYNC and `dma_buf_sync` is the
    // struct it reads; `_IOW` means the kernel writes nothing back.
    let request = unsafe { Setter::<DMA_BUF_IOCTL_SYNC, DmaBufSync>::new(DmaBufSync { flags }) };
    // SAFETY: `request` holds the proof above, and `dmabuf` is a live
    // descriptor borrowed for the whole call.
    unsafe { ioctl::ioctl(dmabuf, request) }
}

/// A shared mapping of a whole descriptor, unmapped on drop.
#[derive(Debug)]
pub struct Mapping {
    ptr: NonNull<c_void>,
    len: usize,
}

impl Mapping {
    /// Maps `len` bytes of `fd` from offset 0, shared, readable and, when
    /// `writable`, writable.
    ///
    /// # Errors
    ///
    /// `EINVAL` for a zero length, or the kernel's error from `mmap`.
    #[allow(unsafe_code)]
    pub fn new(fd: impl AsFd, len: usize, writable: bool) -> rustix::io::Result<Self> {
        if len == 0 {
            return Err(rustix::io::Errno::INVAL);
        }
        let prot = if writable {
            ProtFlags::READ | ProtFlags::WRITE
        } else {
            ProtFlags::READ
        };
        let hint = core::ptr::null_mut();
        // SAFETY: a null hint lets the kernel choose a fresh range, so no
        // existing Rust allocation is replaced; the range is owned by this
        // `Mapping` alone and is unmapped exactly once in `drop`.
        let raw = unsafe { rustix::mm::mmap(hint, len, prot, MapFlags::SHARED, fd, 0) }?;
        let ptr = NonNull::new(raw).ok_or(rustix::io::Errno::FAULT)?;
        Ok(Self { ptr, len })
    }

    /// The mapped length in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the mapping is empty, which [`Mapping::new`] never returns.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Copies `source` to the start of the mapping.
    ///
    /// # Errors
    ///
    /// `EINVAL` when `source` is longer than the mapping.
    #[allow(unsafe_code)]
    pub fn write_from(&mut self, source: &[u8]) -> rustix::io::Result<()> {
        if source.len() > self.len {
            return Err(rustix::io::Errno::INVAL);
        }
        // SAFETY: the destination is this mapping's own range, at least
        // `source.len()` bytes long by the check above; a Rust slice cannot
        // overlap a fresh `mmap` range, and `&mut self` excludes a second copy
        // through this mapping. The caller maps it writable, or the kernel
        // faults the process rather than corrupting memory.
        unsafe {
            core::ptr::copy_nonoverlapping(
                source.as_ptr(),
                self.ptr.as_ptr().cast::<u8>(),
                source.len(),
            );
        }
        Ok(())
    }

    /// Copies the start of the mapping into `target`.
    ///
    /// # Errors
    ///
    /// `EINVAL` when `target` is longer than the mapping.
    #[allow(unsafe_code)]
    pub fn read_into(&self, target: &mut [u8]) -> rustix::io::Result<()> {
        if target.len() > self.len {
            return Err(rustix::io::Errno::INVAL);
        }
        // SAFETY: the source is this mapping's own readable range, at least
        // `target.len()` bytes long by the check above, and `target` is a
        // distinct Rust allocation, so the two cannot overlap. The bytes are
        // copied, never referenced, so a concurrent writer to the shared
        // pages cannot break a Rust aliasing rule.
        unsafe {
            core::ptr::copy_nonoverlapping(
                self.ptr.as_ptr().cast::<u8>(),
                target.as_mut_ptr(),
                target.len(),
            );
        }
        Ok(())
    }
}

impl Drop for Mapping {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: `ptr` and `len` are the range `mmap` returned in `new`, not
        // unmapped before, and no reference into it outlives `self`.
        let _ = unsafe { rustix::mm::munmap(self.ptr.as_ptr(), self.len) };
    }
}
