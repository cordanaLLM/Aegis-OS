// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded fields of a microtransaction receipt (REQ-P11-04).
//!
//! REQ-P11-04 asks for payment data and session keys sealed to specific TPM2
//! platform configuration registers. The scaffold's stand-in for that is
//!
//! ```text
//! format!("tpm2_sig_pcr7_pcr11_{}", tx_id)
//! ```
//!
//! which is a string built from the transaction identifier. It names two
//! registers, seals nothing and signs nothing, and a caller cannot tell it from
//! a real signature by looking at its type.
//!
//! This module keeps the two register numbers as data and refuses to keep the
//! rest. [`PcrSelection`] is the recorded selection, [`AmountCents`] bounds the
//! figure, and [`ReceiptSigning`] has exactly **one** variant, which says on
//! the wire that the receipt is unsigned and that no key is bound. A payload
//! claiming a hardware signature therefore does not decode -- the same device
//! decision D06 used in `aegis-vesta` for the rejected runtime.
//!
//! # What is not here
//!
//! No TPM2 is opened, no quote is taken, no key is bound, no register is read
//! and no hash is computed. The receipt digest is a field the caller supplies.
//! `src/probe.rs` records what the reference profile actually has, which is a
//! TPM2 and no FIDO2 authenticator, and `src/register.rs` records the FIDO2
//! half as a procurement dependency rather than as a stub.

use crate::error::LudusError;

/// The largest platform configuration register index this crate admits.
///
/// Twenty-three is the last register of a TPM 2.0 platform's first locality
/// bank, so the range is `0..=23`. The scaffold declares no range at all.
pub const MAX_PCR_INDEX: u8 = 23;

/// How many register indices a selection can hold, one per admissible index.
pub const MAX_PCR_ENTRIES: usize = 24;

/// Scalar upper bound, in cents, on one receipt amount.
///
/// Chosen here, not recorded: no source states a bound, and an unbounded
/// figure on a payload is a field that can carry anything. It is a payload
/// bound and not a spending policy; nothing in this crate authorises a payment.
pub const MAX_AMOUNT_CENTS: u64 = 1_000_000;

/// One validated platform configuration register index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PcrIndex(u8);

impl PcrIndex {
    /// The lower of the two registers the scaffold's signature tag names.
    ///
    /// Recorded from `tpm2_sig_pcr7_pcr11`. What a platform actually measures
    /// into this register is not asserted here.
    pub const SCAFFOLD_LOW: Self = Self(7);

    /// The higher of the two registers the scaffold's signature tag names.
    pub const SCAFFOLD_HIGH: Self = Self(11);

    /// Parses `index` into a register index.
    ///
    /// # Errors
    ///
    /// Returns [`LudusError::PcrIndexOutOfRange`] past [`MAX_PCR_INDEX`].
    pub fn new(index: u8) -> Result<Self, LudusError> {
        if index > MAX_PCR_INDEX {
            return Err(LudusError::PcrIndexOutOfRange {
                index,
                max: MAX_PCR_INDEX,
            });
        }
        Ok(Self(index))
    }

    /// Returns the register index.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// The registers a receipt declares itself sealed to.
///
/// A bit per admissible index, so a selection is inline, `Copy` and free of
/// duplicates by construction. The empty selection is representable and is
/// refused by the contract, because a receipt that names no register has made
/// no sealing claim at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PcrSelection {
    mask: u32,
}

impl PcrSelection {
    /// The selection the scaffold's signature tag names: registers 7 and 11.
    ///
    /// Written as the mask literal rather than built from
    /// [`PcrIndex::SCAFFOLD_LOW`] and [`PcrIndex::SCAFFOLD_HIGH`], because a
    /// shift is arithmetic the workspace denies in a `const` position.
    /// `tests/receipt_fields.rs` asserts that the literal holds exactly those
    /// two registers, so the two spellings cannot drift apart.
    pub const SCAFFOLD: Self = Self {
        mask: 0b1000_1000_0000,
    };

    /// Builds the empty selection.
    #[must_use]
    pub const fn new() -> Self {
        Self { mask: 0 }
    }

    /// Returns the bit `index` occupies, or zero if it could not be formed.
    ///
    /// A checked shift rather than `1 << index`: the workspace denies
    /// arithmetic that can panic, and [`PcrIndex`] already refuses anything
    /// past [`MAX_PCR_INDEX`], so the zero arm is unreachable by construction
    /// rather than by assumption.
    fn bit(index: PcrIndex) -> u32 {
        1u32.checked_shl(u32::from(index.get())).unwrap_or(0)
    }

    /// Returns the selection with `index` added. Adding twice is idempotent.
    #[must_use]
    pub fn with(self, index: PcrIndex) -> Self {
        Self {
            mask: self.mask | Self::bit(index),
        }
    }

    /// Returns `true` when `index` is selected.
    #[must_use]
    pub fn contains(self, index: PcrIndex) -> bool {
        self.mask & Self::bit(index) != 0
    }

    /// Returns how many registers are selected.
    #[must_use]
    pub const fn count(self) -> u32 {
        self.mask.count_ones()
    }

    /// Returns `true` when no register is selected.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.mask == 0
    }
}

/// One validated receipt amount, in cents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AmountCents(u64);

impl AmountCents {
    /// Parses `cents` into an amount.
    ///
    /// # Errors
    ///
    /// Returns [`LudusError::AmountOutOfRange`] past [`MAX_AMOUNT_CENTS`].
    /// Zero is admissible: a free item still produces a receipt.
    pub fn new(cents: u64) -> Result<Self, LudusError> {
        if cents > MAX_AMOUNT_CENTS {
            return Err(LudusError::AmountOutOfRange {
                cents,
                max: MAX_AMOUNT_CENTS,
            });
        }
        Ok(Self(cents))
    }

    /// Returns the amount in cents.
    ///
    /// Deliberately not a currency value and deliberately not a float: the
    /// scaffold divides by `100.0` to print one, and a rounded figure is not
    /// what a receipt is about.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// How a receipt was signed.
///
/// One variant, on purpose, and the same device `aegis-vesta` used for the
/// runtime D06 rejected: a single-variant enum makes the claim this milestone
/// cannot support **unspellable** rather than merely discouraged. No TPM2 key
/// is bound anywhere in this repository, so no receipt can say it was signed by
/// one, on the wire or in memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum ReceiptSigning {
    /// The receipt is unsigned and no TPM2 key is bound to it.
    #[serde(rename = "stubbed-unsigned-no-tpm2-binding")]
    StubbedUnsigned,
}

impl ReceiptSigning {
    /// The only signing state this build admits.
    pub const ADMITTED: Self = Self::StubbedUnsigned;

    /// Returns the stable tag the wire form carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::StubbedUnsigned => "stubbed-unsigned-no-tpm2-binding",
        }
    }

    /// Returns `false`; no variant of this type carries a signature.
    ///
    /// Written as an exhaustive match rather than as a `false` literal, so a
    /// later variant that did carry one would fail to compile until someone
    /// decided what this returns for it.
    #[must_use]
    pub const fn is_signed(self) -> bool {
        match self {
            Self::StubbedUnsigned => false,
        }
    }
}
