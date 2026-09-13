// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P11 `aegis-ludus`: the launch validator and the receipt schema.
//!
//! The imported P11 scaffold (export-033 `531cbdf98eb5`) is a daemon that
//! prints what it would do: it announces a proprietary SDK it never links,
//! names a chat-platform socket it never opens, returns the literal `42` as a
//! zero-copy video file descriptor, and builds a "hardware non-repudiation
//! signature" with `format!`. Milestone M08 extracts the part that can be
//! checked without a platform SDK, a compositor, a `GPU` or a `TPM2` -- the
//! bounds, the outcomes and the payload -- and turns each string error into a
//! typed one, which is what HISS-07 asks for.
//!
//! # The four things a reviewer should look at
//!
//! * [`LaunchCommandLine`] holds REQ-P11-07's bound of [`MAX_LAUNCH_ARGS`]:
//!   64 arguments accepted, the 65th refused, and the empty line answered
//!   explicitly rather than by a scan that found nothing.
//! * [`AuthenticationOutcome`] is why that matters. The scaffold's
//!   `token_found || !args.is_empty()` calls a line with no token
//!   authenticated; this type has three variants,
//!   [`AuthenticationOutcome::is_authenticated`] is true for one of them, and
//!   [`AuthenticationOutcome::scaffold_verdict`] keeps the old rule beside it
//!   so the disagreement is a value a test asserts.
//! * [`TransactionReceipt`] is the versioned payload P11 hands to P02, with
//!   [`ReceiptSigning`] carrying one variant that says on the wire that the
//!   receipt is unsigned and no key is bound.
//! * [`decision`] records D12 as settled by ADR-0002 -- no platform SDK in this
//!   crate -- and D48 as **open**, which is why no rich-presence payload is
//!   typed at this milestone.
//!
//! # The two credentials are not in the same situation
//!
//! [`REFERENCE_PROFILE_PROBES`] records three commands run by hand on the
//! reference profile at M08. Two found a `TPM 2.0`; the third found no `FIDO2`
//! authenticator at all. So [`register`] records the `TPM2` half as a deferred
//! hardware requirement and the `FIDO2` half as a
//! [`ClaimStatus::ProcurementDependency`], because a stub that covered both the
//! same way would read as coverage for a device nobody has.
//!
//! # Design invariants (see `AGENTS.md`)
//!
//! * no recursion: every function here is a flat check, a bounded sweep or a
//!   field access, and no function in the crate calls itself directly or
//!   through another;
//! * every loop carries a scalar upper bound: [`MAX_LAUNCH_ARGS`],
//!   [`MAX_ARGUMENT_BYTES`], [`MAX_PCR_ENTRIES`] and
//!   [`MAX_CONTRACT_PAYLOAD_BYTES`];
//! * every value on a decision path is `Copy`, so no such path allocates;
//!   `tests/allocation_bounds.rs` is the falsifier, and the one place that is
//!   deliberately not claimed -- a JSON string carrying an escape, which
//!   `serde_json` unescapes into a heap scratch buffer before any field of ours
//!   sees it -- is tested rather than denied;
//! * no `unwrap`, `expect`, `panic!`, slice indexing or unchecked arithmetic,
//!   and no `unsafe` (forbidden at the workspace root).
//!
//! # What this crate does not do
//!
//! It links nothing and connects to nothing. There is no platform SDK and no
//! binding generator, no chat-platform socket, no `D-Bus` connection, no
//! compositor or `PipeWire` stream, no `DMA-BUF` export, no `TPM2` handle, no
//! `FIDO2` device and no filesystem access anywhere in it.
//! `tests/no_steamworks.rs` and `tests/stubbed_effects.rs` sweep the crate's
//! own sources for two recorded lists of identifiers that would be needed to do
//! any of it and fail if one appears outside a comment -- a regression gate over
//! an enumeration, not a proof over every such identifier.
//!
//! ADR-0002 also excludes the SDK from the Aegis **image**. This crate cannot
//! check that: the repository builds no image, and that gate is blocked. What a
//! pass of these tests is evidence about is the bounds, the outcomes and the
//! payload.
//!
//! # Example
//!
//! ```
//! use aegis_ludus::{AuthenticationOutcome, LaunchCommandLine, MAX_LAUNCH_ARGS};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let line = LaunchCommandLine::parse(
//!     "./game_binary -steam +connect_auth_token=aegis_secure_session_token_99",
//! )?;
//! assert_eq!(line.len(), 3);
//! assert_eq!(line.authenticate(), AuthenticationOutcome::TokenPresent);
//!
//! let untokened = LaunchCommandLine::parse("./game_binary -steam")?;
//! assert_eq!(untokened.authenticate(), AuthenticationOutcome::NoLaunchToken);
//! assert!(!untokened.authenticate().is_authenticated());
//! assert!(untokened.authenticate().scaffold_verdict());
//!
//! let empty = LaunchCommandLine::parse("   ")?;
//! assert_eq!(empty.authenticate(), AuthenticationOutcome::EmptyCommandLine);
//!
//! let over = "a ".repeat(MAX_LAUNCH_ARGS + 1);
//! assert!(LaunchCommandLine::parse(&over).is_err());
//! # Ok(())
//! # }
//! ```

pub mod contracts;
pub mod decision;
pub mod error;
pub mod launch;
pub mod probe;
pub mod receipt;
pub mod register;

pub use crate::contracts::graph::EdgeId;
pub use crate::contracts::transaction_receipt::{TransactionReceipt, TransactionReceiptVersion};
pub use crate::contracts::{
    ContractError, Correlation, MAX_CONTRACT_PAYLOAD_BYTES, PayloadBuffer, SchemaId,
};
pub use crate::decision::{
    Citation, D12_STEAMWORKS_EXCLUSION, D48_PRESENCE_INTEGRATION, DecisionState, PresenceDecision,
    PresenceDisposition, SdkDecision, SteamworksDisposition,
};
pub use crate::error::LudusError;
pub use crate::launch::{
    AUTH_TOKEN_PREFIX, AuthenticationOutcome, LaunchArgument, LaunchCommandLine,
    MAX_ARGUMENT_BYTES, MAX_LAUNCH_ARGS,
};
pub use crate::probe::{
    CredentialProbe, FIDO2_AUTHENTICATOR, REFERENCE_PROFILE_PROBES, TPM2_DEVICE,
    TPM2_MAJOR_VERSION, recorded_probe,
};
pub use crate::receipt::{
    AmountCents, MAX_AMOUNT_CENTS, MAX_PCR_ENTRIES, MAX_PCR_INDEX, PcrIndex, PcrSelection,
    ReceiptSigning,
};
pub use crate::register::{ClaimSource, ClaimStatus, P11_RECORDED_CLAIMS, RecordedClaim};
