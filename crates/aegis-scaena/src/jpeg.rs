// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! A baseline JPEG header reader, enough to fill the VA-API
//! `VAProfileJPEGBaseline` buffers for one frame of a Motion-JPEG stream
//! (M27 criterion 5, D80's codec decision).
//!
//! cros-libva has no bitstream parser (D80), so this reads the markers
//! ITU-T T.81 defines for the baseline process and nothing more: `SOI`,
//! `DQT` with 8-bit tables, `DHT` with the two table slots baseline admits,
//! `SOF0`, `DRI`, `SOS` with a full spectral range, the entropy-coded scan up
//! to the next marker that is neither a stuffed byte nor a restart marker, and
//! `EOI`. `APPn` and `COM` segments are skipped by their length. Any other
//! frame type, a progressive or lossless scan, a 12-bit table or a segment
//! running past the data is refused with a [`JpegError`].
//!
//! Every table lands in a fixed array and the scan is a slice of the input,
//! so reading a frame allocates nothing. Every loop is bounded by
//! [`MAX_SEGMENTS`] or by the input's length (HISS-02).

/// The most segments one frame may hold before `EOI`.
pub const MAX_SEGMENTS: usize = 64;

/// The most components a baseline frame this reader admits carries.
pub const MAX_COMPONENTS: usize = 3;

/// The most DC Huffman values one table holds.
pub const MAX_DC_VALUES: usize = 12;

/// The most AC Huffman values one table holds.
pub const MAX_AC_VALUES: usize = 162;

/// Why a frame was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum JpegError {
    /// The data does not start with `SOI`.
    #[error("the frame does not start with SOI")]
    NoStart,
    /// The data ended inside a marker or segment.
    #[error("the data ends inside the segment at byte {at}")]
    Truncated {
        /// Where the segment started.
        at: usize,
    },
    /// A marker this reader does not admit.
    #[error("marker 0xff{marker:02x} at byte {at} is not admitted in a baseline frame")]
    Marker {
        /// The marker's second byte.
        marker: u8,
        /// Where it sits.
        at: usize,
    },
    /// A table or frame field outside the baseline process.
    #[error("the {segment} segment at byte {at} is outside the baseline process")]
    NotBaseline {
        /// Which segment.
        segment: &'static str,
        /// Where it sits.
        at: usize,
    },
    /// The frame lacks a table, a frame header or a scan it needs.
    #[error("the frame has no {what}")]
    Missing {
        /// What is missing.
        what: &'static str,
    },
    /// More segments than [`MAX_SEGMENTS`].
    #[error("the frame holds more than {MAX_SEGMENTS} segments")]
    TooManySegments,
}

/// One component of the frame header.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameComponent {
    /// The component identifier.
    pub id: u8,
    /// Horizontal sampling factor.
    pub horizontal: u8,
    /// Vertical sampling factor.
    pub vertical: u8,
    /// The quantisation table it uses.
    pub quant_table: u8,
}

/// One component of the scan header.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanComponent {
    /// The frame component it selects.
    pub selector: u8,
    /// Its DC table.
    pub dc_table: u8,
    /// Its AC table.
    pub ac_table: u8,
}

/// One Huffman table slot: DC and AC code counts and values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HuffmanSlot {
    /// Codes per length, 1 to 16 bits, for the DC table.
    pub dc_counts: [u8; 16],
    /// The DC values.
    pub dc_values: [u8; MAX_DC_VALUES],
    /// Codes per length for the AC table.
    pub ac_counts: [u8; 16],
    /// The AC values.
    pub ac_values: [u8; MAX_AC_VALUES],
    /// Whether the DC table was defined.
    pub dc_loaded: bool,
    /// Whether the AC table was defined.
    pub ac_loaded: bool,
}

impl HuffmanSlot {
    const EMPTY: Self = Self {
        dc_counts: [0; 16],
        dc_values: [0; MAX_DC_VALUES],
        ac_counts: [0; 16],
        ac_values: [0; MAX_AC_VALUES],
        dc_loaded: false,
        ac_loaded: false,
    };
}

/// Everything one frame's VA-API buffers need.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JpegFrame<'a> {
    /// Width in pixels.
    pub width: u16,
    /// Height in rows.
    pub height: u16,
    /// The frame header's components.
    pub components: [FrameComponent; MAX_COMPONENTS],
    /// How many there are.
    pub component_count: u8,
    /// The four quantisation tables, in the zig-zag order the file stores.
    pub quant: [[u8; 64]; 4],
    /// Which quantisation tables were defined.
    pub quant_loaded: [bool; 4],
    /// The two Huffman table slots.
    pub huffman: [HuffmanSlot; 2],
    /// The scan header's components.
    pub scan: [ScanComponent; MAX_COMPONENTS],
    /// How many there are.
    pub scan_count: u8,
    /// The restart interval in MCUs, 0 for none.
    pub restart_interval: u16,
    /// The entropy-coded scan, restart markers included.
    pub scan_data: &'a [u8],
}

impl JpegFrame<'_> {
    /// The MCUs the frame holds: the largest sampling factors set the MCU.
    #[must_use]
    pub fn mcu_count(&self) -> u32 {
        let components = self
            .components
            .get(..usize::from(self.component_count))
            .unwrap_or_default();
        let most = |pick: fn(&FrameComponent) -> u8| {
            u32::from(components.iter().map(pick).max().unwrap_or(1).max(1))
        };
        let across = u32::from(self.width).div_ceil(most(|c| c.horizontal).saturating_mul(8));
        let down = u32::from(self.height).div_ceil(most(|c| c.vertical).saturating_mul(8));
        across.saturating_mul(down)
    }
}

/// A read position over the input.
struct Cursor<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn byte(&mut self, segment: usize) -> Result<u8, JpegError> {
        let value = *self
            .data
            .get(self.at)
            .ok_or(JpegError::Truncated { at: segment })?;
        self.at = self.at.saturating_add(1);
        Ok(value)
    }

    fn word(&mut self, segment: usize) -> Result<u16, JpegError> {
        let high = self.byte(segment)?;
        let low = self.byte(segment)?;
        Ok(u16::from_be_bytes([high, low]))
    }

    fn take(&mut self, count: usize, segment: usize) -> Result<&'a [u8], JpegError> {
        let end = self
            .at
            .checked_add(count)
            .ok_or(JpegError::Truncated { at: segment })?;
        let slice = self
            .data
            .get(self.at..end)
            .ok_or(JpegError::Truncated { at: segment })?;
        self.at = end;
        Ok(slice)
    }
}

/// A frame being read: the tables and headers seen so far.
struct Builder<'a> {
    frame: JpegFrame<'a>,
    frame_seen: bool,
    scan_seen: bool,
}

impl Builder<'_> {
    const fn new() -> Self {
        Self {
            frame: JpegFrame {
                width: 0,
                height: 0,
                components: [FrameComponent {
                    id: 0,
                    horizontal: 0,
                    vertical: 0,
                    quant_table: 0,
                }; MAX_COMPONENTS],
                component_count: 0,
                quant: [[0; 64]; 4],
                quant_loaded: [false; 4],
                huffman: [HuffmanSlot::EMPTY; 2],
                scan: [ScanComponent {
                    selector: 0,
                    dc_table: 0,
                    ac_table: 0,
                }; MAX_COMPONENTS],
                scan_count: 0,
                restart_interval: 0,
                scan_data: &[],
            },
            frame_seen: false,
            scan_seen: false,
        }
    }

    /// `DQT`: one or more 8-bit tables.
    fn quant(&mut self, body: &[u8], at: usize) -> Result<(), JpegError> {
        let refused = JpegError::NotBaseline { segment: "DQT", at };
        let mut cursor = Cursor { data: body, at: 0 };
        for _ in 0..4 {
            if cursor.at >= body.len() {
                return Ok(());
            }
            let spec = cursor.byte(at)?;
            let slot = usize::from(spec & 0x0f);
            if spec >> 4 != 0 || slot >= 4 {
                return Err(refused);
            }
            let values = cursor.take(64, at)?;
            let table = self.frame.quant.get_mut(slot).ok_or(refused)?;
            table.copy_from_slice(values);
            *self.frame.quant_loaded.get_mut(slot).ok_or(refused)? = true;
        }
        if cursor.at == body.len() {
            Ok(())
        } else {
            Err(refused)
        }
    }

    /// `DHT`: one or more tables in the two baseline slots.
    fn huffman(&mut self, body: &[u8], at: usize) -> Result<(), JpegError> {
        let mut cursor = Cursor { data: body, at: 0 };
        for _ in 0..4 {
            if cursor.at >= body.len() {
                return Ok(());
            }
            self.huffman_table(&mut cursor, at)?;
        }
        if cursor.at == body.len() {
            Ok(())
        } else {
            Err(JpegError::NotBaseline { segment: "DHT", at })
        }
    }

    fn huffman_table(&mut self, cursor: &mut Cursor<'_>, at: usize) -> Result<(), JpegError> {
        let refused = JpegError::NotBaseline { segment: "DHT", at };
        let spec = cursor.byte(at)?;
        let (class, slot) = (spec >> 4, usize::from(spec & 0x0f));
        let counts: [u8; 16] = cursor.take(16, at)?.try_into().map_err(|_| refused)?;
        let total = counts
            .iter()
            .map(|count| usize::from(*count))
            .sum::<usize>();
        let values = cursor.take(total, at)?;
        let table = self.frame.huffman.get_mut(slot).ok_or(refused)?;
        match class {
            0 if total <= MAX_DC_VALUES => {
                table.dc_counts = counts;
                table
                    .dc_values
                    .get_mut(..total)
                    .ok_or(refused)?
                    .copy_from_slice(values);
                table.dc_loaded = true;
            }
            1 if total <= MAX_AC_VALUES => {
                table.ac_counts = counts;
                table
                    .ac_values
                    .get_mut(..total)
                    .ok_or(refused)?
                    .copy_from_slice(values);
                table.ac_loaded = true;
            }
            _ => return Err(refused),
        }
        Ok(())
    }

    /// `SOF0`: 8-bit samples and one to three components.
    fn frame_header(&mut self, body: &[u8], at: usize) -> Result<(), JpegError> {
        let refused = JpegError::NotBaseline {
            segment: "SOF0",
            at,
        };
        let mut cursor = Cursor { data: body, at: 0 };
        let precision = cursor.byte(at)?;
        self.frame.height = cursor.word(at)?;
        self.frame.width = cursor.word(at)?;
        let count = cursor.byte(at)?;
        let admitted = 1..=u8::try_from(MAX_COMPONENTS).map_err(|_| refused)?;
        if precision != 8 || self.frame.width == 0 || self.frame.height == 0 {
            return Err(refused);
        }
        if !admitted.contains(&count) || self.frame_seen {
            return Err(refused);
        }
        for slot in self.frame.components.iter_mut().take(usize::from(count)) {
            let id = cursor.byte(at)?;
            let sampling = cursor.byte(at)?;
            let quant_table = cursor.byte(at)?;
            let (horizontal, vertical) = (sampling >> 4, sampling & 0x0f);
            if !(1..=4).contains(&horizontal) || !(1..=4).contains(&vertical) || quant_table > 3 {
                return Err(refused);
            }
            *slot = FrameComponent {
                id,
                horizontal,
                vertical,
                quant_table,
            };
        }
        self.frame.component_count = count;
        self.frame_seen = true;
        Ok(())
    }

    /// `SOS`: the scan header of a sequential, full-spectrum scan.
    fn scan_header(&mut self, body: &[u8], at: usize) -> Result<(), JpegError> {
        let refused = JpegError::NotBaseline { segment: "SOS", at };
        let mut cursor = Cursor { data: body, at: 0 };
        let count = cursor.byte(at)?;
        if count == 0 || usize::from(count) > MAX_COMPONENTS || !self.frame_seen {
            return Err(refused);
        }
        for slot in self.frame.scan.iter_mut().take(usize::from(count)) {
            let selector = cursor.byte(at)?;
            let tables = cursor.byte(at)?;
            let (dc_table, ac_table) = (tables >> 4, tables & 0x0f);
            if dc_table > 1 || ac_table > 1 {
                return Err(refused);
            }
            *slot = ScanComponent {
                selector,
                dc_table,
                ac_table,
            };
        }
        let spectral = cursor.take(3, at)?;
        if spectral != [0, 63, 0] || cursor.at != body.len() {
            return Err(refused);
        }
        self.frame.scan_count = count;
        Ok(())
    }

    /// Checks that every table the scan names was defined.
    fn complete(&self) -> Result<(), JpegError> {
        if !self.frame_seen {
            return Err(JpegError::Missing {
                what: "frame header",
            });
        }
        if !self.scan_seen {
            return Err(JpegError::Missing { what: "scan" });
        }
        let components = self
            .frame
            .components
            .get(..usize::from(self.frame.component_count))
            .unwrap_or_default();
        for component in components {
            let loaded = self
                .frame
                .quant_loaded
                .get(usize::from(component.quant_table));
            if loaded != Some(&true) {
                return Err(JpegError::Missing {
                    what: "quantisation table",
                });
            }
        }
        self.tables_for_scan()
    }

    fn tables_for_scan(&self) -> Result<(), JpegError> {
        let scan = self
            .frame
            .scan
            .get(..usize::from(self.frame.scan_count))
            .unwrap_or_default();
        for component in scan {
            let dc = self.frame.huffman.get(usize::from(component.dc_table));
            let ac = self.frame.huffman.get(usize::from(component.ac_table));
            if !dc.is_some_and(|slot| slot.dc_loaded) || !ac.is_some_and(|slot| slot.ac_loaded) {
                return Err(JpegError::Missing {
                    what: "Huffman table",
                });
            }
        }
        Ok(())
    }
}

/// Where the entropy-coded scan starting at `from` ends: the first `0xff`
/// followed by a byte that is neither `0x00` (a stuffed byte) nor a restart
/// marker.
fn scan_end(data: &[u8], from: usize) -> Result<usize, JpegError> {
    let tail = data.get(from..).ok_or(JpegError::Truncated { at: from })?;
    let pairs = tail.iter().zip(tail.iter().skip(1)).enumerate();
    for (offset, (first, second)) in pairs {
        if *first == 0xff && *second != 0x00 && !(0xd0..=0xd7).contains(second) {
            return from
                .checked_add(offset)
                .ok_or(JpegError::Truncated { at: from });
        }
    }
    Err(JpegError::Missing { what: "EOI" })
}

/// Reads one segment's body after its marker.
fn segment<'a>(cursor: &mut Cursor<'a>, at: usize) -> Result<&'a [u8], JpegError> {
    let length = usize::from(cursor.word(at)?);
    let body = length.checked_sub(2).ok_or(JpegError::Truncated { at })?;
    cursor.take(body, at)
}

/// Applies one marker's segment to the frame being read.
fn apply<'a>(
    builder: &mut Builder<'a>,
    cursor: &mut Cursor<'a>,
    marker: u8,
    at: usize,
) -> Result<(), JpegError> {
    match marker {
        0xdb => builder.quant(segment(cursor, at)?, at),
        0xc4 => builder.huffman(segment(cursor, at)?, at),
        0xc0 => builder.frame_header(segment(cursor, at)?, at),
        0xdd => {
            let body = segment(cursor, at)?;
            let interval: [u8; 2] = body
                .try_into()
                .map_err(|_| JpegError::NotBaseline { segment: "DRI", at })?;
            builder.frame.restart_interval = u16::from_be_bytes(interval);
            Ok(())
        }
        0xda if builder.scan_seen => Err(JpegError::NotBaseline { segment: "SOS", at }),
        0xda => {
            builder.scan_header(segment(cursor, at)?, at)?;
            let end = scan_end(cursor.data, cursor.at)?;
            builder.frame.scan_data = cursor.data.get(cursor.at..end).unwrap_or_default();
            builder.scan_seen = true;
            cursor.at = end;
            Ok(())
        }
        0xe0..=0xef | 0xfe => segment(cursor, at).map(|_| ()),
        other => Err(JpegError::Marker { marker: other, at }),
    }
}

/// Reads the first frame of `data` and returns it with the number of bytes
/// it occupies, `EOI` included.
///
/// # Errors
///
/// Returns the first [`JpegError`] met.
pub fn parse(data: &[u8]) -> Result<(JpegFrame<'_>, usize), JpegError> {
    if data.get(..2) != Some(&[0xff, 0xd8]) {
        return Err(JpegError::NoStart);
    }
    let mut cursor = Cursor { data, at: 2 };
    let mut builder = Builder::new();
    for _ in 0..MAX_SEGMENTS {
        let at = cursor.at;
        if cursor.byte(at)? != 0xff {
            return Err(JpegError::Marker { marker: 0, at });
        }
        let marker = cursor.byte(at)?;
        if marker == 0xd9 {
            builder.complete()?;
            return Ok((builder.frame, cursor.at));
        }
        apply(&mut builder, &mut cursor, marker, at)?;
    }
    Err(JpegError::TooManySegments)
}
