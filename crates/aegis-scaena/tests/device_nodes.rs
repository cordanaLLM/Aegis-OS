// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! M27 criteria 5 and 6 (b): the decode node is the render node whose device
//! number is the compositor's main device, resolved through a
//! `/sys/class/drm` tree and never from a hard-coded `renderD` number.
//!
//! The tree here is built in a temporary directory with the reference
//! profile's shape: renderD128 on the NVIDIA driver, renderD129 on i915,
//! renderD130 on amdgpu, and a card node. Positive: the main device 226:129
//! resolves to renderD129 and its driver; the wire form of `dev_t` reads
//! back. Negative: every other node is refused naming both numbers; an
//! unreadable `dev` attribute and a missing tree are refused. Boundary: a
//! main device that is a card node matches no render node; a `dev_t` of
//! seven or nine bytes and a malformed attribute are refused; a device file
//! whose `st_rdev` differs from sysfs is refused.

mod common;

use std::path::{Path, PathBuf};

use aegis_scaena::device::{
    DeviceError, dev_from_wire, main_device_node, parse_dev_attribute, render_nodes,
    require_main_device, verify_device_file,
};
use rustix::fs::makedev;

use common::Fallible;

/// A throwaway `/sys/class/drm` tree, removed when dropped.
struct Tree {
    root: PathBuf,
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Builds the reference profile's shape under a fresh temporary directory.
fn tree(name: &str) -> Result<Tree, Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("aegis-scaena-drm-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let drivers = root.join("drivers");
    for (entry, dev, driver) in [
        ("renderD128", "226:128\n", "nvidia"),
        ("renderD129", "226:129\n", "i915"),
        ("renderD130", "226:130\n", "amdgpu"),
        ("card2", "226:2\n", "i915"),
    ] {
        let device = root.join("devices").join(entry);
        std::fs::create_dir_all(&device)?;
        std::fs::create_dir_all(drivers.join(driver))?;
        std::os::unix::fs::symlink(drivers.join(driver), device.join("driver"))?;
        let node = root.join("class").join(entry);
        std::fs::create_dir_all(&node)?;
        std::fs::write(node.join("dev"), dev)?;
        std::os::unix::fs::symlink(&device, node.join("device"))?;
    }
    Ok(Tree { root })
}

impl Tree {
    fn class(&self) -> PathBuf {
        self.root.join("class")
    }
}

// --- Positive -------------------------------------------------------------

/// Positive: the main device resolves to its render node and driver.
#[test]
fn the_main_device_resolves_to_its_render_node() -> Fallible {
    let drm = tree("positive")?;
    let node = main_device_node(&drm.class(), makedev(226, 129))?;
    assert_eq!(node.name, "renderD129");
    assert_eq!(node.driver, "i915");
    assert_eq!(node.number(), (226, 129));
    let names: Vec<String> = render_nodes(&drm.class())?
        .into_iter()
        .map(|n| n.name)
        .collect();
    assert_eq!(names, ["renderD128", "renderD129", "renderD130"]);
    Ok(())
}

/// Positive: `main_device` arrives as the host's `dev_t` bytes.
#[test]
fn the_wire_dev_t_reads_back() {
    let dev = makedev(226, 129);
    assert_eq!(dev_from_wire(&dev.to_ne_bytes()), Some(dev));
    assert_eq!(dev, 0xe281);
}

// --- Negative -------------------------------------------------------------

/// Negative (b): every node that is not the main device is refused, naming
/// both device numbers.
#[test]
fn every_other_render_node_is_refused() -> Fallible {
    let drm = tree("negative")?;
    let main = makedev(226, 129);
    let mut refused = 0;
    for node in render_nodes(&drm.class())? {
        match require_main_device(&node, main) {
            Ok(()) => assert_eq!(node.name, "renderD129"),
            Err(DeviceError::NotMainDevice {
                name,
                node_minor,
                main_minor,
                ..
            }) => {
                assert_eq!((name, main_minor), (node.name.clone(), 129));
                assert_ne!(node_minor, 129);
                refused += 1;
            }
            Err(other) => return Err(other.into()),
        }
    }
    assert_eq!(refused, 2, "the NVIDIA and the amdgpu node");
    Ok(())
}

/// Negative: a node whose `dev` attribute is unreadable stops the
/// enumeration with a typed error, and a missing tree is refused.
#[test]
fn an_unreadable_tree_is_refused() -> Fallible {
    let drm = tree("unreadable")?;
    std::fs::write(drm.class().join("renderD130").join("dev"), "not a number\n")?;
    assert!(matches!(
        render_nodes(&drm.class()),
        Err(DeviceError::DevAttribute { .. })
    ));
    assert!(matches!(
        render_nodes(Path::new("/nonexistent/aegis/drm")),
        Err(DeviceError::Enumerate { .. })
    ));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a main device that is a card node matches no render node; the
/// rule is equality of device numbers, nothing looser.
#[test]
fn a_card_node_main_device_matches_no_render_node() -> Fallible {
    let drm = tree("card")?;
    assert!(matches!(
        main_device_node(&drm.class(), makedev(226, 2)),
        Err(DeviceError::NoNodeForMainDevice {
            major: 226,
            minor: 2,
            ..
        })
    ));
    Ok(())
}

/// Boundary: a `dev_t` of seven or nine bytes and a malformed attribute are
/// refused; a well-formed one with or without its newline is read.
#[test]
fn malformed_device_numbers_are_refused() {
    assert_eq!(dev_from_wire(&[0; 7]), None);
    assert_eq!(dev_from_wire(&[0; 9]), None);
    assert_eq!(parse_dev_attribute("226:129\n"), Some(makedev(226, 129)));
    assert_eq!(parse_dev_attribute("226:129"), Some(makedev(226, 129)));
    for malformed in ["226:", ":129", "226:129:1", "226-129", " 226:129", ""] {
        assert_eq!(parse_dev_attribute(malformed), None, "{malformed:?}");
    }
}

/// Boundary: the device file must carry the number sysfs lists, read with
/// `stat`: `/dev/null` is 1:3 and nothing else.
#[test]
fn the_device_file_must_carry_the_listed_number() -> Fallible {
    let mut node = aegis_scaena::device::RenderNode {
        name: "null".to_owned(),
        dev: makedev(1, 3),
        driver: "mem".to_owned(),
    };
    assert_eq!(
        verify_device_file(&node, Path::new("/dev"))?,
        Path::new("/dev/null")
    );
    node.dev = makedev(226, 129);
    assert!(matches!(
        verify_device_file(&node, Path::new("/dev")),
        Err(DeviceError::NodeMismatch {
            found_major: 1,
            found_minor: 3,
            ..
        })
    ));
    Ok(())
}
