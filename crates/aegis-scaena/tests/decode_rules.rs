// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! M27 criteria 5 and 6 (a), E27-2: the rules the decode half applies, held
//! without a GPU so they run inside `make verify-all`.
//!
//! Positive: the iHD vendor string is admitted; a surface that reports ready
//! on its third poll is ready after three; the reference data are
//! cros-libva's, byte for byte in length, offset and CRC; the recorded frame
//! pins match themselves under the recorded driver; the compositor's format
//! table is read with `pread` into its entries. Negative: the NVIDIA and
//! Mesa vendor strings and Intel's older i965 driver are refused; a surface
//! that never leaves `VASurfaceRendering` fails at its deadline and names the
//! surface; one changed pin under the same driver is a regression. Boundary:
//! a surface ready on its first poll takes one; one poll allowed and not
//! ready fails after exactly one; a status of ready and rendering together is
//! not ready; another driver's run reports its pins instead of holding them
//! (D73); a format table of zero entries is empty, and one whose size is not
//! a whole number of entries, above the bound or past the file is refused.

mod common;

use core::cell::Cell;
use core::time::Duration;

use aegis_scaena::decode::{
    DecodeError, IHD_VENDOR_PREFIX, ReadyWait, is_ready, poll_ready, require_ihd,
};
use aegis_scaena::fixture::{PinComparison, RECORDED_FRAME_CRCS, RECORDED_UNDER, compare_pins};
use aegis_scaena::present::{MAX_FORMAT_TABLE_BYTES, read_format_table};
use aegis_scaena::reference;
use rustix::io::write;

use common::{Fallible, memfd};

/// `VASurfaceRendering`.
const RENDERING: u32 = 1;

/// `VASurfaceReady`.
const READY: u32 = 4;

/// A short wait for the stuck-surface cases.
const SHORT: ReadyWait = ReadyWait {
    deadline: Duration::from_millis(40),
    max_polls: 1_000,
    interval: Duration::from_millis(1),
};

/// A status source that reports `status` until `ready_after` polls, then
/// ready, counting its polls.
fn source(ready_after: u32, status: u32, polls: &Cell<u32>) -> impl FnMut() -> Result<u32, i32> {
    move || {
        polls.set(polls.get().saturating_add(1));
        Ok(if polls.get() >= ready_after {
            READY
        } else {
            status
        })
    }
}

// --- Positive -------------------------------------------------------------

/// Positive: the iHD vendor string the reference profile reports is
/// admitted.
#[test]
fn the_ihd_vendor_string_is_admitted() -> Fallible {
    require_ihd("Intel iHD driver for Intel(R) Gen Graphics - 26.2.4 ()")?;
    assert!(RECORDED_UNDER.starts_with(IHD_VENDOR_PREFIX));
    Ok(())
}

/// Positive: a surface ready on its third poll is ready after three.
#[test]
fn a_surface_ready_on_its_third_poll_takes_three() -> Fallible {
    let polls = Cell::new(0);
    assert_eq!(poll_ready(7, SHORT, source(3, RENDERING, &polls))?, 3);
    Ok(())
}

/// Positive: the reference data are cros-libva's: the 197-byte clip, the
/// slice data at byte 47 and the CRC-32 its test asserts.
#[test]
fn the_reference_data_are_upstreams() {
    assert_eq!(reference::CLIP.len(), 197);
    assert_eq!(reference::SLICE_DATA_OFFSET, 47);
    assert_eq!(
        reference::CLIP.get(..4),
        Some(&[0x00, 0x00, 0x01, 0xb3][..])
    );
    assert_eq!(reference::UPSTREAM_CRC32, 0xa571_3e52);
    assert_eq!(reference::SLICE.first(), Some(&150));
}

/// Positive: the recorded pins match themselves under the recorded driver.
#[test]
fn the_recorded_pins_match_under_their_driver() {
    assert_eq!(
        compare_pins(RECORDED_UNDER, &RECORDED_FRAME_CRCS),
        PinComparison::Matched
    );
}

/// Positive: a format table of two entries is read with `pread`.
#[test]
fn a_format_table_is_read_into_its_entries() -> Fallible {
    let table = memfd("scaena-format-table", 0)?;
    let mut bytes = Vec::new();
    for (format, modifier) in [
        (0x3231_564e_u32, 0_u64),
        (0x3231_564e, 0x0100_0000_0000_0009),
    ] {
        bytes.extend_from_slice(&format.to_ne_bytes());
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&modifier.to_ne_bytes());
    }
    assert_eq!(write(&table, &bytes)?, 32);
    let entries = read_format_table(&table, 32)?;
    assert_eq!(
        entries,
        [(0x3231_564e, 0), (0x3231_564e, 0x0100_0000_0000_0009)]
    );
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative (a): the NVIDIA and Mesa vendor strings, and Intel's older i965
/// driver, are refused, each naming what the driver reported.
#[test]
fn every_other_vendor_string_is_refused() {
    for vendor in [
        "VA-API NVDEC driver [direct backend]",
        "Mesa Gallium driver 26.2.3-arch3.1 for AMD Radeon Graphics",
        "Intel i965 driver for Intel(R) Coffee Lake - 2.4.1",
        "",
    ] {
        assert_eq!(
            require_ihd(vendor),
            Err(DecodeError::NotIhd {
                vendor: vendor.to_owned()
            })
        );
    }
}

/// Negative: a surface that never leaves `VASurfaceRendering` fails at its
/// deadline and names the surface.
#[test]
fn a_stuck_surface_fails_at_its_deadline_naming_it() -> Fallible {
    let polls = Cell::new(0);
    let outcome = poll_ready(42, SHORT, source(u32::MAX, RENDERING, &polls));
    let Err(DecodeError::NotReady {
        surface,
        polls: made,
        status,
    }) = outcome
    else {
        return Err(format!("{outcome:?}").into());
    };
    assert_eq!((surface, status), (42, RENDERING));
    assert!((1..SHORT.max_polls).contains(&made), "{made}");
    let named = DecodeError::NotReady {
        surface,
        polls: made,
        status,
    };
    assert!(named.to_string().contains("surface 42"), "{named}");
    Ok(())
}

/// Negative: one changed pin under the recorded driver is a regression.
#[test]
fn a_changed_pin_under_the_same_driver_is_a_regression() {
    let mut measured = RECORDED_FRAME_CRCS;
    if let Some(last) = measured.last_mut() {
        *last ^= 1;
    }
    assert_eq!(
        compare_pins(RECORDED_UNDER, &measured),
        PinComparison::Regressed { differing: 1 }
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: ready on the first poll takes one; one poll allowed and not
/// ready fails after exactly one; a failed poll is a VA error.
#[test]
fn the_poll_bound_is_exact() -> Fallible {
    let polls = Cell::new(0);
    assert_eq!(poll_ready(1, SHORT, source(1, RENDERING, &polls))?, 1);
    let once = ReadyWait {
        max_polls: 1,
        ..SHORT
    };
    let stuck = Cell::new(0);
    assert!(matches!(
        poll_ready(2, once, source(u32::MAX, RENDERING, &stuck)),
        Err(DecodeError::NotReady { polls: 1, .. })
    ));
    assert_eq!(stuck.get(), 1);
    assert!(matches!(
        poll_ready(3, SHORT, || Err(-1)),
        Err(DecodeError::Va { status: -1, .. })
    ));
    Ok(())
}

/// Boundary: ready and rendering together is not ready; ready alone is.
#[test]
fn ready_and_rendering_together_is_not_ready() {
    assert!(is_ready(READY));
    assert!(!is_ready(RENDERING));
    assert!(!is_ready(READY | RENDERING));
    assert!(!is_ready(0));
}

/// Boundary (D73): another driver's run reports how many pins differ and
/// holds none of them.
#[test]
fn another_driver_reports_its_pins_instead_of_holding_them() {
    assert_eq!(
        compare_pins(
            "Intel iHD driver for Intel(R) Gen Graphics - 26.3.0 ()",
            &[0; 60]
        ),
        PinComparison::OtherDriver { differing: 60 }
    );
}

/// Boundary: an empty table is empty; a size that is not a whole number of
/// entries, above the bound, or past the end of the file is refused.
#[test]
fn format_table_sizes_are_held_to_whole_entries() -> Fallible {
    let table = memfd("scaena-format-table-bounds", 32)?;
    assert!(read_format_table(&table, 0)?.is_empty());
    assert!(read_format_table(&table, 31).is_err());
    assert!(
        read_format_table(&table, 48).is_err(),
        "past the end of the file"
    );
    let above = u32::try_from(MAX_FORMAT_TABLE_BYTES + 16)?;
    assert!(read_format_table(&table, above).is_err());
    Ok(())
}
