// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The two inbound consumers, typed on the producers' own Rust contracts
//! (E16-2).
//!
//! | Method (the graph edge) | Producer | Decoded into |
//! | :-- | :-- | :-- |
//! | `DISPATCH_DECISION_REQUEST` | P06 Justitia | [`aegis_justitia::DecisionRequest`] (M14) |
//! | `EMIT_CARBON_TELEMETRY` | P13 Tellus | [`aegis_tellus::CarbonTelemetry`] (M16) |
//!
//! There is no schema file, no generated binding and no copy of either type
//! in a second language: a message's `params` is handed, as JSON text, to the
//! producer crate's own `decode`, which checks the version tag, the edge and
//! every bound the producer declares. A refusal comes back as the producer's
//! own typed error inside [`ConsumeError`], so an unknown schema version reads
//! as `ContractError::UnknownVersion` from the crate that owns the schema.
//!
//! D31 is still open, so neither payload carries an emission interval or a
//! delivery deadline, and nothing here assumes one.

use serde_json::Value;
use thiserror::Error;

use aegis_justitia::DecisionRequest;
use aegis_tellus::CarbonTelemetry;

use crate::jsonrpc::{INVALID_PARAMS, METHOD_NOT_FOUND};

/// The method P06 dispatches a decision request with: its graph edge.
pub const DISPATCH_DECISION_REQUEST: &str = "DISPATCH_DECISION_REQUEST";

/// The method P13 emits a carbon telemetry update with: its graph edge.
pub const EMIT_CARBON_TELEMETRY: &str = "EMIT_CARBON_TELEMETRY";

/// A decoded inbound message.
#[derive(Debug, Clone, PartialEq)]
pub enum Inbound {
    /// A decision request from P06, as `aegis-justitia` decoded it; boxed,
    /// because it is five times the size of the telemetry update.
    Decision(Box<DecisionRequest>),
    /// A carbon telemetry update from P13, as `aegis-tellus` decoded it.
    Telemetry(CarbonTelemetry),
}

/// Why a message's method or parameters were refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConsumeError {
    /// The method names neither inbound edge.
    #[error("no inbound edge is named {0:?}")]
    UnknownMethod(String),
    /// `params` is absent or not a JSON object.
    #[error("{method} carries its payload as a params object, and this message has none")]
    MissingParams {
        /// The method concerned.
        method: &'static str,
    },
    /// `aegis-justitia` refused the decision request.
    #[error("decision request refused: {0}")]
    Decision(aegis_justitia::ContractError),
    /// `aegis-tellus` refused the telemetry update.
    #[error("carbon telemetry refused: {0}")]
    Telemetry(aegis_tellus::ContractError),
}

impl ConsumeError {
    /// The JSON-RPC error code a Response to the refused message carries.
    #[must_use]
    pub const fn code(&self) -> i64 {
        match self {
            Self::UnknownMethod(_) => METHOD_NOT_FOUND,
            _ => INVALID_PARAMS,
        }
    }
}

/// Returns the params object as JSON text for a producer's `decode`.
fn payload(method: &'static str, params: Option<&Value>) -> Result<String, ConsumeError> {
    params
        .filter(|value| value.is_object())
        .map(Value::to_string)
        .ok_or(ConsumeError::MissingParams { method })
}

/// Decodes one message's parameters into the producer's type its method
/// names.
///
/// # Errors
///
/// Returns [`ConsumeError::UnknownMethod`] for a method that is neither
/// edge, [`ConsumeError::MissingParams`] without a params object, and the
/// producer's own refusal otherwise.
pub fn decode(method: &str, params: Option<&Value>) -> Result<Inbound, ConsumeError> {
    match method {
        DISPATCH_DECISION_REQUEST => {
            let text = payload(DISPATCH_DECISION_REQUEST, params)?;
            DecisionRequest::decode(&text)
                .map(|request| Inbound::Decision(Box::new(request)))
                .map_err(ConsumeError::Decision)
        }
        EMIT_CARBON_TELEMETRY => {
            let text = payload(EMIT_CARBON_TELEMETRY, params)?;
            CarbonTelemetry::decode(&text)
                .map(Inbound::Telemetry)
                .map_err(ConsumeError::Telemetry)
        }
        other => Err(ConsumeError::UnknownMethod(other.to_owned())),
    }
}
