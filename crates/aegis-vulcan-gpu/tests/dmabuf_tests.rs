// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Tests for DMA-BUF sharing (E25-2): the UAPI opcodes, the bounds and
//! modifier refusals, the shared mapping over a memfd, the ioctl wrappers
//! against a descriptor that is no DRM device, and the run without devices.

mod common;

use std::os::fd::AsFd;

use common::{Fallible, TempSysfs};

use aegis_vulcan_gpu::dmabuf::sys::{self, Mapping};
use aegis_vulcan_gpu::dmabuf::{
    DRM_FORMAT_MOD_LINEAR, I915_FORMAT_MOD_X_TILED, UDMABUF_LIMIT_PARAM, pattern, sha256_hex,
};
use aegis_vulcan_gpu::{
    GpuError, ShareBounds, ShareRefusal, check_modifier, read_share_bounds, run_share,
};
use rustix::fs::MemfdFlags;
use rustix::io::Errno;

const PAGE: u64 = 4096;
const LIMIT: u64 = 64 << 20;
const BOUNDS: ShareBounds = ShareBounds {
    page: PAGE,
    limit: LIMIT,
};

fn memfd(len: u64) -> Result<std::os::fd::OwnedFd, Errno> {
    let fd = rustix::fs::memfd_create("aegis-m25-test", MemfdFlags::CLOEXEC)?;
    rustix::fs::ftruncate(&fd, len)?;
    Ok(fd)
}

fn write_limit(fixture: &TempSysfs, text: &str) -> Fallible {
    let path = fixture.sysfs.join(UDMABUF_LIMIT_PARAM);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text)?;
    Ok(())
}

// --- Positive -------------------------------------------------------------

/// Positive: each opcode is the number the system UAPI headers produce.
#[test]
fn the_opcodes_match_the_uapi_headers() {
    assert_eq!(sys::UDMABUF_CREATE, 0x4018_7542);
    assert_eq!(sys::DRM_IOCTL_PRIME_HANDLE_TO_FD, 0xC00C_642D);
    assert_eq!(sys::DRM_IOCTL_PRIME_FD_TO_HANDLE, 0xC00C_642E);
    assert_eq!(sys::DRM_IOCTL_GEM_CLOSE, 0x4008_6409);
    assert_eq!(sys::DMA_BUF_IOCTL_SYNC, 0x4008_6200);
}

/// Positive: the smallest and the largest allocation are admitted.
#[test]
fn one_page_and_the_limit_are_admitted() {
    assert_eq!(BOUNDS.check(PAGE), Ok(()));
    assert_eq!(BOUNDS.check(LIMIT), Ok(()));
    assert_eq!(BOUNDS.check(PAGE * 3), Ok(()));
}

/// Positive: the limit is read in MiB from the module parameter.
#[test]
fn the_limit_is_read_from_the_module_parameter() -> Fallible {
    let fixture = TempSysfs::new()?;
    write_limit(&fixture, "64\n")?;
    assert_eq!(read_share_bounds(&fixture.sysfs, PAGE)?, BOUNDS);
    Ok(())
}

/// Positive: an import declaring the export's own modifier is admitted.
#[test]
fn a_matching_modifier_is_admitted() {
    assert_eq!(
        check_modifier(DRM_FORMAT_MOD_LINEAR, DRM_FORMAT_MOD_LINEAR),
        Ok(())
    );
}

/// Positive: the written bytes come back through a second shared mapping.
#[test]
fn a_shared_mapping_round_trips_the_pattern() -> Fallible {
    let len = 3 * 4096;
    let fd = memfd(u64::try_from(len)?)?;
    let data = pattern(len);
    Mapping::new(&fd, len, true)?.write_from(&data)?;
    let mut back = vec![0_u8; len];
    Mapping::new(&fd, len, false)?.read_into(&mut back)?;
    assert_eq!(sha256_hex(&back), sha256_hex(&data));
    assert_eq!(back, data);
    Ok(())
}

/// Positive: the digest is sha256, and the pattern differs between pages.
#[test]
fn the_digest_is_sha256_and_pages_differ() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let data = pattern(8192);
    assert_eq!(data, pattern(8192));
    assert_ne!(data.get(..4096), data.get(4096..));
}

// --- Negative -------------------------------------------------------------

/// Negative: a zero, an unaligned and an over-limit size are refused.
#[test]
fn out_of_bounds_sizes_are_refused() {
    assert_eq!(BOUNDS.check(0), Err(ShareRefusal::ZeroSize));
    assert_eq!(
        BOUNDS.check(PAGE + 1),
        Err(ShareRefusal::NotPageAligned {
            size: PAGE + 1,
            page: PAGE
        })
    );
    assert_eq!(
        BOUNDS.check(LIMIT + PAGE),
        Err(ShareRefusal::OverLimit {
            size: LIMIT + PAGE,
            limit: LIMIT
        })
    );
}

/// Negative: a tiled declaration against the linear export is refused.
#[test]
fn a_mismatched_modifier_is_refused() {
    let refusal = check_modifier(DRM_FORMAT_MOD_LINEAR, I915_FORMAT_MOD_X_TILED);
    assert_eq!(
        refusal,
        Err(ShareRefusal::ModifierMismatch {
            exported: DRM_FORMAT_MOD_LINEAR,
            declared: I915_FORMAT_MOD_X_TILED
        })
    );
    let text = refusal.err().map(|why| why.to_string()).unwrap_or_default();
    assert!(text.contains("0x0100000000000001"), "{text}");
}

/// Negative: a missing or malformed parameter is an error, not a default.
#[test]
fn a_missing_or_malformed_limit_is_an_error() -> Fallible {
    let fixture = TempSysfs::new()?;
    assert!(matches!(
        read_share_bounds(&fixture.sysfs, PAGE),
        Err(GpuError::Io { .. })
    ));
    write_limit(&fixture, "lots\n")?;
    assert!(matches!(
        read_share_bounds(&fixture.sysfs, PAGE),
        Err(GpuError::Malformed { .. })
    ));
    write_limit(&fixture, "0\n")?;
    assert!(matches!(
        read_share_bounds(&fixture.sysfs, PAGE),
        Err(GpuError::Malformed { .. })
    ));
    Ok(())
}

/// Negative: every ioctl wrapper reports the kernel's refusal on a memfd.
#[test]
fn the_ioctls_refuse_a_descriptor_that_is_no_device() -> Fallible {
    let fd = memfd(4096)?;
    let other = memfd(4096)?;
    assert_eq!(
        sys::udmabuf_create(fd.as_fd(), other.as_fd(), 4096).err(),
        Some(Errno::NOTTY)
    );
    assert_eq!(
        sys::prime_fd_to_handle(fd.as_fd(), other.as_fd()),
        Err(Errno::NOTTY)
    );
    assert_eq!(
        sys::prime_handle_to_fd(fd.as_fd(), 1).err(),
        Some(Errno::NOTTY)
    );
    assert_eq!(sys::gem_close(fd.as_fd(), 1), Err(Errno::NOTTY));
    assert_eq!(sys::dma_buf_sync_read(fd.as_fd(), false), Err(Errno::NOTTY));
    Ok(())
}

/// Negative: copies longer than the mapping and an empty mapping are refused.
#[test]
fn oversized_copies_and_empty_mappings_are_refused() -> Fallible {
    let fd = memfd(4096)?;
    assert_eq!(Mapping::new(&fd, 0, false).err(), Some(Errno::INVAL));
    let mut map = Mapping::new(&fd, 4096, true)?;
    assert_eq!(map.write_from(&[0; 4097]), Err(Errno::INVAL));
    assert_eq!(map.read_into(&mut [0; 4097]), Err(Errno::INVAL));
    Ok(())
}

/// Negative: a run without `/dev/udmabuf` fails loudly after its refusals.
#[test]
fn a_run_without_devices_fails() -> Fallible {
    let fixture = TempSysfs::new()?;
    write_limit(&fixture, "64\n")?;
    let run = run_share(&fixture.sysfs, &fixture.dev)?;
    assert!(run.failed);
    assert!(
        run.lines
            .iter()
            .any(|l| l.starts_with("PASS dmabuf/allocation-bounds-refused"))
    );
    assert!(
        run.lines
            .iter()
            .any(|l| l.starts_with("PASS dmabuf/modifier-mismatch-refused"))
    );
    assert!(
        run.lines
            .iter()
            .any(|l| l.starts_with("FAIL dmabuf/udmabuf-export/1-page"))
    );
    assert!(
        run.lines
            .iter()
            .any(|l| l.starts_with("FAIL dmabuf/udmabuf-export/size-limit"))
    );
    assert!(!run.lines.iter().any(|l| l.contains("read-back")));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: one page below the limit passes, one page above is refused.
#[test]
fn the_limit_is_inclusive() {
    assert_eq!(BOUNDS.check(LIMIT - PAGE), Ok(()));
    assert!(BOUNDS.check(LIMIT + PAGE).is_err());
    assert!(BOUNDS.check(PAGE - 1).is_err());
}

/// Boundary: a copy of exactly the mapped length succeeds.
#[test]
fn an_exact_length_copy_succeeds() -> Fallible {
    let fd = memfd(4096)?;
    let mut map = Mapping::new(&fd, 4096, true)?;
    assert_eq!(map.len(), 4096);
    assert!(!map.is_empty());
    map.write_from(&pattern(4096))?;
    let mut back = [0_u8; 4096];
    map.read_into(&mut back)?;
    assert_eq!(back.to_vec(), pattern(4096));
    Ok(())
}

/// Boundary: a limit of exactly one page admits that page only.
#[test]
fn a_one_page_limit_admits_one_page() -> Fallible {
    let fixture = TempSysfs::new()?;
    write_limit(&fixture, "1\n")?;
    let bounds = read_share_bounds(&fixture.sysfs, 1 << 20)?;
    assert_eq!(bounds.check(1 << 20), Ok(()));
    assert!(bounds.check(2 << 20).is_err());
    assert_eq!(pattern(0), Vec::<u8>::new());
    Ok(())
}
