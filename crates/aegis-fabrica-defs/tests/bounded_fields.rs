// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded field types both M18 schemas are built from.
//!
//! Each type gets the same three questions: does a real value parse, is a
//! value outside the rule refused, and is the rule exact at its edge. The
//! edges matter more here than anywhere else in the crate, because every one
//! of these types exists to refuse something that would otherwise be accepted
//! silently -- an abbreviated revision, an unpinned release, a path that
//! climbs out of the checkout.

use aegis_fabrica_defs::FieldError;
use aegis_fabrica_defs::field::{
    ArtifactDigest, ArtifactSignature, ConfigSymbol, CorrelationId, DIGEST_CHARS, KernelRelease,
    MAX_FIELD_BYTES, MAX_RELEASE_BYTES, MAX_RELEASE_COMPONENTS, MAX_SIGNATURE_CHARS,
    MAX_SYMBOL_BYTES, PackageName, REQUIREMENT_PREFIX, REVISION_CHARS, RelativePath, RequirementId,
    Revision, SNAPSHOT_CHARS, SYMBOL_PREFIX, SnapshotId,
};

/// Builds an owned string of `count` copies of `c`.
fn repeated(c: char, count: usize) -> String {
    core::iter::repeat_n(c, count).collect()
}

/// Returns `true` when a parse failed for being outside its charset.
fn is_charset_refusal<T: core::fmt::Debug>(parsed: &Result<T, FieldError>) -> bool {
    matches!(parsed, Err(FieldError::Charset { .. }))
}

/// Returns `true` when a parse failed for being the wrong shape.
fn is_shape_refusal<T: core::fmt::Debug>(parsed: &Result<T, FieldError>) -> bool {
    matches!(parsed, Err(FieldError::Shape { .. }))
}

// --- Positive -------------------------------------------------------------

/// Positive: every field type parses the value the reviewed payloads carry.
#[test]
fn each_field_type_parses_the_value_the_reviewed_payloads_carry() -> Result<(), FieldError> {
    let correlation = CorrelationId::try_from("aegis-m18-product-input-0001".to_owned())?;
    assert_eq!(correlation.as_str(), "aegis-m18-product-input-0001");
    let revision = Revision::try_from(repeated('a', REVISION_CHARS))?;
    assert_eq!(revision.as_str().len(), REVISION_CHARS);
    let snapshot = SnapshotId::try_from("2026/09/13".to_owned())?;
    assert_eq!(snapshot.as_str().len(), SNAPSHOT_CHARS);
    let path = RelativePath::try_from("build/repart.d".to_owned())?;
    assert_eq!(path.as_str(), "build/repart.d");
    let package = PackageName::try_from("linux-rt".to_owned())?;
    assert_eq!(package.to_string(), "linux-rt");
    let symbol = ConfigSymbol::try_from("CONFIG_BPF_LSM".to_owned())?;
    assert!(symbol.as_str().starts_with(SYMBOL_PREFIX));
    let release = KernelRelease::try_from("7.2.4-1-cachyos".to_owned())?;
    assert_eq!(release.to_string(), "7.2.4-1-cachyos");
    let requirement = RequirementId::try_from("REQ-P07-01".to_owned())?;
    assert!(requirement.as_str().starts_with(REQUIREMENT_PREFIX));
    Ok(())
}

/// Positive: a digest and a signature round-trip through `String`.
///
/// The `From<T> for String` direction is what serde uses to encode, so a type
/// that could not give its text back would encode nothing.
#[test]
fn a_digest_and_a_signature_round_trip_through_string() -> Result<(), FieldError> {
    let digest = ArtifactDigest::try_from(repeated('b', DIGEST_CHARS))?;
    assert_eq!(String::from(digest.clone()), repeated('b', DIGEST_CHARS));
    assert_eq!(digest.to_string().len(), DIGEST_CHARS);
    let signature = ArtifactSignature::try_from("deadbeef".to_owned())?;
    assert_eq!(String::from(signature.clone()), "deadbeef");
    assert_eq!(signature.as_str(), "deadbeef");
    Ok(())
}

/// Positive: a newer release is at least an older one, in both directions.
#[test]
fn release_ordering_reads_the_leading_numeric_components() -> Result<(), FieldError> {
    let host = KernelRelease::try_from("7.2.4-1-cachyos".to_owned())?;
    let floor = KernelRelease::try_from("6.12".to_owned())?;
    let target = KernelRelease::try_from("7.3".to_owned())?;
    assert!(host.at_least(&floor));
    assert!(!floor.at_least(&host));
    assert!(target.at_least(&host));
    assert!(!host.at_least(&target));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: an abbreviated or moving revision is refused, not resolved.
#[test]
fn a_revision_that_is_not_a_full_commit_identifier_is_refused() {
    let short = Revision::try_from(repeated('a', 12));
    assert!(matches!(short, Err(FieldError::Shape { .. })), "{short:?}");
    let moving = Revision::try_from("main".to_owned());
    assert!(
        matches!(moving, Err(FieldError::Charset { .. })),
        "{moving:?}"
    );
    let head = Revision::try_from("HEAD".to_owned());
    assert!(matches!(head, Err(FieldError::Charset { .. })), "{head:?}");
    assert!(matches!(
        Revision::try_from(String::new()),
        Err(FieldError::Empty { .. })
    ));
}

/// Negative: `latest` is refused as a snapshot, which is decision D18's point.
///
/// REQ-P01-01 records the imported configuration as selecting `Release=latest`.
/// The type is what makes that unrepresentable rather than merely discouraged.
#[test]
fn an_unpinned_release_is_not_a_snapshot() {
    assert!(matches!(
        SnapshotId::try_from("latest".to_owned()),
        Err(FieldError::Charset { .. })
    ));
    assert!(matches!(
        SnapshotId::try_from("rolling".to_owned()),
        Err(FieldError::Charset { .. })
    ));
    assert!(matches!(
        SnapshotId::try_from("2026-09-13".to_owned()),
        Err(FieldError::Charset { .. })
    ));
    assert!(matches!(
        SnapshotId::try_from("2026/9/13".to_owned()),
        Err(FieldError::Shape { .. })
    ));
}

/// Negative: a path may not be absolute, climb out, or name a private tree.
#[test]
fn a_path_may_not_be_absolute_or_climb_out_of_the_checkout() {
    for refused in ["/etc/passwd", "../../secret", "build//repart.d", "a/../b"] {
        let parsed = RelativePath::try_from(refused.to_owned());
        assert!(parsed.is_err(), "{refused} was accepted: {parsed:?}");
    }
    let windows = RelativePath::try_from("build\\repart.d".to_owned());
    assert!(matches!(windows, Err(FieldError::Charset { .. })));
}

/// Negative: a symbol without the `CONFIG_` prefix is not a Kconfig symbol.
///
/// The prefix is what makes every rendered fragment line a real assignment; a
/// bare name would render as `BPF_LSM=y`, which no kernel build reads.
#[test]
fn a_symbol_without_the_config_prefix_is_refused() {
    assert!(matches!(
        ConfigSymbol::try_from("BPF_LSM".to_owned()),
        Err(FieldError::Shape { .. })
    ));
    assert!(matches!(
        ConfigSymbol::try_from(SYMBOL_PREFIX.to_owned()),
        Err(FieldError::Shape { .. })
    ));
    assert!(matches!(
        ConfigSymbol::try_from("config_bpf_lsm".to_owned()),
        Err(FieldError::Charset { .. })
    ));
}

/// Negative: a value outside its charset is refused, never transliterated.
#[test]
fn a_value_outside_its_charset_is_refused() {
    assert!(is_charset_refusal(&CorrelationId::try_from(
        "has space".to_owned()
    )));
    assert!(is_charset_refusal(&PackageName::try_from(
        "Linux-RT".to_owned()
    )));
    assert!(is_charset_refusal(&ArtifactDigest::try_from(
        "zz".to_owned()
    )));
}

/// Negative: a value inside its charset but the wrong shape is refused too.
#[test]
fn a_value_of_the_wrong_shape_is_refused() {
    assert!(is_shape_refusal(&PackageName::try_from(
        "-leading-dash".to_owned()
    )));
    assert!(is_shape_refusal(&KernelRelease::try_from(
        "v7.3".to_owned()
    )));
    assert!(is_shape_refusal(&RequirementId::try_from(
        "P07-01".to_owned()
    )));
    assert!(is_shape_refusal(&ArtifactSignature::try_from(
        "abc".to_owned()
    )));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the general field bound is exact at its own width.
#[test]
fn the_general_field_bound_is_exact_at_its_own_width() {
    assert!(CorrelationId::try_from(repeated('a', MAX_FIELD_BYTES)).is_ok());
    let over = CorrelationId::try_from(repeated('a', MAX_FIELD_BYTES.saturating_add(1)));
    assert!(matches!(over, Err(FieldError::TooLong { .. })), "{over:?}");
    assert!(KernelRelease::try_from(repeated('7', MAX_RELEASE_BYTES)).is_ok());
    assert!(KernelRelease::try_from(repeated('7', MAX_RELEASE_BYTES.saturating_add(1))).is_err());
}

/// Boundary: the fixed-width fields accept their own width and nothing else.
#[test]
fn the_fixed_width_fields_accept_their_own_width_and_nothing_else() {
    assert!(Revision::try_from(repeated('0', REVISION_CHARS)).is_ok());
    assert!(Revision::try_from(repeated('0', REVISION_CHARS.saturating_sub(1))).is_err());
    assert!(Revision::try_from(repeated('0', REVISION_CHARS.saturating_add(1))).is_err());
    assert!(ArtifactDigest::try_from(repeated('0', DIGEST_CHARS)).is_ok());
    assert!(ArtifactDigest::try_from(repeated('0', DIGEST_CHARS.saturating_sub(1))).is_err());
    assert!(ArtifactDigest::try_from(repeated('0', DIGEST_CHARS.saturating_add(1))).is_err());
}

/// Boundary: the signature and symbol bounds are exact at their own widths.
#[test]
fn the_signature_and_symbol_bounds_are_exact_at_their_own_widths() {
    assert!(ArtifactSignature::try_from(repeated('0', MAX_SIGNATURE_CHARS)).is_ok());
    let long = ArtifactSignature::try_from(repeated('0', MAX_SIGNATURE_CHARS.saturating_add(2)));
    assert!(matches!(long, Err(FieldError::TooLong { .. })), "{long:?}");

    let widest = format!(
        "{SYMBOL_PREFIX}{}",
        repeated('A', MAX_SYMBOL_BYTES.saturating_sub(SYMBOL_PREFIX.len()))
    );
    assert_eq!(widest.len(), MAX_SYMBOL_BYTES);
    assert!(ConfigSymbol::try_from(widest).is_ok());
    let wider = format!("{SYMBOL_PREFIX}{}", repeated('A', MAX_SYMBOL_BYTES));
    assert!(matches!(
        ConfigSymbol::try_from(wider),
        Err(FieldError::TooLong { .. })
    ));
}

/// Boundary: a release is at least itself, and a missing component is zero.
#[test]
fn a_release_is_at_least_itself_and_a_missing_component_reads_as_zero() -> Result<(), FieldError> {
    let exact = KernelRelease::try_from("7.3".to_owned())?;
    assert!(exact.at_least(&exact));
    let padded = KernelRelease::try_from("7.3.0".to_owned())?;
    assert!(exact.at_least(&padded));
    assert!(padded.at_least(&exact));
    let patched = KernelRelease::try_from("7.3.1".to_owned())?;
    assert!(patched.at_least(&exact));
    assert!(!exact.at_least(&patched));
    Ok(())
}

/// Boundary: a release longer than the component bound still compares.
///
/// The comparison reads at most [`MAX_RELEASE_COMPONENTS`] components, so a
/// pathological release string is bounded rather than refused mid-check.
#[test]
fn a_release_past_the_component_bound_still_compares() -> Result<(), FieldError> {
    let deep: String = vec!["1"; MAX_RELEASE_COMPONENTS.saturating_add(4)].join(".");
    let left = KernelRelease::try_from(deep.clone())?;
    let right = KernelRelease::try_from(deep)?;
    assert!(left.at_least(&right));
    let bigger = KernelRelease::try_from("2.0".to_owned())?;
    assert!(bigger.at_least(&left));
    assert!(!left.at_least(&bigger));
    Ok(())
}
