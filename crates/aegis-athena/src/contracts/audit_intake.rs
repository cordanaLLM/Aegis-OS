// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Consuming the M14 signed audit record on edge
//! `AUDIT_RECONSTRUCTIVE_CANDIDATE`.
//!
//! The record type is [`aegis_justitia::SignedAuditRecord`], typed at
//! milestone M14, and it is **not** redefined here. This module decodes
//! through it, so every rule M14 wrote -- the version tag, the genesis rule,
//! the refusal-needs-a-reason rule, the unsigned-record refusal, the
//! single-variant hash algorithm -- applies to a record arriving at P16
//! exactly as it applied when P06 produced it. A consumer that restated those
//! rules could drift from them; a consumer that decodes through the producer's
//! type cannot.
//!
//! # What this side adds
//!
//! One rule, and it belongs here rather than upstream: which recorded decision
//! admits a promotion. P06 records approvals and refusals alike, and both are
//! valid records; P16 may promote a candidate on the first and must not on the
//! second. [`AuditIntake::admits_promotion`] is that rule, and
//! [`AuditIntake::accept_for_promotion`] is the refusal that enforces it.

use aegis_justitia::{Digest32, RecordStatus, Sequence, SignedAuditRecord, UnixSeconds};

use crate::contracts::{ContractError, Correlation, SchemaId, peek_correlation};

/// One decoded audit record and what P16 reads out of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AuditIntake {
    record: SignedAuditRecord,
}

impl AuditIntake {
    /// The contract a refusal on this edge names.
    pub const SCHEMA: SchemaId = SchemaId::SignedAuditRecord;

    /// Decodes one record through the M14 schema.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::UpstreamAuditRecord`] when the M14 schema
    /// refuses the payload, carrying the correlation identifier this side
    /// could still read. The producer's refusal is not reclassified: this
    /// crate does not decide which of M14's rules a payload broke.
    pub fn decode(text: &str) -> Result<Self, ContractError> {
        match SignedAuditRecord::decode(text) {
            Ok(record) => Ok(Self { record }),
            Err(_) => Err(ContractError::UpstreamAuditRecord {
                correlation: Correlation::new(Self::SCHEMA, peek_correlation(text)),
            }),
        }
    }

    /// Wraps an already-decoded record.
    #[must_use]
    pub const fn from_record(record: SignedAuditRecord) -> Self {
        Self { record }
    }

    /// Returns the record itself, in M14's own type.
    #[must_use]
    pub const fn record(&self) -> &SignedAuditRecord {
        &self.record
    }

    /// Returns the record's position in the P06 chain.
    #[must_use]
    pub const fn sequence(&self) -> Sequence {
        self.record.sequence
    }

    /// Returns the record's own digest.
    #[must_use]
    pub const fn digest(&self) -> Digest32 {
        self.record.digest
    }

    /// Returns when the recorded decision was taken.
    #[must_use]
    pub const fn recorded_at(&self) -> UnixSeconds {
        self.record.recorded_at
    }

    /// Returns `true` when the record is the head of the P06 chain.
    ///
    /// The genesis case: sequence one with an all-zero predecessor link. M14's
    /// own validation already refuses every other pairing, so this is a read
    /// rather than a second check.
    #[must_use]
    pub fn is_chain_head(&self) -> bool {
        self.record.is_chain_head()
    }

    /// Returns `true` when the recorded decision admits a promotion.
    ///
    /// Only [`RecordStatus::Approved`] does. A pending record has not been
    /// decided, and the three refusal statuses are refusals: promoting on any
    /// of them would make the audit trail a formality rather than a gate.
    #[must_use]
    pub const fn admits_promotion(&self) -> bool {
        matches!(self.record.status, RecordStatus::Approved)
    }

    /// Returns the intake when the record admits a promotion.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::PromotionNotAdmitted`] naming the status that
    /// refused it.
    pub fn accept_for_promotion(self) -> Result<Self, ContractError> {
        if self.admits_promotion() {
            return Ok(self);
        }
        Err(ContractError::PromotionNotAdmitted {
            correlation: Correlation::new(Self::SCHEMA, None),
            status: self.record.status.name(),
        })
    }
}
