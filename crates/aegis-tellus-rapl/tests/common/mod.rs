// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Readings documents shared by the M21 RAPL tests.
//!
//! [`RECORDED_RUN`] is the document `make verify-workstation` wrote for run
//! `r20260929T200422-2476` on the reference profile, byte for byte in its
//! values; the others are built from it by changing one thing each.

#![allow(dead_code)]

use serde_json::{Value, json};

/// What every test in this suite returns.
pub type Fallible = Result<(), Box<dyn std::error::Error>>;

/// The recorded run's readings document.
pub const RECORDED_RUN: &str = r#"{
  "schema": "aegis.m21.rapl-readings.v1",
  "zone": "package-0\n",
  "max-energy-range-uj": "65532610987\n",
  "mode": "pair",
  "readings": [
    {"text": "45514845753\n", "monotonic-ns": 172106800168887},
    {"text": "46107552971\n", "monotonic-ns": 172111815220099}
  ],
  "unprivileged": {
    "error": "PermissionError: [Errno 13] Permission denied: '/sys/class/powercap/intel-rapl:0/energy_uj'",
    "monotonic-ns": 172106789021223
  }
}"#;

/// Returns the recorded run as a JSON value.
///
/// # Errors
///
/// Returns the parse failure, which cannot happen for the constant.
pub fn recorded() -> Result<Value, serde_json::Error> {
    serde_json::from_str(RECORDED_RUN)
}

/// Sets `key` on the object `document` to `new`; a non-object is left alone.
pub fn set(document: &mut Value, key: &str, new: Value) {
    if let Some(object) = document.as_object_mut() {
        object.insert(key.to_owned(), new);
    }
}

/// Sets `key` on the object at `outer` inside `document`.
pub fn set_in(document: &mut Value, outer: &str, key: &str, new: Value) {
    if let Some(inner) = document.get_mut(outer) {
        set(inner, key, new);
    }
}

/// Returns a reading of `text` at `nanos`.
#[must_use]
pub fn reading(text: &str, nanos: u64) -> Value {
    json!({"text": text, "monotonic-ns": nanos})
}

/// Returns a wrap-watch document over `values`, ten seconds apart.
#[must_use]
pub fn watch(values: &[u64]) -> Value {
    let readings: Vec<Value> = values
        .iter()
        .zip(0_u64..)
        .map(|(value, index)| {
            reading(
                &format!("{value}\n"),
                index.saturating_mul(10_000_000_000).saturating_add(1),
            )
        })
        .collect();
    json!({
        "schema": "aegis.m21.rapl-readings.v1",
        "zone": "package-0\n",
        "max-energy-range-uj": "65532610987\n",
        "mode": "wrap-watch",
        "readings": readings,
        "unprivileged": {"error": "PermissionError: [Errno 13] Permission denied", "monotonic-ns": 1},
    })
}
