// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Error types for GPU host record operations.
//!
//! Every error carries the path it was operating on to satisfy HISS context rules.

use std::path::PathBuf;

/// Error encountered during GPU host inspection.
#[derive(Debug, thiserror::Error)]
pub enum GpuError {
    /// An I/O error occurred while reading sysfs or stat-ing /dev.
    #[error("I/O error at {path}: {source}")]
    Io {
        /// The path being accessed when the error occurred.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// A required sysfs file or directory was not found.
    #[error("not found at {path}")]
    NotFound {
        /// The path that was expected but not found.
        path: PathBuf,
    },

    /// The content of a sysfs file was malformed or unexpected.
    #[error("malformed content at {path}: {reason}")]
    Malformed {
        /// The path of the malformed file.
        path: PathBuf,
        /// Description of why the content is malformed.
        reason: String,
    },

    /// A directory entry count reached the bounded loop limit with entries left.
    #[error("directory iteration bound exceeded at {path}: bound is {bound}")]
    Bound {
        /// The directory path whose iteration bound was exceeded.
        path: PathBuf,
        /// The iteration bound cap.
        bound: usize,
    },
}

impl GpuError {
    /// Creates an I/O error carrying the path.
    #[must_use]
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    /// Creates a `NotFound` error carrying the path.
    #[must_use]
    pub fn not_found(path: impl Into<PathBuf>) -> Self {
        Self::NotFound { path: path.into() }
    }

    /// Creates a `Malformed` error carrying the path and reason.
    #[must_use]
    pub fn malformed(path: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
        Self::Malformed {
            path: path.into(),
            reason: reason.into(),
        }
    }

    /// Creates a `Bound` error carrying the path and bound cap.
    #[must_use]
    pub fn bound(path: impl Into<PathBuf>, bound: usize) -> Self {
        Self::Bound {
            path: path.into(),
            bound,
        }
    }
}
