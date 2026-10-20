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

pub use aegis_justitia::LedgerError;
use aegis_justitia::ledger::Timestamped;
use aegis_justitia::{
    CanonicalBuffer, Chain, ChainRecord, Digest32, HashAlgorithm, LedgerHasher, PreimageError,
    RecordBody, Sequence, Sha256Hasher, UnixSeconds,
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

impl Timestamped for CheckpointDraft {
    fn at(&self) -> UnixSeconds {
        self.at
    }
}

/// One checkpoint record body, including its chain position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CheckpointBody(RecordBody<CheckpointDraft>);

impl CheckpointBody {
    /// Returns the underlying record body.
    #[must_use]
    pub(crate) const fn record_body(&self) -> &RecordBody<CheckpointDraft> {
        &self.0
    }

    /// Places a draft at a chain position.
    #[must_use]
    pub const fn from_parts(
        sequence: Sequence,
        previous: Digest32,
        algorithm: HashAlgorithm,
        draft: CheckpointDraft,
    ) -> Self {
        Self(RecordBody::from_parts(sequence, previous, algorithm, draft))
    }

    /// Returns the sequence number.
    #[must_use]
    pub const fn sequence(&self) -> Sequence {
        self.0.sequence()
    }

    /// Returns the predecessor link.
    #[must_use]
    pub const fn previous(&self) -> Digest32 {
        self.0.previous()
    }

    /// Returns the algorithm named by the record.
    #[must_use]
    pub const fn algorithm(&self) -> HashAlgorithm {
        self.0.algorithm()
    }

    /// Returns the caller-supplied fields.
    #[must_use]
    pub const fn draft(&self) -> &CheckpointDraft {
        self.0.draft()
    }

    /// Returns a copy of this body carrying a different stage.
    ///
    /// This is the forgery path a tamper test needs: it restates a stored
    /// record with one field changed and leaves the stored digest alone, which
    /// is exactly what a re-walk must detect.
    #[must_use]
    pub const fn with_stage(&self, stage: Stage) -> Self {
        let mut draft = *self.0.draft();
        draft.stage = stage;
        Self(RecordBody::from_parts(
            self.0.sequence(),
            self.0.previous(),
            self.0.algorithm(),
            draft,
        ))
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
        let draft = self.0.draft();
        let mut buffer = self
            .0
            .header(CHECKPOINT_DOMAIN, CONTRACT_VERSION, draft.at)?;
        buffer.write_u8(draft.stage.tag())?;
        write_verdict(&mut buffer, draft.verdict)?;
        write_metrics(&mut buffer, draft.metrics.as_ref())?;
        write_reason(&mut buffer, draft.reason)?;
        buffer.write_framed(draft.candidate.as_bytes())?;
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
        H::digest(preimage.as_slice()).map_err(LedgerError::from)
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

impl ChainRecord for CheckpointRecord {
    type Draft = CheckpointDraft;

    fn body(&self) -> &RecordBody<Self::Draft> {
        self.body.record_body()
    }

    fn digest(&self) -> Digest32 {
        self.digest
    }

    fn verify<H: LedgerHasher>(&self) -> Result<Digest32, LedgerError> {
        self.body.digest::<H>()
    }
}

/// A hash-linked checkpoint ledger over the D02 hashing trait.
///
/// Dispatch is static: there is no `dyn` and no vtable on the record path.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckpointLedger<H: LedgerHasher = Sha256Hasher> {
    chain: Chain<CheckpointRecord>,
    hasher: core::marker::PhantomData<H>,
}

impl<H: LedgerHasher> core::ops::Deref for CheckpointLedger<H> {
    type Target = Chain<CheckpointRecord>;

    fn deref(&self) -> &Self::Target {
        &self.chain
    }
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
            chain: Chain::with_bound(bound),
            hasher: core::marker::PhantomData,
        }
    }

    /// Returns the capacity reserved at construction.
    ///
    /// HISS-03 evidence: this value must not change while the ledger is in use.
    #[must_use]
    pub fn reserved(&self) -> usize {
        self.chain.reserved()
    }

    /// Returns the digest of the latest record, or [`Digest32::GENESIS`].
    #[must_use]
    pub const fn head(&self) -> Digest32 {
        self.chain.head()
    }

    /// Links `draft` onto the chain.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError`] when the record bound is reached, the timestamp
    /// precedes the previous record, the pre-image overflows, or the hash
    /// backend misbehaves. Nothing is committed unless every step succeeds.
    pub fn append(&mut self, draft: CheckpointDraft) -> Result<Digest32, LedgerError> {
        let (sequence, head) = self.chain.next_link(draft.at)?;
        let body = CheckpointBody::from_parts(sequence, head, H::ALGORITHM, draft);
        let digest = body.digest::<H>()?;
        let record = CheckpointRecord::from_parts(body, digest);
        Ok(self.chain.seal(record))
    }

    /// Re-walks the ledger's own records.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError`] from [`Chain::verify`].
    pub fn verify(&self) -> Result<Digest32, LedgerError> {
        self.chain.verify::<H>()
    }
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
    CheckpointRecord::verify_chain_bounded::<H>(records, max)
}
