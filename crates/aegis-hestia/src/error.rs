// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Why a P15 value or a P15 call was refused.
//!
//! Every refusal is a value, never a panic. The imported scaffold
//! (export-030 `689d175667d6`) states each of its rules as an `assert!`, so a
//! query before initialisation, an out-of-range limit or a negative
//! `DMA-BUF` descriptor aborted the process and could only be covered by a
//! test that expects a panic. Here each of those rules is a constructor or a
//! method returning `Result`, which is what HISS-07 asks for and what makes
//! the negative cases ordinary tests.
//!
//! The mapping from scaffold assertion to refusal is fixed and swept by
//! `tests/scaffold_assertions.rs`:
//!
//! | Scaffold assertion | Refusal |
//! | :-- | :-- |
//! | vector store must be initialized | [`HestiaError::NotInitialised`] |
//! | query limit out of bounds | [`HestiaError::QueryLimitOutOfRange`] |
//! | invalid `DMA-BUF` file descriptor | [`HestiaError::InvalidDmaBufFd`] |

use crate::overlay::{MAX_PIXEL_EXTENT, MIN_PIXEL_EXTENT};
use crate::store::{MAX_QUERY_LIMIT, MIN_QUERY_LIMIT};

/// Reasons a P15 value or call is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum HestiaError {
    /// A query was made before the store was initialised.
    ///
    /// REQ-P15-05, the first scaffold assertion.
    #[error("the vector store is not initialised; it cannot answer a query")]
    NotInitialised,
    /// The store was initialised twice.
    ///
    /// Initialising twice is refused rather than silently repeated, so a
    /// second initialisation cannot quietly replace a live store.
    #[error("the vector store is already initialised")]
    AlreadyInitialised,
    /// The query limit is outside the bounds the recorded requirement states.
    ///
    /// REQ-P15-05, the second scaffold assertion.
    #[error("a query limit of {limit} is outside {MIN_QUERY_LIMIT}..={MAX_QUERY_LIMIT}")]
    QueryLimitOutOfRange {
        /// The limit that was refused.
        limit: usize,
    },
    /// The `DMA-BUF` descriptor is negative.
    ///
    /// The third scaffold assertion.
    #[error("a DMA-BUF descriptor of {fd} is not a descriptor")]
    InvalidDmaBufFd {
        /// The descriptor that was refused.
        fd: i32,
    },
    /// A picture-in-picture surface was registered while one was already live.
    #[error("a picture-in-picture surface is already registered")]
    OverlayAlreadyActive,
    /// No picture-in-picture surface is registered.
    #[error("no picture-in-picture surface is registered")]
    OverlayNotActive,
    /// A surface extent is outside the admissible range.
    #[error("a surface extent of {pixels} is outside {MIN_PIXEL_EXTENT}..={MAX_PIXEL_EXTENT}")]
    ExtentOutOfRange {
        /// The extent that was refused.
        pixels: u32,
    },
    /// A storage path is empty, over-long, relative or outside its charset.
    #[error("{reason}")]
    StoragePath {
        /// Why the path was refused.
        reason: &'static str,
    },
}
