// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The `DMA-BUF` export path, typed so M25 can bind it without reshaping.
//!
//! Both P03 descriptors end at a device, and on Linux the thing that hands a
//! buffer from one device to another is a `DMA-BUF` exported through a DRM
//! render node. [`DmaBufExport`] is that path as three fields: which render
//! node (`/dev/dri/renderD<minor>`), which driver owns it, and what the
//! driver's `modeset` parameter reads. Binding a descriptor to real hardware
//! at M25 is therefore filling in three values, not adding a field.
//!
//! # The reference profile, as observed
//!
//! [`REFERENCE_PROFILE_EXPORTS`] records what the reference profile in
//! `planning/hardware-profile.json` reports. The values were read on
//! 2026-09-13 by resolving `/sys/class/drm/renderD*/device/driver` and reading
//! `/sys/module/<driver>/parameters/modeset`:
//!
//! | Node | Driver | `modeset` reads |
//! | :-- | :-- | :-- |
//! | `renderD128` | `nvidia` (`nvidia_drm`) | `Y` |
//! | `renderD129` | `i915` | not readable without privilege |
//! | `renderD130` | `amdgpu` | `-1` |
//!
//! Two of those readings are [`ModesetState::DriverDefault`] rather than
//! [`ModesetState::Enabled`]: `amdgpu` reports the driver's own auto value and
//! the `i915` parameter is root-readable only, so an unprivileged probe cannot
//! distinguish them. That is why the field is a tri-state and not a boolean.
//!
//! # What is admitted
//!
//! [`ModesetState::Disabled`] is refused for every driver: with modesetting
//! off the driver publishes no render node, so there is nothing to export
//! from. [`DrmDriver::NvidiaDrm`] additionally requires an explicit
//! [`ModesetState::Enabled`], which is exactly the `Y` the reference profile
//! reports, because the out-of-tree driver's DRM half -- and with it the PRIME
//! export path -- is what that parameter turns on.
//!
//! # What this module does not do
//!
//! It opens no render node, exports no `DMA-BUF`, imports no file descriptor and
//! reads no `sysfs` file. Every value arrives as data. The table above is a
//! transcription of a probe run by hand, and a test that it round-trips is not
//! evidence that a buffer can be shared between those three devices; that is
//! M25 work over the real devices.

use core::fmt;

/// The lowest DRM render-node minor number.
pub const RENDER_NODE_MINOR_BASE: u32 = 128;

/// The highest render-node minor number this contract admits.
///
/// This is a bound chosen here, not a recorded requirement and not a hard
/// kernel limit. `128..=191` is the legacy compatibility window Linux
/// allocates render minors from: `drivers/gpu/drm/drm_drv.c` asks for
/// `XA_LIMIT(64 * _t, 64 * _t + 63)` and `include/drm/drm_file.h` fixes
/// `DRM_MINOR_RENDER = 2` as ABI, noting that the enum value is what
/// "determines `/dev/dri/renderD*` numbers". Read on 2026-09-13 against
/// Linux 7.2.4, the version `rust-toolchain.toml`'s host runs.
///
/// The same file also allocates from `DRM_EXTENDED_MINOR_LIMIT`
/// (`XA_LIMIT(192, (1 << MINORBITS) - 1)`) once that window is full, so a
/// host with more than 64 DRM devices can publish a render node this contract
/// refuses. The reference profile publishes three. Widening the bound is a
/// decision for whoever meets such a host, not a default taken here.
pub const RENDER_NODE_MINOR_MAX: u32 = 191;

/// The prefix every render-node device name carries.
pub const RENDER_NODE_PREFIX: &str = "renderD";

/// Reasons an export path is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum DmaBufError {
    /// The render-node minor is outside the range Linux reserves.
    #[error(
        "render-node minor {minor} is outside {RENDER_NODE_MINOR_BASE}..={RENDER_NODE_MINOR_MAX}"
    )]
    MinorOutOfRange {
        /// The minor that was refused.
        minor: u32,
    },
    /// The device name is not a render-node name.
    #[error("a render node is named {RENDER_NODE_PREFIX} followed by its minor number")]
    NotARenderNode,
    /// Modesetting is off, so the driver publishes no render node.
    #[error("modesetting is disabled, so this driver publishes no render node to export from")]
    ModesetDisabled,
    /// The out-of-tree driver needs modesetting explicitly enabled.
    #[error("nvidia_drm exports through its DRM half, which needs modeset explicitly enabled")]
    ModesetNotExplicit,
}

impl DmaBufError {
    /// Returns a stable one-line reason, for a correlated contract refusal.
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::MinorOutOfRange { .. } => "the render-node minor is outside the reserved range",
            Self::NotARenderNode => "the device name is not a render-node name",
            Self::ModesetDisabled => "modesetting is disabled, so no render node is published",
            Self::ModesetNotExplicit => "nvidia_drm needs modeset explicitly enabled",
        }
    }
}

/// Returns `true` when `digits` is the one decimal spelling of its value.
///
/// `u32::from_str` admits a leading sign and leading zeros, so `renderD+128`
/// and `renderD0128` would otherwise parse and then render back through
/// [`fmt::Display`] as `renderD128`. No Linux system publishes either
/// spelling: the kernel formats the node name from the allocated minor, so a
/// name that does not round-trip is not a name any device carries.
fn is_canonical_decimal(digits: &str) -> bool {
    if digits.is_empty() {
        return false;
    }
    if digits.starts_with('0') && digits.len() > 1 {
        return false;
    }
    digits.bytes().all(|byte| byte.is_ascii_digit())
}

/// A DRM render node, named by its device minor number.
///
/// The device path is `/dev/dri/renderD<minor>`; [`fmt::Display`] renders the
/// device name, so the wire form stays a small integer and no rendering step
/// allocates.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(into = "u32", try_from = "u32")]
pub struct RenderNode(u32);

impl RenderNode {
    /// Validates a render-node minor number.
    ///
    /// # Errors
    ///
    /// Returns [`DmaBufError::MinorOutOfRange`] outside `128..=191`.
    pub const fn new(minor: u32) -> Result<Self, DmaBufError> {
        if minor < RENDER_NODE_MINOR_BASE || minor > RENDER_NODE_MINOR_MAX {
            return Err(DmaBufError::MinorOutOfRange { minor });
        }
        Ok(Self(minor))
    }

    /// Parses a render-node device name such as `renderD129`.
    ///
    /// # Errors
    ///
    /// Returns [`DmaBufError::NotARenderNode`] when the name does not carry
    /// the [`RENDER_NODE_PREFIX`] followed by the canonical decimal spelling
    /// of a minor, and propagates [`Self::new`] for a minor outside the range.
    pub fn parse(name: &str) -> Result<Self, DmaBufError> {
        let digits = name
            .strip_prefix(RENDER_NODE_PREFIX)
            .ok_or(DmaBufError::NotARenderNode)?;
        if !is_canonical_decimal(digits) {
            return Err(DmaBufError::NotARenderNode);
        }
        let minor: u32 = digits.parse().map_err(|_| DmaBufError::NotARenderNode)?;
        Self::new(minor)
    }

    /// Returns the device minor number.
    #[must_use]
    pub const fn minor(self) -> u32 {
        self.0
    }
}

impl fmt::Display for RenderNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{RENDER_NODE_PREFIX}{}", self.0)
    }
}

impl From<RenderNode> for u32 {
    fn from(value: RenderNode) -> Self {
        value.0
    }
}

impl TryFrom<u32> for RenderNode {
    type Error = DmaBufError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// The DRM driver that owns a render node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum DrmDriver {
    /// The in-tree Intel driver.
    #[serde(rename = "i915")]
    I915,
    /// The in-tree AMD driver.
    #[serde(rename = "amdgpu")]
    Amdgpu,
    /// The DRM half of the out-of-tree NVIDIA driver.
    #[serde(rename = "nvidia_drm")]
    NvidiaDrm,
}

impl DrmDriver {
    /// Every driver the reference profile reports, in render-node order.
    pub const ALL: [Self; 3] = [Self::NvidiaDrm, Self::I915, Self::Amdgpu];

    /// Returns the module name the driver is loaded under.
    #[must_use]
    pub const fn module(self) -> &'static str {
        match self {
            Self::I915 => "i915",
            Self::Amdgpu => "amdgpu",
            Self::NvidiaDrm => "nvidia_drm",
        }
    }

    /// Returns `true` when the driver is built in tree.
    #[must_use]
    pub const fn in_tree(self) -> bool {
        matches!(self, Self::I915 | Self::Amdgpu)
    }
}

/// What a driver's `modeset` parameter reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ModesetState {
    /// The parameter reads an explicit yes.
    Enabled,
    /// The parameter reads an explicit no.
    Disabled,
    /// The parameter reads the driver's own default, or cannot be read
    /// without privilege.
    DriverDefault,
}

impl ModesetState {
    /// Returns the stable name this reading is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::DriverDefault => "driver-default",
        }
    }
}

/// The `DMA-BUF` export path a descriptor binds to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DmaBufExport {
    /// The render node the buffer is exported through.
    pub node: RenderNode,
    /// The driver that owns the node.
    pub driver: DrmDriver,
    /// What that driver's `modeset` parameter reads.
    pub modeset: ModesetState,
}

impl DmaBufExport {
    /// Names an export path.
    #[must_use]
    pub const fn new(node: RenderNode, driver: DrmDriver, modeset: ModesetState) -> Self {
        Self {
            node,
            driver,
            modeset,
        }
    }

    /// Checks that the path could publish a render node to export from.
    ///
    /// # Errors
    ///
    /// Returns [`DmaBufError::ModesetDisabled`] for any driver with
    /// modesetting off, and [`DmaBufError::ModesetNotExplicit`] for
    /// [`DrmDriver::NvidiaDrm`] without an explicit
    /// [`ModesetState::Enabled`].
    pub const fn validate(&self) -> Result<(), DmaBufError> {
        match (self.driver, self.modeset) {
            (_, ModesetState::Disabled) => Err(DmaBufError::ModesetDisabled),
            (DrmDriver::NvidiaDrm, ModesetState::DriverDefault) => {
                Err(DmaBufError::ModesetNotExplicit)
            }
            _ => Ok(()),
        }
    }
}

/// The three export paths the reference profile reports.
///
/// Transcribed on 2026-09-13 from `/sys/class/drm/renderD*/device/driver` and
/// `/sys/module/<driver>/parameters/modeset`. This is development evidence
/// about one workstation, recorded so M25 has something to bind against; it is
/// not a product requirement and it closes no hardware gate.
pub const REFERENCE_PROFILE_EXPORTS: [DmaBufExport; 3] = [
    DmaBufExport {
        node: RenderNode(128),
        driver: DrmDriver::NvidiaDrm,
        modeset: ModesetState::Enabled,
    },
    DmaBufExport {
        node: RenderNode(129),
        driver: DrmDriver::I915,
        modeset: ModesetState::DriverDefault,
    },
    DmaBufExport {
        node: RenderNode(130),
        driver: DrmDriver::Amdgpu,
        modeset: ModesetState::DriverDefault,
    },
];
