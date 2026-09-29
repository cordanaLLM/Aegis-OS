// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `STREAM_DECODED_FRAME`: the decoded-frame descriptor P08 hands P17
//! (REQ-P17-07, D77).
//!
//! Schema `aegis.p08-p17.decoded-frame.v1` is owned by `aegis-scaena`, the
//! consumer, under the rule milestone M14 set
//! (`crates/aegis-compositor/src/chain.rs`): an inbound schema lives with its
//! consumer. It carries `schema`, `correlation_id`, `fourcc`, `modifier`,
//! `width`, `height` and `planes[{offset, pitch}]`, and nothing else: the
//! DMA-BUF itself travels beside the line as exactly one `SCM_RIGHTS`
//! descriptor, so the line never names a file-descriptor number.
//!
//! Every value is `Copy` and lands in a fixed inline slot, so a descriptor
//! owns no heap (HISS-03). The modifier is the 64-bit value `export_prime`
//! returned, carried unchanged: a compositor cannot detect a modifier that
//! misstates the layout, so nothing here rewrites one.

use core::fmt;

use serde::ser::{Error as _, SerializeSeq, SerializeStruct};
use serde::{Serialize, Serializer};

use crate::error::DescriptorError;

/// The one schema this build admits.
pub const SCHEMA_V1: &str = "aegis.p08-p17.decoded-frame.v1";

/// The most planes a descriptor may list: DRM's four.
pub const MAX_PLANES: usize = 4;

/// The longest correlation identifier, in bytes.
pub const MAX_CORRELATION_BYTES: usize = 64;

/// The largest width or height admitted, in pixels.
pub const MAX_DIMENSION: u32 = 16_384;

/// `DRM_FORMAT_MOD_LINEAR` (`drm_fourcc.h`).
pub const MOD_LINEAR: u64 = 0;

/// `I915_FORMAT_MOD_X_TILED`, `fourcc_mod_code(INTEL, 1)`.
pub const MOD_INTEL_X_TILED: u64 = 0x0100_0000_0000_0001;

/// `I915_FORMAT_MOD_4_TILED`, `fourcc_mod_code(INTEL, 9)`.
pub const MOD_INTEL_4_TILED: u64 = 0x0100_0000_0000_0009;

/// `I915_FORMAT_MOD_4_TILED_DG2_RC_CCS`, `fourcc_mod_code(INTEL, 10)`.
pub const MOD_INTEL_4_TILED_DG2_RC_CCS: u64 = 0x0100_0000_0000_000a;

/// `DRM_FORMAT_MOD_INVALID`: no explicit modifier.
pub const MOD_INVALID: u64 = 0x00ff_ffff_ffff_ffff;

/// The contract versions this build admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Schema {
    /// `aegis.p08-p17.decoded-frame.v1`.
    V1,
}

impl Schema {
    /// The schema's name on the wire.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::V1 => SCHEMA_V1,
        }
    }

    /// Parses a schema name.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError::Schema`] for any other name.
    pub fn parse(raw: &[u8]) -> Result<Self, DescriptorError> {
        if raw == SCHEMA_V1.as_bytes() {
            Ok(Self::V1)
        } else {
            Err(DescriptorError::Schema)
        }
    }
}

/// A correlation identifier: 1 to 64 bytes of ASCII letters, digits and
/// `-`, `_`, `.` and `:`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CorrelationId {
    bytes: [u8; MAX_CORRELATION_BYTES],
    len: u8,
}

impl CorrelationId {
    /// Parses `raw`, never truncating or rewriting it.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError::CorrelationId`] when `raw` is empty, longer
    /// than [`MAX_CORRELATION_BYTES`] or holds a byte outside the alphabet.
    pub fn parse(raw: &[u8]) -> Result<Self, DescriptorError> {
        let admitted = |byte: &u8| byte.is_ascii_alphanumeric() || b"-_.:".contains(byte);
        if raw.is_empty() || !raw.iter().all(admitted) {
            return Err(DescriptorError::CorrelationId);
        }
        let mut bytes = [0_u8; MAX_CORRELATION_BYTES];
        bytes
            .get_mut(..raw.len())
            .ok_or(DescriptorError::CorrelationId)?
            .copy_from_slice(raw);
        let len = u8::try_from(raw.len()).map_err(|_| DescriptorError::CorrelationId)?;
        Ok(Self { bytes, len })
    }

    /// The identifier's text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        let bytes = self.bytes.get(..usize::from(self.len)).unwrap_or_default();
        core::str::from_utf8(bytes).unwrap_or_default()
    }
}

impl fmt::Debug for CorrelationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CorrelationId")
            .field(&self.as_str())
            .finish()
    }
}

/// A DRM fourcc code, written on the wire as its four characters.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FourCc(u32);

impl FourCc {
    /// `DRM_FORMAT_NV12`, `fourcc_code('N', 'V', '1', '2')`.
    pub const NV12: Self = Self(u32::from_le_bytes(*b"NV12"));

    /// `DRM_FORMAT_XRGB8888`, `fourcc_code('X', 'R', '2', '4')`.
    pub const XRGB8888: Self = Self(u32::from_le_bytes(*b"XR24"));

    /// `DRM_FORMAT_ARGB8888`, `fourcc_code('A', 'R', '2', '4')`.
    pub const ARGB8888: Self = Self(u32::from_le_bytes(*b"AR24"));

    /// Parses four printable ASCII characters.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError::Fourcc`] for anything else.
    pub fn parse(raw: &[u8]) -> Result<Self, DescriptorError> {
        let chars: [u8; 4] = raw.try_into().map_err(|_| DescriptorError::Fourcc)?;
        if chars.iter().all(|byte| (0x20..=0x7e).contains(byte)) {
            Ok(Self(u32::from_le_bytes(chars)))
        } else {
            Err(DescriptorError::Fourcc)
        }
    }

    /// The fourcc from its 32-bit code, as `zwp_linux_buffer_params_v1`
    /// carries it.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError::Fourcc`] when the code's bytes are not
    /// printable ASCII.
    pub fn from_code(code: u32) -> Result<Self, DescriptorError> {
        Self::parse(&code.to_le_bytes())
    }

    /// The 32-bit code.
    #[must_use]
    pub const fn code(self) -> u32 {
        self.0
    }

    /// The four characters.
    #[must_use]
    pub const fn chars(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }
}

impl fmt::Display for FourCc {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let chars = self.chars();
        formatter.write_str(core::str::from_utf8(&chars).map_err(|_| fmt::Error)?)
    }
}

impl fmt::Debug for FourCc {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "FourCc({self})")
    }
}

/// One plane of a DMA-BUF object: where it starts and its row pitch, in
/// bytes. Every plane lives in the one object the message carries.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Plane {
    /// The plane's first byte within the object.
    pub offset: u32,
    /// Bytes from one row to the next.
    pub pitch: u32,
}

/// One to [`MAX_PLANES`] planes, held inline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Planes {
    slots: [Plane; MAX_PLANES],
    len: u8,
}

impl Planes {
    /// The planes in `planes`, which must hold 1 to [`MAX_PLANES`].
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError::PlaneCount`] for 0 or more than
    /// [`MAX_PLANES`].
    pub fn new(planes: &[Plane]) -> Result<Self, DescriptorError> {
        let count = planes.len();
        let refused = DescriptorError::PlaneCount { count };
        if count == 0 {
            return Err(refused);
        }
        let mut slots = [Plane::default(); MAX_PLANES];
        slots
            .get_mut(..count)
            .ok_or(refused)?
            .copy_from_slice(planes);
        let len = u8::try_from(count).map_err(|_| refused)?;
        Ok(Self { slots, len })
    }

    /// The planes as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[Plane] {
        self.slots.get(..usize::from(self.len)).unwrap_or_default()
    }

    /// How many planes there are.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Always `false`: a plane list holds at least one plane.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// The decoded-frame descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DecodedFrame {
    /// The contract version.
    pub schema: Schema,
    /// The identifier that correlates this crossing.
    pub correlation_id: CorrelationId,
    /// The pixel format.
    pub fourcc: FourCc,
    /// The format modifier `export_prime` returned, unchanged.
    pub modifier: u64,
    /// The frame's width in pixels.
    pub width: u32,
    /// The frame's height in rows.
    pub height: u32,
    /// The planes, all in the one object the message carries.
    pub planes: Planes,
}

impl DecodedFrame {
    /// A version-1 descriptor.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError::Dimensions`] when a dimension is zero or
    /// above [`MAX_DIMENSION`].
    pub fn new(
        correlation_id: CorrelationId,
        fourcc: FourCc,
        modifier: u64,
        size: (u32, u32),
        planes: Planes,
    ) -> Result<Self, DescriptorError> {
        let (width, height) = size;
        let admitted = 1..=MAX_DIMENSION;
        if !admitted.contains(&width) || !admitted.contains(&height) {
            return Err(DescriptorError::Dimensions);
        }
        Ok(Self {
            schema: Schema::V1,
            correlation_id,
            fourcc,
            modifier,
            width,
            height,
            planes,
        })
    }
}

impl Serialize for DecodedFrame {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_struct("DecodedFrame", 7)?;
        out.serialize_field("schema", self.schema.name())?;
        out.serialize_field("correlation_id", self.correlation_id.as_str())?;
        let chars = self.fourcc.chars();
        let fourcc = core::str::from_utf8(&chars).map_err(S::Error::custom)?;
        out.serialize_field("fourcc", fourcc)?;
        out.serialize_field("modifier", &self.modifier)?;
        out.serialize_field("width", &self.width)?;
        out.serialize_field("height", &self.height)?;
        out.serialize_field("planes", &self.planes)?;
        out.end()
    }
}

impl Serialize for Planes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_seq(Some(self.len()))?;
        for plane in self.as_slice() {
            out.serialize_element(plane)?;
        }
        out.end()
    }
}
