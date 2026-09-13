// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The `PGlite` vector store: initialise once, then answer bounded queries.
//!
//! REQ-P15-05 is two rules, and the scaffold states both as assertions inside
//! the query call: the store must be initialised before it is queried, and the
//! query limit must be within `1..=100`. Here the first is a state check
//! returning [`HestiaError::NotInitialised`] and the second is
//! [`QueryLimit::new`], so a limit the requirement refuses never becomes a
//! value and both negative cases are ordinary tests.
//!
//! # What this module does not do
//!
//! It starts no `PGlite` instance, loads no `WebAssembly`, opens no
//! `PostgreSQL` connection, mounts no subvolume, reads no file and computes no
//! cosine distance. [`PgliteVectorStore::query`] returns the hit count the
//! recorded requirement says such a query would return; it searches nothing.
//! `tests/stubbed_effects.rs` is the regression gate that keeps it that way.

use crate::error::HestiaError;
use crate::storage::StoragePath;

/// The buffer the recorded requirement allocates for the store, in megabytes.
///
/// The scaffold's `PGLITE_OPFS_BUFFER_MB` (export-030 `689d175667d6`).
pub const PGLITE_OPFS_BUFFER_MB: usize = 256;

/// The smallest admissible query limit (REQ-P15-05).
pub const MIN_QUERY_LIMIT: usize = 1;

/// The largest admissible query limit (REQ-P15-05).
pub const MAX_QUERY_LIMIT: usize = 100;

/// The width of one embedding the scaffold queries with.
pub const EMBEDDING_DIMENSIONS: usize = 4;

/// Scalar upper bound on the subscribers one store notifies.
///
/// The scaffold's `MAX_SUBSCRIBERS` (export-030 `689d175667d6`). It is
/// recorded here as the bound the notification path would carry; nothing in
/// this crate notifies anyone.
pub const MAX_SUBSCRIBERS: usize = 16;

/// A validated query limit, within `1..=100`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(into = "usize", try_from = "usize")]
pub struct QueryLimit(usize);

impl QueryLimit {
    /// The largest admissible limit, as a value.
    pub const MAX: Self = Self(MAX_QUERY_LIMIT);

    /// The smallest admissible limit, as a value.
    pub const MIN: Self = Self(MIN_QUERY_LIMIT);

    /// Validates a query limit.
    ///
    /// # Errors
    ///
    /// Returns [`HestiaError::QueryLimitOutOfRange`] for zero and for anything
    /// past [`MAX_QUERY_LIMIT`].
    pub const fn new(limit: usize) -> Result<Self, HestiaError> {
        if limit < MIN_QUERY_LIMIT || limit > MAX_QUERY_LIMIT {
            return Err(HestiaError::QueryLimitOutOfRange { limit });
        }
        Ok(Self(limit))
    }

    /// Returns the validated limit.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

impl From<QueryLimit> for usize {
    fn from(value: QueryLimit) -> Self {
        value.0
    }
}

impl TryFrom<usize> for QueryLimit {
    type Error = HestiaError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// One query embedding, of [`EMBEDDING_DIMENSIONS`] components.
///
/// Stored inline, so building one allocates nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Embedding([f32; EMBEDDING_DIMENSIONS]);

impl Embedding {
    /// Builds an embedding from its components.
    #[must_use]
    pub const fn new(components: [f32; EMBEDDING_DIMENSIONS]) -> Self {
        Self(components)
    }

    /// Returns the components.
    #[must_use]
    pub const fn components(&self) -> &[f32; EMBEDDING_DIMENSIONS] {
        &self.0
    }
}

/// Whether the store has been initialised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StoreState {
    /// Not initialised; every query is refused.
    Uninitialised,
    /// Initialised; a bounded query is admitted.
    Initialised,
}

impl StoreState {
    /// Returns the stable name this state is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Uninitialised => "uninitialised",
            Self::Initialised => "initialised",
        }
    }
}

/// How many rows a query accounted for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QueryHits(usize);

impl QueryHits {
    /// Returns the number of rows.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

/// The P15 local-first vector store, as a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PgliteVectorStore {
    path: StoragePath,
    state: StoreState,
    buffer_mb: usize,
}

impl PgliteVectorStore {
    /// Builds an uninitialised store over `path`.
    #[must_use]
    pub const fn new(path: StoragePath) -> Self {
        Self {
            path,
            state: StoreState::Uninitialised,
            buffer_mb: PGLITE_OPFS_BUFFER_MB,
        }
    }

    /// Returns where the store would live.
    #[must_use]
    pub const fn path(&self) -> StoragePath {
        self.path
    }

    /// Returns whether the store has been initialised.
    #[must_use]
    pub const fn state(&self) -> StoreState {
        self.state
    }

    /// Returns the buffer the recorded requirement allocates, in megabytes.
    #[must_use]
    pub const fn buffer_mb(&self) -> usize {
        self.buffer_mb
    }

    /// Initialises the store.
    ///
    /// # Errors
    ///
    /// Returns [`HestiaError::AlreadyInitialised`] on a second call.
    pub const fn initialize(&mut self) -> Result<(), HestiaError> {
        if matches!(self.state, StoreState::Initialised) {
            return Err(HestiaError::AlreadyInitialised);
        }
        self.state = StoreState::Initialised;
        Ok(())
    }

    /// Accounts for one bounded similarity query.
    ///
    /// # Errors
    ///
    /// Returns [`HestiaError::NotInitialised`] while the store is
    /// uninitialised. The limit was already refused if it was outside
    /// `1..=100`, because [`QueryLimit`] is the only way to spell one.
    pub const fn query(
        &self,
        embedding: &Embedding,
        limit: QueryLimit,
    ) -> Result<QueryHits, HestiaError> {
        if matches!(self.state, StoreState::Uninitialised) {
            return Err(HestiaError::NotInitialised);
        }
        let _ = embedding;
        Ok(QueryHits(limit.get()))
    }
}
