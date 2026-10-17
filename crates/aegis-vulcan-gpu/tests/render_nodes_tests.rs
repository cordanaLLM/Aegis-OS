// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Tests for DRM render node discovery and iteration bounds under `/sys/class/drm`.

mod common;

use aegis_vulcan_gpu::error::GpuError;
use aegis_vulcan_gpu::render_nodes::{MAX_RENDER_NODES, inspect_render_nodes};
use common::{Fallible, TempSysfs};

#[test]
fn test_render_nodes_over_bound_returns_bound_error() -> Fallible {
    let fix = TempSysfs::new()?;
    let drm_dir = fix.sysfs.join("class").join("drm");

    // Plant MAX_RENDER_NODES + 1 (257) entries in /sys/class/drm
    for i in 0..=MAX_RENDER_NODES {
        let entry = drm_dir.join(format!("dummy-node-{i}"));
        std::fs::create_dir_all(&entry)?;
    }

    let res = inspect_render_nodes(&fix.sysfs);
    match res {
        Err(GpuError::Bound { path, bound }) => {
            assert_eq!(path, drm_dir);
            assert_eq!(bound, MAX_RENDER_NODES);
            Ok(())
        }
        other => Err(format!(
            "expected GpuError::Bound on {MAX_RENDER_NODES} + 1 drm entries, got {other:?}"
        )
        .into()),
    }
}

#[test]
fn test_render_nodes_exactly_at_bound_ok() -> Fallible {
    let fix = TempSysfs::new()?;
    let drm_dir = fix.sysfs.join("class").join("drm");

    // Plant exactly MAX_RENDER_NODES (256) non-renderD entries
    for i in 0..MAX_RENDER_NODES {
        let entry = drm_dir.join(format!("dummy-card-{i}"));
        std::fs::create_dir_all(&entry)?;
    }

    let nodes = inspect_render_nodes(&fix.sysfs)?;
    assert_eq!(nodes.len(), 0);
    Ok(())
}

#[test]
fn test_render_nodes_discovered_positive() -> Fallible {
    let fix = TempSysfs::new()?;
    fix.add_render_node("renderD128", "0000:06:00.0", "nvidia", Some("1"))?;

    let nodes = inspect_render_nodes(&fix.sysfs)?;
    assert_eq!(nodes.len(), 1);
    let first = nodes.first().ok_or("missing node")?;
    assert_eq!(first.node, "renderD128");
    assert_eq!(first.address, "0000:06:00.0");
    assert_eq!(first.driver, "nvidia");
    assert!(first.modeset_status.contains("nvidia_drm modeset '1'"));
    Ok(())
}
