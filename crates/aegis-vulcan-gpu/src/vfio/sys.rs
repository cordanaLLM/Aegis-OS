// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! VFIO syscall wrappers: ioctl and mmap operations under `aegis-os-m25-unsafe-home`.
//!
//! Every function here is safe to call. Each wraps one ioctl or one mapping,
//! and its `// SAFETY:` proof explains why memory and descriptor invariants hold.
//! The ioctls and structs correspond to Linux UAPI `<linux/vfio.h>`.

use core::ffi::c_void;
use core::ptr::NonNull;
use std::os::fd::{BorrowedFd, FromRawFd, OwnedFd};

use rustix::ioctl::{self, Ioctl, IoctlOutput, Opcode, opcode};
use rustix::mm::{MapFlags, ProtFlags};

/// `VFIO_TYPE`: `(';')` (0x3B).
pub const VFIO_TYPE: u8 = b';';
/// `VFIO_BASE`: 100.
pub const VFIO_BASE: u8 = 100;

/// `VFIO_API_VERSION`: 0.
pub const VFIO_API_VERSION: i32 = 0;
/// `VFIO_TYPE1v2_IOMMU`: 3.
#[allow(non_upper_case_globals)]
pub const VFIO_TYPE1v2_IOMMU: u32 = 3;

/// `VFIO_GROUP_FLAGS_VIABLE`: bit 0.
pub const VFIO_GROUP_FLAGS_VIABLE: u32 = 1 << 0;
/// `VFIO_GROUP_FLAGS_CONTAINER_SET`: bit 1.
pub const VFIO_GROUP_FLAGS_CONTAINER_SET: u32 = 1 << 1;

/// `VFIO_REGION_INFO_FLAG_READ`: bit 0.
pub const VFIO_REGION_INFO_FLAG_READ: u32 = 1 << 0;
/// `VFIO_REGION_INFO_FLAG_WRITE`: bit 1.
pub const VFIO_REGION_INFO_FLAG_WRITE: u32 = 1 << 1;
/// `VFIO_REGION_INFO_FLAG_MMAP`: bit 2.
pub const VFIO_REGION_INFO_FLAG_MMAP: u32 = 1 << 2;
/// `VFIO_REGION_INFO_FLAG_CAPS`: bit 3.
pub const VFIO_REGION_INFO_FLAG_CAPS: u32 = 1 << 3;

/// `VFIO_GET_API_VERSION`: `_IO(';', 100)`.
pub const VFIO_GET_API_VERSION: Opcode = opcode::none(VFIO_TYPE, VFIO_BASE);
/// `VFIO_CHECK_EXTENSION`: `_IO(';', 101)`.
pub const VFIO_CHECK_EXTENSION: Opcode = opcode::none(VFIO_TYPE, VFIO_BASE + 1);
/// `VFIO_SET_IOMMU`: `_IO(';', 102)`.
pub const VFIO_SET_IOMMU: Opcode = opcode::none(VFIO_TYPE, VFIO_BASE + 2);
/// `VFIO_GROUP_GET_STATUS`: `_IO(';', 103)`.
pub const VFIO_GROUP_GET_STATUS: Opcode = opcode::none(VFIO_TYPE, VFIO_BASE + 3);
/// `VFIO_GROUP_SET_CONTAINER`: `_IO(';', 104)`.
pub const VFIO_GROUP_SET_CONTAINER: Opcode = opcode::none(VFIO_TYPE, VFIO_BASE + 4);
/// `VFIO_GROUP_GET_DEVICE_FD`: `_IO(';', 106)`.
pub const VFIO_GROUP_GET_DEVICE_FD: Opcode = opcode::none(VFIO_TYPE, VFIO_BASE + 6);
/// `VFIO_DEVICE_GET_REGION_INFO`: `_IO(';', 108)`.
pub const VFIO_DEVICE_GET_REGION_INFO: Opcode = opcode::none(VFIO_TYPE, VFIO_BASE + 8);

/// `struct vfio_group_status` from `<linux/vfio.h>`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VfioGroupStatus {
    /// Size of structure (caller sets).
    pub argsz: u32,
    /// Group status flags (`VFIO_GROUP_FLAGS_*`).
    pub flags: u32,
}

/// `struct vfio_region_info` from `<linux/vfio.h>`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VfioRegionInfo {
    /// Size of structure (caller sets, 32 bytes).
    pub argsz: u32,
    /// Region flags (`VFIO_REGION_INFO_FLAG_*`).
    pub flags: u32,
    /// Region index (caller sets).
    pub index: u32,
    /// Capability chain offset.
    pub cap_offset: u32,
    /// Region size in bytes.
    pub size: u64,
    /// Region offset from start of device fd.
    pub offset: u64,
}

struct GetApiVersion;

// SAFETY: the opcode is VFIO_GET_API_VERSION, which takes no pointer argument,
// so as_ptr returns null and IS_MUTATING is false; output_from_ptr converts the
// raw integer return code to i32.
#[allow(unsafe_code)]
unsafe impl Ioctl for GetApiVersion {
    type Output = i32;

    const IS_MUTATING: bool = false;

    fn opcode(&self) -> Opcode {
        VFIO_GET_API_VERSION
    }

    fn as_ptr(&mut self) -> *mut c_void {
        core::ptr::null_mut()
    }

    #[allow(unsafe_code)]
    unsafe fn output_from_ptr(out: IoctlOutput, _: *mut c_void) -> rustix::io::Result<i32> {
        Ok(out)
    }
}

/// Queries the VFIO API version from the container fd.
///
/// # Errors
///
/// Returns the kernel ioctl error.
#[allow(unsafe_code)]
pub fn vfio_get_api_version(container: BorrowedFd<'_>) -> rustix::io::Result<i32> {
    let request = GetApiVersion;
    // SAFETY: the opcode is VFIO_GET_API_VERSION, which takes no pointer argument
    // and returns the API version as the ioctl return value. `container` is a live descriptor.
    unsafe { ioctl::ioctl(container, request) }
}

struct CheckExtension(u32);

// SAFETY: the opcode is VFIO_CHECK_EXTENSION, which takes an integer extension ID
// passed in the third argument; IS_MUTATING is false and output_from_ptr returns the
// integer support status.
#[allow(unsafe_code)]
unsafe impl Ioctl for CheckExtension {
    type Output = i32;

    const IS_MUTATING: bool = false;

    fn opcode(&self) -> Opcode {
        VFIO_CHECK_EXTENSION
    }

    fn as_ptr(&mut self) -> *mut c_void {
        core::ptr::without_provenance_mut(self.0 as usize)
    }

    #[allow(unsafe_code)]
    unsafe fn output_from_ptr(out: IoctlOutput, _: *mut c_void) -> rustix::io::Result<i32> {
        Ok(out)
    }
}

/// Checks if an extension (e.g. `VFIO_TYPE1v2_IOMMU`) is supported by the container.
///
/// # Errors
///
/// Returns the kernel ioctl error.
#[allow(unsafe_code)]
pub fn vfio_check_extension(container: BorrowedFd<'_>, extension: u32) -> rustix::io::Result<i32> {
    let request = CheckExtension(extension);
    // SAFETY: the opcode is VFIO_CHECK_EXTENSION, which passes the extension ID as
    // an integer argument in register 3; `container` is a live descriptor.
    unsafe { ioctl::ioctl(container, request) }
}

struct SetIommu(u32);

// SAFETY: the opcode is VFIO_SET_IOMMU, which takes the integer IOMMU model type in
// the third argument; IS_MUTATING is false and output_from_ptr succeeds on 0.
#[allow(unsafe_code)]
unsafe impl Ioctl for SetIommu {
    type Output = ();

    const IS_MUTATING: bool = false;

    fn opcode(&self) -> Opcode {
        VFIO_SET_IOMMU
    }

    fn as_ptr(&mut self) -> *mut c_void {
        core::ptr::without_provenance_mut(self.0 as usize)
    }

    #[allow(unsafe_code)]
    unsafe fn output_from_ptr(_: IoctlOutput, _: *mut c_void) -> rustix::io::Result<()> {
        Ok(())
    }
}

/// Sets the IOMMU model type on the container fd.
///
/// # Errors
///
/// Returns the kernel ioctl error.
#[allow(unsafe_code)]
pub fn vfio_set_iommu(container: BorrowedFd<'_>, iommu_type: u32) -> rustix::io::Result<()> {
    let request = SetIommu(iommu_type);
    // SAFETY: the opcode is VFIO_SET_IOMMU, which takes the IOMMU type as an
    // integer argument; `container` is a live descriptor.
    unsafe { ioctl::ioctl(container, request) }
}

struct GroupGetStatus(VfioGroupStatus);

// SAFETY: the opcode is VFIO_GROUP_GET_STATUS, which expects a pointer to
// vfio_group_status with argsz initialized; the kernel populates the struct, so
// IS_MUTATING is true and output_from_ptr reads the populated struct.
#[allow(unsafe_code)]
unsafe impl Ioctl for GroupGetStatus {
    type Output = VfioGroupStatus;

    const IS_MUTATING: bool = true;

    fn opcode(&self) -> Opcode {
        VFIO_GROUP_GET_STATUS
    }

    fn as_ptr(&mut self) -> *mut c_void {
        (&raw mut self.0).cast::<c_void>()
    }

    #[allow(unsafe_code)]
    unsafe fn output_from_ptr(
        _: IoctlOutput,
        ptr: *mut c_void,
    ) -> rustix::io::Result<VfioGroupStatus> {
        // SAFETY: rustix calls output_from_ptr after a successful ioctl; ptr points to self.0
        // which the kernel populated with the vfio_group_status struct.
        Ok(unsafe { *ptr.cast::<VfioGroupStatus>() })
    }
}

/// Retrieves the status flags of a VFIO group fd.
///
/// # Errors
///
/// Returns the kernel ioctl error.
#[allow(unsafe_code)]
pub fn vfio_group_get_status(group: BorrowedFd<'_>) -> rustix::io::Result<VfioGroupStatus> {
    let status = VfioGroupStatus {
        argsz: u32::try_from(core::mem::size_of::<VfioGroupStatus>()).unwrap_or(8),
        flags: 0,
    };
    let request = GroupGetStatus(status);
    // SAFETY: the opcode is VFIO_GROUP_GET_STATUS; self.0 is an 8-byte VfioGroupStatus
    // initialized with argsz=8; the kernel populates flags. `group` is a live descriptor.
    unsafe { ioctl::ioctl(group, request) }
}

struct GroupSetContainer(i32);

// SAFETY: the opcode is VFIO_GROUP_SET_CONTAINER, which reads an integer container fd
// passed by pointer; IS_MUTATING is false and output_from_ptr succeeds on 0.
#[allow(unsafe_code)]
unsafe impl Ioctl for GroupSetContainer {
    type Output = ();

    const IS_MUTATING: bool = false;

    fn opcode(&self) -> Opcode {
        VFIO_GROUP_SET_CONTAINER
    }

    fn as_ptr(&mut self) -> *mut c_void {
        (&raw mut self.0).cast::<c_void>()
    }

    #[allow(unsafe_code)]
    unsafe fn output_from_ptr(_: IoctlOutput, _: *mut c_void) -> rustix::io::Result<()> {
        Ok(())
    }
}

/// Binds a VFIO group to a container fd.
///
/// # Errors
///
/// Returns the kernel ioctl error.
#[allow(unsafe_code)]
pub fn vfio_group_set_container(
    group: BorrowedFd<'_>,
    container: BorrowedFd<'_>,
) -> rustix::io::Result<()> {
    let raw = std::os::fd::AsRawFd::as_raw_fd(&container);
    let request = GroupSetContainer(raw);
    // SAFETY: the opcode is VFIO_GROUP_SET_CONTAINER; self.0 is an i32 file descriptor
    // passed by pointer; the kernel reads the int descriptor. Both descriptors are live.
    unsafe { ioctl::ioctl(group, request) }
}

struct GroupGetDeviceFd<'a>(&'a std::ffi::CStr);

// SAFETY: the opcode is VFIO_GROUP_GET_DEVICE_FD, which reads a NUL-terminated
// device address string; a successful call returns a newly installed file descriptor
// that the resulting OwnedFd takes ownership of.
#[allow(unsafe_code)]
unsafe impl Ioctl for GroupGetDeviceFd<'_> {
    type Output = OwnedFd;

    const IS_MUTATING: bool = false;

    fn opcode(&self) -> Opcode {
        VFIO_GROUP_GET_DEVICE_FD
    }

    fn as_ptr(&mut self) -> *mut c_void {
        self.0.as_ptr().cast_mut().cast::<c_void>()
    }

    #[allow(unsafe_code)]
    unsafe fn output_from_ptr(out: IoctlOutput, _: *mut c_void) -> rustix::io::Result<OwnedFd> {
        // SAFETY: rustix calls output_from_ptr after a successful ioctl;
        // VFIO_GROUP_GET_DEVICE_FD returns a newly installed file descriptor for the device.
        Ok(unsafe { OwnedFd::from_raw_fd(out) })
    }
}

/// Retrieves a device file descriptor from a bound VFIO group.
///
/// # Errors
///
/// Returns `INVAL` if the device name contains a NUL byte, or the kernel ioctl error.
#[allow(unsafe_code)]
pub fn vfio_group_get_device_fd(
    group: BorrowedFd<'_>,
    device_name: &str,
) -> rustix::io::Result<OwnedFd> {
    let c_name = std::ffi::CString::new(device_name).map_err(|_| rustix::io::Errno::INVAL)?;
    let request = GroupGetDeviceFd(&c_name);
    // SAFETY: the opcode is VFIO_GROUP_GET_DEVICE_FD; the argument is a valid
    // NUL-terminated C string representing the PCI address; `group` is a live descriptor.
    unsafe { ioctl::ioctl(group, request) }
}

struct DeviceGetRegionInfo(VfioRegionInfo);

// SAFETY: the opcode is VFIO_DEVICE_GET_REGION_INFO, which expects a pointer to
// vfio_region_info with argsz and index initialized; the kernel populates flags,
// size, and offset, so IS_MUTATING is true and output_from_ptr reads the struct.
#[allow(unsafe_code)]
unsafe impl Ioctl for DeviceGetRegionInfo {
    type Output = VfioRegionInfo;

    const IS_MUTATING: bool = true;

    fn opcode(&self) -> Opcode {
        VFIO_DEVICE_GET_REGION_INFO
    }

    fn as_ptr(&mut self) -> *mut c_void {
        (&raw mut self.0).cast::<c_void>()
    }

    #[allow(unsafe_code)]
    unsafe fn output_from_ptr(
        _: IoctlOutput,
        ptr: *mut c_void,
    ) -> rustix::io::Result<VfioRegionInfo> {
        // SAFETY: rustix calls output_from_ptr after a successful ioctl; ptr points to self.0
        // which the kernel populated with the 32-byte vfio_region_info.
        Ok(unsafe { *ptr.cast::<VfioRegionInfo>() })
    }
}

/// Queries region information for a given BAR or region index from a device fd.
///
/// # Errors
///
/// Returns the kernel ioctl error.
#[allow(unsafe_code)]
pub fn vfio_device_get_region_info(
    device: BorrowedFd<'_>,
    index: u32,
) -> rustix::io::Result<VfioRegionInfo> {
    let info = VfioRegionInfo {
        argsz: u32::try_from(core::mem::size_of::<VfioRegionInfo>()).unwrap_or(32),
        flags: 0,
        index,
        cap_offset: 0,
        size: 0,
        offset: 0,
    };
    let request = DeviceGetRegionInfo(info);
    // SAFETY: the opcode is VFIO_DEVICE_GET_REGION_INFO; self.0 is a 32-byte VfioRegionInfo
    // initialized with argsz=32 and index set; the kernel populates flags, size, and offset.
    // `device` is a live descriptor borrowed for the call.
    unsafe { ioctl::ioctl(device, request) }
}

/// A 4096-byte MMIO mapping of a VFIO BAR region, unmapped on drop.
#[derive(Debug)]
pub struct VfioBarMapping {
    ptr: NonNull<c_void>,
    len: usize,
}

/// Maps `len` bytes of a device region at `offset` as read-only shared MMIO.
///
/// # Errors
///
/// Returns `INVAL` for a zero length, or the kernel mmap error.
#[allow(unsafe_code)]
pub fn mmap_bar_mmio(
    device: BorrowedFd<'_>,
    offset: u64,
    len: usize,
) -> rustix::io::Result<VfioBarMapping> {
    if len == 0 {
        return Err(rustix::io::Errno::INVAL);
    }
    let hint = core::ptr::null_mut();
    let prot = ProtFlags::READ;
    let flags = MapFlags::SHARED;
    // SAFETY: a null hint lets the kernel allocate a fresh virtual address range.
    // PROT_READ and MAP_SHARED map the device MMIO space as specified by the kernel.
    // The mapping is owned solely by VfioBarMapping and unmapped on drop.
    let raw = unsafe { rustix::mm::mmap(hint, len, prot, flags, device, offset) }?;
    let ptr = NonNull::new(raw).ok_or(rustix::io::Errno::FAULT)?;
    Ok(VfioBarMapping { ptr, len })
}

impl VfioBarMapping {
    /// Reads 4096 bytes using volatile 32-bit MMIO reads.
    ///
    /// # Errors
    ///
    /// Returns `INVAL` if the mapping length is less than 4096 bytes.
    #[allow(unsafe_code)]
    pub fn read_4k_mmio(&self) -> rustix::io::Result<(u32, [u8; 4096])> {
        if self.len < 4096 {
            return Err(rustix::io::Errno::INVAL);
        }
        let mut buffer = [0u8; 4096];
        let mut dword0 = 0u32;
        let dword_ptr = self.ptr.as_ptr().cast::<u32>();
        for (idx, chunk) in buffer.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            // SAFETY: self.ptr is a valid, page-aligned mapping of at least 4096 bytes.
            // idx is in 0..1024, so dword_ptr.add(idx) is 4-byte aligned and within the mapping.
            // Volatile read is mandatory for hardware MMIO register access.
            let val = unsafe { core::ptr::read_volatile(dword_ptr.add(idx)) };
            if idx == 0 {
                dword0 = val;
            }
            *chunk = val.to_ne_bytes();
        }
        Ok((dword0, buffer))
    }

    /// Unmaps the BAR mapping explicitly.
    ///
    /// # Errors
    ///
    /// Returns the kernel's error from `munmap`.
    #[allow(unsafe_code)]
    pub fn unmap(mut self) -> rustix::io::Result<()> {
        let ptr = self.ptr.as_ptr();
        let len = self.len;
        self.len = 0;
        // SAFETY: ptr and len are the valid mapping range returned by mmap;
        // self.len is zeroed so Drop will not unmap a second time.
        unsafe { rustix::mm::munmap(ptr, len) }
    }
}

impl Drop for VfioBarMapping {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        if self.len > 0 {
            // SAFETY: ptr and len are the range returned by mmap, not unmapped before,
            // and no references into it outlive self.
            let _ = unsafe { rustix::mm::munmap(self.ptr.as_ptr(), self.len) };
        }
    }
}
