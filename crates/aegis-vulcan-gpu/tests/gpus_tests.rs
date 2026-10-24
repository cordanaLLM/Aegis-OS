// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Tests for GPU discovery and iteration bounds under `/sys/bus/pci/devices`.

mod common;

use aegis_vulcan_gpu::error::GpuError;
use aegis_vulcan_gpu::gpus::{MAX_PCI_DEVICES, discover_gpus};
use common::{Fallible, TempSysfs};

#[test]
fn test_planted_70_pci_devices_gpu_sorted_last_found() -> Fallible {
    let fix = TempSysfs::new()?;
    let pci_dir = fix.sysfs.join("bus").join("pci").join("devices");

    // Plant 69 non-GPU devices (class 0x020000, Ethernet)
    for i in 0..69 {
        let addr = format!("0000:00:{i:02x}.0");
        let dev = pci_dir.join(&addr);
        std::fs::create_dir_all(&dev)?;
        std::fs::write(dev.join("class"), "0x020000\n")?;
    }

    // Plant 70th device sorted last: 0000:ff:00.0 (GPU, class 0x030000)
    let gpu_addr = "0000:ff:00.0";
    let dev = pci_dir.join(gpu_addr);
    std::fs::create_dir_all(&dev)?;
    std::fs::write(dev.join("class"), "0x030000\n")?;
    std::fs::write(dev.join("vendor"), "0x10de\n")?;
    std::fs::write(dev.join("device"), "0x2684\n")?;

    let gpus = discover_gpus(&fix.sysfs)?;
    assert_eq!(gpus.len(), 1, "GPU sorted 70th must be discovered");
    assert!(gpus.contains_key(gpu_addr));
    Ok(())
}

fn plant_dummy_pci_devices(pci_dir: &std::path::Path, count: usize) -> Fallible {
    for i in 0..count {
        let bus = i / 256;
        let dev = i % 256;
        let addr = format!("0000:{bus:02x}:{dev:02x}.0");
        let dev_path = pci_dir.join(&addr);
        std::fs::create_dir_all(&dev_path)?;
        std::fs::write(dev_path.join("class"), "0x020000\n")?;
    }
    Ok(())
}

#[test]
fn test_pci_devices_over_bound_returns_bound_error() -> Fallible {
    let fix = TempSysfs::new()?;
    let pci_dir = fix.sysfs.join("bus").join("pci").join("devices");

    // Plant MAX_PCI_DEVICES + 1 (1025) devices
    plant_dummy_pci_devices(&pci_dir, MAX_PCI_DEVICES + 1)?;

    let res = discover_gpus(&fix.sysfs);
    match res {
        Err(GpuError::Bound { path, bound }) => {
            assert_eq!(path, pci_dir);
            assert_eq!(bound, MAX_PCI_DEVICES);
            Ok(())
        }
        other => Err(format!(
            "expected GpuError::Bound on {MAX_PCI_DEVICES} + 1 devices, got {other:?}"
        )
        .into()),
    }
}

#[test]
fn test_pci_devices_exactly_at_bound_ok() -> Fallible {
    let fix = TempSysfs::new()?;
    let pci_dir = fix.sysfs.join("bus").join("pci").join("devices");

    // Plant exactly MAX_PCI_DEVICES (1024) devices
    plant_dummy_pci_devices(&pci_dir, MAX_PCI_DEVICES)?;

    let gpus = discover_gpus(&fix.sysfs)?;
    assert_eq!(gpus.len(), 0);
    Ok(())
}
