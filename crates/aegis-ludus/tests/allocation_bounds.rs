// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! HISS-03: the claim that no decision path allocates, made falsifiable.
//!
//! The reason this crate can say a decision path allocates nothing is
//! structural: every value on such a path is `Copy`, so parsing a command line
//! or building a receipt writes into storage that already exists.
//! `assert_no_heap::<T>()` is the executable form: a `Vec`, `String` or `Box`
//! anywhere inside one of these types, at any depth, removes `Copy` and this
//! file stops compiling, which fails the gate.
//!
//! # What is deliberately not claimed
//!
//! **Decoding is not unconditionally allocation-free, and this file says so by
//! exercising the case rather than by denying it.** A payload that spells a
//! value with a JSON escape makes `serde_json` unescape the string into a heap
//! scratch buffer before any field of ours sees it. That is bounded by
//! [`MAX_CONTRACT_PAYLOAD_BYTES`] and never retained, and the decoded value
//! still owns no heap -- which is the claim the crate actually makes.

mod common;

#[path = "../../test_assertions.rs"]
mod test_assertions;

use core::mem::size_of;

use aegis_ludus::{
    AmountCents, AuthenticationOutcome, ContractError, Correlation, CredentialProbe,
    LaunchArgument, LaunchCommandLine, LudusError, MAX_CONTRACT_PAYLOAD_BYTES, MAX_LAUNCH_ARGS,
    PayloadBuffer, PcrIndex, PcrSelection, ReceiptSigning, TransactionReceipt,
};

use common::{Fallible, encoded_receipt, scaffold_receipt, tamper, untokened_line};

/// Accepts only a type that owns no heap, because it is `Copy`.
fn assert_no_heap<T: Copy>() {}

// --- Positive -------------------------------------------------------------

/// Positive: every value on a decision path owns no heap.
///
/// This is the falsifier. Adding a `Vec`, `String` or `Box` to any of these
/// types, or to anything they contain, removes `Copy` and this test stops
/// compiling.
#[test]
fn every_value_on_a_decision_path_owns_no_heap() {
    assert_no_heap::<LaunchArgument>();
    assert_no_heap::<LaunchCommandLine>();
    assert_no_heap::<AuthenticationOutcome>();
    assert_no_heap::<AmountCents>();
    assert_no_heap::<PcrIndex>();
    assert_no_heap::<PcrSelection>();
    assert_no_heap::<ReceiptSigning>();
    assert_no_heap::<TransactionReceipt>();
    assert_no_heap::<CredentialProbe>();
    assert_no_heap::<PayloadBuffer>();
    assert_no_heap::<LudusError>();
    assert_no_heap::<ContractError>();
    assert_no_heap::<Correlation>();
}

/// Positive: filling a command line to its bound changes no size.
#[test]
fn filling_the_command_line_changes_no_storage() -> Fallible {
    let full = LaunchCommandLine::parse(&untokened_line(MAX_LAUNCH_ARGS))?;
    assert_eq!(size_of_val(&full), size_of::<LaunchCommandLine>());
    assert_eq!(full.len(), MAX_LAUNCH_ARGS);
    assert!(size_of::<LaunchCommandLine>() > size_of::<LaunchArgument>());
    let empty = LaunchCommandLine::empty();
    assert_eq!(size_of_val(&empty), size_of_val(&full));
    Ok(())
}

/// Positive: encoding writes into the caller's buffer and returns a borrow of
/// it, so a payload that fits costs no allocation of ours.
#[test]
fn encoding_writes_into_the_callers_buffer() -> Fallible {
    let receipt = scaffold_receipt()?;
    test_assertions::assert_encoding_writes_into_payload_buffer!(
        PayloadBuffer,
        |buffer| receipt.encode_into(buffer).map(str::len),
        MAX_CONTRACT_PAYLOAD_BYTES,
    )?;
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a decoded payload owns no heap even when the input made the
/// parser allocate, which is the case this crate does not deny.
#[test]
fn an_escaped_payload_still_decodes_into_an_inline_value() -> Fallible {
    let text = encoded_receipt(&scaffold_receipt()?)?;
    let escaped = tamper(&text, "tx-ludus-88102", "tx\\u002Dludus-88102")?;
    let decoded = TransactionReceipt::decode(&escaped)?;
    assert_eq!(decoded.transaction, common::identity(common::TRANSACTION)?);
    assert_eq!(size_of_val(&decoded), size_of::<TransactionReceipt>());
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the payload stays well inside the buffer it is encoded into, so
/// the bound is a bound and not a fit.
#[test]
fn the_payload_stays_inside_the_contract_bound() -> Fallible {
    let text = encoded_receipt(&scaffold_receipt()?)?;
    test_assertions::assert_payload_inside_contract_bound(
        text.len(),
        MAX_CONTRACT_PAYLOAD_BYTES,
        size_of::<PayloadBuffer>(),
    );
    Ok(())
}
