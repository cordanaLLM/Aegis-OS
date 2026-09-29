// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The published JSON Schemas (decision D105) against the types they come from.
//!
//! The committed files under `build/` are what a consumer in another language
//! validates against, so the first test here is the drift check: each file must
//! be byte for byte what `aegis_fabrica_defs::schema` renders from the types
//! today. After a deliberate change to a contract type, regenerate both with
//!
//! ```sh
//! AEGIS_WRITE_SCHEMAS=1 cargo test --locked -p aegis-fabrica-defs --test json_schema
//! ```
//!
//! and review the diff; without the variable the test only reads. The other
//! tests hold the properties D105 names -- a closed object at every level, the
//! required fields, the identifier patterns and the decoder's bounds -- so a
//! schemars upgrade that changed how they are expressed fails here by name
//! rather than as an unexplained diff.

mod common;

use aegis_fabrica_defs::field::{MAX_FIELD_BYTES, MAX_SYMBOL_BYTES};
use aegis_fabrica_defs::kernel::{MAX_ARCHITECTURES, MAX_FEATURES};
use aegis_fabrica_defs::manifest::{MAX_BACKOFF_SECONDS, MAX_PACKAGES, MAX_RETRY_ATTEMPTS};
use aegis_fabrica_defs::schema::{
    KERNEL_REQUIREMENT_SCHEMA_PATH, PRODUCT_INPUT_SCHEMA_PATH, kernel_requirement_schema,
    product_input_schema,
};
use serde_json::Value;

use common::{Fallible, repository_root};

/// Set to write the generated schemas over the committed ones.
const WRITE_VARIABLE: &str = "AEGIS_WRITE_SCHEMAS";

/// The regeneration command a drift failure names.
const REGENERATE: &str =
    "AEGIS_WRITE_SCHEMAS=1 cargo test --locked -p aegis-fabrica-defs --test json_schema";

/// The struct definitions each schema carries, with the fields each requires.
const KERNEL_OBJECTS: [(&str, &[&str]); 4] = [
    (
        "",
        &[
            "abi",
            "architectures",
            "correlation-id",
            "features",
            "schema",
        ],
    ),
    ("/$defs/KernelAbi", &["minimum-release"]),
    (
        "/$defs/FeatureRequirement",
        &["probe", "required-by", "state", "symbol"],
    ),
    ("/$defs/ArtifactExpectation", &["digest"]),
];

/// The struct definitions the product input schema carries.
const PRODUCT_OBJECTS: [(&str, &[&str]); 5] = [
    (
        "",
        &[
            "correlation-id",
            "definitions",
            "distribution",
            "kernel",
            "packages",
            "retries",
            "revision",
            "schema",
        ],
    ),
    ("/$defs/DistributionPin", &["id", "snapshot"]),
    (
        "/$defs/DefinitionReferences",
        &["kernel-requirement", "mkosi", "repart", "sysupdate"],
    ),
    ("/$defs/BootKernel", &["default-package", "source"]),
    ("/$defs/RetryPolicy", &["backoff-seconds", "max-attempts"]),
];

/// Returns why `committed` is not `generated`, naming the file and the cure.
fn drift(relative: &str, committed: &str, generated: &str) -> Option<String> {
    if committed.replace("\r\n", "\n") == generated {
        return None;
    }
    Some(format!(
        "{relative} is not what the contract types generate; regenerate it with `{REGENERATE}` \
         and review the diff"
    ))
}

/// Reads one committed schema, writing the generated one first when asked to.
fn committed(relative: &str, generated: &str) -> Result<String, Box<dyn std::error::Error>> {
    let path = repository_root().join(relative);
    if std::env::var_os(WRITE_VARIABLE).is_some() {
        std::fs::write(&path, generated)?;
    }
    Ok(std::fs::read_to_string(&path)?)
}

/// Parses one generated schema.
fn parsed(text: &str) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(text)?)
}

/// Returns the value at `pointer`, or an error naming it.
fn at<'a>(schema: &'a Value, pointer: &str) -> Result<&'a Value, Box<dyn std::error::Error>> {
    schema
        .pointer(pointer)
        .ok_or_else(|| format!("the schema has nothing at {pointer}").into())
}

/// Checks that each listed definition is a closed object with exactly its required fields.
fn check_objects(schema: &Value, objects: &[(&str, &[&str])]) -> Fallible {
    for (pointer, required) in objects {
        assert_eq!(
            at(schema, &format!("{pointer}/type"))?,
            "object",
            "{pointer}"
        );
        assert_eq!(
            at(schema, &format!("{pointer}/additionalProperties"))?,
            &Value::Bool(false),
            "{pointer} admits unknown fields"
        );
        let mut listed: Vec<&str> = at(schema, &format!("{pointer}/required"))?
            .as_array()
            .ok_or("required is not a list")?
            .iter()
            .filter_map(Value::as_str)
            .collect();
        listed.sort_unstable();
        assert_eq!(&listed, required, "{pointer}");
    }
    Ok(())
}

// --- Positive -------------------------------------------------------------

/// Positive: each committed schema is byte for byte what the types generate.
#[test]
fn each_committed_schema_is_what_the_types_generate() -> Fallible {
    for (relative, generated) in [
        (KERNEL_REQUIREMENT_SCHEMA_PATH, kernel_requirement_schema()?),
        (PRODUCT_INPUT_SCHEMA_PATH, product_input_schema()?),
    ] {
        let text = committed(relative, &generated)?;
        if let Some(problem) = drift(relative, &text, &generated) {
            return Err(problem.into());
        }
    }
    Ok(())
}

/// Positive: every struct of both contracts is a closed object with its required fields.
#[test]
fn every_struct_is_a_closed_object_with_its_required_fields() -> Fallible {
    check_objects(&parsed(&kernel_requirement_schema()?)?, &KERNEL_OBJECTS)?;
    check_objects(&parsed(&product_input_schema()?)?, &PRODUCT_OBJECTS)
}

/// Positive: the identifier patterns are published where the fields are.
#[test]
fn the_identifier_patterns_are_published() -> Fallible {
    let kernel = parsed(&kernel_requirement_schema()?)?;
    let feature = "/$defs/FeatureRequirement/properties";
    for (pointer, pattern) in [
        ("/properties/correlation-id", "^[A-Za-z0-9._:-]+$"),
        (
            &format!("{feature}/required-by"),
            concat!(
                "^(?:[A-Z][A-Z0-9-]{0,2}|[A-Z][A-Z0-9-]{4,}|[A-QS-Z][A-Z0-9-]{3}",
                "|R[A-DF-Z0-9-][A-Z0-9-]{2}|RE[A-PR-Z0-9-][A-Z0-9-]|REQ[A-Z0-9])$",
            ),
        ),
        (&format!("{feature}/symbol"), "^CONFIG_[A-Z0-9_]+$"),
    ] {
        assert_eq!(at(&kernel, &format!("{pointer}/pattern"))?, pattern);
        assert_eq!(at(&kernel, &format!("{pointer}/type"))?, "string");
    }
    let product = parsed(&product_input_schema()?)?;
    assert_eq!(
        at(&product, "/properties/revision/pattern")?,
        "^[0-9a-f]{40}$"
    );
    assert_eq!(
        at(
            &product,
            "/$defs/DistributionPin/properties/snapshot/pattern"
        )?,
        "^[0-9]{4}/[0-9]{2}/[0-9]{2}$"
    );
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a committed schema one byte away from the generated one is drift.
///
/// The failure names the file and the command that regenerates it, so the
/// person who changed a type is told what to run rather than shown a diff.
#[test]
fn a_schema_that_differs_by_one_byte_is_drift() -> Fallible {
    let generated = kernel_requirement_schema()?;
    let stale = generated.replacen("\"object\"", "\"array\"", 1);
    let problem = drift(KERNEL_REQUIREMENT_SCHEMA_PATH, &stale, &generated)
        .ok_or("a changed schema was not reported as drift")?;
    assert!(
        problem.contains(KERNEL_REQUIREMENT_SCHEMA_PATH),
        "{problem}"
    );
    assert!(problem.contains(REGENERATE), "{problem}");
    assert_eq!(
        drift(PRODUCT_INPUT_SCHEMA_PATH, &generated, &generated),
        None
    );
    Ok(())
}

/// Negative: no contract struct is published as an array.
///
/// The array form is what the decoder refuses (see `array_form.rs`); the
/// schema must not describe it either.
#[test]
fn no_contract_struct_is_published_as_an_array() -> Fallible {
    for text in [kernel_requirement_schema()?, product_input_schema()?] {
        let schema = parsed(&text)?;
        let definitions = at(&schema, "/$defs")?
            .as_object()
            .ok_or("$defs is not an object")?;
        assert_eq!(at(&schema, "/type")?, "object");
        for (name, definition) in definitions {
            assert_ne!(
                definition.get("type"),
                Some(&Value::from("array")),
                "{name}"
            );
        }
    }
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the list, length and range bounds are the decoder's own constants.
#[test]
fn the_bounds_are_the_decoders_own_constants() -> Fallible {
    let kernel = parsed(&kernel_requirement_schema()?)?;
    for (pointer, expected) in [
        ("/properties/features/minItems", 1),
        ("/properties/features/maxItems", MAX_FEATURES),
        ("/properties/architectures/minItems", 1),
        ("/properties/architectures/maxItems", MAX_ARCHITECTURES),
        ("/properties/correlation-id/maxLength", MAX_FIELD_BYTES),
        (
            "/$defs/FeatureRequirement/properties/required-by/maxLength",
            MAX_FIELD_BYTES,
        ),
        (
            "/$defs/FeatureRequirement/properties/symbol/maxLength",
            MAX_SYMBOL_BYTES,
        ),
    ] {
        assert_eq!(at(&kernel, pointer)?, &Value::from(expected), "{pointer}");
    }
    let product = parsed(&product_input_schema()?)?;
    let retries = "/$defs/RetryPolicy/properties";
    for (pointer, expected) in [
        ("/properties/packages/minItems".to_owned(), 1),
        ("/properties/packages/maxItems".to_owned(), MAX_PACKAGES),
        (format!("{retries}/max-attempts/minimum"), 1),
        (
            format!("{retries}/max-attempts/maximum"),
            usize::try_from(MAX_RETRY_ATTEMPTS)?,
        ),
        (format!("{retries}/backoff-seconds/minimum"), 1),
        (
            format!("{retries}/backoff-seconds/maximum"),
            usize::try_from(MAX_BACKOFF_SECONDS)?,
        ),
    ] {
        assert_eq!(at(&product, &pointer)?, &Value::from(expected), "{pointer}");
    }
    Ok(())
}
