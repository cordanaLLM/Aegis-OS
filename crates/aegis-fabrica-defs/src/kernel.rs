// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The kernel requirement schema Aegis hands to whoever builds the kernel.
//!
//! # A payload, not a fixed symbol list
//!
//! The requirement is a list of rows, each naming a [`ConfigSymbol`], the
//! [`RequiredState`] that symbol must be in, the [`ProbeSource`] that says how
//! the claim is checked on a running system, and the recorded requirement the
//! row comes from. Adding a kernel feature is adding a row to a JSON payload;
//! it is not a change to this crate. That is the difference the milestone asks
//! for: nothing here enumerates BPF LSM, `sched_ext`, BTF, RAPL, VFIO or KVM
//! as named constants.
//!
//! # Who consumes it
//!
//! Decision D70 puts kernel construction in this repository while Nucleus is a
//! scaffold, as milestone M26. [`KernelRequirement::config_fragment`] renders
//! the payload as a Kconfig fragment -- `CONFIG_X=y`, `CONFIG_X=m` and
//! `# CONFIG_X is not set` lines with the requirement identifier above each --
//! which is the form a kernel build applies to a base configuration. The
//! payload is therefore directly consumable by M26 rather than a description
//! of one.
//!
//! # What it is checked against
//!
//! [`ReferenceProfile`] reads `planning/hardware-profile.json`: the recorded
//! kernel release, the architecture, the measured Kconfig symbol states and
//! the capability rows with the command each was observed by.
//! [`KernelRequirement::unmet`] returns what that profile does not satisfy,
//! and an empty result is the positive case. The check fails closed: a symbol
//! the profile does not record at all is [`Unmet::Unobserved`] rather than an
//! assumed default.
//!
//! # What this module does not do
//!
//! It does not build a kernel, boot one, load an eBPF program, read
//! `/proc/config.gz`, or verify a digest or a signature. The artifact digest
//! and signature are field encodings; nothing here produces or checks either.

use std::collections::BTreeMap;

use crate::field::{
    ArtifactDigest, ArtifactSignature, ConfigSymbol, CorrelationId, KernelRelease, RequirementId,
};
use crate::payload::{MAX_PAYLOAD_BYTES, declared_schema};

/// The stable tag the payload's `schema` field carries.
pub const KERNEL_REQUIREMENT_TAG: &str = "aegis.p01-nucleus.kernel-requirement.v1";

/// Scalar bound on the feature rows one requirement payload may carry.
pub const MAX_FEATURES: usize = 64;

/// Scalar bound on the architectures one requirement payload may accept.
pub const MAX_ARCHITECTURES: usize = 4;

/// Scalar bound on the unmet rows one check may report.
///
/// Derived rather than chosen. `unmet_identity` contributes at most three rows
/// -- architecture, release and ABI -- and each feature row contributes at most
/// two, a missing capability and a state that does not match. A payload
/// [`KernelRequirement::validate`] accepts carries at most [`MAX_FEATURES`]
/// feature rows, so the count can never reach this bound and the truncation
/// that enforces it can never drop a row that was actually found. A bound
/// below the reachable maximum would under-report an unmet requirement in
/// silence, which is the opposite of failing closed.
pub const MAX_UNMET: usize = 3 + 2 * MAX_FEATURES;

/// Scalar bound, in bytes, on a reference profile document.
pub const MAX_PROFILE_BYTES: usize = 1 << 20;

/// The contract versions of the kernel requirement this build admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum KernelRequirementVersion {
    /// Version 1, tagged `aegis.p01-nucleus.kernel-requirement.v1`.
    #[serde(rename = "aegis.p01-nucleus.kernel-requirement.v1")]
    V1,
}

/// The architectures a kernel requirement may accept.
///
/// The spellings are mkosi's own `Architecture=` values, so the same word
/// travels from the product input manifest into the image configuration
/// without translation. An architecture outside this list does not decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Architecture {
    /// 64-bit x86, spelled `x86-64`.
    #[serde(rename = "x86-64")]
    X86_64,
    /// 64-bit Arm, spelled `arm64`.
    #[serde(rename = "arm64")]
    Arm64,
}

impl Architecture {
    /// The stable tag this architecture is written as.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::X86_64 => "x86-64",
            Self::Arm64 => "arm64",
        }
    }
}

/// The state a Kconfig symbol is observed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigState {
    /// Compiled into the kernel image, written `=y`.
    BuiltIn,
    /// Built as a loadable module, written `=m`.
    Module,
    /// Not enabled, written `# CONFIG_X is not set`.
    NotSet,
}

impl ConfigState {
    /// Reads a state as a `.config` or reference profile writes it.
    ///
    /// Accepts `y`, `m`, `n`, `not set` and `not-set`; anything else is
    /// `None`, so an unreadable value in the profile is a refusal rather than
    /// a guessed default.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "y" => Some(Self::BuiltIn),
            "m" => Some(Self::Module),
            "n" | "not set" | "not-set" => Some(Self::NotSet),
            _ => None,
        }
    }

    /// The stable tag this state is written as.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::BuiltIn => "y",
            Self::Module => "m",
            Self::NotSet => "not set",
        }
    }
}

/// The state a requirement row demands of its symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RequiredState {
    /// The symbol must be compiled in; a module does not satisfy it.
    BuiltIn,
    /// The symbol must be a loadable module; built-in does not satisfy it.
    Module,
    /// The symbol must be available either way.
    Present,
    /// The symbol must not be enabled at all.
    Absent,
}

impl RequiredState {
    /// Returns `true` when `observed` satisfies this requirement.
    ///
    /// `None` means the profile records nothing about the symbol, which never
    /// satisfies a requirement -- including [`Self::Absent`]. An unrecorded
    /// symbol is an unanswered question, not a measured absence.
    #[must_use]
    pub fn satisfied_by(self, observed: Option<ConfigState>) -> bool {
        match (self, observed) {
            (_, None) => false,
            (Self::BuiltIn, Some(state)) => state == ConfigState::BuiltIn,
            (Self::Module, Some(state)) => state == ConfigState::Module,
            (Self::Present, Some(state)) => state != ConfigState::NotSet,
            (Self::Absent, Some(state)) => state == ConfigState::NotSet,
        }
    }

    /// Renders this requirement for `symbol` as one Kconfig fragment line.
    ///
    /// [`Self::Present`] renders as `=y`: a fragment states one assignment,
    /// and built-in is the strongest assignment that satisfies "either way".
    #[must_use]
    pub fn fragment_line(self, symbol: &ConfigSymbol) -> String {
        match self {
            Self::BuiltIn | Self::Present => format!("{symbol}=y"),
            Self::Module => format!("{symbol}=m"),
            Self::Absent => format!("# {symbol} is not set"),
        }
    }
}

/// How a feature claim is checked on a running system.
///
/// Each variant names one interface, and [`Self::path`] is the exact path the
/// probe reads. A row therefore carries its own falsification recipe: a
/// reviewer runs [`FeatureRequirement::probe_command`] and compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProbeSource {
    /// The kernel's own compiled-in configuration, `/proc/config.gz`.
    KernelConfig,
    /// The active LSM list, `/sys/kernel/security/lsm`.
    LsmList,
    /// The kernel BTF blob CO-RE eBPF programs compile against.
    BtfVmlinux,
    /// The powercap tree the RAPL energy counters appear under.
    Powercap,
    /// The IOMMU group tree VFIO passthrough needs.
    IommuGroups,
}

impl ProbeSource {
    /// The exact path this probe reads.
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::KernelConfig => "/proc/config.gz",
            Self::LsmList => "/sys/kernel/security/lsm",
            Self::BtfVmlinux => "/sys/kernel/btf/vmlinux",
            Self::Powercap => "/sys/class/powercap",
            Self::IommuGroups => "/sys/kernel/iommu_groups",
        }
    }

    /// The reference profile capability row this probe corresponds to.
    ///
    /// `None` for [`Self::KernelConfig`]: the compiled-in configuration is not
    /// a capability row, it is the symbol table every row is checked against.
    #[must_use]
    pub const fn capability(self) -> Option<&'static str> {
        match self {
            Self::KernelConfig => None,
            Self::LsmList => Some("bpf_lsm"),
            Self::BtfVmlinux => Some("btf"),
            Self::Powercap => Some("rapl_energy_counters"),
            Self::IommuGroups => Some("iommu"),
        }
    }
}

/// One required kernel feature.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct FeatureRequirement {
    /// The Kconfig symbol the requirement is about.
    pub symbol: ConfigSymbol,
    /// The state that symbol must be in.
    pub state: RequiredState,
    /// How the claim is checked on a running system.
    pub probe: ProbeSource,
    /// The recorded requirement this row comes from.
    pub required_by: RequirementId,
}

impl FeatureRequirement {
    /// The exact command that checks this row on a running system.
    ///
    /// For a kernel-configuration row the symbol is part of the command, so
    /// the recipe is specific to the row rather than to the interface.
    #[must_use]
    pub fn probe_command(&self) -> String {
        match self.probe {
            ProbeSource::KernelConfig => format!("zgrep {} {}", self.symbol, self.probe.path()),
            ProbeSource::LsmList => format!("cat {}", self.probe.path()),
            ProbeSource::BtfVmlinux | ProbeSource::Powercap | ProbeSource::IommuGroups => {
                format!("ls {}", self.probe.path())
            }
        }
    }
}

/// The kernel version and module ABI the requirement is bounded by.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct KernelAbi {
    /// The oldest release the requirement admits. A hard floor.
    pub minimum_release: KernelRelease,
    /// The release the requirement aims at, when one is named.
    ///
    /// Recorded, never checked against a profile. The M01 drift register's
    /// `target kernel floor` row states that the target version named by the
    /// sources is not a released version, so a check against it would refuse
    /// every kernel that exists.
    #[serde(default)]
    pub target_release: Option<KernelRelease>,
    /// The exact module ABI a conforming artifact reports, when one is fixed.
    ///
    /// `None` while the artifact does not exist yet: a kernel that has not
    /// been built has no ABI to demand.
    #[serde(default)]
    pub module_abi: Option<KernelRelease>,
}

/// The digest and signature a conforming artifact is expected to carry.
///
/// The digest is mandatory inside this object, so a payload cannot promise a
/// signature over nothing; a requirement that expects neither omits the object
/// entirely. Both are field encodings: this crate produces and verifies
/// neither.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ArtifactExpectation {
    /// The sha256 the produced artifact must hash to.
    pub digest: ArtifactDigest,
    /// The signature over that digest, when one is expected.
    #[serde(default)]
    pub signature: Option<ArtifactSignature>,
}

/// The kernel requirement payload.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct KernelRequirement {
    /// The contract version this payload claims.
    pub schema: KernelRequirementVersion,
    /// The identifier threading this requirement and its result together.
    pub correlation_id: CorrelationId,
    /// The architectures a conforming kernel may be built for.
    pub architectures: Vec<Architecture>,
    /// The version and ABI bounds.
    pub abi: KernelAbi,
    /// The features a conforming kernel must provide.
    pub features: Vec<FeatureRequirement>,
    /// What a conforming artifact must hash and be signed to, when known.
    #[serde(default)]
    pub artifact: Option<ArtifactExpectation>,
}

/// Why a kernel requirement payload was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum KernelError {
    /// The payload names a contract version this build does not admit.
    #[error("the payload claims contract version {declared:?}, not {expected:?}")]
    UnknownVersion {
        /// The version the payload claimed.
        declared: String,
        /// The version this build admits.
        expected: &'static str,
    },
    /// The payload is not a well-formed instance of the contract.
    #[error("the payload is not a well-formed kernel requirement: {reason}")]
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
    /// The payload carries no feature at all.
    ///
    /// Refused explicitly rather than read as "no requirements": a consumer
    /// that receives an empty list cannot tell an unwritten payload from a
    /// deliberate one, and would build whatever it liked.
    #[error("the payload lists no required feature; an empty requirement is refused")]
    NoFeatures,
    /// The payload accepts no architecture at all.
    #[error("the payload accepts no architecture")]
    NoArchitectures,
    /// The payload carries more rows than a bound allows.
    #[error("the payload carries more than {bound} {what}")]
    TooMany {
        /// What overflowed, as a plural noun.
        what: &'static str,
        /// The bound it passed.
        bound: usize,
    },
    /// One Kconfig symbol is required twice.
    #[error("the payload requires {symbol} more than once")]
    DuplicateSymbol {
        /// The symbol that repeats.
        symbol: ConfigSymbol,
    },
    /// The target release is older than the minimum release.
    #[error("target release {target} is older than the minimum release {minimum}")]
    InvertedRelease {
        /// The declared minimum.
        minimum: KernelRelease,
        /// The declared target.
        target: KernelRelease,
    },
    /// The reference profile document could not be read.
    #[error("the reference profile could not be read: {reason}")]
    UnreadableProfile {
        /// What the reader refused it for.
        reason: String,
    },
}

/// What a reference profile does not satisfy, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Unmet {
    /// The profile's architecture is not one the requirement accepts.
    ArchitectureNotAccepted {
        /// The architecture the profile reports.
        observed: Architecture,
    },
    /// The profile's kernel is older than the required minimum.
    ReleaseBelowMinimum {
        /// The release the profile reports.
        observed: KernelRelease,
        /// The minimum the requirement demands.
        minimum: KernelRelease,
    },
    /// The profile's module ABI is not the one the requirement fixes.
    AbiMismatch {
        /// The ABI the profile reports.
        observed: KernelRelease,
        /// The ABI the requirement demands.
        required: KernelRelease,
    },
    /// The profile records nothing about a required symbol.
    Unobserved {
        /// The symbol nothing was recorded about.
        symbol: ConfigSymbol,
        /// The probe that would have recorded it.
        probe: ProbeSource,
    },
    /// The profile records the symbol in another state.
    StateMismatch {
        /// The symbol that is in the wrong state.
        symbol: ConfigSymbol,
        /// The state the requirement demands.
        required: RequiredState,
        /// The state the profile reports.
        observed: ConfigState,
    },
    /// The capability a runtime probe reads is absent from the profile.
    CapabilityAbsent {
        /// The probe whose capability row is absent or false.
        probe: ProbeSource,
        /// The capability row that was consulted.
        capability: &'static str,
    },
}

impl core::fmt::Display for Unmet {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ArchitectureNotAccepted { observed } => write!(
                formatter,
                "the profile reports architecture {}, which the requirement does not accept",
                observed.tag()
            ),
            Self::ReleaseBelowMinimum { observed, minimum } => write!(
                formatter,
                "the profile kernel {observed} is older than the required minimum {minimum}"
            ),
            Self::AbiMismatch { observed, required } => write!(
                formatter,
                "the profile module ABI {observed} is not the required {required}"
            ),
            Self::Unobserved { symbol, probe } => write!(
                formatter,
                "{symbol} is not recorded in the profile; read it with {}",
                probe.path()
            ),
            Self::StateMismatch {
                symbol,
                required,
                observed,
            } => write!(
                formatter,
                "{symbol} is {} in the profile, and {required:?} is required",
                observed.tag()
            ),
            Self::CapabilityAbsent { probe, capability } => write!(
                formatter,
                "profile capability {capability} is absent, so {} cannot be read",
                probe.path()
            ),
        }
    }
}

/// One capability row of the reference profile.
#[derive(serde::Deserialize)]
struct ProfileCapability {
    present: bool,
    evidence_command: String,
}

/// The processor facts the requirement check reads.
#[derive(serde::Deserialize)]
struct ProfileCpu {
    architecture: Architecture,
}

/// The kernel facts the requirement check reads.
#[derive(serde::Deserialize)]
struct ProfileKernel {
    release: KernelRelease,
    config: BTreeMap<String, String>,
}

/// The subset of `planning/hardware-profile.json` a requirement is checked against.
///
/// Unknown fields are deliberately allowed here: the profile is a recorded
/// document with a wider purpose than this check, and a field added to it for
/// another milestone must not break the kernel requirement reader.
#[derive(serde::Deserialize)]
struct ProfileDocument {
    cpu: ProfileCpu,
    kernel: ProfileKernel,
    capabilities: BTreeMap<String, ProfileCapability>,
}

/// The measured reference profile a kernel requirement is checked against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceProfile {
    release: KernelRelease,
    architecture: Architecture,
    symbols: BTreeMap<String, ConfigState>,
    capabilities: BTreeMap<String, (bool, String)>,
}

/// Converts the profile's raw symbol map into typed states.
fn typed_symbols(
    raw: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, ConfigState>, KernelError> {
    let mut out: BTreeMap<String, ConfigState> = BTreeMap::new();
    for (name, value) in raw.iter().take(MAX_PROFILE_BYTES) {
        let Some(state) = ConfigState::parse(value) else {
            return Err(KernelError::UnreadableProfile {
                reason: format!("kernel.config[{name}] is {value:?}, not y, m or not set"),
            });
        };
        out.insert(name.clone(), state);
    }
    Ok(out)
}

impl ReferenceProfile {
    /// Reads the recorded reference profile document.
    ///
    /// # Errors
    ///
    /// Returns [`KernelError::UnreadableProfile`] when the document is past
    /// the byte bound, is not the expected shape, or records a Kconfig state
    /// that is not `y`, `m` or `not set`.
    pub fn from_profile_json(text: &str) -> Result<Self, KernelError> {
        if text.len() > MAX_PROFILE_BYTES {
            return Err(KernelError::UnreadableProfile {
                reason: format!(
                    "{} bytes, past the bound of {MAX_PROFILE_BYTES}",
                    text.len()
                ),
            });
        }
        let document: ProfileDocument =
            serde_json::from_str(text).map_err(|error| KernelError::UnreadableProfile {
                reason: error.to_string(),
            })?;
        let symbols = typed_symbols(&document.kernel.config)?;
        let capabilities = document
            .capabilities
            .into_iter()
            .map(|(name, row)| (name, (row.present, row.evidence_command)))
            .collect();
        Ok(Self {
            release: document.kernel.release,
            architecture: document.cpu.architecture,
            symbols,
            capabilities,
        })
    }

    /// The kernel release the profile records.
    #[must_use]
    pub fn release(&self) -> &KernelRelease {
        &self.release
    }

    /// The architecture the profile records.
    #[must_use]
    pub fn architecture(&self) -> Architecture {
        self.architecture
    }

    /// The state the profile records for `symbol`, if it records one.
    #[must_use]
    pub fn state_of(&self, symbol: &ConfigSymbol) -> Option<ConfigState> {
        self.symbols.get(symbol.as_str()).copied()
    }

    /// Whether the profile records `capability` as present.
    #[must_use]
    pub fn capability(&self, capability: &str) -> Option<bool> {
        self.capabilities.get(capability).map(|row| row.0)
    }

    /// The command the profile records `capability` as observed by.
    #[must_use]
    pub fn capability_evidence(&self, capability: &str) -> Option<&str> {
        self.capabilities.get(capability).map(|row| row.1.as_str())
    }
}

/// Refuses a payload whose bounded collections are empty or overlong.
fn check_collections(requirement: &KernelRequirement) -> Result<(), KernelError> {
    if requirement.features.is_empty() {
        return Err(KernelError::NoFeatures);
    }
    if requirement.architectures.is_empty() {
        return Err(KernelError::NoArchitectures);
    }
    if requirement.features.len() > MAX_FEATURES {
        return Err(KernelError::TooMany {
            what: "features",
            bound: MAX_FEATURES,
        });
    }
    if requirement.architectures.len() > MAX_ARCHITECTURES {
        return Err(KernelError::TooMany {
            what: "architectures",
            bound: MAX_ARCHITECTURES,
        });
    }
    Ok(())
}

/// Refuses a payload that names one Kconfig symbol more than once.
fn check_distinct_symbols(requirement: &KernelRequirement) -> Result<(), KernelError> {
    let mut seen: Vec<&str> = Vec::new();
    for feature in requirement.features.iter().take(MAX_FEATURES) {
        let symbol = feature.symbol.as_str();
        if seen.contains(&symbol) {
            return Err(KernelError::DuplicateSymbol {
                symbol: feature.symbol.clone(),
            });
        }
        seen.push(symbol);
    }
    Ok(())
}

impl KernelRequirement {
    /// The contract version tag this type instantiates.
    pub const SCHEMA_TAG: &'static str = KERNEL_REQUIREMENT_TAG;

    /// Checks the invariants the field types cannot express.
    ///
    /// # Errors
    ///
    /// Returns [`KernelError::NoFeatures`] for an empty requirement list,
    /// [`KernelError::NoArchitectures`] for an empty architecture list,
    /// [`KernelError::TooMany`] past either bound,
    /// [`KernelError::DuplicateSymbol`] for a repeated symbol and
    /// [`KernelError::InvertedRelease`] when the target is older than the
    /// minimum.
    pub fn validate(&self) -> Result<(), KernelError> {
        check_collections(self)?;
        check_distinct_symbols(self)?;
        if let Some(target) = self.abi.target_release.as_ref()
            && !target.at_least(&self.abi.minimum_release)
        {
            return Err(KernelError::InvertedRelease {
                minimum: self.abi.minimum_release.clone(),
                target: target.clone(),
            });
        }
        Ok(())
    }

    /// Decodes and validates one kernel requirement payload.
    ///
    /// # Errors
    ///
    /// Returns [`KernelError::TooLong`] past the byte bound,
    /// [`KernelError::UnknownVersion`] for a payload naming another contract
    /// version, [`KernelError::Malformed`] for anything else the schema
    /// refuses, and whatever [`Self::validate`] refuses.
    pub fn decode(text: &str) -> Result<Self, KernelError> {
        if text.len() > MAX_PAYLOAD_BYTES {
            return Err(KernelError::TooLong {
                bytes: text.len(),
                bound: MAX_PAYLOAD_BYTES,
            });
        }
        let decoded: Self =
            serde_json::from_str(text).map_err(|error| match declared_schema(text) {
                Some(tag) if tag != Self::SCHEMA_TAG => KernelError::UnknownVersion {
                    declared: tag,
                    expected: Self::SCHEMA_TAG,
                },
                _ => KernelError::Malformed {
                    reason: error.to_string(),
                },
            })?;
        decoded.validate()?;
        Ok(decoded)
    }

    /// Encodes a validated requirement as JSON.
    ///
    /// # Errors
    ///
    /// Propagates [`Self::validate`], and returns [`KernelError::Malformed`]
    /// when the payload cannot be rendered.
    pub fn encode(&self) -> Result<String, KernelError> {
        self.validate()?;
        serde_json::to_string(self).map_err(|error| KernelError::Malformed {
            reason: error.to_string(),
        })
    }

    /// Renders the payload as a Kconfig fragment (decision D70, milestone M26).
    ///
    /// The output is what a kernel build merges into a base configuration: one
    /// assignment per required feature, each preceded by the recorded
    /// requirement it comes from, plus a header naming the contract version,
    /// the correlation identifier and the minimum release. Nothing else is
    /// emitted, so the fragment can be applied as it stands.
    ///
    /// The payload is validated before a line is written. Every field of this
    /// type is public, so a caller can build a requirement without going
    /// through [`Self::decode`]; a payload past [`MAX_FEATURES`] would then
    /// render short, and M26 would apply a fragment missing rows with nothing
    /// in the output saying so. A requirement that cannot be rendered in full
    /// is refused instead.
    ///
    /// # Errors
    ///
    /// Propagates [`Self::validate`], so a payload past [`MAX_FEATURES`] or
    /// [`MAX_ARCHITECTURES`], one with no feature at all, one repeating a
    /// symbol and one whose target release is older than its minimum are all
    /// refused rather than rendered.
    pub fn config_fragment(&self) -> Result<String, KernelError> {
        self.validate()?;
        let mut out = String::new();
        push_comment(&mut out, Self::SCHEMA_TAG);
        push_comment(
            &mut out,
            &format!("correlation-id: {}", self.correlation_id),
        );
        push_comment(
            &mut out,
            &format!("minimum kernel release: {}", self.abi.minimum_release),
        );
        for feature in self.features.iter().take(MAX_FEATURES) {
            push_comment(&mut out, feature.required_by.as_str());
            out.push_str(&feature.state.fragment_line(&feature.symbol));
            out.push('\n');
        }
        Ok(out)
    }

    /// Returns everything `profile` does not satisfy; empty means it does.
    ///
    /// The check fails closed. A symbol the profile records nothing about is
    /// [`Unmet::Unobserved`], and a runtime probe whose capability row is
    /// absent or false is [`Unmet::CapabilityAbsent`], rather than either
    /// being read as a silent pass.
    ///
    /// [`MAX_UNMET`] is the scalar bound on the result. It is derived from
    /// [`MAX_FEATURES`] rather than chosen, so a payload [`Self::validate`]
    /// accepts cannot reach it and no row that was found is dropped from the
    /// report.
    #[must_use]
    pub fn unmet(&self, profile: &ReferenceProfile) -> Vec<Unmet> {
        let mut out = self.unmet_identity(profile);
        for feature in self.features.iter().take(MAX_FEATURES) {
            out.extend(unmet_feature(feature, profile));
        }
        out.truncate(MAX_UNMET);
        out
    }

    /// The architecture, release and ABI half of [`Self::unmet`].
    fn unmet_identity(&self, profile: &ReferenceProfile) -> Vec<Unmet> {
        let mut out = Vec::new();
        let observed = profile.architecture();
        if !self
            .architectures
            .iter()
            .take(MAX_ARCHITECTURES)
            .any(|accepted| *accepted == observed)
        {
            out.push(Unmet::ArchitectureNotAccepted { observed });
        }
        if !profile.release().at_least(&self.abi.minimum_release) {
            out.push(Unmet::ReleaseBelowMinimum {
                observed: profile.release().clone(),
                minimum: self.abi.minimum_release.clone(),
            });
        }
        if let Some(required) = self.abi.module_abi.as_ref()
            && profile.release() != required
        {
            out.push(Unmet::AbiMismatch {
                observed: profile.release().clone(),
                required: required.clone(),
            });
        }
        out
    }
}

/// Appends one `# ...` comment line to a Kconfig fragment.
fn push_comment(out: &mut String, text: &str) {
    out.push_str("# ");
    out.push_str(text);
    out.push('\n');
}

/// Returns what `profile` does not satisfy about one feature row.
fn unmet_feature(feature: &FeatureRequirement, profile: &ReferenceProfile) -> Vec<Unmet> {
    let mut out = Vec::new();
    if let Some(capability) = feature.probe.capability()
        && profile.capability(capability) != Some(true)
    {
        out.push(Unmet::CapabilityAbsent {
            probe: feature.probe,
            capability,
        });
    }
    match profile.state_of(&feature.symbol) {
        None => out.push(Unmet::Unobserved {
            symbol: feature.symbol.clone(),
            probe: feature.probe,
        }),
        Some(observed) if !feature.state.satisfied_by(Some(observed)) => {
            out.push(Unmet::StateMismatch {
                symbol: feature.symbol.clone(),
                required: feature.state,
                observed,
            });
        }
        Some(_) => {}
    }
    out
}
