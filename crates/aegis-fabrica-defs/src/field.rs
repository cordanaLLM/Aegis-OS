// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded, validated field types both M18 schemas are built from.
//!
//! One file holds every field primitive and every hand-written
//! `Serialize`/`Deserialize` pair, so the wire form of a bounded value is
//! reviewed in one place rather than rediscovered per schema. Three properties
//! are deliberate:
//!
//! * **Validation happens during decoding.** Each type is a newtype over
//!   `String` that can only be built through [`TryFrom<String>`], and serde is
//!   wired to that constructor with `#[serde(try_from = "String")]`. A payload
//!   carrying an over-long, mis-shaped or out-of-charset field is refused by
//!   the decoder rather than accepted and checked later.
//! * **Every field carries a scalar bound**, declared as a constant in this
//!   module, and the length is checked before the charset so a refusal never
//!   echoes an unbounded value back into an error message.
//! * **A shape is refused, not repaired.** [`Revision`] takes a full 40-digit
//!   commit identifier and nothing shorter; [`SnapshotId`] takes a dated
//!   snapshot and refuses `latest`. Neither has a default.

use core::fmt;

/// Scalar bound, in bytes, on a general bounded text field.
pub const MAX_FIELD_BYTES: usize = 128;

/// Width, in characters, of a full git commit identifier.
pub const REVISION_CHARS: usize = 40;

/// Width, in characters, of a sha256 digest in lower-case hexadecimal.
pub const DIGEST_CHARS: usize = 64;

/// Scalar bound, in characters, on a signature in lower-case hexadecimal.
pub const MAX_SIGNATURE_CHARS: usize = 256;

/// Width, in characters, of a dated distribution snapshot (`YYYY/MM/DD`).
pub const SNAPSHOT_CHARS: usize = 10;

/// Scalar bound, in bytes, on a kernel release string.
pub const MAX_RELEASE_BYTES: usize = 64;

/// Scalar bound, in bytes, on a Kconfig symbol name.
pub const MAX_SYMBOL_BYTES: usize = 64;

/// Scalar bound on the dotted numeric components a release comparison reads.
pub const MAX_RELEASE_COMPONENTS: usize = 8;

/// The prefix every Kconfig symbol carries.
pub const SYMBOL_PREFIX: &str = "CONFIG_";

/// The prefix every recorded requirement identifier carries.
pub const REQUIREMENT_PREFIX: &str = "REQ-";

/// Why a bounded field was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FieldError {
    /// The field carried no value at all.
    #[error("{field}: an empty value is not admissible")]
    Empty {
        /// The field that was refused.
        field: &'static str,
    },
    /// The field is longer than its scalar bound allows.
    #[error("{field}: {bytes} bytes exceeds the bound of {bound}")]
    TooLong {
        /// The field that was refused.
        field: &'static str,
        /// How many bytes the value carries.
        bytes: usize,
        /// The bound it passed.
        bound: usize,
    },
    /// The field carries a character its charset does not admit.
    #[error("{field}: {value:?} carries a character outside {expected}")]
    Charset {
        /// The field that was refused.
        field: &'static str,
        /// The value as written, already known to be within the byte bound.
        value: String,
        /// The charset a readable value stays inside.
        expected: &'static str,
    },
    /// The field is inside its charset but not in the shape the field demands.
    #[error("{field}: {value:?} is not {expected}")]
    Shape {
        /// The field that was refused.
        field: &'static str,
        /// The value as written, already known to be within the byte bound.
        value: String,
        /// What a readable value would have been.
        expected: &'static str,
    },
}

/// Refuses an empty value or one past its scalar byte bound.
fn bound(field: &'static str, value: &str, limit: usize) -> Result<(), FieldError> {
    if value.is_empty() {
        return Err(FieldError::Empty { field });
    }
    if value.len() > limit {
        return Err(FieldError::TooLong {
            field,
            bytes: value.len(),
            bound: limit,
        });
    }
    Ok(())
}

/// Refuses a value carrying any character `allowed` does not admit.
fn charset(
    field: &'static str,
    value: &str,
    expected: &'static str,
    allowed: fn(char) -> bool,
) -> Result<(), FieldError> {
    if value.chars().take(MAX_SIGNATURE_CHARS).all(allowed) {
        return Ok(());
    }
    Err(FieldError::Charset {
        field,
        value: value.to_owned(),
        expected,
    })
}

/// Builds a shape refusal for a value already known to be within its bound.
fn shape(field: &'static str, value: &str, expected: &'static str) -> FieldError {
    FieldError::Shape {
        field,
        value: value.to_owned(),
        expected,
    }
}

/// Returns `true` when `c` is a lower-case hexadecimal digit.
fn is_lower_hex(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, 'a'..='f')
}

/// Returns the leading dotted numeric components of a release string.
///
/// `7.2.4-1-cachyos` reads as `[7, 2, 4]` and `6.12` as `[6, 12]`: the scan
/// stops at the first character that is neither a digit nor a dot, which is
/// where a distribution's own suffix begins. The loop is bounded twice, by the
/// release byte bound and by [`MAX_RELEASE_COMPONENTS`].
fn numeric_components(text: &str) -> Vec<u64> {
    let mut out: Vec<u64> = Vec::new();
    let mut current: u64 = 0;
    let mut started = false;
    for c in text.chars().take(MAX_RELEASE_BYTES) {
        if let Some(digit) = c.to_digit(10) {
            current = current.saturating_mul(10).saturating_add(u64::from(digit));
            started = true;
            continue;
        }
        if c != '.' || !started || out.len() >= MAX_RELEASE_COMPONENTS {
            break;
        }
        out.push(current);
        current = 0;
        started = false;
    }
    if started && out.len() < MAX_RELEASE_COMPONENTS {
        out.push(current);
    }
    out
}

/// The identifier threading a request, its result and its audit trail together.
///
/// `docs/integration/stack.md` requires one on every crossing of the Aegis
/// boundary; a payload without it does not decode.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CorrelationId(String);

impl CorrelationId {
    /// The value, as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CorrelationId {
    type Error = FieldError;

    fn try_from(value: String) -> Result<Self, FieldError> {
        bound("correlation-id", &value, MAX_FIELD_BYTES)?;
        charset("correlation-id", &value, "[A-Za-z0-9._:-]", |c| {
            c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-')
        })?;
        Ok(Self(value))
    }
}

impl From<CorrelationId> for String {
    fn from(value: CorrelationId) -> Self {
        value.0
    }
}

impl fmt::Display for CorrelationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// An exact source revision: a full 40-character git commit identifier.
///
/// A branch name, a tag, `HEAD` and an abbreviated identifier are all refused.
/// The point of the field is that the revision names one tree and keeps naming
/// it, which a moving reference does not.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Revision(String);

impl Revision {
    /// The revision, as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Revision {
    type Error = FieldError;

    fn try_from(value: String) -> Result<Self, FieldError> {
        bound("revision", &value, REVISION_CHARS)?;
        charset("revision", &value, "[0-9a-f]", is_lower_hex)?;
        if value.len() != REVISION_CHARS {
            return Err(shape(
                "revision",
                &value,
                "a full 40-character lower-case commit identifier",
            ));
        }
        Ok(Self(value))
    }
}

impl From<Revision> for String {
    fn from(value: Revision) -> Self {
        value.0
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A dated distribution snapshot identifier, `YYYY/MM/DD` (decision D18).
///
/// The format is the one mkosi 27 builds its archive URL from: its Arch
/// support joins `repos/<snapshot>/$repo/os/$arch` onto the archive mirror and
/// formats the snapshot it discovers as `%Y/%m/%d`. `latest` and `rolling` are
/// refused here rather than accepted and warned about, because an unpinned
/// release is exactly the drift REQ-P01-01 records.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SnapshotId(String);

impl SnapshotId {
    /// The snapshot identifier, as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SnapshotId {
    type Error = FieldError;

    fn try_from(value: String) -> Result<Self, FieldError> {
        bound("snapshot", &value, SNAPSHOT_CHARS)?;
        charset("snapshot", &value, "[0-9/]", |c| {
            c.is_ascii_digit() || c == '/'
        })?;
        let parts: Vec<&str> = value.split('/').collect();
        let widths: Vec<usize> = parts.iter().take(4).map(|part| part.len()).collect();
        if widths != vec![4, 2, 2] {
            return Err(shape(
                "snapshot",
                &value,
                "a dated snapshot in the form YYYY/MM/DD",
            ));
        }
        Ok(Self(value))
    }
}

impl From<SnapshotId> for String {
    fn from(value: SnapshotId) -> Self {
        value.0
    }
}

impl fmt::Display for SnapshotId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A repository-relative path to a reviewed configuration input.
///
/// Absolute paths, parent-directory components and backslashes are refused, so
/// a manifest cannot point a consumer at a developer's home directory or climb
/// out of the checkout.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RelativePath(String);

impl RelativePath {
    /// The path, as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RelativePath {
    type Error = FieldError;

    fn try_from(value: String) -> Result<Self, FieldError> {
        bound("path", &value, MAX_FIELD_BYTES)?;
        charset("path", &value, "[A-Za-z0-9._/-]", |c| {
            c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '-')
        })?;
        let climbs = value
            .split('/')
            .take(MAX_FIELD_BYTES)
            .any(|part| part == ".." || part.is_empty());
        if climbs || value.starts_with('/') {
            return Err(shape(
                "path",
                &value,
                "a repository-relative path with no empty or parent component",
            ));
        }
        Ok(Self(value))
    }
}

impl From<RelativePath> for String {
    fn from(value: RelativePath) -> Self {
        value.0
    }
}

impl fmt::Display for RelativePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// One package name in the pinned image content set.
///
/// The charset is the one a pacman package name uses. No version is carried:
/// on a dated snapshot the repository holds exactly one version of each
/// package, so the [`SnapshotId`] is the version pin and a second, separately
/// maintained one could only disagree with it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PackageName(String);

impl PackageName {
    /// The package name, as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for PackageName {
    type Error = FieldError;

    fn try_from(value: String) -> Result<Self, FieldError> {
        bound("package", &value, MAX_FIELD_BYTES)?;
        charset("package", &value, "[a-z0-9@._+-]", |c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '@' | '.' | '_' | '+' | '-')
        })?;
        if !value.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit()) {
            return Err(shape(
                "package",
                &value,
                "a package name starting with a lower-case letter or a digit",
            ));
        }
        Ok(Self(value))
    }
}

impl From<PackageName> for String {
    fn from(value: PackageName) -> Self {
        value.0
    }
}

impl fmt::Display for PackageName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The sha256 of a produced artifact, in lower-case hexadecimal.
///
/// A field encoding only. Nothing in this crate computes or verifies a digest.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ArtifactDigest(String);

impl ArtifactDigest {
    /// The digest, as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ArtifactDigest {
    type Error = FieldError;

    fn try_from(value: String) -> Result<Self, FieldError> {
        bound("artifact-digest", &value, DIGEST_CHARS)?;
        charset("artifact-digest", &value, "[0-9a-f]", is_lower_hex)?;
        if value.len() != DIGEST_CHARS {
            return Err(shape(
                "artifact-digest",
                &value,
                "a 64-character lower-case sha256 digest",
            ));
        }
        Ok(Self(value))
    }
}

impl From<ArtifactDigest> for String {
    fn from(value: ArtifactDigest) -> Self {
        value.0
    }
}

impl fmt::Display for ArtifactDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The signature over a produced artifact, in lower-case hexadecimal.
///
/// A field encoding only. This crate neither produces nor verifies a
/// signature, and carrying one is not evidence that one was checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ArtifactSignature(String);

impl ArtifactSignature {
    /// The signature, as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ArtifactSignature {
    type Error = FieldError;

    fn try_from(value: String) -> Result<Self, FieldError> {
        bound("artifact-signature", &value, MAX_SIGNATURE_CHARS)?;
        charset("artifact-signature", &value, "[0-9a-f]", is_lower_hex)?;
        if value.len() % 2 != 0 {
            return Err(shape(
                "artifact-signature",
                &value,
                "an even number of hexadecimal digits",
            ));
        }
        Ok(Self(value))
    }
}

impl From<ArtifactSignature> for String {
    fn from(value: ArtifactSignature) -> Self {
        value.0
    }
}

impl fmt::Display for ArtifactSignature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// One Kconfig symbol name, such as `CONFIG_BPF_LSM`.
///
/// The type is what keeps the kernel requirement a payload rather than a fixed
/// list: a consumer adds a symbol by adding a row, not by changing this crate.
/// The `CONFIG_` prefix is mandatory so a rendered fragment line is always a
/// Kconfig assignment.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ConfigSymbol(String);

impl ConfigSymbol {
    /// The symbol, as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ConfigSymbol {
    type Error = FieldError;

    fn try_from(value: String) -> Result<Self, FieldError> {
        bound("symbol", &value, MAX_SYMBOL_BYTES)?;
        charset("symbol", &value, "[A-Z0-9_]", |c| {
            c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'
        })?;
        if !value.starts_with(SYMBOL_PREFIX) || value.len() <= SYMBOL_PREFIX.len() {
            return Err(shape(
                "symbol",
                &value,
                "a Kconfig symbol with a CONFIG_ prefix and a name after it",
            ));
        }
        Ok(Self(value))
    }
}

impl From<ConfigSymbol> for String {
    fn from(value: ConfigSymbol) -> Self {
        value.0
    }
}

impl fmt::Display for ConfigSymbol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A kernel release string, such as `6.12` or `7.2.4-1-cachyos`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct KernelRelease(String);

impl KernelRelease {
    /// The release, as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns `true` when this release is `floor` or newer.
    ///
    /// Only the leading dotted numeric components are compared; a
    /// distribution's own suffix is not ordered, because no two distributions
    /// order theirs the same way. A missing component reads as zero, so `7.3`
    /// is newer than `7.2.4` and not newer than `7.3.1`.
    #[must_use]
    pub fn at_least(&self, floor: &Self) -> bool {
        let mine = numeric_components(&self.0);
        let theirs = numeric_components(&floor.0);
        for index in 0..MAX_RELEASE_COMPONENTS {
            let left = mine.get(index).copied().unwrap_or(0);
            let right = theirs.get(index).copied().unwrap_or(0);
            if left != right {
                return left > right;
            }
        }
        true
    }
}

impl TryFrom<String> for KernelRelease {
    type Error = FieldError;

    fn try_from(value: String) -> Result<Self, FieldError> {
        bound("release", &value, MAX_RELEASE_BYTES)?;
        charset("release", &value, "[0-9A-Za-z._+-]", |c| {
            c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-')
        })?;
        if !value.starts_with(|c: char| c.is_ascii_digit()) {
            return Err(shape(
                "release",
                &value,
                "a release starting with a version digit",
            ));
        }
        Ok(Self(value))
    }
}

impl From<KernelRelease> for String {
    fn from(value: KernelRelease) -> Self {
        value.0
    }
}

impl fmt::Display for KernelRelease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The identifier of the recorded requirement a payload row comes from.
///
/// Every asserted feature names one, so a reviewer can go from a Kconfig
/// symbol back to the requirement in `docs/roadmap/requirements.md` that asked
/// for it without reading this crate.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RequirementId(String);

impl RequirementId {
    /// The identifier, as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RequirementId {
    type Error = FieldError;

    fn try_from(value: String) -> Result<Self, FieldError> {
        bound("required-by", &value, MAX_FIELD_BYTES)?;
        charset("required-by", &value, "[A-Z0-9-]", |c| {
            c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-'
        })?;
        if !value.starts_with(REQUIREMENT_PREFIX) || value.len() <= REQUIREMENT_PREFIX.len() {
            return Err(shape(
                "required-by",
                &value,
                "a recorded requirement identifier with a REQ- prefix",
            ));
        }
        Ok(Self(value))
    }
}

impl From<RequirementId> for String {
    fn from(value: RequirementId) -> Self {
        value.0
    }
}

impl fmt::Display for RequirementId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
