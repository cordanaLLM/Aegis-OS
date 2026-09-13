// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The Aegis product input manifest schema (epic E18-1).
//!
//! The positive case is not a hand-written fixture: it is the reviewed
//! `build/product-input.json`, validated against the reviewed definition files
//! `build/repart.d` and `build/sysupdate.d` that milestone M03 shipped. A
//! manifest that pointed at a definition set which does not parse, or which
//! does not meet a recorded requirement, fails here.

mod common;

use aegis_fabrica_defs::manifest::{
    BootKernel, BootKernelSource, DefinitionReferences, DistributionId, DistributionPin,
    MAX_BACKOFF_SECONDS, MAX_PACKAGES, MAX_RETRY_ATTEMPTS, PRODUCT_INPUT_TAG, ProductInputVersion,
    RetryPolicy,
};
use aegis_fabrica_defs::payload::MAX_PAYLOAD_BYTES;
use aegis_fabrica_defs::{Finding, MAX_DEFINITIONS, ManifestError, ProductInputManifest};

use common::{Fallible, reviewed_payload, reviewed_repart_set, reviewed_transfer_set};

/// The reviewed manifest, as this repository ships it.
fn shipped() -> Result<ProductInputManifest, Box<dyn std::error::Error>> {
    Ok(ProductInputManifest::decode(&reviewed_payload(
        "product-input.json",
    )?)?)
}

/// A manifest payload with one field replaced, for the refusal cases.
fn altered(field: &str, value: &str) -> Result<String, Box<dyn std::error::Error>> {
    let text = reviewed_payload("product-input.json")?;
    let mut document: serde_json::Value = serde_json::from_str(&text)?;
    let replacement: serde_json::Value = serde_json::from_str(value)?;
    document
        .as_object_mut()
        .ok_or("the reviewed manifest is not a JSON object")?
        .insert(field.to_owned(), replacement);
    Ok(serde_json::to_string(&document)?)
}

// --- Positive -------------------------------------------------------------

/// Positive: the reviewed manifest decodes with the identity fields stack.md asks for.
#[test]
fn the_reviewed_manifest_decodes_with_a_correlation_id_and_an_exact_revision() -> Fallible {
    let manifest = shipped()?;
    assert_eq!(manifest.schema, ProductInputVersion::V1);
    assert_eq!(ProductInputManifest::SCHEMA_TAG, PRODUCT_INPUT_TAG);
    assert_eq!(
        manifest.correlation_id.as_str(),
        "aegis-m18-product-input-0001"
    );
    assert_eq!(manifest.revision.as_str().len(), 40);
    assert_eq!(manifest.retries.max_attempts, 3);
    assert_eq!(manifest.retries.backoff_seconds, 30);
    Ok(())
}

/// Positive: the reviewed manifest carries the pinned distribution and kernel.
#[test]
fn the_reviewed_manifest_carries_the_pinned_distribution_and_kernel() -> Fallible {
    let manifest = shipped()?;
    assert_eq!(manifest.distribution.id, DistributionId::Arch);
    assert_eq!(manifest.distribution.snapshot.as_str(), "2026/09/13");
    assert_eq!(manifest.kernel.source, BootKernelSource::BuiltHere);
    assert_eq!(manifest.kernel.default_package.as_str(), "linux-rt");
    Ok(())
}

/// Positive: the reviewed manifest references the four configuration inputs.
#[test]
fn the_reviewed_manifest_references_the_four_configuration_inputs() -> Fallible {
    let manifest = shipped()?;
    assert_eq!(manifest.definitions.repart.as_str(), "build/repart.d");
    assert_eq!(manifest.definitions.sysupdate.as_str(), "build/sysupdate.d");
    assert_eq!(manifest.definitions.mkosi.as_str(), "build/mkosi.conf");
    assert_eq!(
        manifest.definitions.kernel_requirement.as_str(),
        "build/kernel-requirement.json"
    );
    Ok(())
}

/// Positive: the reviewed manifest validates the M03 definition files.
///
/// This is the exit criterion. The manifest is not checked against a
/// description of `build/repart.d` and `build/sysupdate.d`; the real files are
/// read and run through the same parsers the M03 gate uses.
#[test]
fn the_reviewed_manifest_validates_the_shipped_definition_files() -> Fallible {
    let manifest = shipped()?;
    let repart = reviewed_repart_set()?;
    let transfers = reviewed_transfer_set()?;
    assert_eq!(repart.len(), 5);
    assert!(!transfers.is_empty());
    manifest.validate_definitions(&repart, &transfers)?;
    Ok(())
}

/// Positive: a validated manifest re-encodes and decodes back to itself.
#[test]
fn a_validated_manifest_round_trips_through_its_own_encoding() -> Fallible {
    let manifest = shipped()?;
    let encoded = manifest.encode()?;
    assert_eq!(ProductInputManifest::decode(&encoded)?, manifest);
    assert!(encoded.contains(PRODUCT_INPUT_TAG));
    Ok(())
}

/// Positive: all three of decision D07's answers are representable.
///
/// D07 settles the boot kernel as "both": a pinned distribution package by
/// default, with a producer artifact overriding it once the M09 contract
/// delivers one, and D70 adds the kernel this repository builds meanwhile. A
/// manifest that could spell only one of the three would have decided D07 by
/// omission.
#[test]
fn every_boot_kernel_source_decision_d07_admits_is_representable() -> Fallible {
    let mut manifest = shipped()?;
    for source in [
        BootKernelSource::DistributionPackage,
        BootKernelSource::BuiltHere,
        BootKernelSource::ProducerArtifact,
    ] {
        manifest.kernel = BootKernel {
            source,
            default_package: manifest.kernel.default_package.clone(),
        };
        let decoded = ProductInputManifest::decode(&manifest.encode()?)?;
        assert_eq!(decoded.kernel.source, source);
    }
    Ok(())
}

/// Positive: a manifest assembled from parts encodes to the reviewed payload.
///
/// Building the same manifest by hand and comparing it with the reviewed file
/// is what keeps `build/product-input.json` a payload of this schema rather
/// than a document that merely looks like one.
#[test]
fn a_manifest_assembled_from_parts_equals_the_reviewed_payload() -> Fallible {
    let reviewed = shipped()?;
    let assembled = ProductInputManifest {
        schema: ProductInputVersion::V1,
        correlation_id: "aegis-m18-product-input-0001".to_owned().try_into()?,
        revision: reviewed.revision.clone(),
        distribution: DistributionPin {
            id: DistributionId::Arch,
            snapshot: "2026/09/13".to_owned().try_into()?,
        },
        definitions: DefinitionReferences {
            repart: "build/repart.d".to_owned().try_into()?,
            sysupdate: "build/sysupdate.d".to_owned().try_into()?,
            mkosi: "build/mkosi.conf".to_owned().try_into()?,
            kernel_requirement: "build/kernel-requirement.json".to_owned().try_into()?,
        },
        packages: vec![
            "linux-rt".to_owned().try_into()?,
            "systemd".to_owned().try_into()?,
            "systemd-ukify".to_owned().try_into()?,
        ],
        kernel: BootKernel {
            source: BootKernelSource::BuiltHere,
            default_package: "linux-rt".to_owned().try_into()?,
        },
        retries: RetryPolicy {
            max_attempts: 3,
            backoff_seconds: 30,
        },
    };
    assert_eq!(assembled, reviewed);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a manifest without a correlation identifier does not decode.
#[test]
fn a_manifest_without_a_correlation_id_is_refused() -> Fallible {
    let text = reviewed_payload("product-input.json")?;
    let mut document: serde_json::Value = serde_json::from_str(&text)?;
    document
        .as_object_mut()
        .ok_or("the reviewed manifest is not a JSON object")?
        .remove("correlation-id");
    let refused = ProductInputManifest::decode(&serde_json::to_string(&document)?);
    assert!(
        matches!(refused, Err(ManifestError::Malformed { .. })),
        "{refused:?}"
    );
    Ok(())
}

/// Negative: a manifest whose revision is not exact does not decode.
#[test]
fn a_manifest_without_an_exact_revision_is_refused() -> Fallible {
    for moving in ["\"main\"", "\"HEAD\"", "\"61f2fe1\""] {
        let refused = ProductInputManifest::decode(&altered("revision", moving)?);
        assert!(
            matches!(refused, Err(ManifestError::Malformed { .. })),
            "{moving} was accepted: {refused:?}"
        );
    }
    Ok(())
}

/// Negative: an unpinned distribution release does not decode (decision D18).
#[test]
fn a_manifest_with_an_unpinned_release_is_refused() -> Fallible {
    let unpinned = altered(
        "distribution",
        "{\"id\": \"arch\", \"snapshot\": \"latest\"}",
    )?;
    let refused = ProductInputManifest::decode(&unpinned);
    assert!(
        matches!(refused, Err(ManifestError::Malformed { .. })),
        "{refused:?}"
    );
    Ok(())
}

/// Negative: a payload naming another contract version is told so by name.
///
/// The refusal is [`ManifestError::UnknownVersion`] rather than a generic
/// malformed input, because the two have different fixes.
#[test]
fn a_payload_naming_another_contract_version_is_refused_as_an_unknown_version() -> Fallible {
    let other = altered("schema", "\"aegis.p01.product-input.v2\"")?;
    let refused = ProductInputManifest::decode(&other);
    let Err(ManifestError::UnknownVersion { declared, expected }) = refused else {
        return Err(format!("expected an unknown-version refusal, got {refused:?}").into());
    };
    assert_eq!(declared, "aegis.p01.product-input.v2");
    assert_eq!(expected, PRODUCT_INPUT_TAG);
    Ok(())
}

/// Negative: an unknown field is refused rather than ignored.
#[test]
fn an_unknown_field_is_refused_rather_than_ignored() -> Fallible {
    let extra = altered("mirror", "\"https://example.invalid\"")?;
    let refused = ProductInputManifest::decode(&extra);
    assert!(
        matches!(refused, Err(ManifestError::Malformed { .. })),
        "{refused:?}"
    );
    Ok(())
}

/// Negative: a manifest pointing at a broken definition set is refused.
///
/// The drop-in below is the one systemd itself refuses: a `[Partition]` with
/// no `Type=`. The manifest carries no hint of it, so the refusal can only
/// come from the definition text the caller supplied.
#[test]
fn a_manifest_is_refused_when_its_definition_set_does_not_parse() -> Fallible {
    let manifest = shipped()?;
    let broken = vec![(
        "50-no-type.conf".to_owned(),
        "[Partition]\nLabel=nowhere\n".to_owned(),
    )];
    let refused = manifest.validate_definitions(&broken, &reviewed_transfer_set()?);
    assert!(
        matches!(refused, Err(ManifestError::DefinitionRefused { .. })),
        "{refused:?}"
    );
    Ok(())
}

/// Negative: a definition set that parses but is not A/B is refused.
///
/// Dropping the alternate root slot from the reviewed set leaves four files
/// that all parse. What fails is the recorded requirement, and the manifest
/// reports the [`Finding`] rather than a generic refusal.
#[test]
fn a_manifest_is_refused_when_its_definition_set_is_not_ab() -> Fallible {
    let manifest = shipped()?;
    let mut repart = reviewed_repart_set()?;
    assert_eq!(repart.len(), 5, "the reviewed repart set is five drop-ins");
    repart.retain(|(name, _)| name != "10-root-b.conf");
    assert_eq!(repart.len(), 4, "dropping one of five leaves four");
    let refused = manifest.validate_definitions(&repart, &reviewed_transfer_set()?);
    let Err(ManifestError::RequirementNotMet { path, finding }) = refused else {
        return Err(format!("expected a requirement refusal, got {refused:?}").into());
    };
    assert_eq!(path, "build/repart.d");
    assert_eq!(finding, Finding::MissingAlternateRootSlot { slots: 1 });
    Ok(())
}

/// Negative: an empty definition set is refused, not read as "nothing to do".
#[test]
fn an_empty_definition_set_is_refused() -> Fallible {
    let manifest = shipped()?;
    let refused = manifest.validate_definitions(&[], &reviewed_transfer_set()?);
    assert!(
        matches!(
            refused,
            Err(ManifestError::NoDefinitions { what: "repart" })
        ),
        "{refused:?}"
    );
    let no_transfer = manifest.validate_definitions(&reviewed_repart_set()?, &[]);
    assert!(
        matches!(
            no_transfer,
            Err(ManifestError::NoDefinitions { what: "sysupdate" })
        ),
        "{no_transfer:?}"
    );
    Ok(())
}

/// Negative: a manifest pinning no package at all is refused.
#[test]
fn a_manifest_pinning_no_package_is_refused() -> Fallible {
    let refused = ProductInputManifest::decode(&altered("packages", "[]")?);
    assert!(
        matches!(refused, Err(ManifestError::NoPackages)),
        "{refused:?}"
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a retry count at the bound is accepted and one above is refused.
#[test]
fn a_retry_count_at_the_bound_is_accepted_and_one_above_is_refused() -> Fallible {
    let mut manifest = shipped()?;
    manifest.retries = RetryPolicy {
        max_attempts: MAX_RETRY_ATTEMPTS,
        backoff_seconds: MAX_BACKOFF_SECONDS,
    };
    manifest.validate()?;

    manifest.retries.max_attempts = MAX_RETRY_ATTEMPTS.saturating_add(1);
    let over = manifest.validate();
    assert!(
        matches!(
            over,
            Err(ManifestError::RetryOutOfRange {
                what: "max-attempts",
                ..
            })
        ),
        "{over:?}"
    );

    manifest.retries.max_attempts = 0;
    assert!(manifest.validate().is_err());

    manifest.retries.max_attempts = 1;
    manifest.retries.backoff_seconds = MAX_BACKOFF_SECONDS.saturating_add(1);
    let backoff = manifest.validate();
    assert!(
        matches!(
            backoff,
            Err(ManifestError::RetryOutOfRange {
                what: "backoff-seconds",
                ..
            })
        ),
        "{backoff:?}"
    );
    Ok(())
}

/// Boundary: the package bound is exact, and one package past it is refused.
#[test]
fn the_package_bound_is_exact() -> Fallible {
    let mut manifest = shipped()?;
    let one = manifest
        .packages
        .first()
        .ok_or("the reviewed manifest pins no package")?
        .clone();
    manifest.packages = vec![one.clone(); MAX_PACKAGES];
    manifest.validate()?;
    manifest.packages.push(one);
    let over = manifest.validate();
    assert!(
        matches!(
            over,
            Err(ManifestError::TooMany {
                what: "packages",
                bound: MAX_PACKAGES
            })
        ),
        "{over:?}"
    );
    Ok(())
}

/// Boundary: the transfer bound is exact, and one transfer past it is refused.
///
/// The repart half has always answered [`ManifestError::TooMany`] past the
/// bound. The transfer half did not: it iterated `take(MAX_DEFINITIONS)` with
/// no guard, so every transfer from index 32 on was never parsed and a broken
/// one there was accepted in silence. The broken definition appended below is
/// what makes the refusal load-bearing rather than decorative.
#[test]
fn the_transfer_bound_is_exact() -> Fallible {
    let manifest = shipped()?;
    let repart = reviewed_repart_set()?;
    let one = reviewed_transfer_set()?
        .first()
        .ok_or("the reviewed transfer set is empty")?
        .clone();
    let at_bound = vec![one; MAX_DEFINITIONS];
    manifest.validate_definitions(&repart, &at_bound)?;

    let mut over = at_bound;
    over.push((
        "99-not-a-transfer.conf".to_owned(),
        "[Transfer]\nNotAKey=whatever\n".to_owned(),
    ));
    assert_eq!(over.len(), MAX_DEFINITIONS.saturating_add(1));
    let refused = manifest.validate_definitions(&repart, &over);
    assert!(
        matches!(
            refused,
            Err(ManifestError::TooMany {
                what: "sysupdate definitions",
                bound: MAX_DEFINITIONS
            })
        ),
        "{refused:?}"
    );
    Ok(())
}

/// Boundary: a payload one byte past the scalar bound is refused unparsed.
#[test]
fn a_payload_past_the_byte_bound_is_refused_before_it_is_parsed() {
    let oversized = "x".repeat(MAX_PAYLOAD_BYTES.saturating_add(1));
    let refused = ProductInputManifest::decode(&oversized);
    assert!(
        matches!(
            refused,
            Err(ManifestError::TooLong {
                bound: MAX_PAYLOAD_BYTES,
                ..
            })
        ),
        "{refused:?}"
    );
    let at_bound = ProductInputManifest::decode(&"x".repeat(MAX_PAYLOAD_BYTES));
    assert!(
        matches!(at_bound, Err(ManifestError::Malformed { .. })),
        "a payload at the bound must reach the parser, got {at_bound:?}"
    );
}
