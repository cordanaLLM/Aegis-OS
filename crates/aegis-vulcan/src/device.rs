// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The `VFIO` device configuration and the bounded device table.
//!
//! REQ-P03-06 records the isolation model as `VFIO` group isolation with an
//! `IOMMU` domain per group. [`DeviceTable`] is that model as data: a fixed
//! array of at most [`MAX_VFIO_DEVICES`] entries where no two devices may
//! claim the same [`IommuGroup`]. Admitting a device is a bounded scan of the
//! array; nothing here grows, and nothing here recurses.
//!
//! # What this module does not do
//!
//! No `VFIO` container is opened, no group is bound, no `IOMMU` domain is
//! attached and `/dev/vfio` is never named. A [`VfioDeviceConfig`] is a
//! validated description of a device a real implementation would bind.

use crate::bar::BarWindow;
use crate::error::VulcanError;

/// Scalar upper bound on the devices one table holds.
///
/// The scaffold's `MAX_VFIO_DEVICES` (export-038 `fe2cb01cfd13`).
pub const MAX_VFIO_DEVICES: usize = 8;

/// The `IOMMU` group a device is isolated in.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct IommuGroup(u32);

impl IommuGroup {
    /// Names an `IOMMU` group.
    #[must_use]
    pub const fn new(group: u32) -> Self {
        Self(group)
    }

    /// Returns the group number.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// One `PCIe` function, as the P03 model describes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct VfioDeviceConfig {
    /// The `IOMMU` group the function is isolated in.
    pub group: IommuGroup,
    /// The PCI vendor identifier.
    pub vendor_id: u16,
    /// The PCI device identifier.
    pub device_id: u16,
    /// The BAR 0 window, page aligned and bounded.
    pub bar0: BarWindow,
}

impl VfioDeviceConfig {
    /// Builds a device configuration from validated parts.
    #[must_use]
    pub const fn new(group: IommuGroup, vendor_id: u16, device_id: u16, bar0: BarWindow) -> Self {
        Self {
            group,
            vendor_id,
            device_id,
            bar0,
        }
    }

    /// Checks the invariants a decoded configuration could arrive without.
    ///
    /// # Errors
    ///
    /// Propagates [`BarWindow::validate`].
    pub const fn validate(&self) -> Result<(), VulcanError> {
        self.bar0.validate()
    }
}

/// A bounded table of admitted devices, one `IOMMU` group each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceTable {
    rows: [Option<VfioDeviceConfig>; MAX_VFIO_DEVICES],
    len: usize,
}

impl Default for DeviceTable {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceTable {
    /// Builds an empty table.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            rows: [None; MAX_VFIO_DEVICES],
            len: 0,
        }
    }

    /// Returns how many devices the table holds.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when the table holds no device.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the device at `slot`, when the table holds one there.
    #[must_use]
    pub fn get(&self, slot: usize) -> Option<VfioDeviceConfig> {
        self.rows.get(slot).copied().flatten()
    }

    /// Returns `true` when some device already holds `group`.
    ///
    /// The scan is bounded by the array, which is [`MAX_VFIO_DEVICES`] long.
    #[must_use]
    pub fn holds_group(&self, group: IommuGroup) -> bool {
        self.rows
            .iter()
            .flatten()
            .any(|device| device.group == group)
    }

    /// Admits one device, returning the slot it landed in.
    ///
    /// # Errors
    ///
    /// Returns [`VulcanError::DeviceTableFull`] past the bound,
    /// [`VulcanError::GroupAlreadyHeld`] when another device already holds the
    /// group, and propagates [`VfioDeviceConfig::validate`].
    pub fn admit(&mut self, device: VfioDeviceConfig) -> Result<usize, VulcanError> {
        device.validate()?;
        if self.holds_group(device.group) {
            return Err(VulcanError::GroupAlreadyHeld {
                group: device.group.get(),
            });
        }
        let slot = self.len;
        let Some(row) = self.rows.get_mut(slot) else {
            return Err(VulcanError::DeviceTableFull);
        };
        *row = Some(device);
        self.len = slot.saturating_add(1);
        Ok(slot)
    }
}
