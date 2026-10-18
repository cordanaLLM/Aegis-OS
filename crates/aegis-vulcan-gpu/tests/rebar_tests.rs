// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Tests for Resizable BAR parsing and validation.

mod common;

use std::path::Path;

use aegis_vulcan_gpu::{inspect_device_rebar, parse_resize_bitmask};
use common::{Fallible, TempSysfs};

#[test]
fn test_resize_bitmask_single_bit() -> Fallible {
    let dummy_path = Path::new("/sys/bus/pci/devices/0000:f6:00.0/resource0_resize");
    // bit 4 => 2^(4 + 20) = 16 MiB = 16777216 bytes
    let sizes = parse_resize_bitmask(dummy_path, "0000000000000010")?;
    assert_eq!(sizes, vec![16_777_216]);
    Ok(())
}

#[test]
fn test_resize_bitmask_full_range() -> Fallible {
    let dummy_path = Path::new("/sys/bus/pci/devices/0000:f6:00.0/resource1_resize");
    // ffc0: bits 6 through 15 => 64 MiB to 32768 MiB (10 sizes)
    let sizes = parse_resize_bitmask(dummy_path, "000000000000ffc0")?;
    assert_eq!(sizes.len(), 10);
    assert_eq!(sizes.first(), Some(&67_108_864)); // 64 MiB
    assert_eq!(sizes.last(), Some(&34_359_738_368)); // 32 GiB
    Ok(())
}

#[test]
fn test_resize_bitmask_zero_mask_malformed() -> Fallible {
    let dummy_path = Path::new("/sys/bus/pci/devices/0000:f6:00.0/resource0_resize");
    let Err(err) = parse_resize_bitmask(dummy_path, "0000000000000000") else {
        return Err("expected error on zero resize bitmask".into());
    };
    assert!(err.to_string().contains("resize bitmask is zero"));
    Ok(())
}

#[test]
fn test_resource_line_missing_error() -> Fallible {
    let fix = TempSysfs::new()?;
    let dev_dir = fix
        .sysfs
        .join("bus")
        .join("pci")
        .join("devices")
        .join("0000:f6:00.0");
    std::fs::create_dir_all(&dev_dir)?;
    // Create resize file for BAR 3
    std::fs::write(dev_dir.join("resource3_resize"), "0000000000000020\n")?;
    // But resource file only has 1 line (BAR 0)
    std::fs::write(
        dev_dir.join("resource"),
        "0x00000000db000000 0x00000000dbffffff 0x0000000000040200\n",
    )?;

    let Err(err) = inspect_device_rebar(&dev_dir) else {
        return Err("expected error when BAR line is missing from resource file".into());
    };
    assert!(err.to_string().contains("missing line for BAR 3"));
    Ok(())
}

#[test]
fn test_device_rebar_measured_positive() -> Fallible {
    let fix = TempSysfs::new()?;
    // BAR 1: 32 GiB now; admits 64 MiB to 32 GiB; at largest: true
    fix.add_rebar(
        "0000:f6:00.0",
        1,
        "000000000000ffc0",
        0x0000_00f0_0000_0000,
        0x0000_00f7_ffff_ffff,
    )?;
    let dev_dir = fix
        .sysfs
        .join("bus")
        .join("pci")
        .join("devices")
        .join("0000:f6:00.0");
    let records = inspect_device_rebar(&dev_dir)?;
    assert_eq!(records.len(), 1);
    let Some(bar1) = records.first() else {
        return Err("expected at least one BAR record".into());
    };
    assert_eq!(bar1.bar, 1);
    assert_eq!(bar1.current_bytes, 34_359_738_368);
    assert!(bar1.at_largest);
    assert_eq!(
        bar1.format_line("0000:f6:00.0"),
        "     0000:f6:00.0 BAR1: 32768 MiB now; admits 64 MiB to 32768 MiB; at its largest: True"
    );
    Ok(())
}

#[test]
fn test_oversize_resource_file_malformed() -> Fallible {
    let fix = TempSysfs::new()?;
    let dev_dir = fix
        .sysfs
        .join("bus")
        .join("pci")
        .join("devices")
        .join("0000:f6:00.0");
    std::fs::create_dir_all(&dev_dir)?;
    std::fs::write(dev_dir.join("resource0_resize"), "0000000000000010\n")?;
    // Plant an oversize resource file (> 4096 bytes)
    let oversize = "0x00000000db000000 0x00000000dbffffff 0x0000000000040200\n".repeat(100);
    assert!(oversize.len() > 4096);
    std::fs::write(dev_dir.join("resource"), oversize)?;

    let res = inspect_device_rebar(&dev_dir);
    match res {
        Err(aegis_vulcan_gpu::GpuError::Malformed { path, reason }) => {
            assert!(path.ends_with("resource"));
            assert!(
                reason.contains("exceeds")
                    || reason.contains("bound")
                    || reason.contains("reaches")
            );
            Ok(())
        }
        other => {
            Err(format!("expected GpuError::Malformed on oversize resource, got {other:?}").into())
        }
    }
}
