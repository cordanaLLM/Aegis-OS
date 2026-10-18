// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Tests for VFIO candidate selection and isolation rules.

mod common;

use std::collections::BTreeMap;

use aegis_vulcan_gpu::vfio::{
    VFIO_CHECK_EXTENSION, VFIO_DEVICE_GET_REGION_INFO, VFIO_GET_API_VERSION,
    VFIO_GROUP_FLAGS_CONTAINER_SET, VFIO_GROUP_FLAGS_VIABLE, VFIO_GROUP_GET_DEVICE_FD,
    VFIO_GROUP_GET_STATUS, VFIO_GROUP_SET_CONTAINER, VFIO_REGION_INFO_FLAG_CAPS,
    VFIO_REGION_INFO_FLAG_MMAP, VFIO_REGION_INFO_FLAG_READ, VFIO_REGION_INFO_FLAG_WRITE,
    VFIO_SET_IOMMU, VfioCandidateInput, VfioDecision, VfioGroupStatus, VfioRegionInfo,
    choose_bar_to_map, evaluate_candidate, format_bar_region, format_region_flags,
    parse_prefetchable_bars, run_map_bar,
};
use common::{Fallible, TempSysfs};

#[test]
fn sole_occupant_headless_admitted() {
    let mut connectors = BTreeMap::new();
    connectors.insert("card1-DP-1".to_string(), "disconnected".to_string());
    connectors.insert("card1-HDMI-A-1".to_string(), "disconnected".to_string());

    let input = VfioCandidateInput {
        address: "0000:03:00.0".to_string(),
        driver: "amdgpu".to_string(),
        iommu_group: "25".to_string(),
        group_members: vec!["0000:03:00.0".to_string()],
        connectors,
        boot_vga: false,
    };

    let decision = evaluate_candidate(&input);
    assert_eq!(decision, VfioDecision::Admitted);
    assert_eq!(
        decision.format_line(&input.address, &input.driver),
        "     0000:03:00.0 (amdgpu): admitted"
    );
}

#[test]
fn group_with_two_functions_refused() -> Fallible {
    let input = VfioCandidateInput {
        address: "0000:03:00.0".to_string(),
        driver: "amdgpu".to_string(),
        iommu_group: "25".to_string(),
        group_members: vec!["0000:03:00.0".to_string(), "0000:03:00.1".to_string()],
        connectors: BTreeMap::new(),
        boot_vga: false,
    };

    let decision = evaluate_candidate(&input);
    match decision {
        VfioDecision::Refused(reasons) => {
            assert_eq!(reasons.len(), 1);
            let first = reasons.first().ok_or("missing reason")?;
            assert!(
                first.contains("shares IOMMU group 25 with 0000:03:00.1"),
                "unexpected reason: {first:?}"
            );
            Ok(())
        }
        VfioDecision::Admitted => Err("expected refused for shared IOMMU group".into()),
    }
}

#[test]
fn connected_output_refused() -> Fallible {
    let mut connectors = BTreeMap::new();
    connectors.insert("card0-DP-1".to_string(), "connected".to_string());
    connectors.insert("card0-HDMI-A-1".to_string(), "disconnected".to_string());

    let input = VfioCandidateInput {
        address: "0000:00:02.0".to_string(),
        driver: "i915".to_string(),
        iommu_group: "0".to_string(),
        group_members: vec!["0000:00:02.0".to_string()],
        connectors,
        boot_vga: false,
    };

    let decision = evaluate_candidate(&input);
    match decision {
        VfioDecision::Refused(reasons) => {
            assert_eq!(reasons.len(), 1);
            let first = reasons.first().ok_or("missing reason")?;
            assert!(
                first.contains("card0-DP-1 (connected)"),
                "unexpected reason: {first:?}"
            );
            Ok(())
        }
        VfioDecision::Admitted => Err("expected refused for connected output".into()),
    }
}

#[test]
fn boot_vga_refused() -> Fallible {
    let input = VfioCandidateInput {
        address: "0000:00:02.0".to_string(),
        driver: "xe".to_string(),
        iommu_group: "0".to_string(),
        group_members: vec!["0000:00:02.0".to_string()],
        connectors: BTreeMap::new(),
        boot_vga: true,
    };

    let decision = evaluate_candidate(&input);
    match decision {
        VfioDecision::Refused(reasons) => {
            assert_eq!(reasons.len(), 1);
            let first = reasons.first().ok_or("missing reason")?;
            assert!(
                first.contains("is the boot VGA device"),
                "unexpected reason: {first:?}"
            );
            Ok(())
        }
        VfioDecision::Admitted => Err("expected refused for boot VGA".into()),
    }
}

#[test]
fn multiple_refusal_reasons_accumulate() -> Fallible {
    let mut connectors = BTreeMap::new();
    connectors.insert("card0-DP-1".to_string(), "connected".to_string());

    let input = VfioCandidateInput {
        address: "0000:00:02.0".to_string(),
        driver: "xe".to_string(),
        iommu_group: "0".to_string(),
        group_members: vec!["0000:00:02.0".to_string(), "0000:00:02.1".to_string()],
        connectors,
        boot_vga: true,
    };

    let decision = evaluate_candidate(&input);
    match decision {
        VfioDecision::Refused(ref reasons) => {
            assert_eq!(reasons.len(), 3);
            let r0 = reasons.first().ok_or("missing r0")?;
            let r1 = reasons.get(1).ok_or("missing r1")?;
            let r2 = reasons.get(2).ok_or("missing r2")?;
            assert!(r0.contains("shares IOMMU group 0"));
            assert!(r1.contains("card0-DP-1 (connected)"));
            assert!(r2.contains("is the boot VGA device"));
            let line = decision.format_line(&input.address, &input.driver);
            assert!(line.contains("refused:"));
            assert!(line.contains("; "));
            Ok(())
        }
        VfioDecision::Admitted => Err("expected refused with 3 reasons".into()),
    }
}

#[test]
fn empty_iommu_group_refused() -> Fallible {
    let mut connectors = BTreeMap::new();
    connectors.insert("card1-DP-1".to_string(), "disconnected".to_string());

    let input = VfioCandidateInput {
        address: "0000:03:00.0".to_string(),
        driver: "amdgpu".to_string(),
        iommu_group: String::new(),
        group_members: vec!["0000:03:00.0".to_string()],
        connectors,
        boot_vga: false,
    };

    let decision = evaluate_candidate(&input);
    match decision {
        VfioDecision::Refused(reasons) => {
            assert_eq!(reasons.len(), 1);
            let first = reasons.first().ok_or("missing reason")?;
            assert_eq!(
                first,
                "0000:03:00.0 has no IOMMU group: the IOMMU is off or the device is not isolated"
            );
            Ok(())
        }
        VfioDecision::Admitted => Err("expected refused for empty IOMMU group".into()),
    }
}

#[test]
fn unknown_connector_status_refused() -> Fallible {
    let mut connectors = BTreeMap::new();
    connectors.insert("card1-DP-1".to_string(), "unknown".to_string());
    connectors.insert("card1-HDMI-A-1".to_string(), "disconnected".to_string());

    let input = VfioCandidateInput {
        address: "0000:03:00.0".to_string(),
        driver: "amdgpu".to_string(),
        iommu_group: "25".to_string(),
        group_members: vec!["0000:03:00.0".to_string()],
        connectors,
        boot_vga: false,
    };

    let decision = evaluate_candidate(&input);
    match decision {
        VfioDecision::Refused(reasons) => {
            assert_eq!(reasons.len(), 1);
            let first = reasons.first().ok_or("missing reason")?;
            assert!(
                first.contains("card1-DP-1") && first.contains("unknown"),
                "unexpected reason: {first:?}"
            );
            Ok(())
        }
        VfioDecision::Admitted => Err("expected refused for unknown connector status".into()),
    }
}

#[test]
fn group_with_only_itself_boundary_admitted() {
    let input = VfioCandidateInput {
        address: "0000:03:00.0".to_string(),
        driver: "amdgpu".to_string(),
        iommu_group: "42".to_string(),
        group_members: vec!["0000:03:00.0".to_string()],
        connectors: BTreeMap::new(),
        boot_vga: false,
    };
    let decision = evaluate_candidate(&input);
    assert_eq!(decision, VfioDecision::Admitted);
}

#[test]
fn writeback_connector_is_no_display_output() {
    let mut connectors = BTreeMap::new();
    connectors.insert("card0-Writeback-1".to_string(), "unknown".to_string());
    connectors.insert("card0-DP-4".to_string(), "disconnected".to_string());

    let input = VfioCandidateInput {
        address: "0000:7d:00.0".to_string(),
        driver: "amdgpu".to_string(),
        iommu_group: "31".to_string(),
        group_members: vec!["0000:7d:00.0".to_string()],
        connectors,
        boot_vga: false,
    };

    assert_eq!(evaluate_candidate(&input), VfioDecision::Admitted);
}

#[test]
fn writeback_beside_an_unknown_output_still_refuses_that_output() -> Fallible {
    let mut connectors = BTreeMap::new();
    connectors.insert("card0-Writeback-1".to_string(), "unknown".to_string());
    connectors.insert("card0-DP-4".to_string(), "unknown".to_string());

    let input = VfioCandidateInput {
        address: "0000:7d:00.0".to_string(),
        driver: "amdgpu".to_string(),
        iommu_group: "31".to_string(),
        group_members: vec!["0000:7d:00.0".to_string()],
        connectors,
        boot_vga: false,
    };

    match evaluate_candidate(&input) {
        VfioDecision::Refused(reasons) => {
            let joined = reasons.join("; ");
            if joined.contains("card0-DP-4 (unknown)") && !joined.contains("Writeback") {
                Ok(())
            } else {
                Err(format!("unexpected reasons: {joined}").into())
            }
        }
        VfioDecision::Admitted => Err("an unknown DP output must refuse".into()),
    }
}

#[test]
fn positive_pre_open_admits_and_fails_on_missing_group_node() -> Fallible {
    let fix = TempSysfs::new()?;
    let addr = "0000:03:00.0";
    fix.add_gpu(addr, "0x8086", "0x56a5", "vfio-pci", "0", "42", &[addr])?;

    // Create container /dev/vfio/vfio in the fake dev tree
    let vfio_dir = fix.dev.join("vfio");
    std::fs::create_dir_all(&vfio_dir)?;
    std::fs::write(vfio_dir.join("vfio"), "")?;
    // fix.dev/vfio/42 is deliberately absent.

    let run = run_map_bar(&fix.sysfs, &fix.dev, addr);
    assert!(run.failed, "run must fail on missing group node");
    assert_eq!(run.lines.len(), 1);
    let line = run.lines.first().ok_or("missing line")?;
    assert!(
        line.starts_with("FAIL: open "),
        "expected FAIL: open ..., got: {line}"
    );
    assert!(line.contains("42"), "line must name the group path: {line}");
    Ok(())
}

#[test]
fn negative_driver_xe_refused_naming_xe_opens_nothing() -> Fallible {
    let fix = TempSysfs::new()?;
    let addr = "0000:03:00.0";
    fix.add_gpu(addr, "0x8086", "0x56a5", "xe", "0", "17", &[addr])?;

    let vfio_dir = fix.dev.join("vfio");
    std::fs::create_dir_all(&vfio_dir)?;
    std::fs::write(vfio_dir.join("vfio"), "")?;
    std::fs::write(vfio_dir.join("17"), "")?;

    let run = run_map_bar(&fix.sysfs, &fix.dev, addr);
    assert!(run.failed, "run must fail for driver xe");
    let line = run.lines.first().ok_or("missing line")?;
    assert!(
        line.starts_with("FAIL: candidate: refused: "),
        "unexpected: {line}"
    );
    assert!(line.contains("xe"), "must name driver xe: {line}");
    assert!(
        line.contains("expected vfio-pci"),
        "must state expected vfio-pci: {line}"
    );
    Ok(())
}

#[test]
fn negative_group_shared_refused() -> Fallible {
    let fix = TempSysfs::new()?;
    let addr = "0000:03:00.0";
    let other = "0000:03:00.1";
    fix.add_gpu(
        addr,
        "0x8086",
        "0x56a5",
        "vfio-pci",
        "0",
        "25",
        &[addr, other],
    )?;

    let run = run_map_bar(&fix.sysfs, &fix.dev, addr);
    assert!(run.failed, "run must fail for shared group");
    let line = run.lines.first().ok_or("missing line")?;
    assert!(
        line.starts_with("FAIL: candidate: refused: "),
        "unexpected: {line}"
    );
    assert!(
        line.contains("shares IOMMU group 25 with 0000:03:00.1"),
        "unexpected: {line}"
    );
    Ok(())
}

#[test]
fn negative_connected_connector_refused() -> Fallible {
    let fix = TempSysfs::new()?;
    let addr = "0000:03:00.0";
    fix.add_gpu(addr, "0x8086", "0x56a5", "vfio-pci", "0", "42", &[addr])?;
    let dev_dir = fix.sysfs.join("bus").join("pci").join("devices").join(addr);
    std::fs::create_dir_all(dev_dir.join("drm").join("card0"))?;
    fix.add_connector("card0", "DP-1", "connected")?;

    let run = run_map_bar(&fix.sysfs, &fix.dev, addr);
    assert!(run.failed, "run must fail for connected connector");
    let line = run.lines.first().ok_or("missing line")?;
    assert!(
        line.starts_with("FAIL: candidate: refused: "),
        "unexpected: {line}"
    );
    assert!(
        line.contains("card0-DP-1 (connected)"),
        "unexpected: {line}"
    );
    Ok(())
}

#[test]
fn boundary_writeback_unknown_status_admitted() -> Fallible {
    let fix = TempSysfs::new()?;
    let addr = "0000:03:00.0";
    fix.add_gpu(addr, "0x8086", "0x56a5", "vfio-pci", "0", "42", &[addr])?;
    let dev_dir = fix.sysfs.join("bus").join("pci").join("devices").join(addr);
    std::fs::create_dir_all(dev_dir.join("drm").join("card0"))?;
    fix.add_connector("card0", "Writeback-1", "unknown")?;
    fix.add_connector("card0", "DP-1", "disconnected")?;

    let vfio_dir = fix.dev.join("vfio");
    std::fs::create_dir_all(&vfio_dir)?;
    std::fs::write(vfio_dir.join("vfio"), "")?;

    let run = run_map_bar(&fix.sysfs, &fix.dev, addr);
    assert!(run.failed);
    let line = run.lines.first().ok_or("missing line")?;
    assert!(
        line.starts_with("FAIL: open "),
        "expected FAIL: open ..., got: {line}"
    );
    assert!(line.contains("42"), "must reach group open: {line}");
    Ok(())
}

#[test]
fn boundary_format_bar_region_zero_size_skipped() {
    let info_zero = VfioRegionInfo {
        argsz: 32,
        flags: VFIO_REGION_INFO_FLAG_READ
            | VFIO_REGION_INFO_FLAG_WRITE
            | VFIO_REGION_INFO_FLAG_MMAP,
        index: 0,
        cap_offset: 0,
        size: 0,
        offset: 0,
    };
    assert_eq!(format_bar_region(0, &info_zero), None);

    let info_nonzero = VfioRegionInfo {
        argsz: 32,
        flags: VFIO_REGION_INFO_FLAG_READ
            | VFIO_REGION_INFO_FLAG_WRITE
            | VFIO_REGION_INFO_FLAG_MMAP,
        index: 0,
        cap_offset: 0,
        size: 16_777_216,
        offset: 0x1000,
    };
    assert_eq!(
        format_bar_region(0, &info_nonzero),
        Some("BAR0 size=16777216 flags=r,w,mmap offset=0x1000".to_string())
    );
}

#[test]
fn boundary_choose_bar_to_map() {
    let regions = vec![
        VfioRegionInfo {
            argsz: 32,
            flags: VFIO_REGION_INFO_FLAG_MMAP,
            index: 0,
            cap_offset: 0,
            size: 0,
            offset: 0,
        },
        VfioRegionInfo {
            argsz: 32,
            flags: VFIO_REGION_INFO_FLAG_MMAP,
            index: 1,
            cap_offset: 0,
            size: 4095,
            offset: 0x1000,
        },
        VfioRegionInfo {
            argsz: 32,
            flags: VFIO_REGION_INFO_FLAG_READ | VFIO_REGION_INFO_FLAG_WRITE,
            index: 2,
            cap_offset: 0,
            size: 8192,
            offset: 0x2000,
        },
        VfioRegionInfo {
            argsz: 32,
            flags: VFIO_REGION_INFO_FLAG_MMAP | VFIO_REGION_INFO_FLAG_READ,
            index: 3,
            cap_offset: 0,
            size: 4096,
            offset: 0x4000,
        },
        VfioRegionInfo {
            argsz: 32,
            flags: VFIO_REGION_INFO_FLAG_MMAP,
            index: 4,
            cap_offset: 0,
            size: 16_384,
            offset: 0x8000,
        },
    ];

    assert_eq!(choose_bar_to_map(&regions, &[true; 6]), Some(3));
}

#[test]
fn region_info_struct_packing() {
    assert_eq!(core::mem::size_of::<VfioRegionInfo>(), 32);
    assert_eq!(core::mem::align_of::<VfioRegionInfo>(), 8);
    assert_eq!(core::mem::offset_of!(VfioRegionInfo, argsz), 0);
    assert_eq!(core::mem::offset_of!(VfioRegionInfo, flags), 4);
    assert_eq!(core::mem::offset_of!(VfioRegionInfo, index), 8);
    assert_eq!(core::mem::offset_of!(VfioRegionInfo, cap_offset), 12);
    assert_eq!(core::mem::offset_of!(VfioRegionInfo, size), 16);
    assert_eq!(core::mem::offset_of!(VfioRegionInfo, offset), 24);
}

#[test]
fn group_status_struct_packing_and_decode() {
    assert_eq!(core::mem::size_of::<VfioGroupStatus>(), 8);
    assert_eq!(core::mem::offset_of!(VfioGroupStatus, argsz), 0);
    assert_eq!(core::mem::offset_of!(VfioGroupStatus, flags), 4);

    let status = VfioGroupStatus {
        argsz: 8,
        flags: VFIO_GROUP_FLAGS_VIABLE | VFIO_GROUP_FLAGS_CONTAINER_SET,
    };
    assert_eq!(status.flags & VFIO_GROUP_FLAGS_VIABLE, 1);
    assert_eq!(status.flags & VFIO_GROUP_FLAGS_CONTAINER_SET, 2);
}

#[test]
fn region_flags_formatting() {
    assert_eq!(format_region_flags(0), "");
    assert_eq!(format_region_flags(VFIO_REGION_INFO_FLAG_READ), "r");
    assert_eq!(
        format_region_flags(VFIO_REGION_INFO_FLAG_READ | VFIO_REGION_INFO_FLAG_WRITE),
        "r,w"
    );
    assert_eq!(
        format_region_flags(
            VFIO_REGION_INFO_FLAG_READ | VFIO_REGION_INFO_FLAG_WRITE | VFIO_REGION_INFO_FLAG_MMAP
        ),
        "r,w,mmap"
    );
    assert_eq!(
        format_region_flags(VFIO_REGION_INFO_FLAG_READ | VFIO_REGION_INFO_FLAG_CAPS),
        "r,caps"
    );
}

#[test]
fn vfio_opcodes_match_uapi_numbers() {
    assert_eq!(VFIO_GET_API_VERSION, 0x3b64);
    assert_eq!(VFIO_CHECK_EXTENSION, 0x3b65);
    assert_eq!(VFIO_SET_IOMMU, 0x3b66);
    assert_eq!(VFIO_GROUP_GET_STATUS, 0x3b67);
    assert_eq!(VFIO_GROUP_SET_CONTAINER, 0x3b68);
    assert_eq!(VFIO_GROUP_GET_DEVICE_FD, 0x3b6a);
    assert_eq!(VFIO_DEVICE_GET_REGION_INFO, 0x3b6c);
}

/// The Arc A380 at 0000:03:00.0, read from sysfs `resource` on 2026-10-18:
/// BAR0 16 MiB registers (non-prefetchable), BAR2 8 GiB VRAM (prefetchable).
const ARC_RESOURCE: &str = "\
0x00000000d9000000 0x00000000d9ffffff 0x0000000000140204
0x0000000000000000 0x0000000000000000 0x0000000000000000
0x000000fa00000000 0x000000fbffffffff 0x000000000014220c
0x0000000000000000 0x0000000000000000 0x0000000000000000
0x0000000000000000 0x0000000000000000 0x0000000000000000
0x0000000000000000 0x0000000000000000 0x0000000000000000
0x00000000da000000 0x00000000da1fffff 0x0000000000046200
";

fn mmap_region(index: u32, size: u64) -> VfioRegionInfo {
    VfioRegionInfo {
        argsz: 32,
        flags: VFIO_REGION_INFO_FLAG_READ
            | VFIO_REGION_INFO_FLAG_WRITE
            | VFIO_REGION_INFO_FLAG_MMAP,
        index,
        cap_offset: 0,
        size,
        offset: u64::from(index) << 40,
    }
}

#[test]
fn positive_arc_resource_marks_only_bar2_prefetchable() {
    let bars = parse_prefetchable_bars(ARC_RESOURCE);
    assert_eq!(bars, [false, false, true, false, false, false]);
}

#[test]
fn positive_arc_layout_maps_the_vram_bar_not_registers() {
    let mut regions: Vec<VfioRegionInfo> = (0..6).map(|i| mmap_region(i, 0)).collect();
    if let Some(bar0) = regions.get_mut(0) {
        bar0.size = 16 << 20;
    }
    if let Some(bar2) = regions.get_mut(2) {
        bar2.size = 8 << 30;
    }
    let prefetchable = parse_prefetchable_bars(ARC_RESOURCE);
    assert_eq!(choose_bar_to_map(&regions, &prefetchable), Some(2));
}

#[test]
fn negative_only_register_bar_refuses_to_map() {
    let regions = vec![mmap_region(0, 16 << 20)];
    let prefetchable = parse_prefetchable_bars(ARC_RESOURCE);
    assert_eq!(choose_bar_to_map(&regions, &prefetchable), None);
}

#[test]
fn boundary_malformed_or_short_resource_is_not_prefetchable() {
    assert_eq!(parse_prefetchable_bars(""), [false; 6]);
    let text = "garbage\n0x0 0x0\n0x0 0x0 0xzz\n0x0 0x1fff 0x2000\n";
    assert_eq!(
        parse_prefetchable_bars(text),
        [false, false, false, true, false, false]
    );
}
