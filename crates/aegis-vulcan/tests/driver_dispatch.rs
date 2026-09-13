// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The driver's two-state machine: map once, then dispatch.
//!
//! The second scaffold assertion -- no dispatch without a mapped window -- is
//! [`VulcanError::BarUnmapped`] here, so the negative case is an ordinary
//! test. The device table holds the isolation model REQ-P03-06 records.

mod common;

use aegis_vulcan::{
    DeviceTable, IommuGroup, MAX_VFIO_DEVICES, MapState, UserSpacePcieDriver, VfioDeviceConfig,
    VulcanError,
};

use common::{Fallible, bar, device, mapped, request};

// --- Positive -------------------------------------------------------------

/// Positive: a fresh driver maps, then dispatches, then completes.
#[test]
fn a_driver_maps_then_dispatches() -> Fallible {
    let mut driver = UserSpacePcieDriver::new(device()?);
    assert_eq!(driver.state(), MapState::Unmapped);
    assert_eq!(driver.state().name(), "unmapped");

    driver.map_bar()?;
    assert_eq!(driver.state(), MapState::Mapped);
    assert_eq!(driver.state().name(), "mapped");
    assert_eq!(driver.config(), device()?);

    let receipt = driver.execute_direct_dma(request(8)?)?;
    assert_eq!(receipt.bytes, 8 * 512);
    assert_eq!(driver.ring_head().get(), 0);
    Ok(())
}

/// Positive: the device table admits devices in distinct groups.
#[test]
fn the_device_table_admits_distinct_groups() -> Fallible {
    let mut table = DeviceTable::new();
    assert!(table.is_empty());
    assert_eq!(table.admit(device()?)?, 0);
    let second = VfioDeviceConfig::new(IommuGroup::new(13), 0x8086, 0x1234, bar()?);
    assert_eq!(table.admit(second)?, 1);
    assert_eq!(table.len(), 2);
    assert_eq!(table.get(1), Some(second));
    assert!(table.holds_group(IommuGroup::new(12)));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: an unmapped driver refuses to dispatch, and does not advance.
#[test]
fn an_unmapped_driver_refuses_to_dispatch() -> Fallible {
    let mut driver = UserSpacePcieDriver::new(device()?);
    assert_eq!(
        driver.execute_direct_dma(request(64)?),
        Err(VulcanError::BarUnmapped)
    );
    assert_eq!(driver.complete(), Err(VulcanError::BarUnmapped));
    assert_eq!(driver.ring_tail().get(), 0);
    Ok(())
}

/// Negative: mapping twice is refused rather than repeated.
#[test]
fn mapping_twice_is_refused() -> Fallible {
    let mut driver = mapped()?;
    assert_eq!(driver.map_bar(), Err(VulcanError::BarAlreadyMapped));
    assert_eq!(driver.state(), MapState::Mapped);
    Ok(())
}

/// Negative: two devices may not claim one IOMMU group (REQ-P03-06).
#[test]
fn two_devices_may_not_share_one_group() -> Fallible {
    let mut table = DeviceTable::new();
    table.admit(device()?)?;
    let clash = VfioDeviceConfig::new(IommuGroup::new(12), 0x8086, 0x1234, bar()?);
    assert_eq!(
        table.admit(clash),
        Err(VulcanError::GroupAlreadyHeld { group: 12 })
    );
    assert_eq!(table.len(), 1);
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the table fills at its bound and refuses the next device.
#[test]
fn the_device_table_is_full_at_its_bound() -> Fallible {
    let mut table = DeviceTable::new();
    for slot in 0..MAX_VFIO_DEVICES {
        let group = u32::try_from(slot).unwrap_or_default();
        let row = VfioDeviceConfig::new(IommuGroup::new(group), 0x144d, 0xa808, bar()?);
        assert_eq!(table.admit(row)?, slot);
    }
    assert_eq!(table.len(), MAX_VFIO_DEVICES);
    let overflow = VfioDeviceConfig::new(IommuGroup::new(999), 0x144d, 0xa808, bar()?);
    assert_eq!(table.admit(overflow), Err(VulcanError::DeviceTableFull));
    assert_eq!(table.get(MAX_VFIO_DEVICES), None);
    Ok(())
}

/// Boundary: an empty table holds nothing and answers every query.
#[test]
fn an_empty_table_answers_without_holding_anything() {
    let table = DeviceTable::default();
    assert_eq!(table.len(), 0);
    assert!(table.is_empty());
    assert_eq!(table.get(0), None);
    assert!(!table.holds_group(IommuGroup::new(0)));
}
