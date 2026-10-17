// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! VFIO candidate selection and isolation checks.
//!
//! Pure functions evaluating whether a GPU may be bound to VFIO for guest passthrough.
//! Never opens or binds any device.

use std::collections::BTreeMap;

/// Per-GPU facts required to evaluate VFIO candidate eligibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VfioCandidateInput {
    /// PCI address of the candidate device (e.g. "0000:7d:00.0").
    pub address: String,
    /// Currently bound kernel driver (e.g. "amdgpu").
    pub driver: String,
    /// IOMMU group identifier (e.g. "31").
    pub iommu_group: String,
    /// All PCI function addresses present in the device's IOMMU group.
    pub group_members: Vec<String>,
    /// Connector statuses associated with this device (name -> "connected"/"disconnected").
    pub connectors: BTreeMap<String, String>,
    /// Whether this device was selected as the boot VGA device by firmware/kernel.
    pub boot_vga: bool,
}

/// Outcome of evaluating a GPU for VFIO passthrough.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VfioDecision {
    /// Candidate is sole occupant, headless, and non-boot VGA.
    Admitted,
    /// Candidate is disqualified with every applicable refusal reason.
    Refused(Vec<String>),
}

impl VfioDecision {
    /// Formats the single-line summary matching the gate log shape.
    #[must_use]
    pub fn format_line(&self, address: &str, driver: &str) -> String {
        match self {
            Self::Admitted => format!("     {address} ({driver}): admitted"),
            Self::Refused(reasons) => {
                let joined = reasons.join("; ");
                format!("     {address} ({driver}): refused: {joined}")
            }
        }
    }
}

/// Whether `connector` (a sysfs name such as `card0-Writeback-1`) is a DRM
/// writeback connector. The kernel names that type `Writeback`; it captures
/// composed frames into memory, drives no display and always reports status
/// `unknown`, so it is no reason to keep a device from VFIO.
fn is_writeback(connector: &str) -> bool {
    connector.split('-').nth(1) == Some("Writeback")
}

/// Evaluates a candidate device for VFIO passthrough admission.
///
/// A device is refused if:
/// 1. It has no IOMMU group (IOMMU off or not isolated);
/// 2. It shares its IOMMU group with any other PCI function;
/// 3. It has any display connector not in the "disconnected" state;
/// 4. It is the primary boot VGA device.
#[must_use]
pub fn evaluate_candidate(candidate: &VfioCandidateInput) -> VfioDecision {
    let mut reasons = Vec::new();

    // 1. Check IOMMU group isolation
    if candidate.iommu_group.trim().is_empty() {
        reasons.push(format!(
            "{} has no IOMMU group: the IOMMU is off or the device is not isolated",
            candidate.address
        ));
    } else {
        let mut others: Vec<&str> = candidate
            .group_members
            .iter()
            .filter(|&m| m != &candidate.address)
            .map(String::as_str)
            .collect();
        others.sort_unstable();

        if !others.is_empty() {
            let others_str = others.join(", ");
            reasons.push(format!(
                "{} shares IOMMU group {} with {others_str}",
                candidate.address, candidate.iommu_group
            ));
        }
    }

    // 2. Check for connectors that are not disconnected
    let mut not_disconnected: Vec<String> = candidate
        .connectors
        .iter()
        .filter(|(name, status)| !is_writeback(name) && *status != "disconnected")
        .map(|(name, status)| format!("{name} ({status})"))
        .collect();
    not_disconnected.sort_unstable();

    if !not_disconnected.is_empty() {
        let conns_str = not_disconnected.join(", ");
        reasons.push(format!(
            "{} has display connector not disconnected: {conns_str}",
            candidate.address
        ));
    }

    // 3. Check boot VGA status
    if candidate.boot_vga {
        reasons.push(format!("{} is the boot VGA device", candidate.address));
    }

    if reasons.is_empty() {
        VfioDecision::Admitted
    } else {
        VfioDecision::Refused(reasons)
    }
}
