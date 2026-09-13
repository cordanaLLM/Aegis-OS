// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Findings: what systemd accepts but the Aegis A/B contract does not.
//!
//! A [`DefinitionError`](crate::DefinitionError) says the definition is not
//! valid input. A [`Finding`] says the definition is valid input that does not
//! meet a recorded requirement, and names the requirement. The split matters
//! because the two have different owners: an error is systemd's rule, a
//! finding is this repository's.

use core::fmt;

/// Scalar bound on the findings one check may report.
pub const MAX_FINDINGS: usize = 32;

/// What a definition set fails to meet, and which requirement says so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// The ESP may grow no smaller than 512M (REQ-P01-10).
    EspBelowMinimum {
        /// The declared lower bound, in bytes.
        bytes: u64,
    },
    /// The ESP may grow no larger than 1G (REQ-P01-10).
    EspAboveMaximum {
        /// The declared upper bound, in bytes.
        bytes: u64,
    },
    /// The ESP declares no size bound at all (REQ-P01-10).
    EspUnbounded,
    /// A verity data partition has no hash partition with the same match key.
    ///
    /// systemd refuses this too, at image-build time. The parser reports it
    /// before the build so a half-written set is caught in review.
    MissingVerityHash {
        /// The match key with no partner.
        match_key: String,
    },
    /// A verity hash partition has no data partition with the same match key.
    MissingVerityData {
        /// The match key with no partner.
        match_key: String,
    },
    /// The set does not declare two root slots (REQ-P01-04, REQ-P02-04).
    MissingAlternateRootSlot {
        /// How many root partitions the set declares.
        slots: usize,
    },
    /// The stateful partition is not TPM2-sealed (REQ-P02-03).
    VarNotEncrypted,
    /// The transfer keeps fewer than two instances, so it is not A/B.
    ///
    /// systemd does not refuse this: it prints `InstancesMax= value must be at
    /// least 2, bumping: 1` and silently repairs the definition. A repaired
    /// definition is exactly the drift this parser exists to report.
    SingleSlotTransfer {
        /// The declared instance count.
        instances_max: u32,
    },
    /// The transfer target is writable, against the read-only root (REQ-P02-04).
    WritableTransferTarget,
}

impl fmt::Display for Finding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EspBelowMinimum { bytes } => {
                write!(
                    formatter,
                    "REQ-P01-10: ESP SizeMinBytes={bytes} is below 512M"
                )
            }
            Self::EspAboveMaximum { bytes } => {
                write!(
                    formatter,
                    "REQ-P01-10: ESP SizeMaxBytes={bytes} is above 1G"
                )
            }
            Self::EspUnbounded => {
                write!(formatter, "REQ-P01-10: the ESP declares no size bound")
            }
            Self::MissingVerityHash { match_key } => write!(
                formatter,
                "REQ-P02-07: verity data partition with VerityMatchKey={match_key} has no hash partner"
            ),
            Self::MissingVerityData { match_key } => write!(
                formatter,
                "REQ-P02-07: verity hash partition with VerityMatchKey={match_key} has no data partner"
            ),
            Self::MissingAlternateRootSlot { slots } => write!(
                formatter,
                "REQ-P02-04: the A/B contract needs two root slots, the set declares {slots}"
            ),
            Self::VarNotEncrypted => {
                write!(
                    formatter,
                    "REQ-P02-03: the var partition is not TPM2-sealed"
                )
            }
            Self::SingleSlotTransfer { instances_max } => write!(
                formatter,
                "REQ-P02-04: the transfer keeps {instances_max} instance(s); A/B needs at least 2"
            ),
            Self::WritableTransferTarget => write!(
                formatter,
                "REQ-P02-04: the transfer target is writable; the root slot is read-only"
            ),
        }
    }
}
