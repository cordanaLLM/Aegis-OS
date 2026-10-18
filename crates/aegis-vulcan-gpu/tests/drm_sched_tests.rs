// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Tests for DRM scheduler holder and provider detection.

mod common;

use aegis_vulcan_gpu::{inspect_holders, is_provider, run_record};
use common::{Fallible, TempSysfs};

#[test]
fn test_amdgpu_holder_provider_vs_i915_not() -> Fallible {
    let fix = TempSysfs::new()?;
    fix.set_holders(&["amdgpu", "xe"])?;
    fix.add_render_node("renderD129", "0000:f3:00.0", "i915", None)?;
    fix.add_render_node("renderD130", "0000:fd:00.0", "amdgpu", Some("-1"))?;

    let holders = inspect_holders(&fix.sysfs)?;
    assert!(holders.contains(&"amdgpu".to_string()));
    assert!(!holders.contains(&"i915".to_string()));

    assert!(is_provider("amdgpu", &holders));
    assert!(!is_provider("i915", &holders));
    Ok(())
}

#[test]
fn test_xe_bound_intel_is_provider() -> Fallible {
    let fix = TempSysfs::new()?;
    fix.set_holders(&["amdgpu", "xe"])?;
    fix.add_gpu(
        "0000:f3:00.0",
        "0x8086",
        "0x56a5",
        "xe",
        "0",
        "917",
        &["0000:f3:00.0"],
    )?;
    fix.add_render_node("renderD130", "0000:f3:00.0", "xe", None)?;

    let run = run_record(&fix.sysfs, &fix.dev)?;
    assert!(run.is_pass());
    assert_eq!(run.document.drm_sched.arc_driver, Some("xe".to_string()));
    assert!(
        run.lines.iter().any(
            |l| l.contains("xe (renderD130, the Intel card) is a drm_sched provider: a holder")
        )
    );
    Ok(())
}

#[test]
fn test_unknown_driver_is_not_provider() -> Fallible {
    let fix = TempSysfs::new()?;
    fix.set_holders(&["amdgpu"])?;
    let holders = inspect_holders(&fix.sysfs)?;
    assert!(!is_provider("nvidia", &holders));
    Ok(())
}
