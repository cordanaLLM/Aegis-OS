// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P15 `aegis-hestia`: the Rust half of decision D09.
//!
//! The imported P15 scaffold (export-030 `689d175667d6`) is a daemon that
//! prints what it would do, guarded by three `assert!` calls. Milestone M17
//! extracts the part that can be checked without a `PGlite` instance or a
//! Wayland compositor -- the state machine and the bounds -- and restates each
//! assertion as a refusal, which is what HISS-07 asks for: a query before
//! initialisation, a limit outside `1..=100` or a negative `DMA-BUF`
//! descriptor is a `Result`, not a process abort, so the negative cases are
//! ordinary tests.
//!
//! # Decision D09, applied
//!
//! Two imported sources disagreed about where P15 lives: one places a Rust
//! daemon under `crates/`, the other's directory map lists only a Svelte
//! package. D09 settled it as **both**, joined by a typed boundary, and M17 is
//! where that is applied. This crate is the Rust half: the storage and vector
//! logic. [`HestiaView`] is the boundary -- a versioned, bounded, `Copy`
//! snapshot the interface reads -- and [`decision`] is the register that
//! records what was decided. No Svelte package, manifest or lockfile is added
//! by this milestone, and none is claimed.
//!
//! # The four things a reviewer should look at
//!
//! * [`PgliteVectorStore`] holds REQ-P15-05: a query before
//!   [`PgliteVectorStore::initialize`] is [`HestiaError::NotInitialised`], and
//!   [`QueryLimit`] holds the `1..=100` bound so an out-of-range limit never
//!   becomes a value.
//! * [`WaylandPipMediaController`] holds the descriptor rule through
//!   [`DmaBufFd`], and registers at most one surface at a time.
//! * [`HestiaView`] is the D09 boundary, and [`contracts`] carries the
//!   versioned [`OverlayRegistration`] P15 hands to P04 Mercurius.
//! * [`register`] records the P15 claims this crate cannot check, including
//!   the one REQ-P15-06 itself marks as unverified proposal data.
//!
//! # Design invariants (see `AGENTS.md`)
//!
//! * no recursion: every function here is a flat check or a field access, and
//!   no function in the crate calls itself directly or through another;
//! * every loop carries a scalar upper bound: [`MAX_STORAGE_PATH_LEN`],
//!   [`MAX_CORRELATION_LEN`], [`MAX_SURFACE_ID_LEN`] and
//!   [`MAX_CONTRACT_PAYLOAD_BYTES`];
//! * every value on a decision path is `Copy`, so no such path allocates;
//!   `tests/allocation_bounds.rs` is the falsifier, and the one place that is
//!   deliberately not claimed -- a JSON string carrying an escape, which
//!   `serde_json` unescapes into a heap scratch buffer before any field of
//!   ours sees it -- is tested rather than denied;
//! * no `unwrap`, `expect`, `panic!`, slice indexing or unchecked arithmetic,
//!   and no `unsafe` (forbidden at the workspace root).
//!
//! # What this crate does not do
//!
//! It stores nothing and displays nothing. There is no `PGlite` instance, no
//! `WebAssembly` runtime, no `PostgreSQL` connection, no `pgvector` search, no
//! Btrfs subvolume, no filesystem access, no Wayland connection, no
//! `wlr-layer-shell` binding and no `DMA-BUF` import anywhere in it. A query
//! returns the hit count the recorded requirement says it would return.
//! `tests/stubbed_effects.rs` sweeps the crate's own sources for a recorded
//! list of identifiers that would be needed to do any of it and fails if one
//! appears -- a regression gate over an enumeration, not a proof over every
//! such identifier.
//!
//! A pass of this crate's tests is evidence about the state machine and the
//! bounds. It closes no hardware, accessibility or user-interface gate.
//!
//! # Example
//!
//! ```
//! use aegis_hestia::{
//!     Embedding, HestiaView, PgliteVectorStore, QueryLimit, StoragePath,
//!     WaylandPipMediaController,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut store = PgliteVectorStore::new(StoragePath::parse("/var/pglite")?);
//! assert!(store.query(&Embedding::new([0.0; 4]), QueryLimit::new(5)?).is_err());
//!
//! store.initialize()?;
//! let hits = store.query(&Embedding::new([0.12, 0.45, 0.88, 0.03]), QueryLimit::new(5)?)?;
//! assert_eq!(hits.get(), 5);
//!
//! assert!(QueryLimit::new(0).is_err());
//! assert!(QueryLimit::new(101).is_err());
//!
//! let view = HestiaView::snapshot(&store, &WaylandPipMediaController::new());
//! assert!(view.store.initialised);
//! assert_eq!(view.store.max_query_limit, 100);
//! # Ok(())
//! # }
//! ```

pub mod contracts;
pub mod decision;
pub mod error;
pub mod id;
pub mod overlay;
pub mod register;
pub mod storage;
pub mod store;
pub mod view;

pub use crate::contracts::graph::EdgeId;
pub use crate::contracts::overlay_registration::{OverlayRegistration, OverlayRegistrationVersion};
pub use crate::contracts::{
    ContractError, Correlation, MAX_CONTRACT_PAYLOAD_BYTES, PayloadBuffer, SchemaId,
};
pub use crate::decision::{
    Citation, D09_HESTIA_LOCATION, DecisionRecord, DecisionState, HestiaLocation,
};
pub use crate::error::HestiaError;
pub use crate::id::{CorrelationId, IdError, MAX_CORRELATION_LEN, MAX_SURFACE_ID_LEN, SurfaceId};
pub use crate::overlay::{
    DmaBufFd, MAX_PIXEL_EXTENT, MIN_PIXEL_EXTENT, PipSurface, PixelExtent, ShellLayer,
    WaylandPipMediaController,
};
pub use crate::register::{ClaimSource, ClaimStatus, P15_RECORDED_CLAIMS, RecordedClaim};
pub use crate::storage::{
    MAX_STORAGE_PATH_LEN, PGLITE_SUBVOLUME, SCAFFOLD_STORAGE_PATH, StoragePath,
};
pub use crate::store::{
    EMBEDDING_DIMENSIONS, Embedding, MAX_QUERY_LIMIT, MAX_SUBSCRIBERS, MIN_QUERY_LIMIT,
    PGLITE_OPFS_BUFFER_MB, PgliteVectorStore, QueryHits, QueryLimit, StoreState,
};
pub use crate::view::{HestiaView, PipView, StoreView, ViewVersion};
