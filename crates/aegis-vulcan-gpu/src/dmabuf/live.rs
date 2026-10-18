// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The live DMA-BUF run: export through `/dev/udmabuf`, import into every
//! render node, hand one driver's export to a second driver, read back.
//!
//! The run passes when, for the smallest and the largest allocation, two render
//! nodes on different drivers each import the export and hand back the same
//! dma-buf, the second driver imports the first driver's re-export, and the
//! bytes read back match. An import a further driver refuses is recorded with
//! the kernel's error and never gates. The run stops at [`SHARE_DEADLINE`].

use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::Path;
use std::time::{Duration, Instant};

use rustix::fs::{MemfdFlags, Mode, OFlags, SealFlags};

use crate::dmabuf::sys::{self, Mapping};
use crate::dmabuf::{
    DRM_FORMAT_MOD_LINEAR, I915_FORMAT_MOD_X_TILED, ShareBounds, check_modifier, pattern,
    read_share_bounds, sha256_hex,
};
use crate::error::GpuError;
use crate::render_nodes::{RenderNodeInfo, inspect_render_nodes};

/// The most render nodes one run imports into.
pub const MAX_IMPORTERS: usize = 8;

/// The wall-clock budget of one run, checked before every step.
pub const SHARE_DEADLINE: Duration = Duration::from_secs(60);

/// The outcome lines of one run and whether any gating line failed.
#[derive(Debug, Default)]
pub struct ShareRun {
    /// One `PASS`, `FAIL` or `RECORD` line per case, in run order.
    pub lines: Vec<String>,
    /// True when a gating case failed.
    pub failed: bool,
}

impl ShareRun {
    fn pass(&mut self, mut line: String) {
        line.insert_str(0, "PASS ");
        self.lines.push(line);
    }

    fn fail(&mut self, mut line: String) {
        line.insert_str(0, "FAIL ");
        self.lines.push(line);
        self.failed = true;
    }

    fn record(&mut self, mut line: String) {
        line.insert_str(0, "RECORD ");
        line.push_str(" (recorded, never gating)");
        self.lines.push(line);
    }
}

/// A sealed memfd, its udmabuf export, and the digest of what was written.
struct Export {
    _memfd: OwnedFd,
    dmabuf: OwnedFd,
    len: usize,
    digest: String,
    write: Duration,
}

/// Runs every case against the render nodes under `sysfs` and `dev`.
///
/// # Errors
///
/// [`GpuError`] when the render nodes or the udmabuf limit cannot be read;
/// a failed case is a `FAIL` line, not an error.
pub fn run_share(sysfs: &Path, dev: &Path) -> Result<ShareRun, GpuError> {
    let started = Instant::now();
    let page = u64::try_from(rustix::param::page_size()).unwrap_or(0);
    let bounds = read_share_bounds(sysfs, page)?;
    let mut nodes = inspect_render_nodes(sysfs)?;
    nodes.truncate(MAX_IMPORTERS);
    let mut run = ShareRun::default();
    refuse_out_of_bounds(&bounds, &mut run);
    refuse_modifier(&mut run);
    for (label, size) in [("1-page", bounds.page), ("size-limit", bounds.limit)] {
        if started.elapsed() > SHARE_DEADLINE {
            run.fail(format!(
                "dmabuf/{label}: the {SHARE_DEADLINE:?} deadline passed first"
            ));
            break;
        }
        share_one(dev, &nodes, label, size, &mut run);
    }
    Ok(run)
}

/// Boundary: 0 bytes, an unaligned size and one page over the limit are refused.
fn refuse_out_of_bounds(bounds: &ShareBounds, run: &mut ShareRun) {
    let over = bounds.limit.saturating_add(bounds.page);
    let unaligned = bounds.page.saturating_add(1);
    let refused: Vec<String> = [0, unaligned, over]
        .into_iter()
        .filter_map(|size| bounds.check(size).err().map(|why| format!("{size}: {why}")))
        .collect();
    let line = format!("dmabuf/allocation-bounds-refused: {}", refused.join("; "));
    if refused.len() == 3 && bounds.check(bounds.page).is_ok() && bounds.check(bounds.limit).is_ok()
    {
        run.pass(format!("{line}; before any syscall"));
    } else {
        run.fail(line);
    }
}

/// Negative: a tiled declaration against the linear export is refused.
fn refuse_modifier(run: &mut ShareRun) {
    match check_modifier(DRM_FORMAT_MOD_LINEAR, I915_FORMAT_MOD_X_TILED) {
        Err(why) => run.pass(format!(
            "dmabuf/modifier-mismatch-refused: {why}; before any import"
        )),
        Ok(()) => {
            run.fail("dmabuf/modifier-mismatch-refused: the mismatch was admitted".to_owned());
        }
    }
}

/// Exports one buffer of `size` bytes and runs the import cases on it.
fn share_one(dev: &Path, nodes: &[RenderNodeInfo], label: &str, size: u64, run: &mut ShareRun) {
    let export = match export(dev, size) {
        Ok(export) => export,
        Err(why) => return run.fail(format!("dmabuf/udmabuf-export/{label}: {why}")),
    };
    let mut imported: Vec<(&RenderNodeInfo, OwnedFd)> = Vec::new();
    for node in nodes {
        match import(dev, node, export.dmabuf.as_fd()) {
            Ok((reexport, line)) => {
                run.pass(format!("dmabuf/udmabuf-to-{}/{label}: {line}", node.driver));
                imported.push((node, reexport));
            }
            Err(why) => run.record(format!("dmabuf/udmabuf-to-{}/{label}: {why}", node.driver)),
        }
    }
    cross_driver(dev, &imported, label, export.dmabuf.as_fd(), run);
    read_back(&export, label, run);
}

/// Hands the first importer's re-export to the first importer on another driver.
fn cross_driver(
    dev: &Path,
    imported: &[(&RenderNodeInfo, OwnedFd)],
    label: &str,
    original: BorrowedFd<'_>,
    run: &mut ShareRun,
) {
    let Some((first, reexport)) = imported.first() else {
        return run.fail(format!(
            "dmabuf/cross-driver/{label}: no render node imported the export"
        ));
    };
    let Some((second, _)) = imported
        .iter()
        .find(|(node, _)| node.driver != first.driver)
    else {
        return run.fail(format!(
            "dmabuf/cross-driver/{label}: only {} imported; a second driver is required",
            first.driver
        ));
    };
    let case = format!("dmabuf/{}-to-{}/{label}", first.driver, second.driver);
    match import(dev, second, reexport.as_fd()) {
        Ok((again, line)) => match same_file(again.as_fd(), original) {
            Ok(true) => run.pass(format!(
                "{case}: {line}; what it imported is {}'s ({}) re-export, the same udmabuf export",
                first.node, first.driver
            )),
            Ok(false) => run.fail(format!("{case}: the second re-export is another dma-buf")),
            Err(why) => run.fail(format!("{case}: {why}")),
        },
        Err(why) => run.fail(format!("{case}: {why}")),
    }
}

/// Creates the sealed memfd, writes the pattern, and exports it through udmabuf.
fn export(dev: &Path, size: u64) -> Result<Export, String> {
    let len = usize::try_from(size).map_err(|_| format!("{size} bytes does not fit in usize"))?;
    let flags = MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING;
    let memfd = rustix::fs::memfd_create("aegis-m25-dmabuf", flags)
        .map_err(|err| format!("memfd_create: {err}"))?;
    rustix::fs::ftruncate(&memfd, size).map_err(|err| format!("ftruncate {size}: {err}"))?;
    rustix::fs::fcntl_add_seals(&memfd, SealFlags::SHRINK)
        .map_err(|err| format!("F_ADD_SEALS F_SEAL_SHRINK: {err}"))?;
    let data = pattern(len);
    let digest = sha256_hex(&data);
    let started = Instant::now();
    Mapping::new(&memfd, len, true)
        .and_then(|mut map| map.write_from(&data))
        .map_err(|err| format!("writing the memfd: {err}"))?;
    let write = started.elapsed();
    let path = dev.join("udmabuf");
    let udmabuf = open_rw(&path)?;
    let dmabuf = sys::udmabuf_create(udmabuf.as_fd(), memfd.as_fd(), size)
        .map_err(|err| format!("UDMABUF_CREATE on {}: {err}", path.display()))?;
    Ok(Export {
        _memfd: memfd,
        dmabuf,
        len,
        digest,
        write,
    })
}

/// Imports `dmabuf` into `node`, re-exports it, checks identity, closes the handle.
fn import(
    dev: &Path,
    node: &RenderNodeInfo,
    dmabuf: BorrowedFd<'_>,
) -> Result<(OwnedFd, String), String> {
    let render = open_rw(&dev.join("dri").join(&node.node))?;
    let started = Instant::now();
    let handle = sys::prime_fd_to_handle(render.as_fd(), dmabuf)
        .map_err(|err| format!("{} PRIME_FD_TO_HANDLE refused: {err}", node.node))?;
    let elapsed = started.elapsed();
    let reexport = sys::prime_handle_to_fd(render.as_fd(), handle);
    let closed = sys::gem_close(render.as_fd(), handle);
    let reexport = reexport.map_err(|err| format!("{} PRIME_HANDLE_TO_FD: {err}", node.node))?;
    closed.map_err(|err| format!("{} GEM_CLOSE {handle}: {err}", node.node))?;
    let inode = rustix::fs::fstat(dmabuf)
        .map_err(|err| format!("fstat: {err}"))?
        .st_ino;
    if !same_file(reexport.as_fd(), dmabuf)? {
        return Err(format!(
            "{} re-exported another dma-buf than it imported",
            node.node
        ));
    }
    let line = format!(
        "{} ({}, {}) imported it and re-exported the same dma-buf (inode {inode}); import {}",
        node.node,
        node.driver,
        node.address,
        millis(elapsed)
    );
    Ok((reexport, line))
}

/// Reads the buffer back through a dma-buf mapping and matches the digest.
fn read_back(export: &Export, label: &str, run: &mut ShareRun) {
    let case = format!("dmabuf/read-back/{label}");
    let dmabuf = export.dmabuf.as_fd();
    let mut target = vec![0_u8; export.len];
    let started = Instant::now();
    let result = Mapping::new(dmabuf, export.len, false).and_then(|map| {
        sys::dma_buf_sync_read(dmabuf, false)?;
        let copied = map.read_into(&mut target);
        sys::dma_buf_sync_read(dmabuf, true)?;
        copied
    });
    let elapsed = started.elapsed();
    if let Err(err) = result {
        return run.fail(format!("{case}: {err}"));
    }
    let digest = sha256_hex(&target);
    let line = format!(
        "{case}: {} bytes read back through the dma-buf mapping between \
         DMA_BUF_IOCTL_SYNC start and end, sha256 {digest}; write {}, read {} ({} bytes/s); \
         a CPU read, no GPU read is claimed",
        export.len,
        millis(export.write),
        millis(elapsed),
        rate(export.len, elapsed)
    );
    if digest == export.digest {
        run.pass(line);
    } else {
        run.fail(format!("{line}; expected sha256 {}", export.digest));
    }
}

/// Whether two descriptors name the same file (device and inode).
fn same_file(left: BorrowedFd<'_>, right: BorrowedFd<'_>) -> Result<bool, String> {
    let left = rustix::fs::fstat(left).map_err(|err| format!("fstat: {err}"))?;
    let right = rustix::fs::fstat(right).map_err(|err| format!("fstat: {err}"))?;
    Ok(left.st_dev == right.st_dev && left.st_ino == right.st_ino)
}

/// Opens `path` read-write and close-on-exec.
fn open_rw(path: &Path) -> Result<OwnedFd, String> {
    rustix::fs::open(path, OFlags::RDWR | OFlags::CLOEXEC, Mode::empty())
        .map_err(|err| format!("open {}: {err}", path.display()))
}

/// A duration as milliseconds with three decimals.
fn millis(elapsed: Duration) -> String {
    let micros = elapsed.as_micros();
    let whole = micros.checked_div(1000).unwrap_or(0);
    let part = micros.checked_rem(1000).unwrap_or(0);
    format!("{whole}.{part:03} ms")
}

/// Bytes per second over `elapsed`, or 0 for an unmeasurably short span.
fn rate(len: usize, elapsed: Duration) -> u128 {
    let micros = elapsed.as_micros();
    u128::try_from(len)
        .ok()
        .and_then(|bytes| bytes.checked_mul(1_000_000))
        .and_then(|scaled| scaled.checked_div(micros))
        .unwrap_or(0)
}
