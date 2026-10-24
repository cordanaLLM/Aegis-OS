// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Audit ledger and hash-linked chain state.

pub mod chain;

#[cfg(feature = "jsonl")]
pub use amussis_decision::ledger::jsonl;

pub use amussis_decision::ledger::*;
pub use chain::{Chain, ChainRecord, Timestamped};

impl ChainRecord for amussis_decision::ledger::AuditRecord {
    type Draft = amussis_decision::ledger::RecordDraft;

    fn algorithm(&self) -> amussis_decision::ledger::hash::HashAlgorithm {
        self.body().algorithm()
    }

    fn sequence(&self) -> amussis_decision::ledger::Sequence {
        self.body().sequence()
    }

    fn previous(&self) -> amussis_decision::ledger::hash::Digest32 {
        self.body().previous()
    }

    fn at(&self) -> amussis_decision::time::UnixSeconds {
        self.body().draft().at
    }

    fn digest(&self) -> amussis_decision::ledger::hash::Digest32 {
        // The inherent accessor of the library's record, not this trait method.
        amussis_decision::ledger::AuditRecord::digest(self)
    }

    fn verify<H: amussis_decision::ledger::hash::LedgerHasher>(
        &self,
    ) -> Result<amussis_decision::ledger::hash::Digest32, amussis_decision::ledger::LedgerError>
    {
        self.body().digest::<H>()
    }
}

impl Timestamped for amussis_decision::ledger::RecordDraft {
    fn at(&self) -> amussis_decision::time::UnixSeconds {
        self.at
    }
}
