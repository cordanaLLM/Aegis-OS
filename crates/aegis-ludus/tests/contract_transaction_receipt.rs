// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E08-3, the P11 half: the transaction receipt P11 hands to P02.
//!
//! Positive: the descriptor round-trips. Negative: **a receipt without a
//! signature field is rejected**. Boundary: the register selection is exercised
//! at both ends of its range on the wire.

mod common;

use aegis_ludus::{
    AuthenticationOutcome, ContractError, EdgeId, MAX_CONTRACT_PAYLOAD_BYTES, MAX_PCR_INDEX,
    PayloadBuffer, PcrIndex, PcrSelection, ReceiptSigning, SchemaId, TransactionReceipt,
    TransactionReceiptVersion,
};

use common::{Fallible, encoded_receipt, receipt, scaffold_receipt, tamper};

// --- Positive -------------------------------------------------------------

/// Positive: a receipt round-trips with every field intact.
#[test]
fn a_receipt_round_trips() -> Fallible {
    let original = scaffold_receipt()?;
    let text = encoded_receipt(&original)?;
    let decoded = TransactionReceipt::decode(&text)?;
    assert_eq!(decoded, original);
    assert_eq!(decoded.schema, TransactionReceiptVersion::V1);
    assert_eq!(decoded.edge, EdgeId::AttestGameTransaction);
    assert_eq!(decoded.signature, ReceiptSigning::StubbedUnsigned);
    assert_eq!(decoded.sealed_to.count(), 2);
    assert_eq!(decoded.amount_cents.get(), common::AMOUNT_CENTS);
    assert_eq!(decoded.issued_at.get(), 1_000);
    assert_eq!(decoded.receipt_digest, common::receipt_digest());
    assert_eq!(
        decoded.launch_authentication,
        AuthenticationOutcome::TokenPresent
    );
    Ok(())
}

/// Positive: the wire form says on its face that the receipt is unsigned, and
/// names the two registers rather than a bitmask integer.
#[test]
fn the_wire_form_says_the_receipt_is_unsigned() -> Fallible {
    let text = encoded_receipt(&scaffold_receipt()?)?;
    assert!(text.contains("\"signature\":\"stubbed-unsigned-no-tpm2-binding\""));
    assert!(text.contains("\"sealed-to\":[7,11]"));
    assert!(text.contains("\"edge\":\"ATTEST_GAME_TRANSACTION\""));
    assert!(text.contains("\"schema\":\"aegis.p11-p02.transaction-receipt.v1\""));
    assert!(text.contains("\"launch-authentication\":\"token-present\""));
    Ok(())
}

/// Positive: the schema and the payload's own constants agree.
#[test]
fn the_schema_and_the_payload_constants_agree() -> Fallible {
    assert_eq!(TransactionReceipt::SCHEMA, SchemaId::TransactionReceipt);
    assert_eq!(
        SchemaId::TransactionReceipt.edge(),
        TransactionReceipt::EDGE
    );
    assert_eq!(
        SchemaId::TransactionReceipt.tag(),
        "aegis.p11-p02.transaction-receipt.v1"
    );
    assert_eq!(
        SchemaId::TransactionReceipt.to_string(),
        SchemaId::TransactionReceipt.tag()
    );
    assert_eq!(TransactionReceipt::SIGNING, ReceiptSigning::ADMITTED);

    let correlation = scaffold_receipt()?.correlation();
    assert_eq!(correlation.schema(), TransactionReceipt::SCHEMA);
    assert_eq!(correlation.id(), Some(common::correlation()?));
    assert!(correlation.to_string().contains(common::CORRELATION));
    Ok(())
}

/// Positive: the edge is the graph of record's, with its single source and the
/// presence edge held back beside it.
#[test]
fn the_edge_is_the_graph_of_records() {
    assert_eq!(EdgeId::ALL, [EdgeId::AttestGameTransaction]);
    assert_eq!(
        EdgeId::AttestGameTransaction.name(),
        "ATTEST_GAME_TRANSACTION"
    );
    assert_eq!(EdgeId::AttestGameTransaction.consumer(), "P02");
    assert_eq!(
        EdgeId::AttestGameTransaction.recorded_transport(),
        "Hardware TPM2 PCR Quote"
    );
    assert_eq!(EdgeId::AttestGameTransaction.sources(), 1);
    assert_eq!(EdgeId::UNTYPED_PRESENCE_EDGE, "DISPATCH_RICH_PRESENCE");
}

// --- Negative -------------------------------------------------------------

/// Negative: a receipt without a signature field is rejected.
///
/// This is the epic's negative case. The field is required and has no default,
/// so the payload is refused rather than decoded into an assumed signing state.
#[test]
fn a_receipt_without_a_signature_field_is_rejected() -> Fallible {
    let text = encoded_receipt(&scaffold_receipt()?)?;
    let stripped = tamper(
        &text,
        "\"signature\":\"stubbed-unsigned-no-tpm2-binding\",",
        "",
    )?;
    assert!(!stripped.contains("signature"));
    let refusal = TransactionReceipt::decode(&stripped);
    assert!(matches!(refusal, Err(ContractError::Malformed { .. })));
    let Err(error) = refusal else {
        return Err("a receipt with no signature field must be refused".into());
    };
    assert_eq!(error.correlation().id(), Some(common::correlation()?));
    Ok(())
}

/// Negative: a receipt claiming a hardware signature does not decode either.
///
/// The signing type has one variant, so there is no tag a payload can use to
/// say a TPM2 signed it. That is the point of the single variant: the claim is
/// unspellable rather than merely discouraged.
#[test]
fn a_receipt_claiming_a_hardware_signature_is_rejected() -> Fallible {
    let text = encoded_receipt(&scaffold_receipt()?)?;
    for claimed in [
        "tpm2-pcr-quote",
        "tpm2_sig_pcr7_pcr11_tx_ludus_88102",
        "signed",
    ] {
        let forged = tamper(
            &text,
            "\"stubbed-unsigned-no-tpm2-binding\"",
            &format!("\"{claimed}\""),
        )?;
        assert!(matches!(
            TransactionReceipt::decode(&forged),
            Err(ContractError::Malformed { .. })
        ));
    }
    Ok(())
}

/// Negative: a payload naming another contract version, another edge or an
/// unknown field is refused.
#[test]
fn a_payload_naming_the_wrong_contract_is_refused() -> Fallible {
    let text = encoded_receipt(&scaffold_receipt()?)?;

    let other_version = tamper(
        &text,
        "aegis.p11-p02.transaction-receipt.v1",
        "aegis.p11-p02.transaction-receipt.v2",
    )?;
    assert!(matches!(
        TransactionReceipt::decode(&other_version),
        Err(ContractError::UnknownVersion { .. })
    ));

    let other_edge = tamper(&text, "ATTEST_GAME_TRANSACTION", "DISPATCH_RICH_PRESENCE")?;
    assert!(matches!(
        TransactionReceipt::decode(&other_edge),
        Err(ContractError::Malformed { .. })
    ));

    let unknown_field = tamper(&text, "{", "{\"refund\":true,")?;
    assert!(matches!(
        TransactionReceipt::decode(&unknown_field),
        Err(ContractError::Malformed { .. })
    ));
    Ok(())
}

/// Negative: a payload carrying an out-of-range amount or register index is
/// refused by the decoder, not accepted and checked afterwards.
#[test]
fn a_payload_carrying_an_out_of_range_value_is_refused() -> Fallible {
    let text = encoded_receipt(&scaffold_receipt()?)?;

    let over_amount = tamper(&text, "\"amount-cents\":1499", "\"amount-cents\":1000001")?;
    assert!(matches!(
        TransactionReceipt::decode(&over_amount),
        Err(ContractError::Malformed { .. })
    ));

    let over_register = tamper(&text, "\"sealed-to\":[7,11]", "\"sealed-to\":[7,24]")?;
    assert!(matches!(
        TransactionReceipt::decode(&over_register),
        Err(ContractError::Malformed { .. })
    ));
    Ok(())
}

/// Negative: a receipt sealed to no register at all is refused, in memory and
/// on the wire.
#[test]
fn a_receipt_sealed_to_nothing_is_refused() -> Fallible {
    let empty = receipt(PcrSelection::new(), AuthenticationOutcome::TokenPresent)?;
    assert!(matches!(
        empty.validate(),
        Err(ContractError::NoSealedRegisters { .. })
    ));
    let mut buffer = PayloadBuffer::new();
    assert!(matches!(
        empty.encode_into(&mut buffer),
        Err(ContractError::NoSealedRegisters { .. })
    ));
    assert!(buffer.is_empty());

    let text = encoded_receipt(&scaffold_receipt()?)?;
    let emptied = tamper(&text, "\"sealed-to\":[7,11]", "\"sealed-to\":[]")?;
    assert!(matches!(
        TransactionReceipt::decode(&emptied),
        Err(ContractError::NoSealedRegisters { .. })
    ));
    Ok(())
}

/// Negative: a payload past the byte bound is refused before it is parsed.
#[test]
fn a_payload_past_the_byte_bound_is_refused() {
    let text = "x".repeat(MAX_CONTRACT_PAYLOAD_BYTES + 1);
    let refusal = TransactionReceipt::decode(&text);
    assert!(matches!(
        refusal,
        Err(ContractError::PayloadTooLong {
            max: MAX_CONTRACT_PAYLOAD_BYTES,
            ..
        })
    ));
    let Err(error) = refusal else {
        return;
    };
    assert_eq!(error.correlation().id(), None);
    assert!(error.correlation().to_string().contains("uncorrelated"));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a receipt sealed to one register is admissible, and one sealed to
/// every admissible register still fits the payload bound.
#[test]
fn the_register_selection_holds_at_both_ends() -> Fallible {
    let single = receipt(
        PcrSelection::new().with(PcrIndex::SCAFFOLD_LOW),
        AuthenticationOutcome::NoLaunchToken,
    )?;
    let decoded = TransactionReceipt::decode(&encoded_receipt(&single)?)?;
    assert_eq!(decoded.sealed_to.count(), 1);
    assert!(decoded.sealed_to.contains(PcrIndex::SCAFFOLD_LOW));

    let mut every = PcrSelection::new();
    for raw in 0..=MAX_PCR_INDEX {
        every = every.with(PcrIndex::new(raw)?);
    }
    let full = receipt(every, AuthenticationOutcome::EmptyCommandLine)?;
    let text = encoded_receipt(&full)?;
    assert!(text.len() < MAX_CONTRACT_PAYLOAD_BYTES);
    let decoded = TransactionReceipt::decode(&text)?;
    assert_eq!(decoded.sealed_to.count(), u32::from(MAX_PCR_INDEX) + 1);
    assert_eq!(
        decoded.launch_authentication,
        AuthenticationOutcome::EmptyCommandLine
    );
    Ok(())
}

/// Boundary: every authentication outcome survives the wire, including the one
/// the scaffold could not express.
#[test]
fn every_authentication_outcome_round_trips() -> Fallible {
    for outcome in AuthenticationOutcome::ALL {
        let value = receipt(PcrSelection::SCAFFOLD, outcome)?;
        let decoded = TransactionReceipt::decode(&encoded_receipt(&value)?)?;
        assert_eq!(decoded.launch_authentication, outcome);
        assert_eq!(
            decoded.launch_authentication.is_authenticated(),
            outcome.is_authenticated()
        );
    }
    Ok(())
}
