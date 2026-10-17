// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Test fixture helpers for simulating sysfs and /dev environments.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Common result type for test setups and executions.
pub type Fallible = Result<(), Box<dyn std::error::Error>>;

/// An isolated temporary sysfs and /dev directory tree for testing.
pub struct TempSysfs {
    /// Root directory of the temporary fixture.
    pub root: PathBuf,
    /// Path to `<root>/sys`.
    pub sysfs: PathBuf,
    /// Path to `<root>/dev`.
    pub dev: PathBuf,
}

impl TempSysfs {
    /// Creates a new temporary sysfs fixture.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory structure cannot be created.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let name = format!("aegis-gpu-test-{pid}-{count}");
        let root = std::env::temp_dir().join(name);
        let sysfs = root.join("sys");
        let dev = root.join("dev");

        std::fs::create_dir_all(sysfs.join("class").join("drm"))?;
        std::fs::create_dir_all(sysfs.join("module").join("gpu_sched").join("holders"))?;
        std::fs::create_dir_all(sysfs.join("bus").join("pci").join("devices"))?;
        std::fs::create_dir_all(dev.join("dma_heap"))?;

        Ok(Self { root, sysfs, dev })
    }

    /// Adds a DRM render node and creates its device/driver symlinks.
    ///
    /// # Errors
    ///
    /// Returns an error if files or symlinks cannot be created.
    pub fn add_render_node(
        &self,
        node: &str,
        pci_addr: &str,
        driver: &str,
        modeset: Option<&str>,
    ) -> Fallible {
        let node_dir = self.sysfs.join("class").join("drm").join(node);
        std::fs::create_dir_all(&node_dir)?;

        let dev_dir = self
            .sysfs
            .join("bus")
            .join("pci")
            .join("devices")
            .join(pci_addr);
        std::fs::create_dir_all(&dev_dir)?;

        // Create symlink device -> ../../../bus/pci/devices/<pci_addr>
        let device_link = node_dir.join("device");
        let _ = std::os::unix::fs::symlink(
            format!("../../../bus/pci/devices/{pci_addr}"),
            &device_link,
        );

        // Create symlink driver -> ../../../../bus/pci/drivers/<driver>
        let driver_dir = self
            .sysfs
            .join("bus")
            .join("pci")
            .join("drivers")
            .join(driver);
        std::fs::create_dir_all(&driver_dir)?;
        let driver_link = dev_dir.join("driver");
        let _ = std::os::unix::fs::symlink(
            format!("../../../../bus/pci/drivers/{driver}"),
            &driver_link,
        );

        if let Some(val) = modeset {
            let mod_name = if driver == "nvidia" {
                "nvidia_drm"
            } else {
                driver
            };
            let params_dir = self.sysfs.join("module").join(mod_name).join("parameters");
            std::fs::create_dir_all(&params_dir)?;
            std::fs::write(params_dir.join("modeset"), format!("{val}\n"))?;
        }
        Ok(())
    }

    /// Sets the list of `gpu_sched` holder module names.
    ///
    /// # Errors
    ///
    /// Returns an error if files cannot be created.
    pub fn set_holders(&self, holders: &[&str]) -> Fallible {
        let holders_dir = self.sysfs.join("module").join("gpu_sched").join("holders");
        std::fs::create_dir_all(&holders_dir)?;
        for h in holders {
            let holder_path = holders_dir.join(h);
            std::fs::write(&holder_path, "")?;
        }
        Ok(())
    }

    /// Adds a full GPU device under `/sys/bus/pci/devices/<addr>`.
    ///
    /// # Errors
    ///
    /// Returns an error if device files cannot be created.
    #[allow(clippy::too_many_arguments)]
    pub fn add_gpu(
        &self,
        addr: &str,
        vendor: &str,
        device: &str,
        driver: &str,
        boot_vga: &str,
        iommu_group: &str,
        members: &[&str],
    ) -> Fallible {
        let dev_dir = self
            .sysfs
            .join("bus")
            .join("pci")
            .join("devices")
            .join(addr);
        std::fs::create_dir_all(&dev_dir)?;

        std::fs::write(dev_dir.join("class"), "0x030000\n")?;
        std::fs::write(dev_dir.join("vendor"), format!("{vendor}\n"))?;
        std::fs::write(dev_dir.join("device"), format!("{device}\n"))?;
        std::fs::write(dev_dir.join("boot_vga"), format!("{boot_vga}\n"))?;
        std::fs::write(dev_dir.join("driver_override"), "(null)\n")?;

        // driver symlink
        let driver_dir = self
            .sysfs
            .join("bus")
            .join("pci")
            .join("drivers")
            .join(driver);
        std::fs::create_dir_all(&driver_dir)?;
        let _ = std::os::unix::fs::symlink(
            format!("../../../../bus/pci/drivers/{driver}"),
            dev_dir.join("driver"),
        );

        // iommu group
        let group_dir = self
            .sysfs
            .join("kernel")
            .join("iommu_groups")
            .join(iommu_group);
        let group_devices = group_dir.join("devices");
        std::fs::create_dir_all(&group_devices)?;
        for m in members {
            std::fs::write(group_devices.join(m), "")?;
        }
        let _ = std::os::unix::fs::symlink(
            format!("../../../../kernel/iommu_groups/{iommu_group}"),
            dev_dir.join("iommu_group"),
        );
        Ok(())
    }

    /// Adds a display connector under `/sys/class/drm/<card>-<connector>`.
    ///
    /// # Errors
    ///
    /// Returns an error if connector files cannot be created.
    pub fn add_connector(&self, card: &str, connector: &str, status: &str) -> Fallible {
        let class_drm = self.sysfs.join("class").join("drm");
        std::fs::create_dir_all(class_drm.join(card))?;

        let conn_dir = class_drm.join(format!("{card}-{connector}"));
        std::fs::create_dir_all(&conn_dir)?;
        std::fs::write(conn_dir.join("status"), format!("{status}\n"))?;
        Ok(())
    }

    /// Adds a Resizable BAR configuration for a GPU device.
    ///
    /// # Errors
    ///
    /// Returns an error if `ReBAR` files cannot be created.
    pub fn add_rebar(
        &self,
        addr: &str,
        bar: usize,
        mask_hex: &str,
        start: u64,
        end: u64,
    ) -> Fallible {
        let dev_dir = self
            .sysfs
            .join("bus")
            .join("pci")
            .join("devices")
            .join(addr);
        std::fs::create_dir_all(&dev_dir)?;

        std::fs::write(
            dev_dir.join(format!("resource{bar}_resize")),
            format!("{mask_hex}\n"),
        )?;

        let res_path = dev_dir.join("resource");
        let mut lines = Vec::new();
        if let Ok(existing) = std::fs::read_to_string(&res_path) {
            lines = existing.lines().map(String::from).collect();
        }
        while lines.len() <= bar {
            lines.push("0x0000000000000000 0x0000000000000000 0x0000000000040200".to_owned());
        }
        if let Some(target) = lines.get_mut(bar) {
            *target = format!("{start:#018x} {end:#018x} 0x000000000014220c");
        }
        std::fs::write(&res_path, lines.join("\n") + "\n")?;
        Ok(())
    }

    /// Configures the `/dev/dma_heap/system` fixture.
    ///
    /// # Errors
    ///
    /// Returns an error if heap files cannot be written or removed.
    pub fn set_dma_heap(&self, present: bool) -> Fallible {
        let heap_dir = self.dev.join("dma_heap");
        std::fs::create_dir_all(&heap_dir)?;
        let heap_file = heap_dir.join("system");
        if present {
            std::fs::write(&heap_file, "")?;
        } else if heap_file.exists() {
            std::fs::remove_file(&heap_file)?;
        }
        Ok(())
    }
}

impl Drop for TempSysfs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
