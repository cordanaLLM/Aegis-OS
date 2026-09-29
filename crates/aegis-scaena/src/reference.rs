// SPDX-FileCopyrightText: 2007-2008 Intel Corporation
// SPDX-FileCopyrightText: 2022 The ChromiumOS Authors
// SPDX-License-Identifier: BSD-3-Clause AND MIT

//! The reference MPEG-2 intra frame (M27 criteria 5 and 8).
//!
//! Third-party data, not Aegis's: the 16 x 16 MPEG-2 clip with one I frame
//! and the VA-API parameters that decode it, as cros-libva's
//! `libva_utils_mpeg2vldemo` test carries them at the revision D80 pins
//! (`lib/src/lib.rs`, BSD-3-Clause, copyright 2022 The `ChromiumOS`
//! Authors), adapted there from libva-utils `decode/mpeg2vldemo.cpp` (MIT,
//! copyright 2007-2008 Intel Corporation). The licence texts are
//! `LICENSES/BSD-3-Clause.txt` and `LICENSES/MIT.txt`. The values are copied
//! unchanged; only their names and this layout are new.
//!
//! [`UPSTREAM_CRC32`] is cros-libva's own assertion over the decoded frame's
//! visible NV12 lines (lib/src/lib.rs:265, `crc_nv12_image`). It is
//! upstream's value and is not re-recorded here: a driver that yields another
//! fails the case, and adopting a new value is a recorded decision.

/// The frame's width and height in pixels.
pub const SIZE: u32 = 16;

/// The clip: sequence, sequence extension, GOP, picture and picture coding
/// extension headers, then one slice.
pub const CLIP: [u8; 197] = [
    0x00, 0x00, 0x01, 0xb3, 0x01, 0x00, 0x10, 0x13, 0xff, 0xff, 0xe0, 0x18, 0x00, 0x00, 0x01, 0xb5,
    0x14, 0x8a, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x01, 0xb8, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x00, 0x0f, 0xff, 0xf8, 0x00, 0x00, 0x01, 0xb5, 0x8f, 0xff, 0xf3, 0x41, 0x80, 0x00,
    0x00, 0x01, 0x01, 0x13, 0xe1, 0x00, 0x15, 0x81, 0x54, 0xe0, 0x2a, 0x05, 0x43, 0x00, 0x2d, 0x60,
    0x18, 0x01, 0x4e, 0x82, 0xb9, 0x58, 0xb1, 0x83, 0x49, 0xa4, 0xa0, 0x2e, 0x05, 0x80, 0x4b, 0x7a,
    0x00, 0x01, 0x38, 0x20, 0x80, 0xe8, 0x05, 0xff, 0x60, 0x18, 0xe0, 0x1d, 0x80, 0x98, 0x01, 0xf8,
    0x06, 0x00, 0x54, 0x02, 0xc0, 0x18, 0x14, 0x03, 0xb2, 0x92, 0x80, 0xc0, 0x18, 0x94, 0x42, 0x2c,
    0xb2, 0x11, 0x64, 0xa0, 0x12, 0x5e, 0x78, 0x03, 0x3c, 0x01, 0x80, 0x0e, 0x80, 0x18, 0x80, 0x6b,
    0xca, 0x4e, 0x01, 0x0f, 0xe4, 0x32, 0xc9, 0xbf, 0x01, 0x42, 0x69, 0x43, 0x50, 0x4b, 0x01, 0xc9,
    0x45, 0x80, 0x50, 0x01, 0x38, 0x65, 0xe8, 0x01, 0x03, 0xf3, 0xc0, 0x76, 0x00, 0xe0, 0x03, 0x20,
    0x28, 0x18, 0x01, 0xa9, 0x34, 0x04, 0xc5, 0xe0, 0x0b, 0x0b, 0x04, 0x20, 0x06, 0xc0, 0x89, 0xff,
    0x60, 0x12, 0x12, 0x8a, 0x2c, 0x34, 0x11, 0xff, 0xf6, 0xe2, 0x40, 0xc0, 0x30, 0x1b, 0x7a, 0x01,
    0xa9, 0x0d, 0x00, 0xac, 0x64,
];

/// Where the slice data starts in [`CLIP`].
pub const SLICE_DATA_OFFSET: usize = 47;

/// `MPEG2PictureCodingExtension::new` arguments: `intra_dc_precision`,
/// `picture_structure`, `top_field_first`, `frame_pred_frame_dct`,
/// `concealment_motion_vectors`, `q_scale_type`, `intra_vlc_format`,
/// `alternate_scan`, `repeat_first_field`, `progressive_frame`,
/// `is_first_field`.
pub const PICTURE_CODING_EXTENSION: [u32; 11] = [0, 3, 0, 1, 0, 0, 0, 0, 0, 1, 1];

/// `PictureParameterBufferMPEG2::new`: forward and backward reference
/// surfaces (none), `picture_coding_type` and `f_code`.
pub const PICTURE_REFERENCES: (u32, u32) = (0xffff_ffff, 0xffff_ffff);

/// `picture_coding_type` (1, an I frame) and `f_code`.
pub const PICTURE_CODING: (i32, i32) = (1, 0xffff);

/// `IQMatrixBufferMPEG2::new`: which of the four matrices are loaded.
pub const IQ_LOAD: [i32; 4] = [1, 1, 0, 0];

/// The intra quantiser matrix.
pub const INTRA_QUANTISER_MATRIX: [u8; 64] = [
    8, 16, 16, 19, 16, 19, 22, 22, 22, 22, 22, 22, 26, 24, 26, 27, 27, 27, 26, 26, 26, 26, 27, 27,
    27, 29, 29, 29, 34, 34, 34, 29, 29, 29, 27, 27, 29, 29, 32, 32, 34, 34, 37, 38, 37, 35, 35, 34,
    35, 38, 38, 40, 40, 40, 48, 48, 46, 46, 56, 56, 58, 69, 69, 83,
];

/// The non-intra quantiser matrix: 16, then zeros.
pub const NON_INTRA_QUANTISER_MATRIX: [u8; 64] = {
    let mut matrix = [0_u8; 64];
    matrix[0] = 16;
    matrix
};

/// `SliceParameterBufferMPEG2::new` arguments: `slice_data_size`,
/// `slice_data_offset`, `slice_data_flag`, `macroblock_offset`,
/// `slice_horizontal_position`, `slice_vertical_position`.
pub const SLICE: [u32; 6] = [150, 0, 0, 38, 0, 0];

/// `quantiser_scale_code` and `intra_slice_flag`.
pub const SLICE_CODES: (i32, i32) = (2, 0);

/// cros-libva's CRC-32 over the decoded frame's visible NV12 lines.
pub const UPSTREAM_CRC32: u32 = 0xa571_3e52;
