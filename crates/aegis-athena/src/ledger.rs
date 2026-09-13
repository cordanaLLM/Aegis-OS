// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The checkpoint ledger, ported to Rust under decision D02.
//!
//! # What was ported, and what was decided
//!
//! The imported P16 scaffold's ledger is not a ledger: `log_ledger_entry`
//! formats a JSON line and appends it to a file. There is no chain, no digest
//! and no verification, so an edit to a stored line is undetectable. Its own
//! header calls it a "BLAKE3 experiment ledger" (REQ-P16-01) and the reference
//! checkpoint ledger describes itself the same way (REQ-P16-03), while the
//! only implementation that hashes anything at all uses SHA-256 (REQ-P16-09).
//!
//! Decision D02 settles the disagreement: one algorithm behind a
//! hash-algorithm trait, SHA-256, MD5 excluded, BLAKE3 only through a later
//! recorded decision. That trait already exists -- M02 created
//! [`LedgerHasher`] for the P06 audit ledger -- so this module reuses it
//! rather than declaring a second one. There is exactly one hash
//! implementation in the workspace, and the name BLAKE3 appears in
//! [`crate::register`] as a superseded claim rather than anywhere in code.
//!
//! # The chain
//!
//! Each record commits to the digest of its predecessor, so a re-walk detects
//! any edit to a stored record. The walk is iterative and bounded; the
//! recursion NASA rule 1 forbids has no place here. An empty chain verifies as
//! [`Digest32::GENESIS`], and the first record's `previous` is that same
//! all-zero link, which is what makes the genesis case a boundary rather than
//! a special case.
//!
//! The digest is taken over a canonical, domain-separated, length-prefixed
//! pre-image built with [`CanonicalBuffer`] rather than over a serialiser's
//! output, so no `serde_json` release can alter a historical digest. The
//! domain tag is [`CHECKPOINT_DOMAIN`], which differs from the P06 audit
//! domain, so a record of one kind cannot be replayed as the other even though
//! both chains hash with the same function.
//!
//! # What is deliberately absent
//!
//! No signature. The P06 audit ledger has a signer boundary that fails closed
//! until M20 provides TPM2 sealing; this ledger has none at all, because
//! nothing in the imported material asks the checkpoint ledger to be signed.
//! A chain detects edits; it does not establish who wrote it.
//!
//! No file. The scaffold's `OpenOptions::new().create(true).append(true)`
//! is not ported: this ledger is an in-memory `Vec` reserved to its bound at
//! construction, so appending never reallocates, and nothing here opens a
//! path. `/var/log/aegis/checkpoint_ledger.jsonl` is named in this comment and
//! nowhere in the code.

use aegis_justitia::{
    CanonicalBuffer, Digest32, HashAlgorithm, HashError, LedgerHasher, PreimageError, Sequence,
    Sha256Hasher, UnixSeconds,
};
use aegis_tellus::CandidateId;

use crate::CONTRACT_VERSION;
use crate::metrics::CandidateMetrics;
use crate::pareto::ParetoVerdict;
use crate::stage::{InvalidationReason, Stage};

/// Domain-separation tag prefixed to every canonical checkpoint pre-image.
pub const CHECKPOINT_DOMAIN: &[u8] = b"aegis.athena.checkpoint.v1\0";

/// Scalar upper bound on the records one ledger holds.
pub const MAX_CHECKPOINT_RECORDS: usize = 4096;

/// Reasons the checkpoint ledger refuses a record or a chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum LedgerError {
    /// A record names an algorithm the ledger does not use.
    #[error("record algorithm {found} does not match the ledger algorithm {expected}")]
    AlgorithmMismatch {
        /// The algorithm the ledger uses.
        expected: HashAlgorithm,
        /// The algorithm the record names.
        found: HashAlgorithm,
    },
    /// A record does not link to its predecessor.
    #[error("record {at} does not link to its predecessor")]
    PreviousHashMismatch {
        /// The sequence number of the offending record.
        at: u64,
    },
    /// A record's stored digest does not match its content.
    #[error("record {at} has a stored digest that does not match its content")]
    DigestMismatch {
        /// The sequence number of the offending record.
        at: u64,
    },
    /// A record's sequence number is not the expected successor.
    #[error("expected sequence {expected}, found {found}")]
    SequenceGap {
        /// The sequence number the chain expected.
        expected: u64,
        /// The sequence number observed.
        found: u64,
    },
    /// The chain or the ledger exceeded its scalar bound.
    #[error("record count exceeds the bound of {max}")]
    LengthBoundExceeded {
        /// The scalar bound.
        max: usize,
    },
    /// A record carries a timestamp earlier than its predecessor.
    #[error("record timestamp {observed} precedes the previous record at {last}")]
    NonMonotonicTimestamp {
        /// The previous record's timestamp.
        last: u64,
        /// The timestamp observed.
        observed: u64,
    },
    /// The canonical pre-image could not be built.
    #[error("canonical pre-image: {0}")]
    Preimage(#[from] PreimageError),
    /// The digest could not be produced.
    #[error("digest: {0}")]
    Hash(#[from] HashError),
}

/// The caller-supplied fields of one checkpoint record.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CheckpointDraft {
    /// The candidate the record is about.
    pub candidate: CandidateId,
    /// The stage the candidate reached.
    pub stage: Stage,
    /// The metrics the gate compared, once a check has produced any.
    ///
    /// `None` before the check stage. A record written earlier carries no
    /// metrics rather than fabricated ones, and the pre-image commits to the
    /// difference with a presence byte.
    pub metrics: Option<CandidateMetrics>,
    /// Which objectives the candidate cleared, once a check has decided.
    pub verdict: Option<ParetoVerdict>,
    /// Why the candidate was withdrawn, for a withdrawal and only for one.
    pub reason: Option<InvalidationReason>,
    /// When the record was written.
    pub at: UnixSeconds,
}

/// One checkpoint record body, including its chain position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CheckpointBody {
    sequence: Sequence,
    previous: Digest32,
    algorithm: HashAlgorithm,
    draft: CheckpointDraft,
}

impl CheckpointBody {
    /// Places a draft at a chain position.
    #[must_use]
    pub const fn from_parts(
        sequence: Sequence,
        previous: Digest32,
        algorithm: HashAlgorithm,
        draft: CheckpointDraft,
    ) -> Self {
        Self {
            sequence,
            previous,
            algorithm,
            draft,
        }
    }

    /// Returns the sequence number.
    #[must_use]
    pub const fn sequence(&self) -> Sequence {
        self.sequence
    }

    /// Returns the predecessor link.
    #[must_use]
    pub const fn previous(&self) -> Digest32 {
        self.previous
    }

    /// Returns the algorithm named by the record.
    #[must_use]
    pub const fn algorithm(&self) -> HashAlgorithm {
        self.algorithm
    }

    /// Returns the caller-supplied fields.
    #[must_use]
    pub const fn draft(&self) -> &CheckpointDraft {
        &self.draft
    }

    /// Returns a copy of this body carrying a different stage.
    ///
    /// This is the forgery path a tamper test needs: it restates a stored
    /// record with one field changed and leaves the stored digest alone, which
    /// is exactly what a re-walk must detect.
    #[must_use]
    pub const fn with_stage(&self, stage: Stage) -> Self {
        let mut draft = self.draft;
        draft.stage = stage;
        Self { draft, ..*self }
    }

    /// Builds the canonical, domain-separated, length-prefixed pre-image.
    ///
    /// Each metric is committed to as its IEEE-754 bit pattern, which is exact
    /// and reversible; the metric constructors admit only finite values, so no
    /// two spellings of one number reach here.
    ///
    /// # Errors
    ///
    /// Returns [`PreimageError::Overflow`] when the encoding would exceed the
    /// fixed buffer. The encoding never truncates.
    pub fn canonical_preimage(&self) -> Result<CanonicalBuffer, PreimageError> {
        let mut buffer = CanonicalBuffer::new();
        buffer.write_tag(CHECKPOINT_DOMAIN)?;
        buffer.write_framed(CONTRACT_VERSION.as_bytes())?;
        buffer.write_u8(self.algorithm.tag())?;
        buffer.write_u64(self.sequence.get())?;
        buffer.write_digest(&self.previous)?;
        buffer.write_u64(self.draft.at.get())?;
        buffer.write_u8(self.draft.stage.tag())?;
        write_verdict(&mut buffer, self.draft.verdict)?;
        write_metrics(&mut buffer, self.draft.metrics.as_ref())?;
        write_reason(&mut buffer, self.draft.reason)?;
        buffer.write_framed(self.draft.candidate.as_bytes())?;
        Ok(buffer)
    }

    /// Computes the digest of this body under `H`.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError`] when the pre-image overflows or the hash
    /// backend returns an output of the wrong width.
    pub fn digest<H: LedgerHasher>(&self) -> Result<Digest32, LedgerError> {
        let preimage = self.canonical_preimage()?;
        Ok(H::digest(preimage.as_slice())?)
    }
}

/// Commits to the Pareto verdict, if one has been reached.
///
/// The absent case writes a zero byte and the present case writes the bit
/// field plus one, so "no verdict" and "no objective cleared" differ.
fn write_verdict(
    buffer: &mut CanonicalBuffer,
    verdict: Option<ParetoVerdict>,
) -> Result<(), PreimageError> {
    match verdict {
        None => buffer.write_u8(0),
        Some(verdict) => buffer.write_u8(verdict.bits().saturating_add(1)),
    }
}

/// Commits to the four metrics, each as its exact bit pattern.
///
/// The presence byte comes first, so a record with no metrics is not a record
/// whose metrics happened to be zero.
fn write_metrics(
    buffer: &mut CanonicalBuffer,
    metrics: Option<&CandidateMetrics>,
) -> Result<(), PreimageError> {
    let Some(metrics) = metrics else {
        return buffer.write_u8(0);
    };
    buffer.write_u8(1)?;
    buffer.write_u64(metrics.latency.get().to_bits())?;
    buffer.write_u64(metrics.memory.get().to_bits())?;
    buffer.write_u64(metrics.carbon.get().to_bits())?;
    buffer.write_u64(metrics.retention.get().to_bits())
}

/// Commits to the withdrawal reason, if any.
///
/// The absent case writes a zero byte rather than nothing, so a record with no
/// reason and one with a reason never share a pre-image length.
fn write_reason(
    buffer: &mut CanonicalBuffer,
    reason: Option<InvalidationReason>,
) -> Result<(), PreimageError> {
    match reason {
        None => buffer.write_u8(0),
        Some(reason) => buffer.write_u8(reason.tag()),
    }
}

/// One sealed checkpoint record.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CheckpointRecord {
    body: CheckpointBody,
    digest: Digest32,
}

impl CheckpointRecord {
    /// Assembles a record from stored parts.
    ///
    /// The digest is carried rather than recomputed, so a re-walk can detect a
    /// stored record whose content no longer matches its digest.
    #[must_use]
    pub const fn from_parts(body: CheckpointBody, digest: Digest32) -> Self {
        Self { body, digest }
    }

    /// Returns the record body.
    #[must_use]
    pub const fn body(&self) -> &CheckpointBody {
        &self.body
    }

    /// Returns the stored digest.
    #[must_use]
    pub const fn digest(&self) -> Digest32 {
        self.digest
    }
}

/// A hash-linked checkpoint ledger over the D02 hashing trait.
///
/// Dispatch is static: there is no `dyn` and no vtable on the record path.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckpointLedger<H: LedgerHasher> {
    records: Vec<CheckpointRecord>,
    head: Digest32,
    next_sequence: Sequence,
    last_at: Option<UnixSeconds>,
    bound: usize,
    hasher: core::marker::PhantomData<H>,
}

/// The SHA-256 checkpoint ledger, the only instantiation M05 ships (D02).
pub type Sha256CheckpointLedger = CheckpointLedger<Sha256Hasher>;

impl<H: LedgerHasher> Default for CheckpointLedger<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: LedgerHasher> CheckpointLedger<H> {
    /// Builds a ledger with the default record bound.
    #[must_use]
    pub fn new() -> Self {
        Self::with_bound(MAX_CHECKPOINT_RECORDS)
    }

    /// Builds a ledger with an explicit scalar record bound.
    ///
    /// The record list is reserved to `bound` here, once, so that
    /// [`Self::append`] never reallocates.
    #[must_use]
    pub fn with_bound(bound: usize) -> Self {
        Self {
            records: Vec::with_capacity(bound),
            head: Digest32::GENESIS,
            next_sequence: Sequence::FIRST,
            last_at: None,
            bound,
            hasher: core::marker::PhantomData,
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

    /// Returns the digest of the most recent record, or the genesis link.
    #[must_use]
    pub const fn head(&self) -> Digest32 {
        self.head
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
    pub fn records(&self) -> &[CheckpointRecord] {
        &self.records
    }

    /// Links `draft` onto the chain.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError`] when the record bound is reached, the timestamp
    /// precedes the previous record, the pre-image overflows, or the hash
    /// backend misbehaves. Nothing is committed unless every step succeeds.
    pub fn append(&mut self, draft: CheckpointDraft) -> Result<Digest32, LedgerError> {
        if self.records.len() >= self.bound {
            return Err(LedgerError::LengthBoundExceeded { max: self.bound });
        }
        if let Some(last) = self.last_at
            && draft.at < last
        {
            return Err(LedgerError::NonMonotonicTimestamp {
                last: last.get(),
                observed: draft.at.get(),
            });
        }
        let body = CheckpointBody::from_parts(self.next_sequence, self.head, H::ALGORITHM, draft);
        let digest = body.digest::<H>()?;
        let record = CheckpointRecord::from_parts(body, digest);
        self.commit(&record, draft.at);
        Ok(digest)
    }

    /// Commits a sealed record. Called only after every fallible step passed.
    fn commit(&mut self, record: &CheckpointRecord, at: UnixSeconds) {
        self.head = record.digest();
        // Saturating, not wrapping: at the u64 ceiling the next append fails
        // with a sequence gap rather than silently restarting the chain at one.
        self.next_sequence = Sequence::new(record.body().sequence().get().saturating_add(1));
        self.last_at = Some(at);
        self.records.push(*record);
    }

    /// Re-walks the ledger's own records.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError`] from [`verify_chain_bounded`].
    pub fn verify(&self) -> Result<Digest32, LedgerError> {
        verify_chain_bounded::<H>(&self.records, self.bound)
    }
}

/// Verifies one link of a chain against the state that precedes it.
fn verify_link<H: LedgerHasher>(
    previous: Digest32,
    expected: Sequence,
    record: &CheckpointRecord,
) -> Result<(), LedgerError> {
    let body = record.body();
    if body.algorithm() != H::ALGORITHM {
        return Err(LedgerError::AlgorithmMismatch {
            expected: H::ALGORITHM,
            found: body.algorithm(),
        });
    }
    if body.sequence() != expected {
        return Err(LedgerError::SequenceGap {
            expected: expected.get(),
            found: body.sequence().get(),
        });
    }
    if body.previous() != previous {
        return Err(LedgerError::PreviousHashMismatch {
            at: body.sequence().get(),
        });
    }
    if body.digest::<H>()? != record.digest() {
        return Err(LedgerError::DigestMismatch {
            at: body.sequence().get(),
        });
    }
    Ok(())
}

/// Re-walks `records` under the default bound [`MAX_CHECKPOINT_RECORDS`].
///
/// An empty chain verifies as [`Digest32::GENESIS`].
///
/// # Errors
///
/// Returns [`LedgerError`] on the first broken link.
pub fn verify_chain<H: LedgerHasher>(
    records: &[CheckpointRecord],
) -> Result<Digest32, LedgerError> {
    verify_chain_bounded::<H>(records, MAX_CHECKPOINT_RECORDS)
}

/// Re-walks `records` under an explicit scalar bound.
///
/// The walk is iterative; the bound is checked before the walk starts, so an
/// oversized chain is refused without being traversed.
///
/// # Errors
///
/// Returns [`LedgerError::LengthBoundExceeded`] when the chain is longer than
/// `max`, and otherwise the first broken link.
pub fn verify_chain_bounded<H: LedgerHasher>(
    records: &[CheckpointRecord],
    max: usize,
) -> Result<Digest32, LedgerError> {
    if records.len() > max {
        return Err(LedgerError::LengthBoundExceeded { max });
    }
    let mut previous = Digest32::GENESIS;
    let mut expected = Sequence::FIRST;
    let mut last_at: Option<UnixSeconds> = None;
    for record in records.iter().take(max) {
        verify_link::<H>(previous, expected, record)?;
        let at = record.body().draft().at;
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
