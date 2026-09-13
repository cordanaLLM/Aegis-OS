// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The Aegis product input manifest: what this repository hands to a builder.
//!
//! `docs/integration/stack.md` states the boundary in one sentence: each
//! direction needs a schema, a correlation identifier, an exact revision,
//! bounded retries and a recorded result before it counts as connected. This
//! module is the Aegis-owned half of that sentence, typed.
//!
//! | Field | What it fixes |
//! | :-- | :-- |
//! | `correlation-id` | the identifier threading a request to its result |
//! | `revision` | the exact source revision the inputs were read at |
//! | `distribution` | the dated snapshot every package version comes from (D18) |
//! | `definitions` | the repart, sysupdate, mkosi and kernel-requirement references |
//! | `packages` | the image content set the recorded requirements name |
//! | `kernel` | the boot kernel identity (D07, amended by D70) |
//! | `retries` | the bounded retry budget for one request |
//!
//! # Validation is against the real files
//!
//! [`ProductInputManifest::validate_definitions`] does not check that the
//! referenced paths exist. It takes the text of the reviewed definitions and
//! runs them through the same [`RepartDefinition`] and [`TransferDefinition`]
//! parsers the M03 gate uses, so a manifest is valid only when the definition
//! set it points at is valid and meets every recorded requirement. Reading the
//! files is the caller's job; this crate opens nothing.
//!
//! # What this module does not do
//!
//! It contacts no producer repository, runs no builder, parses no mkosi
//! configuration and resolves no revision. The mkosi reference is a path the
//! mkosi gate in `tools/verify_mkosi_definitions.py` resolves; the revision is
//! a recorded value and no git command is run to confirm it.

use crate::field::{CorrelationId, PackageName, RelativePath, Revision, SnapshotId};
use crate::finding::Finding;
use crate::payload::{MAX_PAYLOAD_BYTES, declared_schema};
use crate::repart::{MAX_DEFINITIONS, RepartDefinition, check_repart_set};
use crate::sysupdate::TransferDefinition;
use crate::{DefinitionError, MAX_FINDINGS};

/// The stable tag the payload's `schema` field carries.
pub const PRODUCT_INPUT_TAG: &str = "aegis.p01.product-input.v1";

/// Scalar bound on the packages one manifest may pin.
pub const MAX_PACKAGES: usize = 64;

/// The most delivery attempts one request may make.
pub const MAX_RETRY_ATTEMPTS: u32 = 5;

/// The longest backoff, in seconds, one request may wait between attempts.
pub const MAX_BACKOFF_SECONDS: u32 = 3600;

/// The contract versions of the product input manifest this build admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum ProductInputVersion {
    /// Version 1, tagged `aegis.p01.product-input.v1`.
    #[serde(rename = "aegis.p01.product-input.v1")]
    V1,
}

/// The distributions an Aegis image may be built from.
///
/// The spelling is mkosi's own `Distribution=` value, so the manifest and the
/// image configuration name the same thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DistributionId {
    /// Arch Linux, spelled `arch`.
    #[serde(rename = "arch")]
    Arch,
}

/// The pinned distribution input (decision D18).
///
/// There is no unpinned spelling: [`SnapshotId`] refuses `latest`, which is
/// the value REQ-P01-01 records as the reason the image inputs are not
/// reproducible today.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DistributionPin {
    /// The distribution the image is built from.
    pub id: DistributionId,
    /// The dated snapshot every package version is resolved against.
    pub snapshot: SnapshotId,
}

/// Where the reviewed configuration inputs live in this repository.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DefinitionReferences {
    /// The `repart.d(5)` drop-in directory.
    pub repart: RelativePath,
    /// The `sysupdate.d(5)` transfer directory.
    pub sysupdate: RelativePath,
    /// The mkosi configuration the image gate resolves.
    pub mkosi: RelativePath,
    /// The kernel requirement payload the kernel build consumes.
    pub kernel_requirement: RelativePath,
}

/// Where the boot kernel comes from (decision D07, amended by D70).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BootKernelSource {
    /// The pinned distribution package, which D07 makes the default.
    DistributionPackage,
    /// The kernel this repository builds while Nucleus is a scaffold (D70, M26).
    BuiltHere,
    /// A producer artifact consumed through the M09 contract, which D07 lets
    /// override the default once it exists.
    ProducerArtifact,
}

/// The boot kernel identity.
///
/// All three of D07's facts are recorded at once: which source is in force,
/// the distribution package that boots by default, and the requirement payload
/// any kernel must satisfy. A manifest therefore says who currently builds the
/// kernel without losing what the kernel has to be.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct BootKernel {
    /// Which source is in force.
    pub source: BootKernelSource,
    /// The pinned distribution package D07 makes the default.
    pub default_package: PackageName,
}

/// The bounded retry budget for one request across the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RetryPolicy {
    /// How many attempts one request may make in total.
    pub max_attempts: u32,
    /// How long to wait between attempts, in seconds.
    pub backoff_seconds: u32,
}

/// The product input manifest payload.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ProductInputManifest {
    /// The contract version this payload claims.
    pub schema: ProductInputVersion,
    /// The identifier threading this request and its result together.
    pub correlation_id: CorrelationId,
    /// The exact source revision the referenced inputs were read at.
    pub revision: Revision,
    /// The pinned distribution input.
    pub distribution: DistributionPin,
    /// Where the reviewed configuration inputs live.
    pub definitions: DefinitionReferences,
    /// The image content set the recorded requirements name.
    pub packages: Vec<PackageName>,
    /// The boot kernel identity.
    pub kernel: BootKernel,
    /// The bounded retry budget.
    pub retries: RetryPolicy,
}

/// Why a product input manifest was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ManifestError {
    /// The payload names a contract version this build does not admit.
    #[error("the payload claims contract version {declared:?}, not {expected:?}")]
    UnknownVersion {
        /// The version the payload claimed.
        declared: String,
        /// The version this build admits.
        expected: &'static str,
    },
    /// The payload is not a well-formed instance of the contract.
    #[error("the payload is not a well-formed product input manifest: {reason}")]
    Malformed {
        /// What the decoder refused it for.
        reason: String,
    },
    /// The payload does not fit the scalar byte bound.
    #[error("the payload is {bytes} bytes, past the bound of {bound}")]
    TooLong {
        /// How many bytes the payload carries.
        bytes: usize,
        /// The bound it passed.
        bound: usize,
    },
    /// The payload pins no package at all.
    #[error("the payload pins no package; an empty content set is refused")]
    NoPackages,
    /// The payload carries more rows than a bound allows.
    #[error("the payload carries more than {bound} {what}")]
    TooMany {
        /// What overflowed, as a plural noun.
        what: &'static str,
        /// The bound it passed.
        bound: usize,
    },
    /// The retry budget is outside its declared bounds.
    #[error("{what} of {value} is outside 1..={bound}")]
    RetryOutOfRange {
        /// Which half of the budget is out of range.
        what: &'static str,
        /// The value the payload declared.
        value: u32,
        /// The bound it passed.
        bound: u32,
    },
    /// The definition set the manifest points at is empty.
    #[error("{what} referenced by the manifest holds no definition")]
    NoDefinitions {
        /// Which referenced set is empty.
        what: &'static str,
    },
    /// A referenced definition is not valid input.
    #[error("{path}: {source}")]
    DefinitionRefused {
        /// The manifest reference the definition was read under.
        path: String,
        /// Why the definition parser refused it.
        source: DefinitionError,
    },
    /// A referenced definition set does not meet a recorded requirement.
    #[error("{path}: {finding}")]
    RequirementNotMet {
        /// The manifest reference the definition set was read under.
        path: String,
        /// The requirement the set does not meet.
        finding: Finding,
    },
}

/// Refuses a retry budget outside its declared bounds.
fn check_retries(retries: RetryPolicy) -> Result<(), ManifestError> {
    if retries.max_attempts == 0 || retries.max_attempts > MAX_RETRY_ATTEMPTS {
        return Err(ManifestError::RetryOutOfRange {
            what: "max-attempts",
            value: retries.max_attempts,
            bound: MAX_RETRY_ATTEMPTS,
        });
    }
    if retries.backoff_seconds == 0 || retries.backoff_seconds > MAX_BACKOFF_SECONDS {
        return Err(ManifestError::RetryOutOfRange {
            what: "backoff-seconds",
            value: retries.backoff_seconds,
            bound: MAX_BACKOFF_SECONDS,
        });
    }
    Ok(())
}

/// Parses one referenced repart set and reports the requirements it misses.
fn check_repart(
    path: &str,
    sources: &[(String, String)],
) -> Result<Vec<RepartDefinition>, ManifestError> {
    if sources.is_empty() {
        return Err(ManifestError::NoDefinitions { what: "repart" });
    }
    if sources.len() > MAX_DEFINITIONS {
        return Err(ManifestError::TooMany {
            what: "repart definitions",
            bound: MAX_DEFINITIONS,
        });
    }
    let mut set: Vec<RepartDefinition> = Vec::new();
    for (name, text) in sources.iter().take(MAX_DEFINITIONS) {
        let parsed = RepartDefinition::parse(name, text).map_err(|source| {
            ManifestError::DefinitionRefused {
                path: path.to_owned(),
                source,
            }
        })?;
        set.push(parsed);
    }
    Ok(set)
}

/// Parses one referenced transfer set and reports the requirements it misses.
fn check_transfers(path: &str, sources: &[(String, String)]) -> Result<(), ManifestError> {
    if sources.is_empty() {
        return Err(ManifestError::NoDefinitions { what: "sysupdate" });
    }
    if sources.len() > MAX_DEFINITIONS {
        return Err(ManifestError::TooMany {
            what: "sysupdate definitions",
            bound: MAX_DEFINITIONS,
        });
    }
    for (name, text) in sources.iter().take(MAX_DEFINITIONS) {
        let transfer = TransferDefinition::parse(name, text).map_err(|source| {
            ManifestError::DefinitionRefused {
                path: path.to_owned(),
                source,
            }
        })?;
        if let Some(finding) = transfer.findings().into_iter().next() {
            return Err(ManifestError::RequirementNotMet {
                path: path.to_owned(),
                finding,
            });
        }
    }
    Ok(())
}

impl ProductInputManifest {
    /// The contract version tag this type instantiates.
    pub const SCHEMA_TAG: &'static str = PRODUCT_INPUT_TAG;

    /// Checks the invariants the field types cannot express.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError::NoPackages`] for an empty content set,
    /// [`ManifestError::TooMany`] past the package bound and
    /// [`ManifestError::RetryOutOfRange`] for a retry budget outside its
    /// bounds.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.packages.is_empty() {
            return Err(ManifestError::NoPackages);
        }
        if self.packages.len() > MAX_PACKAGES {
            return Err(ManifestError::TooMany {
                what: "packages",
                bound: MAX_PACKAGES,
            });
        }
        check_retries(self.retries)
    }

    /// Decodes and validates one product input manifest payload.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError::TooLong`] past the byte bound,
    /// [`ManifestError::UnknownVersion`] for a payload naming another contract
    /// version, [`ManifestError::Malformed`] for anything else the schema
    /// refuses, and whatever [`Self::validate`] refuses.
    pub fn decode(text: &str) -> Result<Self, ManifestError> {
        if text.len() > MAX_PAYLOAD_BYTES {
            return Err(ManifestError::TooLong {
                bytes: text.len(),
                bound: MAX_PAYLOAD_BYTES,
            });
        }
        let decoded: Self =
            serde_json::from_str(text).map_err(|error| match declared_schema(text) {
                Some(tag) if tag != Self::SCHEMA_TAG => ManifestError::UnknownVersion {
                    declared: tag,
                    expected: Self::SCHEMA_TAG,
                },
                _ => ManifestError::Malformed {
                    reason: error.to_string(),
                },
            })?;
        decoded.validate()?;
        Ok(decoded)
    }

    /// Encodes a validated manifest as JSON.
    ///
    /// # Errors
    ///
    /// Propagates [`Self::validate`], and returns [`ManifestError::Malformed`]
    /// when the payload cannot be rendered.
    pub fn encode(&self) -> Result<String, ManifestError> {
        self.validate()?;
        serde_json::to_string(self).map_err(|error| ManifestError::Malformed {
            reason: error.to_string(),
        })
    }

    /// Validates the definition sets this manifest references.
    ///
    /// `repart` and `transfers` are `(file name, text)` pairs the caller read
    /// from the referenced directories. Every drop-in must parse, the set must
    /// meet every recorded requirement, and every transfer must parse with no
    /// finding of its own.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError::NoDefinitions`] for an empty set,
    /// [`ManifestError::TooMany`] for a set past [`MAX_DEFINITIONS`],
    /// [`ManifestError::DefinitionRefused`] for a definition the parser
    /// refuses, and [`ManifestError::RequirementNotMet`] for a set that parses
    /// but does not meet a recorded requirement.
    pub fn validate_definitions(
        &self,
        repart: &[(String, String)],
        transfers: &[(String, String)],
    ) -> Result<(), ManifestError> {
        let repart_path = self.definitions.repart.as_str();
        let set = check_repart(repart_path, repart)?;
        if let Some(finding) = check_repart_set(&set).into_iter().take(MAX_FINDINGS).next() {
            return Err(ManifestError::RequirementNotMet {
                path: repart_path.to_owned(),
                finding,
            });
        }
        check_transfers(self.definitions.sysupdate.as_str(), transfers)
    }
}
