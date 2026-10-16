// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Why a sandbox step was refused or could not complete.

use aegis_athena::AthenaError;
use aegis_vesta::{ContractError, IdError, VestaError};
use saeptum_microvm::MicroVmError;

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
    /// The microVM, its configuration, its framing or its deadlines refused
    /// (`saeptum-microvm`, cut unit CUT-1632-04).
    #[error(transparent)]
    MicroVm(#[from] MicroVmError),
    /// An operating-system call this crate makes failed.
    #[error("{what}: {source}")]
    Io {
        /// What was being done.
        what: &'static str,
        /// The underlying error.
        #[source]
        source: std::io::Error,
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
