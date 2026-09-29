// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Why a sandbox step was refused or could not complete.

use aegis_athena::AthenaError;
use aegis_vesta::{ContractError, IdError, VestaError};

/// Reasons a sandbox step fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SandboxError {
    /// The controller or a validated P10 value refused.
    #[error(transparent)]
    Vesta(#[from] VestaError),
    /// The candidate evaluation payload was refused by its contract.
    #[error(transparent)]
    Contract(#[from] ContractError),
    /// A candidate metric was refused by P16's constructors.
    #[error(transparent)]
    Athena(#[from] AthenaError),
    /// An identifier was refused.
    #[error(transparent)]
    Id(#[from] IdError),
    /// An operating-system call failed.
    #[error("{what}: {source}")]
    Io {
        /// What was being done.
        what: &'static str,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
    /// A step did not finish within its deadline (HISS-02).
    #[error("{what} missed its {millis} ms deadline")]
    Deadline {
        /// What was waited for.
        what: &'static str,
        /// The deadline, in milliseconds.
        millis: u128,
    },
    /// A message on the wire broke the protocol.
    #[error("{0}")]
    Protocol(String),
    /// A check this crate holds refused a value.
    #[error("{0}")]
    Refused(String),
}

impl SandboxError {
    /// Wraps an I/O error with what was being done.
    #[must_use]
    pub const fn io(what: &'static str, source: std::io::Error) -> Self {
        Self::Io { what, source }
    }
}
