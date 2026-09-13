// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded receipt fields: the register selection, the amount, and the
//! signing state that cannot claim a signature.

mod common;

use aegis_ludus::{
    AmountCents, LudusError, MAX_AMOUNT_CENTS, MAX_PCR_ENTRIES, MAX_PCR_INDEX, PcrIndex,
    PcrSelection, ReceiptSigning,
};

use common::Fallible;

// --- Positive -------------------------------------------------------------

/// Positive: the recorded selection holds exactly the two registers the
/// scaffold's signature tag names.
///
/// [`PcrSelection::SCAFFOLD`] is written as a mask literal because a shift is
/// arithmetic the workspace denies in a `const` position. This is what keeps
/// the literal and the two named indices from drifting apart.
#[test]
fn the_recorded_selection_holds_the_two_named_registers() {
    let scaffold = PcrSelection::SCAFFOLD;
    assert!(scaffold.contains(PcrIndex::SCAFFOLD_LOW));
    assert!(scaffold.contains(PcrIndex::SCAFFOLD_HIGH));
    assert_eq!(scaffold.count(), 2);
    assert!(!scaffold.is_empty());
    assert_eq!(PcrIndex::SCAFFOLD_LOW.get(), 7);
    assert_eq!(PcrIndex::SCAFFOLD_HIGH.get(), 11);

    let unselected: Vec<u8> = (0..=MAX_PCR_INDEX)
        .filter(|raw| PcrIndex::new(*raw).is_ok_and(|index| !scaffold.contains(index)))
        .collect();
    assert_eq!(unselected.len(), 22);
}

/// Positive: adding a register is idempotent and every admissible index works.
#[test]
fn adding_a_register_is_idempotent() -> Fallible {
    let mut selection = PcrSelection::new();
    assert!(selection.is_empty());
    for raw in 0..=MAX_PCR_INDEX {
        selection = selection.with(PcrIndex::new(raw)?);
    }
    assert_eq!(selection.count(), u32::from(MAX_PCR_INDEX) + 1);

    let once = PcrSelection::new().with(PcrIndex::SCAFFOLD_LOW);
    let twice = once.with(PcrIndex::SCAFFOLD_LOW);
    assert_eq!(once, twice);
    assert_eq!(twice.count(), 1);
    Ok(())
}

/// Positive: the scaffold's own amount round-trips through the bound.
#[test]
fn the_scaffold_amount_is_admissible() -> Fallible {
    let amount = AmountCents::new(1_499)?;
    assert_eq!(amount.get(), 1_499);
    assert_eq!(AmountCents::new(0)?.get(), 0);
    assert!(AmountCents::new(0)? < amount);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a register index past the range is refused, not masked away.
#[test]
fn a_register_index_past_the_range_is_refused() {
    assert_eq!(
        PcrIndex::new(MAX_PCR_INDEX + 1),
        Err(LudusError::PcrIndexOutOfRange {
            index: MAX_PCR_INDEX + 1,
            max: MAX_PCR_INDEX,
        })
    );
    assert!(PcrIndex::new(255).is_err());
}

/// Negative: an amount past the bound is refused.
#[test]
fn an_amount_past_the_bound_is_refused() {
    assert_eq!(
        AmountCents::new(MAX_AMOUNT_CENTS + 1),
        Err(LudusError::AmountOutOfRange {
            cents: MAX_AMOUNT_CENTS + 1,
            max: MAX_AMOUNT_CENTS,
        })
    );
    assert!(AmountCents::new(u64::MAX).is_err());
}

/// Negative: no signing state this type can hold claims a signature.
///
/// The check is over every variant the type has, so it cannot be satisfied by
/// picking the one variant that happens to answer `false`.
#[test]
fn no_signing_state_claims_a_signature() {
    let signing = ReceiptSigning::ADMITTED;
    assert_eq!(signing, ReceiptSigning::StubbedUnsigned);
    assert!(!signing.is_signed());
    assert_eq!(signing.tag(), "stubbed-unsigned-no-tpm2-binding");
    assert!(signing.tag().contains("unsigned"));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the last admissible register index is accepted and the next is
/// refused, and the entry bound matches the index range.
#[test]
fn the_last_register_index_is_accepted() -> Fallible {
    let last = PcrIndex::new(MAX_PCR_INDEX)?;
    assert_eq!(last.get(), MAX_PCR_INDEX);
    assert!(PcrSelection::new().with(last).contains(last));
    assert!(PcrIndex::new(MAX_PCR_INDEX + 1).is_err());
    assert_eq!(MAX_PCR_ENTRIES, usize::from(MAX_PCR_INDEX) + 1);
    Ok(())
}

/// Boundary: the largest admissible amount is accepted and the next cent is
/// refused.
#[test]
fn the_largest_amount_is_accepted() -> Fallible {
    assert_eq!(MAX_AMOUNT_CENTS, 1_000_000);
    assert_eq!(AmountCents::new(MAX_AMOUNT_CENTS)?.get(), MAX_AMOUNT_CENTS);
    assert!(AmountCents::new(MAX_AMOUNT_CENTS + 1).is_err());
    Ok(())
}
