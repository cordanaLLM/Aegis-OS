// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The microtransaction receipt P11 Ludus hands to P02 Janus and Vallum.
//!
//! # Where the fields come from
//!
//! | Field | Source |
//! | :-- | :-- |
//! | `edge` | export-062 `1ce919ed54bb`, edge `ATTEST_GAME_TRANSACTION` (REQ-P11-06) |
//! | `transaction`, `amount-cents` | export-033 `531cbdf98eb5`, `verify_tpm2_transaction` |
//! | `sealed-to` | the two registers the scaffold's signature tag names (REQ-P11-04) |
//! | `receipt-digest` | the caller's digest of the receipt body |
//! | `launch-authentication` | the outcome of the launch validator that opened the session |
//! | `correlation-id`, `issued-at` | `docs/integration/stack.md` and the M14 field encodings |
//!
//! # The signature field is required and says the receipt is unsigned
//!
//! [`ReceiptSigning`] has one variant, whose wire tag is
//! `stubbed-unsigned-no-tpm2-binding`. Two things follow, and both are the
//! point:
//!
//! * a payload that omits the field **does not decode** -- there is no default,
//!   so a receipt cannot arrive silent about its own signing state;
//! * a payload that claims a hardware signature does not decode either, because
//!   there is no variant to name. No TPM2 key is bound anywhere in this
//!   repository, so no receipt produced here can say one was.
//!
//! The reference profile does have the chip: `ls /sys/class/tpm/` prints
//! `tpm0` and `cat /sys/class/tpm/tpm0/tpm_version_major` prints `2`. It does
//! **not** have the other credential P11 names: the recorded `lsusb` filter for
//! an authenticator matched nothing. [`probe`](crate::probe) records all three
//! readings, and `register` records the FIDO2 half as a procurement dependency
//! rather than as something a stub covers.
//!
//! # What this module does not do
//!
//! No transport is implemented, no register is read, no quote is taken and no
//! hash is computed. The digest is a field the caller supplies, and a receipt
//! that names no register at all is refused rather than accepted as sealed to
//! nothing.

use aegis_justitia::{Digest32, Identity, UnixSeconds};

use crate::contracts::graph::EdgeId;
use crate::contracts::{
    ContractError, Correlation, PayloadBuffer, SchemaId, decode_text, encode_into,
};
use crate::launch::AuthenticationOutcome;
use crate::receipt::{AmountCents, PcrSelection, ReceiptSigning};

/// The contract versions of the transaction receipt this build admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum TransactionReceiptVersion {
    /// Version 1, tagged `aegis.p11-p02.transaction-receipt.v1`.
    #[serde(rename = "aegis.p11-p02.transaction-receipt.v1")]
    V1,
}

/// One microtransaction receipt P11 asks P02 to record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct TransactionReceipt {
    /// The contract version this payload claims.
    pub schema: TransactionReceiptVersion,
    /// The subsystem-graph edge the payload travels on.
    pub edge: EdgeId,
    /// The identifier threading this receipt to its record.
    pub correlation_id: Identity,
    /// The transaction the receipt is about.
    pub transaction: Identity,
    /// What the transaction cost, in cents.
    pub amount_cents: AmountCents,
    /// The digest of the receipt body the caller computed.
    pub receipt_digest: Digest32,
    /// The platform registers the receipt declares itself sealed to.
    pub sealed_to: PcrSelection,
    /// How the receipt was signed. Never a claim of hardware signing.
    pub signature: ReceiptSigning,
    /// What the launch validator concluded about the session that produced it.
    pub launch_authentication: AuthenticationOutcome,
    /// When the receipt was issued.
    pub issued_at: UnixSeconds,
}

impl TransactionReceipt {
    /// The contract this type instantiates.
    pub const SCHEMA: SchemaId = SchemaId::TransactionReceipt;

    /// The edge the receipt travels on, kept from the graph of record.
    pub const EDGE: EdgeId = EdgeId::AttestGameTransaction;

    /// The signing state every receipt this build produces carries.
    pub const SIGNING: ReceiptSigning = ReceiptSigning::ADMITTED;

    /// Returns what a refusal of this payload is about.
    #[must_use]
    pub const fn correlation(&self) -> Correlation {
        Correlation::new(Self::SCHEMA, Some(self.correlation_id))
    }

    /// Checks the invariants the field types cannot express.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::NoSealedRegisters`] when the receipt names no
    /// platform register: a receipt whose whole purpose is a sealing claim must
    /// say what it is sealed to, and an empty selection is a claim about
    /// nothing.
    ///
    /// The edge needs no check of its own: [`EdgeId`] admits exactly the edge
    /// this schema travels on, so a payload naming another is refused by the
    /// decoder before this runs. Neither does the signature: the field is
    /// required and its type has one variant, so a payload that omits it or
    /// claims a hardware signature is refused by the decoder too.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.sealed_to.is_empty() {
            return Err(ContractError::NoSealedRegisters {
                correlation: self.correlation(),
            });
        }
        Ok(())
    }

    /// Encodes a validated receipt into `buffer`.
    ///
    /// # Errors
    ///
    /// Propagates [`Self::validate`], and returns
    /// [`ContractError::PayloadTooLong`] when the payload does not fit.
    pub fn encode_into<'b>(&self, buffer: &'b mut PayloadBuffer) -> Result<&'b str, ContractError> {
        self.validate()?;
        encode_into(self, self.correlation(), buffer)
    }

    /// Decodes and validates one transaction receipt payload.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::UnknownVersion`] for a payload naming another
    /// contract version, [`ContractError::Malformed`] for anything else the
    /// schema refuses -- an absent `signature` field, an unknown field, an
    /// out-of-range amount or register index -- [`ContractError::PayloadTooLong`]
    /// past the byte bound, and [`ContractError::NoSealedRegisters`] for a
    /// receipt sealed to nothing.
    pub fn decode(text: &str) -> Result<Self, ContractError> {
        let decoded: Self = decode_text(Self::SCHEMA, text)?;
        decoded.validate()?;
        Ok(decoded)
    }
}
