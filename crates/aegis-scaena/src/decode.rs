// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The decode half the display slice drives, through cros-libva (D80): the
//! P08 stand-in that produces the frames P17 attaches (M27 criteria 5 and 6).
//!
//! In order, every step checked against content rather than exit status:
//!
//! 1. [`VaDevice::open`] opens the render node the compositor names as its
//!    main device and reads the VA vendor string, and [`require_ihd`] refuses
//!    any driver but iHD, so `LIBVA_DRIVER_NAME=nvidia` in the environment is
//!    refused before anything is decoded (negative (a)): a decode's exit
//!    status alone does not show which GPU decoded;
//! 2. [`decode_reference`] decodes cros-libva's reference MPEG-2 intra frame
//!    and holds its visible NV12 lines to the CRC-32 cros-libva asserts;
//! 3. [`JpegPipeline`] decodes each Motion-JPEG fixture frame through
//!    `VAProfileJPEGBaseline` and `VAEntrypointVLD`, reads its index blocks
//!    back with `create_image` before the surface is exported, and exports it
//!    with `export_prime` (NV12, composed layers) as one DMA-BUF fd.
//!
//! cros-libva's `sync` wraps `vaSyncSurface`, which takes no timeout, so
//! before `sync` or `create_image` every decode is polled with
//! `Surface::query_status` until `VASurfaceReady`, under a deadline and a
//! bounded number of polls ([`poll_ready`], HISS-02); a missed deadline names
//! the surface.
//!
//! What is not claimed: cros-libva heap-allocates for every decoded picture,
//! the deviation D83 limits to that binding. This module maps VA images, which
//! is the decoder's memory; the fd P17 receives is never mapped.

use core::time::Duration;
use std::os::fd::OwnedFd;
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;

use cros_libva::{
    BorrowedBufferType, BufferType, Config, Context, Display, HuffmanTable,
    HuffmanTableBufferJPEGBaseline, HuffmanTableBufferJPEGBaselineHuffmanTable, IQMatrix,
    IQMatrixBufferJPEGBaseline, IQMatrixBufferMPEG2, Image, MPEG2PictureCodingExtension, Picture,
    PictureParameter, PictureParameterBufferJPEGBaseline,
    PictureParameterBufferJPEGBaselineComponent, PictureParameterBufferMPEG2, SliceParameter,
    SliceParameterBufferJPEGBaseline, SliceParameterBufferMPEG2, Surface, UsageHint,
    VAConfigAttrib, VAConfigAttribType, VAEntrypoint, VAImageFormat, VAProfile, VARectangle,
    VASliceParameterBufferJPEGBaselineComponent, VASurfaceStatus, VaError,
};

use crate::content::{Nv12Layout, crc_nv12};
use crate::descriptor::{FourCc, Plane};
use crate::jpeg::JpegFrame;
use crate::reference;

/// How the iHD driver's vendor string starts.
pub const IHD_VENDOR_PREFIX: &str = "Intel iHD driver";

/// Why a decode step was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum DecodeError {
    /// The VA display could not be opened on the node.
    #[error("the VA display on {path} could not be opened: {reason}")]
    Open {
        /// The node.
        path: String,
        /// libva's reason.
        reason: String,
    },
    /// The driver returned no vendor string.
    #[error("the VA driver returned no vendor string")]
    NoVendor,
    /// The vendor string does not name the iHD driver (M27 negative (a)).
    #[error("the VA vendor string {vendor:?} does not name the iHD driver")]
    NotIhd {
        /// What the driver reported.
        vendor: String,
    },
    /// The driver does not advertise the profile with the VLD entrypoint and
    /// 4:2:0 surfaces.
    #[error("the driver does not decode {profile} through VAEntrypointVLD into 4:2:0 surfaces")]
    Unsupported {
        /// The profile.
        profile: &'static str,
    },
    /// A libva call failed.
    #[error("{stage} failed with VA status {status}")]
    Va {
        /// The call.
        stage: &'static str,
        /// The `VAStatus`.
        status: i32,
    },
    /// The decoded surface did not reach `VASurfaceReady` in time (HISS-02).
    #[error(
        "surface {surface} did not reach VASurfaceReady within its deadline ({polls} polls, last status {status})"
    )]
    NotReady {
        /// The VA surface id.
        surface: u32,
        /// How many polls were made.
        polls: u32,
        /// The last status read.
        status: u32,
    },
    /// No NV12 image format is offered for readback.
    #[error("the driver offers no NV12 image format")]
    NoNv12Image,
    /// The reference frame decoded to another CRC-32 (M27 criterion 5).
    #[error(
        "the reference MPEG-2 frame decoded to CRC-32 {found:#010x}, not cros-libva's {expected:#010x}, under driver {vendor:?}"
    )]
    ReferenceCrc {
        /// cros-libva's value.
        expected: u32,
        /// What this driver produced.
        found: u32,
        /// The driver that produced it.
        vendor: String,
    },
    /// A decoded image's layout does not fit its mapping.
    #[error("the decoded image's layout does not fit its mapping")]
    ImageLayout,
    /// `export_prime` returned something other than one NV12 object with two
    /// planes in one composed layer.
    #[error("export_prime returned {what}")]
    Export {
        /// What it returned.
        what: &'static str,
    },
    /// No surface with that index.
    #[error("no decode surface {index}")]
    NoSurface {
        /// The index asked for.
        index: usize,
    },
}

impl DecodeError {
    fn va(stage: &'static str) -> impl FnOnce(VaError) -> Self {
        move |error| Self::Va {
            stage,
            status: error.va_status(),
        }
    }
}

/// Refuses every driver but iHD, read from the VA vendor string.
///
/// # Errors
///
/// Returns [`DecodeError::NotIhd`] naming the vendor string found.
pub fn require_ihd(vendor: &str) -> Result<(), DecodeError> {
    if vendor.starts_with(IHD_VENDOR_PREFIX) {
        Ok(())
    } else {
        Err(DecodeError::NotIhd {
            vendor: vendor.to_owned(),
        })
    }
}

/// How long a decode may take to reach `VASurfaceReady`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadyWait {
    /// The deadline from the first poll.
    pub deadline: Duration,
    /// The most polls made.
    pub max_polls: u32,
    /// The pause between polls.
    pub interval: Duration,
}

/// The wait every decode in the display slice runs under.
pub const READY_WAIT: ReadyWait = ReadyWait {
    deadline: Duration::from_secs(2),
    max_polls: 4_000,
    interval: Duration::from_micros(500),
};

/// Whether a `vaQuerySurfaceStatus` value means the decode is done.
#[must_use]
pub const fn is_ready(status: u32) -> bool {
    status & VASurfaceStatus::VASurfaceReady != 0
        && status & VASurfaceStatus::VASurfaceRendering == 0
}

/// Polls `status` until the surface is ready, under `wait` (HISS-02).
/// Returns how many polls it took.
///
/// # Errors
///
/// Returns [`DecodeError::NotReady`] naming the surface when the deadline
/// passes or the polls run out, and [`DecodeError::Va`] when a poll fails.
pub fn poll_ready(
    surface: u32,
    wait: ReadyWait,
    mut status: impl FnMut() -> Result<u32, i32>,
) -> Result<u32, DecodeError> {
    let started = Instant::now();
    let mut last = 0;
    for polls in 1..=wait.max_polls {
        last = status().map_err(|status| DecodeError::Va {
            stage: "vaQuerySurfaceStatus",
            status,
        })?;
        if is_ready(last) {
            return Ok(polls);
        }
        if started.elapsed() >= wait.deadline {
            return Err(DecodeError::NotReady {
                surface,
                polls,
                status: last,
            });
        }
        std::thread::sleep(wait.interval);
    }
    Err(DecodeError::NotReady {
        surface,
        polls: wait.max_polls,
        status: last,
    })
}

/// Polls one VA surface until it is ready.
fn wait_surface(surface: &Surface<()>, wait: ReadyWait) -> Result<u32, DecodeError> {
    poll_ready(surface.id(), wait, || {
        surface.query_status().map_err(|error| error.va_status())
    })
}

/// A decode config, its context and the surfaces it decodes into.
type Pipeline = (Config, Rc<Context>, Vec<Surface<()>>);

/// A VA display on one render node, with its vendor string.
pub struct VaDevice {
    display: Rc<Display>,
    vendor: String,
    nv12: VAImageFormat,
}

impl core::fmt::Debug for VaDevice {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("VaDevice")
            .field("vendor", &self.vendor)
            .finish_non_exhaustive()
    }
}

impl VaDevice {
    /// Opens the VA display on `path` and reads its vendor string. The driver
    /// is whatever libva loads, `LIBVA_DRIVER_NAME` included; nothing is
    /// decoded until [`require_ihd`] has accepted the vendor string.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::Open`], [`DecodeError::NoVendor`],
    /// [`DecodeError::Va`] or [`DecodeError::NoNv12Image`].
    pub fn open(path: &Path) -> Result<Self, DecodeError> {
        let display = Display::open_drm_display(path).map_err(|error| DecodeError::Open {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;
        let vendor = display
            .query_vendor_string()
            .map_err(|_| DecodeError::NoVendor)?;
        let nv12 = display
            .query_image_formats()
            .map_err(DecodeError::va("vaQueryImageFormats"))?
            .into_iter()
            .find(|format| format.fourcc == FourCc::NV12.code())
            .ok_or(DecodeError::NoNv12Image)?;
        Ok(Self {
            display,
            vendor,
            nv12,
        })
    }

    /// The VA vendor string, which names the driver and its version.
    #[must_use]
    pub fn vendor(&self) -> &str {
        &self.vendor
    }

    /// Refuses a profile the driver does not decode through VLD into 4:2:0
    /// surfaces, and returns the config attributes to create it with.
    fn decode_config(
        &self,
        profile: cros_libva::VAProfile::Type,
        name: &'static str,
    ) -> Result<Vec<VAConfigAttrib>, DecodeError> {
        let unsupported = DecodeError::Unsupported { profile: name };
        let entrypoints = self
            .display
            .query_config_entrypoints(profile)
            .map_err(|_| unsupported.clone())?;
        if !entrypoints.contains(&VAEntrypoint::VAEntrypointVLD) {
            return Err(unsupported);
        }
        let mut attributes = vec![VAConfigAttrib {
            type_: VAConfigAttribType::VAConfigAttribRTFormat,
            value: 0,
        }];
        self.display
            .get_config_attributes(profile, VAEntrypoint::VAEntrypointVLD, &mut attributes)
            .map_err(DecodeError::va("vaGetConfigAttributes"))?;
        let formats = attributes.first().map_or(0, |attribute| attribute.value);
        if formats & cros_libva::VA_RT_FORMAT_YUV420 == 0
            || formats == cros_libva::VA_ATTRIB_NOT_SUPPORTED
        {
            return Err(unsupported);
        }
        Ok(attributes)
    }

    /// Creates `count` NV12 decode surfaces of `width` x `height`, a config
    /// for `profile` and a context over them.
    fn pipeline(
        &self,
        (profile, name): (cros_libva::VAProfile::Type, &'static str),
        (width, height): (u32, u32),
        count: usize,
    ) -> Result<Pipeline, DecodeError> {
        let attributes = self.decode_config(profile, name)?;
        let config = self
            .display
            .create_config(attributes, profile, VAEntrypoint::VAEntrypointVLD)
            .map_err(DecodeError::va("vaCreateConfig"))?;
        let surfaces = self
            .display
            .create_surfaces(
                cros_libva::VA_RT_FORMAT_YUV420,
                Some(FourCc::NV12.code()),
                width,
                height,
                Some(UsageHint::USAGE_HINT_DECODER | UsageHint::USAGE_HINT_EXPORT),
                vec![(); count],
            )
            .map_err(DecodeError::va("vaCreateSurfaces"))?;
        let context = self
            .display
            .create_context(&config, width, height, Some(&surfaces), true)
            .map_err(DecodeError::va("vaCreateContext"))?;
        Ok((config, context, surfaces))
    }

    /// Reads `surface` back as an NV12 image of `width` x `height` and hands
    /// the mapped bytes and their layout to `inspect`.
    fn read_back<T>(
        &self,
        surface: &Surface<()>,
        (width, height): (u32, u32),
        inspect: impl FnOnce(&[u8], Nv12Layout) -> T,
    ) -> Result<T, DecodeError> {
        let image = Image::create_from(surface, self.nv12, (width, height), (width, height))
            .map_err(DecodeError::va("vaCreateImage/vaGetImage"))?;
        let layout = nv12_layout(image.image(), (width, height))?;
        Ok(inspect(image.as_ref(), layout))
    }
}

/// The layout of a mapped NV12 `VAImage` at its visible size.
fn nv12_layout(
    image: &cros_libva::VAImage,
    (width, height): (u32, u32),
) -> Result<Nv12Layout, DecodeError> {
    let wide = |value: u32| usize::try_from(value).map_err(|_| DecodeError::ImageLayout);
    if image.format.fourcc != FourCc::NV12.code() || image.num_planes != 2 {
        return Err(DecodeError::ImageLayout);
    }
    let [luma_offset, chroma_offset, ..] = image.offsets;
    let [luma_pitch, chroma_pitch, ..] = image.pitches;
    Ok(Nv12Layout {
        offsets: [wide(luma_offset)?, wide(chroma_offset)?],
        pitches: [wide(luma_pitch)?, wide(chroma_pitch)?],
        width: wide(width)?,
        height: wide(height)?,
    })
}

/// The reference frame's picture parameters, built from [`reference`].
fn reference_picture() -> PictureParameterBufferMPEG2 {
    let [
        dc,
        structure,
        top,
        frame_dct,
        concealment,
        q_scale,
        vlc,
        scan,
        repeat,
        progressive,
        first,
    ] = reference::PICTURE_CODING_EXTENSION;
    let extension = MPEG2PictureCodingExtension::new(
        dc,
        structure,
        top,
        frame_dct,
        concealment,
        q_scale,
        vlc,
        scan,
        repeat,
        progressive,
        first,
    );
    let size = u16::try_from(reference::SIZE).unwrap_or(u16::MAX);
    let (forward, backward) = reference::PICTURE_REFERENCES;
    let (coding_type, f_code) = reference::PICTURE_CODING;
    PictureParameterBufferMPEG2::new(
        size,
        size,
        forward,
        backward,
        coding_type,
        f_code,
        &extension,
    )
}

/// The reference frame's three parameter buffers, built from [`reference`].
fn reference_buffers() -> [BufferType; 3] {
    let [size_bytes, offset, flag, macroblock, horizontal, vertical] = reference::SLICE;
    let (quantiser, intra) = reference::SLICE_CODES;
    let slice = SliceParameterBufferMPEG2::new(
        size_bytes, offset, flag, macroblock, horizontal, vertical, quantiser, intra,
    );
    let [intra_load, non_intra_load, chroma_intra, chroma_non_intra] = reference::IQ_LOAD;
    let matrix = IQMatrixBufferMPEG2::new(
        intra_load,
        non_intra_load,
        chroma_intra,
        chroma_non_intra,
        reference::INTRA_QUANTISER_MATRIX,
        reference::NON_INTRA_QUANTISER_MATRIX,
        [0; 64],
        [0; 64],
    );
    [
        BufferType::PictureParameter(PictureParameter::MPEG2(reference_picture())),
        BufferType::SliceParameter(SliceParameter::MPEG2(slice)),
        BufferType::IQMatrix(IQMatrix::MPEG2(matrix)),
    ]
}

/// Decodes one picture into `surface` from `buffers` and the slice data,
/// polls it ready under `wait`, and syncs it.
fn decode_picture(
    context: &Rc<Context>,
    surface: &Surface<()>,
    buffers: impl IntoIterator<Item = BufferType>,
    slice_data: &[u8],
    wait: ReadyWait,
) -> Result<u32, DecodeError> {
    let mut picture = Picture::new(0, Rc::clone(context), surface);
    for buffer in buffers {
        picture.add_buffer(
            context
                .create_buffer(buffer)
                .map_err(DecodeError::va("vaCreateBuffer"))?,
        );
    }
    picture.add_buffer(
        context
            .create_buffer_borrowed(BorrowedBufferType::SliceData(slice_data))
            .map_err(DecodeError::va("vaCreateBuffer"))?,
    );
    let ended = picture
        .begin()
        .map_err(DecodeError::va("vaBeginPicture"))?
        .render()
        .map_err(DecodeError::va("vaRenderPicture"))?
        .end()
        .map_err(DecodeError::va("vaEndPicture"))?;
    let polls = wait_surface(surface, wait)?;
    ended
        .sync()
        .map_err(|(error, _)| DecodeError::va("vaSyncSurface")(error))?;
    Ok(polls)
}

/// Decodes the reference MPEG-2 intra frame and returns the CRC-32 of its
/// visible NV12 lines after holding it to [`reference::UPSTREAM_CRC32`].
///
/// # Errors
///
/// Returns [`DecodeError::ReferenceCrc`] naming the driver when the value
/// differs, and the other [`DecodeError`]s of the steps before.
pub fn decode_reference(device: &VaDevice, wait: ReadyWait) -> Result<u32, DecodeError> {
    let size = (reference::SIZE, reference::SIZE);
    let profile = (VAProfile::VAProfileMPEG2Main, "VAProfileMPEG2Main");
    let (_config, context, surfaces) = device.pipeline(profile, size, 1)?;
    let surface = surfaces
        .first()
        .ok_or(DecodeError::NoSurface { index: 0 })?;
    let slice_data = reference::CLIP
        .get(reference::SLICE_DATA_OFFSET..)
        .unwrap_or_default();
    decode_picture(&context, surface, reference_buffers(), slice_data, wait)?;
    let found = device.read_back(surface, size, crc_nv12)??;
    if found == reference::UPSTREAM_CRC32 {
        Ok(found)
    } else {
        Err(DecodeError::ReferenceCrc {
            expected: reference::UPSTREAM_CRC32,
            found,
            vendor: device.vendor.clone(),
        })
    }
}

impl From<crate::content::ContentError> for DecodeError {
    fn from(_: crate::content::ContentError) -> Self {
        Self::ImageLayout
    }
}

/// One exported surface: its DMA-BUF, the modifier the driver chose, both
/// NV12 planes and the object's size.
#[derive(Debug)]
pub struct Exported {
    /// The DMA-BUF.
    pub fd: OwnedFd,
    /// The modifier `export_prime` returned, unchanged.
    pub modifier: u64,
    /// The luma and chroma planes.
    pub planes: [Plane; 2],
    /// The object's size as the driver reported it.
    pub size: u32,
    /// The exported width and height.
    pub dimensions: (u32, u32),
}

/// A Motion-JPEG decode pipeline over a fixed set of surfaces.
pub struct JpegPipeline {
    _config: Config,
    context: Rc<Context>,
    surfaces: Vec<Surface<()>>,
    size: (u32, u32),
}

impl core::fmt::Debug for JpegPipeline {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("JpegPipeline")
            .field("surfaces", &self.surfaces.len())
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

/// The picture parameters for one frame.
fn jpeg_picture(frame: &JpegFrame<'_>) -> PictureParameterBufferJPEGBaseline {
    let components = core::array::from_fn(|index| {
        let component = frame.components.get(index).copied().unwrap_or_default();
        PictureParameterBufferJPEGBaselineComponent::new(
            component.id,
            component.horizontal,
            component.vertical,
            component.quant_table,
        )
    });
    PictureParameterBufferJPEGBaseline::new(
        frame.width,
        frame.height,
        components,
        frame.component_count,
        0,
        0,
        VARectangle::default(),
    )
}

/// The Huffman tables for one frame.
fn jpeg_huffman(frame: &JpegFrame<'_>) -> HuffmanTableBufferJPEGBaseline {
    let tables = frame.huffman.map(|slot| {
        HuffmanTableBufferJPEGBaselineHuffmanTable::new(
            slot.dc_counts,
            slot.dc_values,
            slot.ac_counts,
            slot.ac_values,
            [0; 2],
        )
    });
    let loaded = frame
        .huffman
        .map(|slot| u8::from(slot.dc_loaded && slot.ac_loaded));
    HuffmanTableBufferJPEGBaseline::new(loaded, tables)
}

/// The slice parameters for one frame's single scan.
fn jpeg_slice(frame: &JpegFrame<'_>) -> SliceParameterBufferJPEGBaseline {
    let components = core::array::from_fn(|index| {
        let component = frame.scan.get(index).copied().unwrap_or_default();
        VASliceParameterBufferJPEGBaselineComponent::new(
            component.selector,
            component.dc_table,
            component.ac_table,
        )
    });
    SliceParameterBufferJPEGBaseline::new(
        u32::try_from(frame.scan_data.len()).unwrap_or(u32::MAX),
        0,
        cros_libva::VA_SLICE_DATA_FLAG_ALL,
        0,
        0,
        components,
        frame.scan_count,
        frame.restart_interval,
        frame.mcu_count(),
    )
}

/// The four parameter buffers of one frame.
fn jpeg_buffers(frame: &JpegFrame<'_>) -> [BufferType; 4] {
    let quant = IQMatrixBufferJPEGBaseline::new(frame.quant_loaded.map(u8::from), frame.quant);
    [
        BufferType::PictureParameter(PictureParameter::JPEGBaseline(jpeg_picture(frame))),
        BufferType::IQMatrix(IQMatrix::JPEGBaseline(quant)),
        BufferType::HuffmanTable(HuffmanTable::JPEGBaseline(jpeg_huffman(frame))),
        BufferType::SliceParameter(SliceParameter::JPEGBaseline(jpeg_slice(frame))),
    ]
}

impl JpegPipeline {
    /// `count` surfaces of `width` x `height` and a baseline JPEG context.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::Unsupported`] when the driver has no
    /// `VAProfileJPEGBaseline` VLD decode, and [`DecodeError::Va`] otherwise.
    pub fn new(device: &VaDevice, size: (u32, u32), count: usize) -> Result<Self, DecodeError> {
        let profile = (VAProfile::VAProfileJPEGBaseline, "VAProfileJPEGBaseline");
        let (config, context, surfaces) = device.pipeline(profile, size, count)?;
        Ok(Self {
            _config: config,
            context,
            surfaces,
            size,
        })
    }

    fn surface(&self, index: usize) -> Result<&Surface<()>, DecodeError> {
        self.surfaces
            .get(index)
            .ok_or(DecodeError::NoSurface { index })
    }

    /// The VA surface id behind `index`, for the record.
    #[must_use]
    pub fn surface_id(&self, index: usize) -> Option<u32> {
        self.surfaces.get(index).map(Surface::id)
    }

    /// Decodes `frame` into surface `index`, polled ready under `wait`.
    /// Returns how many polls it took.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::NotReady`] naming the surface, or
    /// [`DecodeError::Va`].
    pub fn decode(
        &self,
        index: usize,
        frame: &JpegFrame<'_>,
        wait: ReadyWait,
    ) -> Result<u32, DecodeError> {
        let surface = self.surface(index)?;
        decode_picture(
            &self.context,
            surface,
            jpeg_buffers(frame),
            frame.scan_data,
            wait,
        )
    }

    /// Reads surface `index` back and hands its mapped bytes to `inspect`.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::Va`] or [`DecodeError::ImageLayout`].
    pub fn inspect<T>(
        &self,
        device: &VaDevice,
        index: usize,
        inspect: impl FnOnce(&[u8], Nv12Layout) -> T,
    ) -> Result<T, DecodeError> {
        device.read_back(self.surface(index)?, self.size, inspect)
    }

    /// Fills surface `index` with zeros through a written-back image: the
    /// planted zero-filled surface of M27's negative case.
    ///
    /// Like every decode, the surface is polled ready under `wait` before
    /// `create_image` and again after the image is written back (its drop
    /// issues `vaPutImage`), before `sync` (HISS-02).
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::NotReady`] naming the surface, or
    /// [`DecodeError::Va`].
    pub fn plant_zeros(
        &self,
        device: &VaDevice,
        index: usize,
        wait: ReadyWait,
    ) -> Result<(), DecodeError> {
        let surface = self.surface(index)?;
        wait_surface(surface, wait)?;
        let mut image = Image::create_from(surface, device.nv12, self.size, self.size)
            .map_err(DecodeError::va("vaCreateImage/vaGetImage"))?;
        image.as_mut().fill(0);
        drop(image);
        wait_surface(surface, wait)?;
        surface.sync().map_err(DecodeError::va("vaSyncSurface"))
    }

    /// Exports surface `index` with `export_prime` and refuses anything but
    /// one NV12 object with both planes in one composed layer.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::Va`] or [`DecodeError::Export`].
    pub fn export(&self, index: usize) -> Result<Exported, DecodeError> {
        let descriptor = self
            .surface(index)?
            .export_prime()
            .map_err(DecodeError::va("vaExportSurfaceHandle"))?;
        if descriptor.fourcc != FourCc::NV12.code() {
            return Err(DecodeError::Export {
                what: "a fourcc other than NV12",
            });
        }
        let [layer] = descriptor.layers.as_slice() else {
            return Err(DecodeError::Export {
                what: "other than one composed layer",
            });
        };
        if layer.num_planes != 2 || layer.object_index != [0; 4] {
            return Err(DecodeError::Export {
                what: "a layer that is not two planes in object 0",
            });
        }
        let planes = [0, 1].map(|plane| Plane {
            offset: layer.offset.get(plane).copied().unwrap_or_default(),
            pitch: layer.pitch.get(plane).copied().unwrap_or_default(),
        });
        let dimensions = (descriptor.width, descriptor.height);
        let mut objects = descriptor.objects.into_iter();
        let (Some(object), None) = (objects.next(), objects.next()) else {
            return Err(DecodeError::Export {
                what: "other than one object",
            });
        };
        Ok(Exported {
            fd: object.fd,
            modifier: object.drm_format_modifier,
            planes,
            size: object.size,
            dimensions,
        })
    }
}
