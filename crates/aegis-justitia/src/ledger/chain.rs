// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Generic hash-linked chain state and record contract.

use crate::ledger::hash::{Digest32, HashAlgorithm, LedgerHasher};
use crate::ledger::{LedgerError, RecordBody, Sequence, verify_link};
use crate::time::UnixSeconds;

/// Trait for drafts that carry a timestamp.
pub trait Timestamped {
    /// Returns when the draft was written.
    fn at(&self) -> UnixSeconds;
}

/// A record in a verified hash-linked chain.
pub trait ChainRecord: Sized {
    /// Associated draft type carried by the record body.
    type Draft: Timestamped;

    /// Returns the record body.
    fn body(&self) -> &RecordBody<Self::Draft>;

    /// Returns the stored digest.
    fn digest(&self) -> Digest32;

    /// Computes the digest of this record under `H`.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError`] on computation failure.
    fn verify<H: LedgerHasher>(&self) -> Result<Digest32, LedgerError>;

    /// Returns the algorithm named by the record.
    fn algorithm(&self) -> HashAlgorithm {
        self.body().algorithm()
    }

    /// Returns the sequence number.
    fn sequence(&self) -> Sequence {
        self.body().sequence()
    }

    /// Returns the predecessor link.
    fn previous(&self) -> Digest32 {
        self.body().previous()
    }

    /// Returns when the record was written.
    fn at(&self) -> UnixSeconds {
        self.body().draft().at()
    }

    /// Re-walks `records` under an explicit scalar bound.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError::LengthBoundExceeded`] when the chain is longer than
    /// `max`, and otherwise the first broken link.
    fn verify_chain_bounded<H: LedgerHasher>(
        records: &[Self],
        max: usize,
    ) -> Result<Digest32, LedgerError> {
        if records.len() > max {
            return Err(LedgerError::LengthBoundExceeded { max });
        }
        let mut previous = Digest32::GENESIS;
        let mut expected = Sequence::FIRST;
        let mut last_at: Option<UnixSeconds> = None;
        for record in records.iter().take(max) {
            verify_link::<H, Self>(previous, expected, record)?;
            let at = record.at();
            if let Some(last) = last_at
                && at < last
            {
                return Err(LedgerError::NonMonotonicTimestamp {
                    last: last.get(),
                    observed: at.get(),
                });
            }
            previous = record.digest();
            expected = expected
                .next()
                .ok_or(LedgerError::LengthBoundExceeded { max })?;
            last_at = Some(at);
        }
        Ok(previous)
    }
}

/// Generic hash-linked chain state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chain<R> {
    records: Vec<R>,
    head: Digest32,
    next_sequence: Sequence,
    last_at: Option<UnixSeconds>,
    bound: usize,
}

impl<R> Chain<R> {
    /// Builds an empty chain with a scalar capacity bound.
    #[must_use]
    pub fn with_bound(bound: usize) -> Self {
        Self {
            records: Vec::with_capacity(bound),
            head: Digest32::GENESIS,
            next_sequence: Sequence::FIRST,
            last_at: None,
            bound,
        }
    }

    /// Returns the scalar record bound.
    #[must_use]
    pub const fn bound(&self) -> usize {
        self.bound
    }

    /// Returns the capacity reserved at construction.
    ///
    /// HISS-03 evidence: this value must not change while the ledger is in use.
    #[must_use]
    pub fn reserved(&self) -> usize {
        self.records.capacity()
    }

    /// Returns the digest of the latest record, or [`Digest32::GENESIS`].
    #[must_use]
    pub const fn head(&self) -> Digest32 {
        self.head
    }

    /// Returns the next sequence number to be issued.
    #[must_use]
    pub const fn next_sequence(&self) -> Sequence {
        self.next_sequence
    }

    /// Returns the number of records held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns `true` when the ledger holds no record.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Returns the records held.
    #[must_use]
    pub fn records(&self) -> &[R] {
        &self.records
    }

    /// Verifies that an append at timestamp `at` is valid before hashing and signing.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError::LengthBoundExceeded`] when the chain is full, or
    /// [`LedgerError::NonMonotonicTimestamp`] when `at` precedes the latest record.
    pub fn check_append(&self, at: UnixSeconds) -> Result<(), LedgerError> {
        if self.records.len() >= self.bound {
            return Err(LedgerError::LengthBoundExceeded { max: self.bound });
        }
        if let Some(last) = self.last_at
            && at < last
        {
            return Err(LedgerError::NonMonotonicTimestamp {
                last: last.get(),
                observed: at.get(),
            });
        }
        Ok(())
    }

    /// Verifies that an append at `at` is valid and returns the next sequence number and predecessor digest.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError::LengthBoundExceeded`] when the chain is full, or
    /// [`LedgerError::NonMonotonicTimestamp`] when `at` precedes the latest record.
    pub fn next_link(&self, at: UnixSeconds) -> Result<(Sequence, Digest32), LedgerError> {
        self.check_append(at)?;
        Ok((self.next_sequence, self.head))
    }

    /// Commits a sealed record to the chain.
    fn commit(&mut self, record: R, digest: Digest32, sequence: Sequence, at: UnixSeconds) {
        self.head = digest;
        // Saturating, not wrapping: at the u64 ceiling the next append fails with
        // a sequence gap rather than silently restarting the chain at one.
        self.next_sequence = Sequence::new(sequence.get().saturating_add(1));
        self.last_at = Some(at);
        self.records.push(record);
    }
}

impl<R: ChainRecord> Chain<R> {
    /// Re-walks the chain's records under the chain's scalar bound.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError`] from [`ChainRecord::verify_chain_bounded`].
    pub fn verify<H: LedgerHasher>(&self) -> Result<Digest32, LedgerError> {
        R::verify_chain_bounded::<H>(&self.records, self.bound)
    }

    /// Commits a sealed record to the chain and returns its digest.
    pub fn seal(&mut self, record: R) -> Digest32 {
        let digest = record.digest();
        let sequence = record.sequence();
        let at = record.at();
        self.commit(record, digest, sequence, at);
        digest
    }
}
