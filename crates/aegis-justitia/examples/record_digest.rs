// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Prints the canonical SHA-256 digest of a fixed audit-record body.
//!
//! Milestone M20 E20-1 development fixture for TPM2 quote verification.
//! `record_digest` prints the digest of the approved record;
//! `record_digest other` prints the digest of the rejected record.

#![forbid(unsafe_code)]

use std::error::Error;
use std::io::Write;
use std::process::ExitCode;

use aegis_justitia::identity::{AgentId, IntentId, MakerId};
use aegis_justitia::killswitch::KillswitchState;
use aegis_justitia::ledger::hash::{Digest32, HashAlgorithm};
use aegis_justitia::ledger::sha256::Sha256Hasher;
use aegis_justitia::ledger::{
    RecordBody, RecordDraft, RecordPrincipals, RecordStatus, RecordSubject, Sequence,
};
use aegis_justitia::risk::{ActionType, RiskLevel};
use aegis_justitia::time::UnixSeconds;

const USAGE: &str = "usage: record_digest [other]";

fn build_draft(status: RecordStatus) -> Result<RecordDraft, Box<dyn Error>> {
    Ok(RecordDraft {
        subject: Some(RecordSubject {
            intent_id: IntentId::parse("intent-e20-1")?,
            agent: AgentId::parse("agent-01")?,
            action_type: ActionType::FileModification,
        }),
        principals: Some(RecordPrincipals {
            level: RiskLevel::LOWEST,
            maker: MakerId::parse("maker-01")?,
            checkers: None,
        }),
        status,
        block_reason: None,
        killswitch: KillswitchState::Armed,
        at: UnixSeconds::new(1_760_000_000),
    })
}

fn main() -> Result<ExitCode, Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).take(2).collect();
    let status = match args.as_slice() {
        [] => RecordStatus::Approved,
        [arg] if arg == "other" => RecordStatus::Rejected,
        _ => {
            let _ = writeln!(std::io::stderr().lock(), "{USAGE}");
            return Ok(ExitCode::from(64));
        }
    };

    let draft = build_draft(status)?;
    let body = RecordBody::from_parts(
        Sequence::new(0),
        Digest32::GENESIS,
        HashAlgorithm::Sha256,
        draft,
    );
    let digest = body.digest::<Sha256Hasher>()?;
    let _ = writeln!(std::io::stdout().lock(), "DIGEST {digest}");
    Ok(ExitCode::SUCCESS)
}
