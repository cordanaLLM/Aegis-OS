// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@pm.me>
// SPDX-License-Identifier: EUPL-1.2

//! The optional JSON Lines renderer, behind the `jsonl` feature.
//!
//! Asserts the JSON Lines feature wiring in Cargo.toml and ledger re-export.

#![cfg(feature = "jsonl")]

mod common;

use aegis_justitia::ledger::jsonl;
use aegis_justitia::{
    ActionType, AgentId, AuditLedger, BlockReason, HashAlgorithm, InMemorySink, IntentId,
    JustitiaError, KillswitchState, RecordDraft, RecordStatus, RecordSubject, Sha256Hasher,
    SignerBinding, UnixSeconds,
};

use common::{FIXTURE_AUDIT_BOUND, Fallible, FixtureError, FixtureSigner, io_deadline, named_for};

type Ledger = AuditLedger<Sha256Hasher, FixtureSigner, InMemorySink>;

/// Builds a fixture ledger holding the one record `draft` seals to.
fn ledger_of(draft: &RecordDraft) -> Result<Ledger, JustitiaError> {
    let mut ledger: Ledger = AuditLedger::with_bound(
        SignerBinding::Bound(FixtureSigner::new()?),
        InMemorySink::with_bound(FIXTURE_AUDIT_BOUND),
        io_deadline(),
        FIXTURE_AUDIT_BOUND,
    );
    ledger.append(*draft)?;
    Ok(ledger)
}

/// Builds a fixture ledger holding one record with `status` and `reason`.
fn one_record(status: RecordStatus, reason: Option<BlockReason>) -> Result<Ledger, JustitiaError> {
    ledger_of(&RecordDraft {
        subject: Some(RecordSubject {
            intent_id: IntentId::parse("intent-1")?,
            agent: AgentId::parse("agent-01")?,
            action_type: ActionType::FileModification,
        }),
        principals: named_for(status)?,
        status,
        block_reason: reason,
        killswitch: KillswitchState::Armed,
        at: UnixSeconds::new(100),
    })
}

// --- Positive -------------------------------------------------------------

/// Positive: a sealed record renders and parses back unchanged.
#[test]
fn a_sealed_record_renders_and_parses_back() -> Fallible<()> {
    let ledger = one_record(RecordStatus::Approved, None)?;
    let record = ledger.records().first().ok_or(FixtureError::NoRecord)?;
    let text = jsonl::render_line(record).map_err(|_| FixtureError::Render)?;
    assert!(text.contains("\"algorithm\":\"sha-256\""));
    assert!(text.contains("\"status\":\"approved\""));
    let parsed = jsonl::parse_line(&text).map_err(|_| FixtureError::Render)?;
    assert_eq!(parsed.sequence, 1);
    assert_eq!(parsed.algorithm, HashAlgorithm::Sha256);
    assert!(jsonl::uses_sha256(&parsed));
    assert!(!parsed.killswitch_engaged);
    assert_eq!(parsed.block_reason, None);
    Ok(())
}
