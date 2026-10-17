// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Tests for VFIO candidate selection and isolation rules.

use std::collections::BTreeMap;

use aegis_vulcan_gpu::vfio::{VfioCandidateInput, VfioDecision, evaluate_candidate};

type Fallible = Result<(), Box<dyn std::error::Error>>;

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
