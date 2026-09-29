// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The payloads Aegis issues name Aegis requirements (decision D103).
//!
//! Since D103 the schema's `required-by` admits any identifier of nucleus
//! ADR-0007's form, so a producer's own `FLAVOR-BASE` decodes under the shared
//! schema id. The decoder no longer holds this repository's payloads to the
//! `REQ-` form; this file does. Every `required-by` anywhere in a payload Aegis
//! publishes under `build/` must be an Aegis identifier
//! (`RequirementId::is_aegis_requirement`) with a row of its own in
//! `docs/roadmap/requirements.md`, so a reviewer can follow a Kconfig symbol
//! back to the requirement that asked for it.
//!
//! The sweep walks each payload's JSON rather than its decoded rows, so a
//! `required-by` added to the product input, or anywhere a later version puts
//! one, is covered the day it lands. The walk is a worklist with a scalar
//! bound, not recursion (HISS-01, HISS-02), and fails closed past the bound.

mod common;

use aegis_fabrica_defs::field::RequirementId;
use serde_json::Value;

use common::{Fallible, repository_root, reviewed_payload};

/// The payloads Aegis issues, under `build/`.
const ISSUED: [&str; 3] = [
    "kernel-requirement.json",
    "kernel-requirement.reference.json",
    "product-input.json",
];

/// The key whose values the sweep checks.
const KEY: &str = "required-by";

/// Scalar bound on the JSON values one sweep visits.
const MAX_NODES: usize = 4096;

/// Returns a string value as written, and any other value as its JSON text.
fn text_of(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_owned)
}

/// Returns every `required-by` string in `document`, or why the walk stopped.
///
/// A `required-by` that is not a string is returned as its JSON text, so the
/// check below refuses it rather than skipping it.
fn required_by_values(document: &Value) -> Result<Vec<String>, String> {
    let mut found = Vec::new();
    let mut pending = vec![document];
    for _ in 0..MAX_NODES {
        let Some(node) = pending.pop() else {
            return Ok(found);
        };
        match node {
            Value::Object(entries) => {
                found.extend(entries.get(KEY).map(text_of));
                pending.extend(entries.values());
            }
            Value::Array(items) => pending.extend(items),
            _ => {}
        }
    }
    if pending.is_empty() {
        return Ok(found);
    }
    Err(format!(
        "the payload holds more than {MAX_NODES} JSON values"
    ))
}

/// Returns what is wrong with one issued payload's requirement identifiers.
fn issued_problems(document: &Value, recorded: &str) -> Vec<String> {
    let values = match required_by_values(document) {
        Ok(values) => values,
        Err(problem) => return vec![problem],
    };
    let mut problems = Vec::new();
    for value in values {
        match RequirementId::try_from(value.clone()) {
            Ok(id) if !id.is_aegis_requirement() => {
                problems.push(format!("{value} is not an Aegis requirement identifier"));
            }
            Ok(_) if !recorded.contains(&format!("| {value} |")) => {
                problems.push(format!(
                    "{value} has no row in docs/roadmap/requirements.md"
                ));
            }
            Ok(_) => {}
            Err(error) => problems.push(error.to_string()),
        }
    }
    problems
}

/// The recorded requirement register.
fn recorded() -> Result<String, Box<dyn std::error::Error>> {
    Ok(std::fs::read_to_string(
        repository_root()
            .join("docs")
            .join("roadmap")
            .join("requirements.md"),
    )?)
}

/// One issued payload as JSON, with the `required-by` of its first feature replaced.
fn with_first_required_by(value: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let mut document: Value = serde_json::from_str(&reviewed_payload("kernel-requirement.json")?)?;
    *document
        .pointer_mut("/features/0/required-by")
        .ok_or("the reviewed requirement has no first feature")? = Value::from(value);
    Ok(document)
}

// --- Positive -------------------------------------------------------------

/// Positive: every `required-by` Aegis issues is a recorded Aegis requirement.
#[test]
fn every_required_by_aegis_issues_is_a_recorded_aegis_requirement() -> Fallible {
    let register = recorded()?;
    let mut checked = 0_usize;
    for name in ISSUED {
        let document: Value = serde_json::from_str(&reviewed_payload(name)?)?;
        let problems = issued_problems(&document, &register);
        assert_eq!(problems, Vec::<String>::new(), "{name}");
        checked = checked.saturating_add(required_by_values(&document)?.len());
    }
    assert!(checked >= 26, "the sweep read only {checked} identifiers");
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a producer's identifier decodes, yet an issued payload may not carry it.
///
/// This is the split D103 makes: the decoder accepts `FLAVOR-BASE`, and the
/// issued-payload rule refuses it by name.
#[test]
fn a_producer_identifier_decodes_but_is_refused_in_an_issued_payload() -> Fallible {
    let document = with_first_required_by("FLAVOR-BASE")?;
    let decoded =
        aegis_fabrica_defs::KernelRequirement::decode(&serde_json::to_string(&document)?)?;
    assert_eq!(
        decoded.features.first().map(|row| row.required_by.as_str()),
        Some("FLAVOR-BASE")
    );
    let problems = issued_problems(&document, &recorded()?);
    assert_eq!(
        problems,
        vec!["FLAVOR-BASE is not an Aegis requirement identifier".to_owned()]
    );
    Ok(())
}

/// Negative: an Aegis-shaped identifier with no recorded row is refused too.
#[test]
fn an_aegis_identifier_without_a_recorded_row_is_refused() -> Fallible {
    let document = with_first_required_by("REQ-P99-99")?;
    let problems = issued_problems(&document, &recorded()?);
    assert_eq!(
        problems,
        vec!["REQ-P99-99 has no row in docs/roadmap/requirements.md".to_owned()]
    );
    let prefix = with_first_required_by("REQ-P07-0")?;
    assert_eq!(issued_problems(&prefix, &recorded()?).len(), 1);
    let trailing = with_first_required_by("REQ-P07-")?;
    assert_eq!(
        issued_problems(&trailing, &recorded()?),
        vec!["REQ-P07- has no row in docs/roadmap/requirements.md".to_owned()]
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the walk is exact at its bound and fails closed one past it.
#[test]
fn the_walk_is_exact_at_its_bound_and_fails_closed_past_it() {
    let at_bound = Value::Array(vec![Value::Null; MAX_NODES.saturating_sub(1)]);
    assert_eq!(required_by_values(&at_bound), Ok(Vec::new()));
    let past = Value::Array(vec![Value::Null; MAX_NODES]);
    assert!(required_by_values(&past).is_err());
    let problems = issued_problems(&past, "");
    assert_eq!(problems.len(), 1, "{problems:?}");
}
