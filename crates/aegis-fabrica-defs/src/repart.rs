// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `repart.d(5)` partition drop-ins.
//!
//! The known-key list is the set `repart.d(5)` documents on the admitted
//! systemd floor, read from that manual page rather than from memory; a key
//! outside it is refused here because systemd only warns and carries on.

use crate::error::DefinitionError;
use crate::finding::{Finding, MAX_FINDINGS};
use crate::size::{ESP_MAX_BYTES, ESP_MIN_BYTES, parse_size};
use crate::unit::{MAX_ENTRIES, UnitFile};

/// The only section a repart drop-in may declare.
pub const PARTITION_SECTION: &str = "Partition";

/// The `Type=` value of an EFI system partition.
pub const ESP_TYPE: &str = "esp";

/// The `Type=` value of a root partition.
pub const ROOT_TYPE: &str = "root";

/// The `Type=` value of a stateful `/var` partition.
pub const VAR_TYPE: &str = "var";

/// Scalar bound on the drop-ins one definition set may hold.
pub const MAX_DEFINITIONS: usize = 32;

/// Every key `repart.d(5)` defines on the admitted systemd floor.
pub const KNOWN_KEYS: [&str; 47] = [
    "AddValidateFS",
    "BlockDeviceReplace",
    "Compression",
    "CompressionLevel",
    "CopyBlocks",
    "CopyFiles",
    "DefaultSubvolume",
    "Discard",
    "Encrypt",
    "EncryptKDF",
    "EncryptedVolume",
    "ExcludeFiles",
    "ExcludeFilesTarget",
    "FactoryReset",
    "FileSystemSectorSize",
    "Flags",
    "Format",
    "GrowFileSystem",
    "Integrity",
    "IntegrityAlgorithm",
    "KeyFile",
    "Label",
    "MakeDirectories",
    "MakeSymlinks",
    "Minimize",
    "MountPoint",
    "NoAuto",
    "PaddingMaxBytes",
    "PaddingMinBytes",
    "PaddingWeight",
    "Priority",
    "ReadOnly",
    "SizeMaxBytes",
    "SizeMinBytes",
    "SplitName",
    "Subvolumes",
    "SupplementFor",
    "TPM2PCRs",
    "Type",
    "UUID",
    "Verity",
    "VerityDataBlockSizeBytes",
    "VerityHashBlockSizeBytes",
    "VerityMatchKey",
    "VolumeLabel",
    "VolumeName",
    "Weight",
];

/// One reviewed partition drop-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepartDefinition {
    unit: UnitFile,
    partition_type: String,
    size_min: Option<u64>,
    size_max: Option<u64>,
}

/// Reads an optional size value, refusing one that is not a systemd size.
fn optional_size(unit: &UnitFile, key: &str) -> Result<Option<u64>, DefinitionError> {
    let Some(raw) = unit.value(PARTITION_SECTION, key) else {
        return Ok(None);
    };
    let Some(bytes) = parse_size(raw) else {
        return Err(DefinitionError::MalformedValue {
            name: unit.name().to_owned(),
            line: unit.line_of(PARTITION_SECTION, key).unwrap_or(0),
            key: key.to_owned(),
            value: raw.to_owned(),
            expected: "size in bytes with an optional K, M, G, T, P or E suffix",
        });
    };
    Ok(Some(bytes))
}

/// Refuses any section or key the drop-in syntax does not define.
fn check_vocabulary(unit: &UnitFile) -> Result<(), DefinitionError> {
    for entry in unit.entries().iter().take(MAX_ENTRIES) {
        if entry.section != PARTITION_SECTION {
            return Err(DefinitionError::UnknownSection {
                name: unit.name().to_owned(),
                line: entry.line,
                section: entry.section.clone(),
            });
        }
        if !KNOWN_KEYS.contains(&entry.key.as_str()) {
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

impl RepartDefinition {
    /// Parses the drop-in `name` from `text`.
    ///
    /// # Errors
    ///
    /// Returns [`DefinitionError`] for anything systemd refuses -- a missing
    /// `Type=`, an unreadable size, `SizeMinBytes=` above `SizeMaxBytes=` --
    /// and for the unknown keys and repeats systemd only warns about.
    pub fn parse(name: &str, text: &str) -> Result<Self, DefinitionError> {
        let unit = UnitFile::parse(name, text)?;
        check_vocabulary(&unit)?;
        let Some(partition_type) = unit.value(PARTITION_SECTION, "Type") else {
            return Err(DefinitionError::MissingKey {
                name: name.to_owned(),
                section: PARTITION_SECTION.to_owned(),
                key: "Type",
            });
        };
        let partition_type = partition_type.to_owned();
        let size_min = optional_size(&unit, "SizeMinBytes")?;
        let size_max = optional_size(&unit, "SizeMaxBytes")?;
        if let (Some(minimum), Some(maximum)) = (size_min, size_max)
            && minimum > maximum
        {
            return Err(DefinitionError::InvertedSize {
                name: name.to_owned(),
                minimum,
                maximum,
            });
        }
        Ok(Self {
            unit,
            partition_type,
            size_min,
            size_max,
        })
    }

    /// The parsed drop-in, for callers that need the raw assignments.
    #[must_use]
    pub fn unit(&self) -> &UnitFile {
        &self.unit
    }

    /// The `Type=` value, which systemd requires and this crate refuses to default.
    #[must_use]
    pub fn partition_type(&self) -> &str {
        &self.partition_type
    }

    /// The `Label=` value, if the drop-in sets one.
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.unit.value(PARTITION_SECTION, "Label")
    }

    /// `SizeMinBytes=` in bytes, if the drop-in sets one.
    #[must_use]
    pub fn size_min(&self) -> Option<u64> {
        self.size_min
    }

    /// `SizeMaxBytes=` in bytes, if the drop-in sets one.
    #[must_use]
    pub fn size_max(&self) -> Option<u64> {
        self.size_max
    }

    /// The `Verity=` role, if the drop-in declares one.
    #[must_use]
    pub fn verity(&self) -> Option<&str> {
        self.unit.value(PARTITION_SECTION, "Verity")
    }

    /// The `VerityMatchKey=` that pairs a data partition with its hash partition.
    #[must_use]
    pub fn verity_match_key(&self) -> Option<&str> {
        self.unit.value(PARTITION_SECTION, "VerityMatchKey")
    }

    /// `true` when the drop-in seals the partition with the TPM2.
    #[must_use]
    pub fn is_tpm2_encrypted(&self) -> bool {
        self.unit
            .value(PARTITION_SECTION, "Encrypt")
            .is_some_and(|value| value == "tpm2" || value == "key-file+tpm2")
    }

    /// Renders the drop-in back to its canonical form.
    #[must_use]
    pub fn render(&self) -> String {
        self.unit.render()
    }

    /// Requirements this one drop-in does not meet.
    #[must_use]
    pub fn findings(&self) -> Vec<Finding> {
        let mut out = Vec::new();
        if self.partition_type == ESP_TYPE {
            out.extend(self.esp_findings());
        }
        if self.partition_type == VAR_TYPE && !self.is_tpm2_encrypted() {
            out.push(Finding::VarNotEncrypted);
        }
        out.truncate(MAX_FINDINGS);
        out
    }

    /// The ESP size-bound findings (REQ-P01-10).
    fn esp_findings(&self) -> Vec<Finding> {
        match (self.size_min, self.size_max) {
            (None, None) => vec![Finding::EspUnbounded],
            (minimum, maximum) => {
                let mut out = Vec::new();
                if let Some(bytes) = minimum
                    && bytes < ESP_MIN_BYTES
                {
                    out.push(Finding::EspBelowMinimum { bytes });
                }
                if let Some(bytes) = maximum
                    && bytes > ESP_MAX_BYTES
                {
                    out.push(Finding::EspAboveMaximum { bytes });
                }
                out
            }
        }
    }
}

/// Returns the verity pairing findings for a whole definition set.
fn verity_findings(definitions: &[RepartDefinition]) -> Vec<Finding> {
    let mut out = Vec::new();
    for definition in definitions.iter().take(MAX_DEFINITIONS) {
        let (Some(role), Some(key)) = (definition.verity(), definition.verity_match_key()) else {
            continue;
        };
        let partner = if role == "data" { "hash" } else { "data" };
        let paired = definitions
            .iter()
            .take(MAX_DEFINITIONS)
            .any(|other| other.verity() == Some(partner) && other.verity_match_key() == Some(key));
        if paired {
            continue;
        }
        let match_key = key.to_owned();
        out.push(if role == "data" {
            Finding::MissingVerityHash { match_key }
        } else {
            Finding::MissingVerityData { match_key }
        });
    }
    out
}

/// Checks a whole definition set against the A/B and verity requirements.
///
/// Per-drop-in findings come first, in set order, then the findings that only
/// exist across files: the verity pairing and the two root slots.
#[must_use]
pub fn check_repart_set(definitions: &[RepartDefinition]) -> Vec<Finding> {
    let mut out = Vec::new();
    for definition in definitions.iter().take(MAX_DEFINITIONS) {
        out.extend(definition.findings());
    }
    out.extend(verity_findings(definitions));
    let slots = definitions
        .iter()
        .take(MAX_DEFINITIONS)
        .filter(|definition| definition.partition_type() == ROOT_TYPE)
        .count();
    if slots != 2 {
        out.push(Finding::MissingAlternateRootSlot { slots });
    }
    out.truncate(MAX_FINDINGS);
    out
}
