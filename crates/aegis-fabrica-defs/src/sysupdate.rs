// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `sysupdate.d(5)` transfer definitions.
//!
//! The per-section key lists are what `sysupdate.d(5)` documents on the
//! admitted systemd floor. They are what rejected the imported transfer: none
//! of `File=`, `Partitions=`, `Writable=`, `Format=`, `MatchKey=` or
//! `VerityMatchKey=` appears in any of them.

use crate::error::DefinitionError;
use crate::finding::{Finding, MAX_FINDINGS};
use crate::unit::{MAX_ENTRIES, UnitFile};

/// The `[Transfer]` section name.
pub const TRANSFER_SECTION: &str = "Transfer";

/// The `[Source]` section name.
pub const SOURCE_SECTION: &str = "Source";

/// The `[Target]` section name.
pub const TARGET_SECTION: &str = "Target";

/// The instance count an A/B transfer needs (REQ-P02-04).
pub const AB_INSTANCES: u32 = 2;

/// Every key `[Transfer]` defines on the admitted systemd floor.
pub const TRANSFER_KEYS: [&str; 7] = [
    "AppStream",
    "ChangeLog",
    "Features",
    "MinVersion",
    "ProtectVersion",
    "RequisiteFeatures",
    "Verify",
];

/// Every key `[Source]` defines on the admitted systemd floor.
pub const SOURCE_KEYS: [&str; 3] = ["MatchPattern", "Path", "Type"];

/// Every key `[Target]` defines on the admitted systemd floor.
pub const TARGET_KEYS: [&str; 16] = [
    "CurrentSymlink",
    "InstancesMax",
    "MatchPartitionType",
    "MatchPattern",
    "Mode",
    "PartitionFlags",
    "PartitionGrowFileSystem",
    "PartitionNoAuto",
    "PartitionUUID",
    "Path",
    "PathRelativeTo",
    "ReadOnly",
    "RemoveTemporary",
    "TriesDone",
    "TriesLeft",
    "Type",
];

/// One reviewed transfer definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferDefinition {
    unit: UnitFile,
    instances_max: Option<u32>,
}

/// Returns the key list for `section`, or `None` when the section is not defined.
fn keys_of(section: &str) -> Option<&'static [&'static str]> {
    match section {
        TRANSFER_SECTION => Some(&TRANSFER_KEYS),
        SOURCE_SECTION => Some(&SOURCE_KEYS),
        TARGET_SECTION => Some(&TARGET_KEYS),
        _ => None,
    }
}

/// Refuses any section or key `sysupdate.d(5)` does not define.
fn check_vocabulary(unit: &UnitFile) -> Result<(), DefinitionError> {
    for entry in unit.entries().iter().take(MAX_ENTRIES) {
        let Some(keys) = keys_of(&entry.section) else {
            return Err(DefinitionError::UnknownSection {
                name: unit.name().to_owned(),
                line: entry.line,
                section: entry.section.clone(),
            });
        };
        if !keys.contains(&entry.key.as_str()) {
            return Err(DefinitionError::UnknownKey {
                name: unit.name().to_owned(),
                line: entry.line,
                section: entry.section.clone(),
                key: entry.key.clone(),
            });
        }
    }
    Ok(())
}

/// Refuses a section that is present but missing a key systemd requires.
fn require(unit: &UnitFile, section: &str, key: &'static str) -> Result<(), DefinitionError> {
    if unit.value(section, key).is_some() {
        return Ok(());
    }
    Err(DefinitionError::MissingKey {
        name: unit.name().to_owned(),
        section: section.to_owned(),
        key,
    })
}

/// Reads `InstancesMax=`, refusing a value that is not a count.
fn instances_max(unit: &UnitFile) -> Result<Option<u32>, DefinitionError> {
    let Some(raw) = unit.value(TARGET_SECTION, "InstancesMax") else {
        return Ok(None);
    };
    let Ok(count) = raw.parse::<u32>() else {
        return Err(DefinitionError::MalformedValue {
            name: unit.name().to_owned(),
            line: unit.line_of(TARGET_SECTION, "InstancesMax").unwrap_or(0),
            key: "InstancesMax".to_owned(),
            value: raw.to_owned(),
            expected: "decimal instance count",
        });
    };
    Ok(Some(count))
}

impl TransferDefinition {
    /// Parses the transfer `name` from `text`.
    ///
    /// # Errors
    ///
    /// Returns [`DefinitionError`] for the keys and sections
    /// `sysupdate.d(5)` does not define, for a `[Source]` or `[Target]`
    /// without its mandatory `Type=` and `MatchPattern=`, and for an
    /// unreadable `InstancesMax=`.
    pub fn parse(name: &str, text: &str) -> Result<Self, DefinitionError> {
        let unit = UnitFile::parse(name, text)?;
        check_vocabulary(&unit)?;
        for section in [SOURCE_SECTION, TARGET_SECTION] {
            require(&unit, section, "Type")?;
            require(&unit, section, "MatchPattern")?;
        }
        let instances_max = instances_max(&unit)?;
        Ok(Self {
            unit,
            instances_max,
        })
    }

    /// The parsed drop-in, for callers that need the raw assignments.
    #[must_use]
    pub fn unit(&self) -> &UnitFile {
        &self.unit
    }

    /// The `[Source] Type=` resource kind.
    #[must_use]
    pub fn source_type(&self) -> Option<&str> {
        self.unit.value(SOURCE_SECTION, "Type")
    }

    /// The `[Target] Type=` resource kind.
    #[must_use]
    pub fn target_type(&self) -> Option<&str> {
        self.unit.value(TARGET_SECTION, "Type")
    }

    /// The `[Target] MatchPattern=`, which selects the slots to rotate.
    #[must_use]
    pub fn target_pattern(&self) -> Option<&str> {
        self.unit.value(TARGET_SECTION, "MatchPattern")
    }

    /// `InstancesMax=`, if the transfer sets one.
    #[must_use]
    pub fn instances_max(&self) -> Option<u32> {
        self.instances_max
    }

    /// `true` when the target is declared read-only.
    #[must_use]
    pub fn is_read_only(&self) -> bool {
        self.unit
            .value(TARGET_SECTION, "ReadOnly")
            .is_some_and(|value| matches!(value, "1" | "yes" | "true" | "on"))
    }

    /// Renders the transfer back to its canonical form.
    #[must_use]
    pub fn render(&self) -> String {
        self.unit.render()
    }

    /// Requirements this transfer does not meet.
    ///
    /// An absent `InstancesMax=` is a single-slot finding rather than a pass:
    /// systemd's own default keeps the contract implicit, and REQ-P02-04 asks
    /// for it in writing.
    #[must_use]
    pub fn findings(&self) -> Vec<Finding> {
        let mut out = Vec::new();
        let declared = self.instances_max.unwrap_or(0);
        if declared < AB_INSTANCES {
            out.push(Finding::SingleSlotTransfer {
                instances_max: declared,
            });
        }
        if !self.is_read_only() {
            out.push(Finding::WritableTransferTarget);
        }
        out.truncate(MAX_FINDINGS);
        out
    }
}
