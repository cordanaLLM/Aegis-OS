// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fixtures shared by the integration tests.
//!
//! Everything here reaches `aegis-ludus` through its public API only, and
//! nothing here uses `unwrap` or `expect`: a test that cannot build its own
//! input returns the failure instead of panicking through a lint the workspace
//! denies.

#![allow(dead_code)]

use aegis_justitia::{Digest32, Identity, UnixSeconds};
use aegis_ludus::{
    AmountCents, AuthenticationOutcome, LaunchCommandLine, PayloadBuffer, PcrSelection,
    ReceiptSigning, TransactionReceipt, TransactionReceiptVersion,
};

/// What every test in this suite returns.
pub type Fallible = Result<(), Box<dyn std::error::Error>>;

/// The correlation identifier the fixtures thread through every payload.
pub const CORRELATION: &str = "m08-fixture-0001";

/// The transaction the scaffold signs, with its underscores made admissible.
pub const TRANSACTION: &str = "tx-ludus-88102";

/// The amount the scaffold's sample transaction carries, in cents.
pub const AMOUNT_CENTS: u64 = 1_499;

/// The scaffold's own sample launch command line.
pub const SAMPLE_COMMAND_LINE: &str =
    "./game_binary -steam +connect_auth_token=aegis_secure_session_token_99";

/// Returns the fixture correlation identifier.
///
/// # Errors
///
/// Propagates [`Identity::parse`].
pub fn correlation() -> Result<Identity, Box<dyn std::error::Error>> {
    Ok(Identity::parse(CORRELATION)?)
}

/// Returns an identity, so a fixture that stops parsing fails the suite.
///
/// # Errors
///
/// Propagates [`Identity::parse`].
pub fn identity(raw: &str) -> Result<Identity, Box<dyn std::error::Error>> {
    Ok(Identity::parse(raw)?)
}

/// Returns a command line of `count` arguments, none carrying the token.
#[must_use]
pub fn untokened_line(count: usize) -> String {
    let mut text = String::new();
    for index in 0..count {
        if index > 0 {
            text.push(' ');
        }
        text.push_str("-flag");
    }
    text
}

/// Returns a command line of `count` arguments whose last carries the token.
#[must_use]
pub fn tokened_line(count: usize) -> String {
    let mut text = untokened_line(count.saturating_sub(1));
    if !text.is_empty() {
        text.push(' ');
    }
    text.push_str("+connect_auth_token=fixture");
    text
}

/// Returns the scaffold's sample command line, parsed.
///
/// # Errors
///
/// Propagates [`LaunchCommandLine::parse`].
pub fn sample_line() -> Result<LaunchCommandLine, Box<dyn std::error::Error>> {
    Ok(LaunchCommandLine::parse(SAMPLE_COMMAND_LINE)?)
}

/// Returns the digest the fixtures put on a receipt.
#[must_use]
pub fn receipt_digest() -> Digest32 {
    Digest32::from_bytes([0x11u8; 32])
}

/// Returns a well-formed receipt sealed to `sealed_to` with `outcome`.
///
/// # Errors
///
/// Propagates the field constructors.
pub fn receipt(
    sealed_to: PcrSelection,
    outcome: AuthenticationOutcome,
) -> Result<TransactionReceipt, Box<dyn std::error::Error>> {
    Ok(TransactionReceipt {
        schema: TransactionReceiptVersion::V1,
        edge: TransactionReceipt::EDGE,
        correlation_id: correlation()?,
        transaction: identity(TRANSACTION)?,
        amount_cents: AmountCents::new(AMOUNT_CENTS)?,
        receipt_digest: receipt_digest(),
        sealed_to,
        signature: ReceiptSigning::ADMITTED,
        launch_authentication: outcome,
        issued_at: UnixSeconds::new(1_000),
    })
}

/// Returns the fixture receipt the scaffold's own transaction would produce.
///
/// # Errors
///
/// Propagates the field constructors.
pub fn scaffold_receipt() -> Result<TransactionReceipt, Box<dyn std::error::Error>> {
    receipt(PcrSelection::SCAFFOLD, AuthenticationOutcome::TokenPresent)
}

/// Renders a receipt to owned text.
///
/// # Errors
///
/// Propagates the contract refusal.
pub fn encoded_receipt(value: &TransactionReceipt) -> Result<String, Box<dyn std::error::Error>> {
    let mut buffer = PayloadBuffer::new();
    Ok(value.encode_into(&mut buffer)?.to_owned())
}

/// Returns `text` with the first occurrence of `from` replaced by `to`.
///
/// # Errors
///
/// Returns a failure when `from` does not occur, so a tamper that stops
/// applying fails the suite instead of testing the untampered payload.
pub fn tamper(text: &str, from: &str, to: &str) -> Result<String, Box<dyn std::error::Error>> {
    if !text.contains(from) {
        return Err(format!("the fixture payload does not contain {from:?}").into());
    }
    Ok(text.replacen(from, to, 1))
}

#[path = "../../../source_effects.rs"]
mod source_effects;

#[allow(unused_imports)]
pub use source_effects::{LINE_BOUND, WALK_BOUND, hits, rust_sources, scan_sources, scan_text};

/// Returns the crate's own `src/` directory.
#[must_use]
pub fn source_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}
