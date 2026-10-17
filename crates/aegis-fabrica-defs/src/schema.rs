// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The published JSON Schemas of both M18 contracts (decision D105).
//!
//! A consumer in another language validates a payload against a JSON Schema,
//! not against this crate, so each contract is published as one: generated
//! from the Rust types with schemars, never written by hand, and committed at
//! [`KERNEL_REQUIREMENT_SCHEMA_PATH`] and [`PRODUCT_INPUT_SCHEMA_PATH`].
//! `tests/json_schema.rs` renders both and fails when a committed file is not
//! byte for byte what the types generate, so a change to a type that forgets
//! the schema fails the crate gate.
//!
//! Each schema is JSON Schema 2020-12, and states what the types state:
//! `type: object` with `additionalProperties: false` for every struct, at the
//! top and nested; the `required` fields; each bounded field's `pattern` and
//! `maxLength`; and the list and range bounds the decoder enforces. What a
//! JSON Schema cannot state stays the decoder's alone: a repeated Kconfig
//! symbol, a target release older than the minimum, and anything else
//! [`crate::KernelRequirement::validate`] and
//! [`crate::ProductInputManifest::validate`] refuse. A payload a schema
//! accepts can therefore still be refused by the decoder, never the reverse.

use crate::{KernelRequirement, ProductInputManifest};

/// Where the kernel requirement's JSON Schema is committed, from the repository root.
pub const KERNEL_REQUIREMENT_SCHEMA_PATH: &str = "build/kernel-requirement.schema.json";

/// Where the product input manifest's JSON Schema is committed, from the repository root.
pub const PRODUCT_INPUT_SCHEMA_PATH: &str = "build/product-input.schema.json";

/// Renders the kernel requirement's JSON Schema as the committed file holds it.
///
/// Two-space indented, every object's keys sorted, one trailing
/// newline.
///
/// # Errors
///
/// Returns the serializer's error when the schema cannot be rendered.
pub fn kernel_requirement_schema() -> Result<String, serde_json::Error> {
    render(&schemars::schema_for!(KernelRequirement))
}

/// Renders the product input manifest's JSON Schema as the committed file holds it.
///
/// Two-space indented, every object's keys sorted, one trailing
/// newline.
///
/// # Errors
///
/// Returns the serializer's error when the schema cannot be rendered.
pub fn product_input_schema() -> Result<String, serde_json::Error> {
    render(&schemars::schema_for!(ProductInputManifest))
}

/// Renders one generated schema in the committed file's form.
///
/// The keys are sorted at every level so the bytes do not depend on whether
/// `serde_json`'s `preserve_order` feature is on: Cargo unifies features over
/// the packages one build selects, so a workspace build that includes a crate
/// enabling it (gpui does) would otherwise write the keys in another order
/// than a build of this crate alone.
fn render(schema: &schemars::Schema) -> Result<String, serde_json::Error> {
    let mut value = serde_json::to_value(schema)?;
    value.sort_all_objects();
    let mut text = serde_json::to_string_pretty(&value)?;
    text.push('\n');
    Ok(text)
}
