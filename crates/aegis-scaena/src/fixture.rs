// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The committed baseline Motion-JPEG fixture and its regression pins (M27
//! criteria 5 and 8).
//!
//! `fixtures/index-blocks-60.mjpeg` is generated for Aegis and carries the
//! project licence (REUSE.toml). It is 60 baseline JPEG frames of 256 x 64,
//! 4:2:0, each drawing its frame number, 1 to 60, as the index row
//! [`crate::content`] reads back. Its provenance, run once with ffmpeg
//! n9.0.2 and never by a gate, is recorded in `docs/build/display.md` and
//! `tools/verify_display.py`, which pins its sha256 and refuses another file
//! before anything is built.
//!
//! [`RECORDED_FRAME_CRCS`] is the CRC-32 of each decoded frame's visible NV12
//! lines as the first passing run measured it, under the driver named by
//! [`RECORDED_UNDER`]. It is a regression pin only, the D73 pattern: the
//! content check is the index row, and a driver update that changes a value
//! is re-measured and recorded with the driver version that produced it.

/// The fixture's bytes, as committed.
pub const MJPEG: &[u8] = include_bytes!("../fixtures/index-blocks-60.mjpeg");

/// The VA vendor string the pins below were measured under, on the Arc A380
/// (renderD129, i915) of the reference profile on 2026-09-29.
pub const RECORDED_UNDER: &str = "Intel iHD driver for Intel(R) Gen Graphics - 26.2.4 ()";

/// Each frame's CRC-32 over its visible NV12 lines, frame 1 first.
pub const RECORDED_FRAME_CRCS: [u32; 60] = [
    0x895e_99b6,
    0xcb72_c16f,
    0xeabf_ad50,
    0xf9fd_0f81,
    0xd830_63be,
    0x9a1c_3b67,
    0xbbd1_5758,
    0x670e_099a,
    0x46c3_65a5,
    0x04ef_3d7c,
    0x2522_5143,
    0x3660_f392,
    0x17ad_9fad,
    0x5581_c774,
    0x744c_ab4b,
    0x02fe_3e4a,
    0x2333_5275,
    0x611f_0aac,
    0x40d2_6693,
    0x5390_c442,
    0x725d_a87d,
    0x3071_f0a4,
    0x11bc_9c9b,
    0xcd63_c259,
    0xecae_ae66,
    0xae82_f6bf,
    0x8f4f_9a80,
    0x9c0d_3851,
    0xbdc0_546e,
    0xffec_0cb7,
    0xde21_6088,
    0xbb3c_80fb,
    0x9af1_ecc4,
    0xd8dd_b41d,
    0xf910_d822,
    0xea52_7af3,
    0xcb9f_16cc,
    0x89b3_4e15,
    0xa87e_222a,
    0x74a1_7ce8,
    0x556c_10d7,
    0x1740_480e,
    0x368d_2431,
    0x25cf_86e0,
    0x0402_eadf,
    0x462e_b206,
    0x67e3_de39,
    0x1151_4b38,
    0x309c_2707,
    0x72b0_7fde,
    0x537d_13e1,
    0x403f_b130,
    0x61f2_dd0f,
    0x23de_85d6,
    0x0213_e9e9,
    0xdecc_b72b,
    0xff01_db14,
    0xbd2d_83cd,
    0x9ce0_eff2,
    0x8fa2_4d23,
];

/// How the recorded pins compare with one run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinComparison {
    /// Same driver, every value equal.
    Matched,
    /// Same driver, `differing` values changed: a regression.
    Regressed {
        /// How many frames differ.
        differing: usize,
    },
    /// Another driver ran: the values are reported, not held (D73).
    OtherDriver {
        /// How many frames differ.
        differing: usize,
    },
}

/// Compares one run's per-frame CRC-32 values with the recorded pins.
#[must_use]
pub fn compare_pins(vendor: &str, measured: &[u32; 60]) -> PinComparison {
    let differing = RECORDED_FRAME_CRCS
        .iter()
        .zip(measured)
        .filter(|(recorded, found)| recorded != found)
        .count();
    if vendor != RECORDED_UNDER {
        PinComparison::OtherDriver { differing }
    } else if differing == 0 {
        PinComparison::Matched
    } else {
        PinComparison::Regressed { differing }
    }
}
