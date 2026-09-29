// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The array form of every contract struct is refused (the 2026-09-29 defect).
//!
//! serde's derived decoder reads a struct from a JSON array of its values in
//! declaration order as readily as from an object, and `deny_unknown_fields`
//! has no key to refuse there. A probe on 2026-09-29 decoded the reviewed
//! kernel requirement written as
//! `["aegis.p01-nucleus.kernel-requirement.v1", "aegis-m18-...", ["x86-64"], {...}, [...]]`
//! and `KernelRequirement::decode` returned `Ok`. Every case below builds the
//! array form from the reviewed payloads themselves, so it is the most valid
//! array there can be: the right values, in the right order.

mod common;

use aegis_fabrica_defs::kernel::{ArtifactExpectation, FeatureRequirement, KernelAbi};
use aegis_fabrica_defs::manifest::{
    BootKernel, DefinitionReferences, DistributionPin, RetryPolicy,
};
use aegis_fabrica_defs::{KernelError, KernelRequirement, ManifestError, ProductInputManifest};
use serde_json::Value;

use common::{Fallible, reviewed_payload};

/// The kernel requirement's fields, in declaration order.
const KERNEL_FIELDS: [&str; 5] = [
    "schema",
    "correlation-id",
    "architectures",
    "abi",
    "features",
];

/// The product input manifest's fields, in declaration order.
const PRODUCT_FIELDS: [&str; 8] = [
    "schema",
    "correlation-id",
    "revision",
    "distribution",
    "definitions",
    "packages",
    "kernel",
    "retries",
];

/// What every refusal of the array form says it expected instead.
const EXPECTED: &str = "expected a JSON object";

/// Reads one reviewed payload as a JSON value.
fn reviewed(name: &str) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(&reviewed_payload(name)?)?)
}

/// Returns the values of `object` under `keys`, in that order, as one JSON array.
fn as_array(object: &Value, keys: &[&str]) -> Result<Value, Box<dyn std::error::Error>> {
    let mut values = Vec::new();
    for key in keys {
        values.push(object.get(*key).ok_or(format!("no {key}"))?.clone());
    }
    Ok(Value::Array(values))
}

/// Returns a copy of the value at `pointer`, or an error naming it.
fn pick(document: &Value, pointer: &str) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(document
        .pointer(pointer)
        .ok_or(format!("the reviewed payload has nothing at {pointer}"))?
        .clone())
}

/// Replaces the value at `pointer` inside `document` with `value`.
fn replaced(
    document: &Value,
    pointer: &str,
    value: Value,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut copy = document.clone();
    *copy
        .pointer_mut(pointer)
        .ok_or(format!("the reviewed payload has nothing at {pointer}"))? = value;
    Ok(serde_json::to_string(&copy)?)
}

/// Asserts that a kernel requirement decode was refused as malformed, for the object reason.
fn kernel_refused(text: &str) -> Fallible {
    let refused = KernelRequirement::decode(text);
    let Err(KernelError::Malformed { reason }) = refused else {
        return Err(format!("expected a malformed refusal, got {refused:?}").into());
    };
    assert!(reason.contains(EXPECTED), "{reason}");
    Ok(())
}

/// Asserts that a product input decode was refused as malformed, for the object reason.
fn product_refused(text: &str) -> Fallible {
    let refused = ProductInputManifest::decode(text);
    let Err(ManifestError::Malformed { reason }) = refused else {
        return Err(format!("expected a malformed refusal, got {refused:?}").into());
    };
    assert!(reason.contains(EXPECTED), "{reason}");
    Ok(())
}

// --- Positive -------------------------------------------------------------

/// Positive: every nested struct still decodes from its object form, directly.
///
/// The refusal is of a form, not of a type: serde's own entry point reaches the
/// same object-only decoder `decode` does.
#[test]
fn every_nested_struct_decodes_from_its_object_form() -> Fallible {
    let kernel = reviewed("kernel-requirement.json")?;
    let abi: KernelAbi = serde_json::from_value(pick(&kernel, "/abi")?)?;
    assert_eq!(abi.minimum_release.as_str(), "6.12");
    let feature: FeatureRequirement = serde_json::from_value(pick(&kernel, "/features/0")?)?;
    assert_eq!(feature.symbol.as_str(), "CONFIG_PREEMPT_RT");
    let artifact: ArtifactExpectation =
        serde_json::from_str(&format!("{{\"digest\": \"{}\"}}", "a".repeat(64)))?;
    assert_eq!(artifact.signature, None);

    let product = reviewed("product-input.json")?;
    let pin: DistributionPin = serde_json::from_value(pick(&product, "/distribution")?)?;
    assert_eq!(pin.snapshot.as_str(), "2026/09/13");
    let references: DefinitionReferences = serde_json::from_value(pick(&product, "/definitions")?)?;
    assert_eq!(references.mkosi.as_str(), "build/mkosi.conf");
    let kernel_identity: BootKernel = serde_json::from_value(pick(&product, "/kernel")?)?;
    assert_eq!(kernel_identity.default_package.as_str(), "linux-rt");
    let retries: RetryPolicy = serde_json::from_value(pick(&product, "/retries")?)?;
    assert_eq!(retries.max_attempts, 3);
    Ok(())
}

/// Positive: both reviewed payloads decode through serde's entry point as well.
#[test]
fn both_reviewed_payloads_decode_through_serde_directly() -> Fallible {
    let kernel: KernelRequirement =
        serde_json::from_str(&reviewed_payload("kernel-requirement.json")?)?;
    assert_eq!(kernel.features.len(), 14);
    let product: ProductInputManifest =
        serde_json::from_str(&reviewed_payload("product-input.json")?)?;
    assert_eq!(product.packages.len(), 3);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: the probe's form -- the reviewed requirement as an array -- is refused.
#[test]
fn the_kernel_requirement_as_an_array_is_refused() -> Fallible {
    let array = as_array(&reviewed("kernel-requirement.json")?, &KERNEL_FIELDS)?;
    let text = serde_json::to_string(&array)?;
    kernel_refused(&text)?;
    let direct = serde_json::from_str::<KernelRequirement>(&text);
    assert!(
        direct.is_err(),
        "serde's entry point decoded the array form"
    );
    Ok(())
}

/// Negative: each nested struct of the kernel requirement is refused as an array.
#[test]
fn each_nested_kernel_struct_is_refused_as_an_array() -> Fallible {
    let document = reviewed("kernel-requirement.json")?;
    let abi = as_array(
        &pick(&document, "/abi")?,
        &["minimum-release", "target-release"],
    )?;
    kernel_refused(&replaced(&document, "/abi", abi)?)?;
    let feature = as_array(
        &pick(&document, "/features/0")?,
        &["symbol", "state", "probe", "required-by"],
    )?;
    kernel_refused(&replaced(&document, "/features/0", feature)?)?;
    let mut with_artifact = document.clone();
    with_artifact
        .as_object_mut()
        .ok_or("the reviewed requirement is not an object")?
        .insert("artifact".to_owned(), Value::from(vec!["a".repeat(64)]));
    kernel_refused(&serde_json::to_string(&with_artifact)?)?;
    for (text, what) in [
        ("[\"6.12\"]", "KernelAbi"),
        ("[\"a\"]", "ArtifactExpectation"),
    ] {
        let abi = serde_json::from_str::<KernelAbi>(text);
        let artifact = serde_json::from_str::<ArtifactExpectation>(text);
        assert!(abi.is_err() && artifact.is_err(), "{what}: {text}");
    }
    Ok(())
}

/// Negative: the product input manifest as an array is refused.
#[test]
fn the_product_input_as_an_array_is_refused() -> Fallible {
    let array = as_array(&reviewed("product-input.json")?, &PRODUCT_FIELDS)?;
    let text = serde_json::to_string(&array)?;
    product_refused(&text)?;
    assert!(serde_json::from_str::<ProductInputManifest>(&text).is_err());
    Ok(())
}

/// Negative: each nested struct of the product input is refused as an array.
#[test]
fn each_nested_product_struct_is_refused_as_an_array() -> Fallible {
    let document = reviewed("product-input.json")?;
    let nested: [(&str, &[&str]); 4] = [
        ("/distribution", &["id", "snapshot"]),
        (
            "/definitions",
            &["repart", "sysupdate", "mkosi", "kernel-requirement"],
        ),
        ("/kernel", &["source", "default-package"]),
        ("/retries", &["max-attempts", "backoff-seconds"]),
    ];
    for (pointer, keys) in nested {
        let value = pick(&document, pointer)?;
        product_refused(&replaced(&document, pointer, as_array(&value, keys)?)?)?;
    }
    Ok(())
}

/// Negative: an array led by another version is malformed, not an unknown version.
///
/// The lenient version peek reads objects only, so the array form is refused
/// for its form whatever its first element claims. The one-element array is
/// the case the peek alone decides: a peek that read arrays would take its one
/// element as the claimed version and answer `UnknownVersion`, where a longer
/// array fails the peek on its extra elements either way.
#[test]
fn an_array_led_by_another_version_is_malformed_not_an_unknown_version() -> Fallible {
    kernel_refused("[\"aegis.p01-nucleus.kernel-requirement.v2\"]")?;
    product_refused("[\"aegis.p01.product-input.v2\"]")?;
    let mut kernel = as_array(&reviewed("kernel-requirement.json")?, &KERNEL_FIELDS)?;
    if let Some(first) = kernel.get_mut(0) {
        *first = Value::from("aegis.p01-nucleus.kernel-requirement.v2");
    }
    kernel_refused(&serde_json::to_string(&kernel)?)?;
    let mut product = as_array(&reviewed("product-input.json")?, &PRODUCT_FIELDS)?;
    if let Some(first) = product.get_mut(0) {
        *first = Value::from("aegis.p01.product-input.v2");
    }
    product_refused(&serde_json::to_string(&product)?)
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the smallest arrays and the smallest non-objects are refused too.
///
/// An empty array, a one-element array and `null` are each one step from the
/// probe's form; none of them decodes as a payload or as a nested struct.
#[test]
fn the_smallest_arrays_and_non_objects_are_refused() -> Fallible {
    for text in [
        "[]",
        "[\"aegis.p01-nucleus.kernel-requirement.v1\"]",
        "null",
    ] {
        kernel_refused(text)?;
    }
    for text in ["[]", "[\"aegis.p01.product-input.v1\"]", "null"] {
        product_refused(text)?;
    }
    let document = reviewed("product-input.json")?;
    product_refused(&replaced(&document, "/retries", Value::Array(Vec::new()))?)?;
    assert!(serde_json::from_str::<RetryPolicy>("[3, 30]").is_err());
    assert!(
        serde_json::from_str::<RetryPolicy>("{\"max-attempts\": 3, \"backoff-seconds\": 30}")
            .is_ok()
    );
    Ok(())
}
