// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Which render node decodes: the one whose `dev_t` is the compositor's
//! `zwp_linux_dmabuf_v1` main device (M27 criteria 5 and 6).
//!
//! The node is resolved through `/sys/class/drm` and never from a hard-coded
//! `renderD` number: every `renderD*` entry's `dev` attribute is read as
//! `major:minor`, and the node whose device number equals the main device is
//! the only one admitted. A node whose device number differs is refused with
//! [`DeviceError::NotMainDevice`] before anything is opened on it, so a decode
//! on another GPU (the amdgpu or the NVIDIA node on the reference profile)
//! never reaches the compositor. The kernel driver bound to each node is read
//! from its `device/driver` link for the record; it decides nothing.
//!
//! Enumeration happens once per run, not per frame, so its `String`s are not
//! on the frame path.

use std::path::{Path, PathBuf};

use rustix::fs::{Dev, major, makedev, minor, stat};

use crate::error::errno;

/// Where the kernel lists DRM devices.
pub const SYS_CLASS_DRM: &str = "/sys/class/drm";

/// Where the device nodes live.
pub const DEV_DRI: &str = "/dev/dri";

/// The most `/sys/class/drm` entries one enumeration reads.
pub const MAX_DRM_ENTRIES: usize = 256;

/// The longest `dev` attribute read, in bytes.
const MAX_DEV_ATTRIBUTE: usize = 32;

/// Why a render node was not admitted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum DeviceError {
    /// `/sys/class/drm` could not be listed.
    #[error("{path} could not be listed: {reason}")]
    Enumerate {
        /// The directory.
        path: String,
        /// Why.
        reason: String,
    },
    /// A `dev` attribute is not `major:minor`.
    #[error("{path} does not hold a major:minor device number")]
    DevAttribute {
        /// The attribute file.
        path: String,
    },
    /// No render node has the main device's number.
    #[error(
        "no render node under {root} has device number {major}:{minor}, the compositor's main device"
    )]
    NoNodeForMainDevice {
        /// The sysfs root searched.
        root: String,
        /// The main device's major number.
        major: u32,
        /// The main device's minor number.
        minor: u32,
    },
    /// The node is not the compositor's main device (M27 negative (b)).
    #[error(
        "render node {name} is {node_major}:{node_minor}, not the compositor's main device {main_major}:{main_minor}"
    )]
    NotMainDevice {
        /// The node refused.
        name: String,
        /// Its major number.
        node_major: u32,
        /// Its minor number.
        node_minor: u32,
        /// The main device's major number.
        main_major: u32,
        /// The main device's minor number.
        main_minor: u32,
    },
    /// The device file's `st_rdev` differs from its sysfs `dev` attribute.
    #[error(
        "{path} has device number {found_major}:{found_minor}, not the {major}:{minor} sysfs lists"
    )]
    NodeMismatch {
        /// The device file.
        path: String,
        /// Its `st_rdev` major.
        found_major: u32,
        /// Its `st_rdev` minor.
        found_minor: u32,
        /// The sysfs major.
        major: u32,
        /// The sysfs minor.
        minor: u32,
    },
    /// `stat` on the device file failed.
    #[error("stat on {path} failed with errno {errno}")]
    Stat {
        /// The device file.
        path: String,
        /// The raw errno.
        errno: i32,
    },
}

/// One render node as `/sys/class/drm` lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderNode {
    /// The entry's name, `renderD` and a number.
    pub name: String,
    /// Its device number.
    pub dev: Dev,
    /// The kernel driver bound to its device, or `unbound`.
    pub driver: String,
}

impl RenderNode {
    /// The device file under `dev_dir`.
    #[must_use]
    pub fn path(&self, dev_dir: &Path) -> PathBuf {
        dev_dir.join(&self.name)
    }

    /// `major:minor`, for the record.
    #[must_use]
    pub fn number(&self) -> (u32, u32) {
        (major(self.dev), minor(self.dev))
    }
}

/// Parses a sysfs `dev` attribute, `major:minor` and a newline.
#[must_use]
pub fn parse_dev_attribute(text: &str) -> Option<Dev> {
    let (high, low) = text.trim_end_matches('\n').split_once(':')?;
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    if !digits(high) || !digits(low) {
        return None;
    }
    Some(makedev(high.parse().ok()?, low.parse().ok()?))
}

/// The main device's number as `zwp_linux_dmabuf_feedback_v1.main_device`
/// carries it: a `dev_t` in the host's byte order.
#[must_use]
pub fn dev_from_wire(bytes: &[u8]) -> Option<Dev> {
    let array: [u8; 8] = bytes.try_into().ok()?;
    Some(u64::from_ne_bytes(array))
}

/// Reads one entry's `dev` attribute.
fn read_dev(entry: &Path) -> Result<Dev, DeviceError> {
    let path = entry.join("dev");
    let refused = || DeviceError::DevAttribute {
        path: path.display().to_string(),
    };
    let text = std::fs::read_to_string(&path).map_err(|_| refused())?;
    if text.len() > MAX_DEV_ATTRIBUTE {
        return Err(refused());
    }
    parse_dev_attribute(&text).ok_or_else(refused)
}

/// The driver bound to an entry's device, read from its `device/driver` link.
fn read_driver(entry: &Path) -> String {
    std::fs::read_link(entry.join("device").join("driver"))
        .ok()
        .and_then(|target| {
            target
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "unbound".to_owned())
}

/// Every `renderD*` entry under `root`, sorted by name.
///
/// # Errors
///
/// Returns [`DeviceError::Enumerate`] when `root` cannot be listed and
/// [`DeviceError::DevAttribute`] for an entry whose `dev` is unreadable.
pub fn render_nodes(root: &Path) -> Result<Vec<RenderNode>, DeviceError> {
    let entries = std::fs::read_dir(root).map_err(|error| DeviceError::Enumerate {
        path: root.display().to_string(),
        reason: error.to_string(),
    })?;
    let mut nodes = Vec::new();
    for entry in entries.flatten().take(MAX_DRM_ENTRIES) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("renderD") {
            continue;
        }
        let path = entry.path();
        nodes.push(RenderNode {
            dev: read_dev(&path)?,
            driver: read_driver(&path),
            name,
        });
    }
    nodes.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(nodes)
}

/// Refuses `node` unless its device number is the main device's (M27
/// negative (b)).
///
/// # Errors
///
/// Returns [`DeviceError::NotMainDevice`] naming both numbers.
pub fn require_main_device(node: &RenderNode, main_device: Dev) -> Result<(), DeviceError> {
    if node.dev == main_device {
        return Ok(());
    }
    Err(DeviceError::NotMainDevice {
        name: node.name.clone(),
        node_major: major(node.dev),
        node_minor: minor(node.dev),
        main_major: major(main_device),
        main_minor: minor(main_device),
    })
}

/// The one render node under `root` whose device number is `main_device`.
///
/// # Errors
///
/// Returns the enumeration's errors and
/// [`DeviceError::NoNodeForMainDevice`] when no node matches.
pub fn main_device_node(root: &Path, main_device: Dev) -> Result<RenderNode, DeviceError> {
    render_nodes(root)?
        .into_iter()
        .find(|node| require_main_device(node, main_device).is_ok())
        .ok_or_else(|| DeviceError::NoNodeForMainDevice {
            root: root.display().to_string(),
            major: major(main_device),
            minor: minor(main_device),
        })
}

/// Checks that the device file for `node` under `dev_dir` is that node:
/// its `st_rdev` equals the number sysfs lists.
///
/// # Errors
///
/// Returns [`DeviceError::Stat`] or [`DeviceError::NodeMismatch`].
pub fn verify_device_file(node: &RenderNode, dev_dir: &Path) -> Result<PathBuf, DeviceError> {
    let path = node.path(dev_dir);
    let status = stat(&path).map_err(|error| DeviceError::Stat {
        path: path.display().to_string(),
        errno: errno(error),
    })?;
    if status.st_rdev != node.dev {
        return Err(DeviceError::NodeMismatch {
            path: path.display().to_string(),
            found_major: major(status.st_rdev),
            found_minor: minor(status.st_rdev),
            major: major(node.dev),
            minor: minor(node.dev),
        });
    }
    Ok(path)
}
