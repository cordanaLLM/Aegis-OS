// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Exit criterion 5: the reference profile's export paths bind without
//! reshaping either descriptor.
//!
//! The three render nodes were read on the reference profile on 2026-09-13 by
//! resolving `/sys/class/drm/renderD*/device/driver` and reading
//! `/sys/module/<driver>/parameters/modeset`. This file binds each of them
//! into both descriptors and requires them to round-trip, which is what
//! "without reshaping the descriptor" means: the field is already there, and
//! M25 fills it in.
//!
//! # Scope
//!
//! Nothing here opens a render node or exports a buffer. A round-trip through
//! these three values is evidence about the descriptor's shape, not evidence
//! that the reference profile can share a buffer between those three devices.
//! That is M25 work over the real devices, and the profile still records P03's
//! peer-to-peer requirement as missing.

mod common;

use aegis_vulcan::{
    DmaBufError, DmaBufExport, DrmDriver, MediaIngestDescriptor, ModesetState, PayloadBuffer,
    REFERENCE_PROFILE_EXPORTS, RENDER_NODE_MINOR_BASE, RENDER_NODE_MINOR_MAX, RenderNode,
    WeightStreamDescriptor,
};

use common::{Fallible, media_ingest, weight_stream};

/// The three render nodes the reference profile reports, as observed.
const OBSERVED: [(u32, &str, ModesetState); 3] = [
    (128, "renderD128", ModesetState::Enabled),
    (129, "renderD129", ModesetState::DriverDefault),
    (130, "renderD130", ModesetState::DriverDefault),
];

// --- Positive -------------------------------------------------------------

/// Positive: every recorded export path is valid and renders its device name.
#[test]
fn every_reference_export_path_is_valid() -> Fallible {
    assert_eq!(REFERENCE_PROFILE_EXPORTS.len(), OBSERVED.len());
    for (export, (minor, name, modeset)) in REFERENCE_PROFILE_EXPORTS.iter().zip(OBSERVED) {
        assert_eq!(RenderNode::new(minor)?, export.node);
        assert_eq!(export.node.minor(), minor);
        assert_eq!(export.node.to_string(), name);
        assert_eq!(export.modeset, modeset);
        assert_eq!(export.validate(), Ok(()));
    }
    Ok(())
}

/// Positive: the drivers are the ones the probe reported, per node.
#[test]
fn each_reference_node_carries_the_driver_that_was_observed() {
    let drivers: Vec<(u32, &'static str)> = REFERENCE_PROFILE_EXPORTS
        .iter()
        .map(|export| (export.node.minor(), export.driver.module()))
        .collect();
    assert_eq!(
        drivers,
        vec![(128, "nvidia_drm"), (129, "i915"), (130, "amdgpu")]
    );
    assert!(DrmDriver::I915.in_tree());
    assert!(DrmDriver::Amdgpu.in_tree());
    assert!(!DrmDriver::NvidiaDrm.in_tree());
    assert_eq!(DrmDriver::ALL.len(), 3);
}

/// Positive: each export path binds into both descriptors and round-trips.
///
/// This is the criterion itself: the descriptor is not reshaped to hold any of
/// the three, and each of the six payloads decodes back to what was encoded.
#[test]
fn every_reference_export_path_binds_into_both_descriptors() -> Fallible {
    for export in REFERENCE_PROFILE_EXPORTS {
        let mut buffer = PayloadBuffer::new();

        let mut stream = weight_stream(64)?;
        stream.export = export;
        let text = stream.encode_into(&mut buffer)?;
        assert_eq!(WeightStreamDescriptor::decode(text)?, stream);

        let mut ingest = media_ingest(64)?;
        ingest.export = export;
        let mut second = PayloadBuffer::new();
        let text = ingest.encode_into(&mut second)?;
        assert_eq!(MediaIngestDescriptor::decode(text)?, ingest);
    }
    Ok(())
}

/// Positive: a render node parses back from the device name it renders.
#[test]
fn a_render_node_parses_from_its_device_name() -> Fallible {
    for (minor, name, _) in OBSERVED {
        assert_eq!(RenderNode::parse(name)?, RenderNode::new(minor)?);
    }
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: modesetting off publishes no render node, for any driver.
#[test]
fn a_disabled_modeset_publishes_no_render_node() -> Fallible {
    for driver in DrmDriver::ALL {
        let export = DmaBufExport::new(RenderNode::new(129)?, driver, ModesetState::Disabled);
        assert_eq!(export.validate(), Err(DmaBufError::ModesetDisabled));
    }
    Ok(())
}

/// Negative: the out-of-tree driver needs modesetting explicitly enabled.
///
/// The reference profile reads `Y` for it, which is exactly the value this
/// requires; the in-tree drivers are admitted on their own default.
#[test]
fn the_out_of_tree_driver_needs_an_explicit_modeset() -> Fallible {
    let node = RenderNode::new(128)?;
    let implicit = DmaBufExport::new(node, DrmDriver::NvidiaDrm, ModesetState::DriverDefault);
    assert_eq!(implicit.validate(), Err(DmaBufError::ModesetNotExplicit));

    let explicit = DmaBufExport::new(node, DrmDriver::NvidiaDrm, ModesetState::Enabled);
    assert_eq!(explicit.validate(), Ok(()));

    let in_tree = DmaBufExport::new(node, DrmDriver::I915, ModesetState::DriverDefault);
    assert_eq!(in_tree.validate(), Ok(()));
    Ok(())
}

/// Negative: a name that is not a render-node name is refused.
///
/// `renderD+128` and `renderD0128` are on the list because `u32::from_str`
/// accepts both and neither renders back through `Display`: before the
/// canonical-spelling check they parsed as `renderD128`, which is a name no
/// Linux system publishes.
#[test]
fn a_name_that_is_not_a_render_node_is_refused() {
    for name in [
        "card0",
        "renderD",
        "renderDxyz",
        "",
        "render129",
        "renderD 129",
        "renderD+128",
        "renderD-128",
        "renderD0128",
    ] {
        assert_eq!(RenderNode::parse(name), Err(DmaBufError::NotARenderNode));
    }
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the reserved minor range is exact on both sides.
#[test]
fn the_render_node_minor_range_is_exact() {
    assert_eq!(RENDER_NODE_MINOR_BASE, 128);
    assert_eq!(RENDER_NODE_MINOR_MAX, 191);
    assert!(RenderNode::new(RENDER_NODE_MINOR_BASE).is_ok());
    assert!(RenderNode::new(RENDER_NODE_MINOR_MAX).is_ok());
    assert_eq!(
        RenderNode::new(RENDER_NODE_MINOR_BASE.saturating_sub(1)),
        Err(DmaBufError::MinorOutOfRange { minor: 127 })
    );
    assert_eq!(
        RenderNode::new(RENDER_NODE_MINOR_MAX.saturating_add(1)),
        Err(DmaBufError::MinorOutOfRange { minor: 192 })
    );
    assert_eq!(
        RenderNode::parse("renderD192"),
        Err(DmaBufError::MinorOutOfRange { minor: 192 })
    );
}

/// Boundary: every refusal carries a stable one-line reason for a contract.
#[test]
fn every_export_refusal_has_a_stable_reason() {
    let reasons = [
        DmaBufError::MinorOutOfRange { minor: 0 }.reason(),
        DmaBufError::NotARenderNode.reason(),
        DmaBufError::ModesetDisabled.reason(),
        DmaBufError::ModesetNotExplicit.reason(),
    ];
    assert_eq!(reasons.len(), 4);
    for reason in reasons {
        assert!(!reason.is_empty());
        assert!(!reason.contains('\n'));
    }
    assert_eq!(ModesetState::Enabled.name(), "enabled");
    assert_eq!(ModesetState::Disabled.name(), "disabled");
    assert_eq!(ModesetState::DriverDefault.name(), "driver-default");
}

/// Boundary: the canonical-spelling rule is exact at its own edges.
///
/// A single `0` is the one decimal spelling of zero, so it reaches the range
/// check and is refused as a minor rather than as a name. `00` is one
/// character longer and is refused as a name, which is the `len() > 1` edge.
/// Every name a node renders parses back to that node, which is the property
/// the rule exists to keep.
#[test]
fn the_canonical_spelling_rule_is_exact() -> Fallible {
    assert_eq!(
        RenderNode::parse("renderD0"),
        Err(DmaBufError::MinorOutOfRange { minor: 0 })
    );
    assert_eq!(
        RenderNode::parse("renderD00"),
        Err(DmaBufError::NotARenderNode)
    );
    for minor in [RENDER_NODE_MINOR_BASE, RENDER_NODE_MINOR_MAX] {
        let node = RenderNode::new(minor)?;
        assert_eq!(RenderNode::parse(&node.to_string())?, node);
    }
    Ok(())
}
