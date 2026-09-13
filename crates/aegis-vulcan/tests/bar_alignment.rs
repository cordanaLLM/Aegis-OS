// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E17-1, the BAR half: an aligned BAR is accepted, a misaligned one is
//! rejected, and the alignment rule is exact at its own boundary.
//!
//! REQ-P03-04 is the rule, and the scaffold states it as an assertion inside
//! the mapping call. Here it is [`BarAddress::new`], so the negative case is
//! this ordinary test rather than a test that expects a process abort.

mod common;

use aegis_vulcan::{
    BAR_ALIGNMENT_BYTES, BarAddress, BarSize, BarWindow, MAX_BAR_BYTES, UserSpacePcieDriver,
    VulcanError,
};

use common::{ALIGNED_BAR, BAR_BYTES, Fallible, MISALIGNED_BAR, device};

// --- Positive -------------------------------------------------------------

/// Positive: an aligned BAR is accepted, and mapping it yields the window.
#[test]
fn an_aligned_bar_is_accepted_and_maps() -> Fallible {
    let address = BarAddress::new(ALIGNED_BAR)?;
    assert_eq!(address.get(), ALIGNED_BAR);
    assert_eq!(address.get() % BAR_ALIGNMENT_BYTES, 0);

    let mut driver = UserSpacePcieDriver::new(device()?);
    let window = driver.map_bar()?;
    assert_eq!(window.address.get(), ALIGNED_BAR);
    assert_eq!(window.size.get(), BAR_BYTES);
    assert_eq!(window.end(), ALIGNED_BAR.saturating_add(BAR_BYTES));
    Ok(())
}

/// Positive: a window validates the relationship its fields cannot.
#[test]
fn a_validated_window_restates_its_own_invariant() -> Fallible {
    let window = BarWindow::parse(ALIGNED_BAR, BAR_BYTES)?;
    assert_eq!(window.validate(), Ok(()));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a misaligned BAR is rejected, and is named in the refusal.
#[test]
fn a_misaligned_bar_is_rejected() {
    assert_eq!(
        BarAddress::new(MISALIGNED_BAR),
        Err(VulcanError::BarMisaligned {
            address: MISALIGNED_BAR
        })
    );
    assert_eq!(
        BarWindow::parse(MISALIGNED_BAR, BAR_BYTES),
        Err(VulcanError::BarMisaligned {
            address: MISALIGNED_BAR
        })
    );
}

/// Negative: the address is refused, never rounded to the nearest page.
#[test]
fn a_misaligned_bar_is_not_rounded() {
    let rounded = MISALIGNED_BAR.saturating_sub(1);
    assert!(BarAddress::new(MISALIGNED_BAR).is_err());
    assert_eq!(
        BarAddress::new(rounded).map(BarAddress::get),
        Ok(ALIGNED_BAR)
    );
}

/// Negative: a window that would leave the address space is refused.
#[test]
fn a_window_past_the_address_space_is_refused() -> Fallible {
    let address = BarAddress::new(u64::MAX.saturating_sub(BAR_ALIGNMENT_BYTES - 1))?;
    let size = BarSize::new(BAR_ALIGNMENT_BYTES.saturating_mul(2))?;
    assert_eq!(
        BarWindow::new(address, size),
        Err(VulcanError::BarWindowOverflow {
            address: address.get(),
            bytes: size.get(),
        })
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the alignment rule is exact on both sides of one page.
#[test]
fn the_alignment_rule_is_exact_at_a_page() {
    assert!(BarAddress::new(0).is_ok());
    assert!(BarAddress::new(BAR_ALIGNMENT_BYTES).is_ok());
    assert!(BarAddress::new(BAR_ALIGNMENT_BYTES.saturating_sub(1)).is_err());
    assert!(BarAddress::new(BAR_ALIGNMENT_BYTES.saturating_add(1)).is_err());
}

/// Boundary: the window width is admitted at its bound and refused past it.
#[test]
fn the_window_width_is_admitted_at_its_bound() {
    assert!(BarSize::new(0).is_err());
    assert!(BarSize::new(1).is_ok());
    assert_eq!(
        BarSize::new(MAX_BAR_BYTES).map(BarSize::get),
        Ok(MAX_BAR_BYTES)
    );
    assert_eq!(
        BarSize::new(MAX_BAR_BYTES.saturating_add(1)),
        Err(VulcanError::BarSizeOutOfRange {
            bytes: MAX_BAR_BYTES.saturating_add(1)
        })
    );
}

/// Boundary: the width is deliberately not required to be page aligned.
///
/// REQ-P03-04 is a rule about the address. A width rule would refuse windows
/// the recorded requirement admits, so the type does not invent one, and this
/// case is what would fail if someone added it.
#[test]
fn the_window_width_need_not_be_page_aligned() -> Fallible {
    let window = BarWindow::parse(ALIGNED_BAR, 1)?;
    assert_eq!(window.size.get(), 1);
    assert_eq!(window.end(), ALIGNED_BAR.saturating_add(1));
    Ok(())
}
