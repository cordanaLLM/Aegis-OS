// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! What the host sends a guest: one candidate to evaluate.
//!
//! The answer travels as `aegis-vesta`'s
//! [`CandidateEvaluation`](aegis_vesta::CandidateEvaluation), the P10-to-P16
//! payload M06 typed. The question has no edge of its own in the graph of
//! record: it is P10 talking to its own sandbox, so it is versioned here, in
//! the crate that sends and receives it, and it is not a subsystem contract.
//!
//! The four metrics are the P16 Pareto gate's inputs, and the guest judges
//! them with [`aegis_athena::ParetoVerdict`], so a verdict is computed inside
//! the microVM rather than echoed back.

use aegis_athena::{CandidateMetrics, LatencyMs, MemoryMb, NullModelRetention, SciCarbonRate};
use aegis_vesta::{CorrelationId, Label, VmId, VmmIdentity};

use crate::error::SandboxError;

/// The request's schema tag.
pub const REQUEST_SCHEMA_TAG: &str = "aegis.m21.guest-evaluation-request.v1";

/// Scalar upper bound, in bytes, on one encoded request line.
pub const MAX_REQUEST_BYTES: usize = 1024;

/// The guest `AF_VSOCK` port the evaluation service listens on.
pub const EVALUATION_PORT: u32 = 5210;

/// The request versions this build admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum RequestVersion {
    /// Version 1, tagged [`REQUEST_SCHEMA_TAG`].
    #[serde(rename = "aegis.m21.guest-evaluation-request.v1")]
    V1,
}

/// The four Pareto inputs, as they travel.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct WireMetrics {
    /// Latency, in milliseconds.
    pub latency_ms: f64,
    /// Resident memory, in megabytes.
    pub memory_mb: f64,
    /// SCI carbon rate.
    pub carbon_rate: f64,
    /// Null-model retention.
    pub retention: f64,
}

impl WireMetrics {
    /// Validates the four values through P16's constructors.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxError::Athena`] for any value those refuse.
    pub fn metrics(&self) -> Result<CandidateMetrics, SandboxError> {
        Ok(CandidateMetrics::new(
            LatencyMs::new(self.latency_ms)?,
            MemoryMb::new(self.memory_mb)?,
            SciCarbonRate::new(self.carbon_rate)?,
            NullModelRetention::new(self.retention)?,
        ))
    }
}

/// One candidate for a guest to evaluate.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct EvaluationRequest {
    /// The request version.
    pub schema: RequestVersion,
    /// Threads the answer back to this request.
    pub correlation_id: CorrelationId,
    /// The candidate to evaluate.
    pub candidate: Label,
    /// The sandbox the host admitted the request into.
    pub vm_id: VmId,
    /// The monitor running that sandbox (decision D58).
    pub vmm: VmmIdentity,
    /// The candidate's metrics.
    pub metrics: WireMetrics,
}

impl EvaluationRequest {
    /// Encodes the request as one newline-terminated line.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxError::Athena`] for metrics P16 refuses, and
    /// [`SandboxError::Protocol`] when the line would pass
    /// [`MAX_REQUEST_BYTES`] or does not serialise.
    pub fn encode_line(&self) -> Result<String, SandboxError> {
        self.metrics.metrics()?;
        let mut line = serde_json::to_string(self).map_err(|error| {
            SandboxError::Protocol(format!("the request does not encode: {error}"))
        })?;
        line.push('\n');
        if line.len() > MAX_REQUEST_BYTES {
            return Err(SandboxError::Protocol(format!(
                "the request is {} bytes; the bound is {MAX_REQUEST_BYTES}",
                line.len()
            )));
        }
        Ok(line)
    }

    /// Decodes and validates one request line.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxError::Protocol`] for a line past the bound or one the
    /// schema refuses -- an unknown version, field or monitor tag included --
    /// and [`SandboxError::Athena`] for metrics P16 refuses.
    pub fn decode_line(text: &str) -> Result<Self, SandboxError> {
        if text.len() > MAX_REQUEST_BYTES {
            return Err(SandboxError::Protocol(format!(
                "the request is {} bytes; the bound is {MAX_REQUEST_BYTES}",
                text.len()
            )));
        }
        let body = text.strip_suffix('\n').unwrap_or(text);
        let request: Self = serde_json::from_str(body)
            .map_err(|error| SandboxError::Protocol(format!("the request is refused: {error}")))?;
        request.metrics.metrics()?;
        Ok(request)
    }
}
