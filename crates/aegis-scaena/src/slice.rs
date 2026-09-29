// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The display slice's run on the reference profile (M27 criteria 5, 6, 7
//! and 9), driven by `make verify-display` through
//! `tools/verify_display.py`, which starts it with `LIBVA_DRIVER_NAME` set
//! for the child process only.
//!
//! [`run_display`] is the positive run and the negatives that live in it:
//! the decode node is the render node whose device number is the
//! compositor's main device and every other node is refused (b); the VA
//! vendor string must name iHD; the reference MPEG-2 frame must decode to
//! cros-libva's CRC-32; a planted zero-filled surface must fail the content
//! check; every one of the 60 fixture frames is decoded, read back against
//! its index row before export, exported, sent over a `SOCK_SEQPACKET` pair
//! with its fd in `SCM_RIGHTS`, checked and attached to the layer surface; at
//! frame 30 a descriptor naming NV12 with `INTEL_4_TILED_DG2_RC_CCS`, which
//! the compositor did not advertise, is refused client-side, and frame 31
//! still presents (c). [`run_driver_probe`] is negative (a), run with the
//! session's `LIBVA_DRIVER_NAME=nvidia`: it must be refused before anything
//! is decoded or attached.
//!
//! The output is one line per fact, `info <key>: <value>`, and one line per
//! case, `PASS <case>` or `FAIL <case>` with indented detail lines. A host
//! without the reference profile's capabilities is a [`Verdict::Skipped`]
//! with its reason, never a pass. Every pass is development evidence for the
//! client half only (D79).

use std::fmt::Display;
use std::io::Write;
use std::os::fd::AsFd;
use std::path::Path;

use crate::attach::{self, FileIdentity};
use crate::content::{self, LumaPlane, Nv12Layout};
use crate::decode::{DecodeError, Exported, JpegPipeline, READY_WAIT, VaDevice, require_ihd};
use crate::descriptor::{
    CorrelationId, DecodedFrame, FourCc, MOD_INTEL_4_TILED_DG2_RC_CCS, Planes,
};
use crate::device::{self, RenderNode};
use crate::error::{AttachError, DescriptorError, TransportError};
use crate::fixture::{self, PinComparison};
use crate::jpeg::{self, JpegError};
use crate::line::{LineBuffer, Request};
use crate::present::{DEADLINES, PresentError, Presenter};
use crate::transport::{Deadlines, FrameReceiver, FrameSender, pair};

/// The kernel drivers the iHD VA driver runs on.
pub const INTEL_DRIVERS: [&str; 2] = ["i915", "xe"];

/// The fixture frame after which the unadvertised pair is planted.
pub const PLANT_AFTER: usize = 30;

/// How long the uncommitted presentation feedback is waited for.
pub const UNANSWERED_DEADLINE: core::time::Duration = core::time::Duration::from_millis(250);

/// The socket pair's deadlines.
const SOCKET_DEADLINES: Deadlines = Deadlines {
    receive: core::time::Duration::from_secs(1),
    send: core::time::Duration::from_secs(1),
};

/// How a run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Every case passed.
    Passed,
    /// This many cases failed.
    Failed(u32),
    /// The host lacks a capability the reference profile has.
    Skipped(String),
}

/// Why a run stopped before its cases were decided.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SliceError {
    /// Writing the report failed.
    #[error("the report could not be written: {0}")]
    Io(#[from] std::io::Error),
    /// A Wayland step failed.
    #[error(transparent)]
    Present(#[from] PresentError),
    /// A decode step failed.
    #[error(transparent)]
    Decode(#[from] DecodeError),
    /// The render node could not be resolved.
    #[error(transparent)]
    Device(#[from] device::DeviceError),
    /// The fixture did not parse.
    #[error("fixture frame {frame}: {error}")]
    Jpeg {
        /// The frame number.
        frame: usize,
        /// Why.
        error: JpegError,
    },
    /// A decoded frame failed its content check.
    #[error("fixture frame {frame}: {error}")]
    Content {
        /// The frame number.
        frame: usize,
        /// Why.
        error: content::ContentError,
    },
    /// A descriptor could not be built.
    #[error(transparent)]
    Descriptor(#[from] DescriptorError),
    /// The socket pair failed.
    #[error(transparent)]
    Transport(#[from] TransportError),
    /// The attach checks refused a frame that should pass.
    #[error(transparent)]
    Attach(#[from] AttachError),
    /// A step produced something the run does not admit.
    #[error("{0}")]
    Refused(String),
}

/// The report the run writes.
struct Report<'w> {
    out: &'w mut dyn Write,
    failed: u32,
}

impl Report<'_> {
    fn info(&mut self, key: &str, value: impl Display) -> Result<(), SliceError> {
        writeln!(self.out, "info {key}: {value}")?;
        Ok(())
    }

    fn case(&mut self, name: &str, passed: bool, notes: &[String]) -> Result<(), SliceError> {
        writeln!(self.out, "{} {name}", if passed { "PASS" } else { "FAIL" })?;
        for note in notes {
            writeln!(self.out, "     {note}")?;
        }
        if !passed {
            self.failed = self.failed.saturating_add(1);
        }
        Ok(())
    }

    fn verdict(&self) -> Verdict {
        if self.failed == 0 {
            Verdict::Passed
        } else {
            Verdict::Failed(self.failed)
        }
    }
}

/// Maps a setup failure that means a missing capability onto a skip.
fn skip_reason(error: &SliceError) -> Option<String> {
    match error {
        SliceError::Present(PresentError::Connect { reason }) => {
            Some(format!("no Wayland compositor could be reached ({reason})"))
        }
        SliceError::Present(missing @ PresentError::Missing { .. }) => Some(missing.to_string()),
        SliceError::Decode(unsupported @ DecodeError::Unsupported { .. }) => {
            Some(unsupported.to_string())
        }
        _ => None,
    }
}

/// Resolves the decode node from the compositor's main device through
/// `/sys/class/drm`, and refuses every other render node (negative (b)).
fn decode_node(report: &mut Report<'_>, main_device: u64) -> Result<RenderNode, SliceError> {
    let root = Path::new(device::SYS_CLASS_DRM);
    let (major, minor) = (
        rustix::fs::major(main_device),
        rustix::fs::minor(main_device),
    );
    report.info("compositor main device", format_args!("{major}:{minor}"))?;
    let node = device::main_device_node(root, main_device)?;
    let path = device::verify_device_file(&node, Path::new(device::DEV_DRI))?;
    report.info(
        "decode node",
        format_args!(
            "{} ({major}:{minor}, kernel driver {})",
            path.display(),
            node.driver
        ),
    )?;
    let mut notes = Vec::new();
    for other in device::render_nodes(root)?
        .iter()
        .filter(|other| other.dev != node.dev)
    {
        match device::require_main_device(other, main_device) {
            Err(refused) => notes.push(format!("refused ({}): {refused}", other.driver)),
            Ok(()) => return Err(SliceError::Refused(format!("{} was admitted", other.name))),
        }
    }
    if notes.is_empty() {
        writeln!(
            report.out,
            "SKIP display/other-render-node-refused: this host has one render node"
        )?;
    } else {
        report.case("display/other-render-node-refused", true, &notes)?;
    }
    Ok(node)
}

/// Opens the VA display on the decode node and requires the iHD driver.
fn open_decoder(
    report: &mut Report<'_>,
    node: &RenderNode,
) -> Result<Option<VaDevice>, SliceError> {
    let path = node.path(Path::new(device::DEV_DRI));
    let decoder = VaDevice::open(&path)?;
    report.info("VA vendor string", decoder.vendor())?;
    match require_ihd(decoder.vendor()) {
        Ok(()) => {
            report.case("display/driver-is-ihd", true, &[])?;
            Ok(Some(decoder))
        }
        Err(refused) => {
            report.case("display/driver-is-ihd", false, &[refused.to_string()])?;
            Ok(None)
        }
    }
}

/// The reference MPEG-2 frame's CRC-32 case.
fn reference_case(report: &mut Report<'_>, decoder: &VaDevice) -> Result<(), SliceError> {
    match crate::decode::decode_reference(decoder, READY_WAIT) {
        Ok(crc) => report.case(
            "display/mpeg2-reference-crc",
            true,
            &[format!(
                "CRC-32 {crc:#010x} over the visible NV12 lines, cros-libva's value"
            )],
        ),
        Err(error @ DecodeError::ReferenceCrc { .. }) => {
            report.case("display/mpeg2-reference-crc", false, &[error.to_string()])
        }
        Err(other) => Err(other.into()),
    }
}

/// Reads one decoded frame back: its CRC-32 and its index row.
fn read_back(
    bytes: &[u8],
    layout: Nv12Layout,
    expected: u32,
) -> Result<u32, content::ContentError> {
    let [offset, _] = layout.offsets;
    let [pitch, _] = layout.pitches;
    content::check_index(
        LumaPlane {
            bytes,
            offset,
            pitch,
        },
        expected,
    )?;
    content::crc_nv12(bytes, layout)
}

/// The planted zero-filled surface must fail the content check.
fn planted_zero_case(
    report: &mut Report<'_>,
    decoder: &VaDevice,
    pipeline: &JpegPipeline,
) -> Result<(), SliceError> {
    let index = fixture_frames();
    pipeline.plant_zeros(decoder, index, READY_WAIT)?;
    let outcome = pipeline.inspect(decoder, index, |bytes, layout| read_back(bytes, layout, 1))?;
    let note = match outcome {
        Err(refused) => format!("refused before export: {refused}"),
        Ok(crc) => format!("a zero-filled surface passed the content check (CRC-32 {crc:#010x})"),
    };
    report.case(
        "display/planted-zero-surface-refused",
        outcome.is_err(),
        &[note],
    )
}

/// The fixture frame count as a `usize`.
const fn fixture_frames() -> usize {
    content::FIXTURE_FRAMES
}

/// What the frame loop measured.
#[derive(Debug)]
struct Tally {
    crcs: [u32; 60],
    identity_equal: u32,
    modifier_unchanged: u32,
    modifier: Option<u64>,
    max_polls: u32,
    planted: Option<Result<AttachError, String>>,
    frames: u32,
}

/// Everything the frame loop drives.
struct Loop<'a> {
    decoder: &'a VaDevice,
    pipeline: &'a JpegPipeline,
    presenter: &'a mut Presenter,
    sender: FrameSender,
    receiver: FrameReceiver,
    send_buffer: LineBuffer,
    receive_buffer: LineBuffer,
    tally: Tally,
}

/// `m27-fixture-NN`, written into a stack buffer.
fn correlation(number: usize) -> Result<CorrelationId, DescriptorError> {
    let mut text = *b"m27-fixture-00";
    let tens = number.checked_div(10).and_then(|tens| tens.checked_rem(10));
    let ones = number.checked_rem(10);
    let tens = tens.and_then(|digit| u8::try_from(digit).ok()).unwrap_or(0);
    let ones = ones.and_then(|digit| u8::try_from(digit).ok()).unwrap_or(0);
    if let (Some(high), Some(low)) = (text.get(12).copied(), text.get(13).copied()) {
        let digits = [high.saturating_add(tens), low.saturating_add(ones)];
        if let Some(slot) = text.get_mut(12..14) {
            slot.copy_from_slice(&digits);
        }
    }
    CorrelationId::parse(&text)
}

/// The descriptor for one exported frame, its modifier unchanged.
fn descriptor(exported: &Exported, number: usize) -> Result<DecodedFrame, SliceError> {
    let size = (content::FIXTURE_WIDTH, content::FIXTURE_HEIGHT);
    let planes = Planes::new(&exported.planes)?;
    Ok(DecodedFrame::new(
        correlation(number)?,
        FourCc::NV12,
        exported.modifier,
        size,
        planes,
    )?)
}

impl Loop<'_> {
    /// Sends one frame over the socket pair and returns what P17 received,
    /// checked.
    fn cross(
        &mut self,
        id: u64,
        frame: DecodedFrame,
        fd: std::os::fd::BorrowedFd<'_>,
    ) -> Result<attach::Checked, SliceError> {
        let request = Request { id, frame };
        self.sender
            .send_request(&request, fd, &mut self.send_buffer)?;
        let (received, received_fd) = self.receiver.receive_request(&mut self.receive_buffer)?;
        if received.id != id {
            return Err(SliceError::Refused(format!(
                "request {id} arrived as {}",
                received.id
            )));
        }
        Ok(attach::check(received.frame, received_fd)?)
    }

    /// Plants NV12 with `INTEL_4_TILED_DG2_RC_CCS` over a duplicate of the
    /// frame's fd; P17 must refuse it before any request is sent.
    fn plant_unadvertised(
        &mut self,
        frame: DecodedFrame,
        exported: &Exported,
    ) -> Result<(), SliceError> {
        let planted = DecodedFrame {
            modifier: MOD_INTEL_4_TILED_DG2_RC_CCS,
            ..frame
        };
        let duplicate = exported.fd.as_fd().try_clone_to_owned().map_err(|error| {
            SliceError::Refused(format!("the exported fd could not be duplicated: {error}"))
        })?;
        let checked = self.cross(1_000, planted, duplicate.as_fd())?;
        drop(duplicate);
        self.tally.planted = Some(match self.presenter.present(checked) {
            Err(PresentError::Attach(refused @ AttachError::NotAdvertised { .. })) => Ok(refused),
            Err(other) => Err(other.to_string()),
            Ok(()) => Err("the unadvertised pair was attached".to_owned()),
        });
        Ok(())
    }

    /// Decodes, reads back, exports, sends, checks and presents frame
    /// `index` of the fixture.
    fn step(&mut self, index: usize, frame: &jpeg::JpegFrame<'_>) -> Result<(), SliceError> {
        let number = index.saturating_add(1);
        let expected = u32::try_from(number).map_err(|_| SliceError::Refused("frame".into()))?;
        let polls = self.pipeline.decode(index, frame, READY_WAIT)?;
        self.tally.max_polls = self.tally.max_polls.max(polls);
        let read = self
            .pipeline
            .inspect(self.decoder, index, |bytes, layout| {
                read_back(bytes, layout, expected)
            })?;
        let crc = read.map_err(|error| SliceError::Content {
            frame: number,
            error,
        })?;
        if let Some(slot) = self.tally.crcs.get_mut(index) {
            *slot = crc;
        }
        let exported = self.pipeline.export(index)?;
        let exported_identity: FileIdentity = attach::identity(exported.fd.as_fd())?;
        let descriptor = descriptor(&exported, number)?;
        if number == PLANT_AFTER.saturating_add(1) {
            self.plant_unadvertised(descriptor, &exported)?;
        }
        let checked = self.cross(u64::from(expected), descriptor, exported.fd.as_fd())?;
        drop(exported.fd);
        self.tally.identity_equal = self
            .tally
            .identity_equal
            .saturating_add(u32::from(checked.identity() == exported_identity));
        let unchanged = checked.frame().modifier == exported.modifier;
        self.tally.modifier_unchanged = self
            .tally
            .modifier_unchanged
            .saturating_add(u32::from(unchanged));
        self.tally.modifier.get_or_insert(exported.modifier);
        self.presenter.present(checked)?;
        self.tally.frames = self.tally.frames.saturating_add(1);
        Ok(())
    }

    /// Runs every fixture frame in order.
    fn run(&mut self) -> Result<(), SliceError> {
        let mut rest = fixture::MJPEG;
        for index in 0..fixture_frames() {
            let (frame, used) = jpeg::parse(rest).map_err(|error| SliceError::Jpeg {
                frame: index.saturating_add(1),
                error,
            })?;
            self.step(index, &frame)?;
            rest = rest.get(used..).unwrap_or_default();
        }
        if rest.is_empty() {
            Ok(())
        } else {
            Err(SliceError::Refused(format!(
                "{} bytes follow the 60th frame",
                rest.len()
            )))
        }
    }
}

/// Writes the frame loop's cases.
fn frame_cases(
    report: &mut Report<'_>,
    tally: &Tally,
    presenter: &Presenter,
    vendor: &str,
) -> Result<(), SliceError> {
    let counts = presenter.counts();
    let all = u32::try_from(fixture_frames()).unwrap_or(u32::MAX);
    let every_frame = counts.created == all
        && counts.failed == 0
        && counts.presented == all
        && counts.discarded == 0;
    report.case(
        "display/fixture-frames-present",
        every_frame && tally.frames == all,
        &[
            format!("{} frames decoded and read back against their index rows, frame 1 and frame 60 included", tally.frames),
            format!("{} created, {} failed, {} committed, {} presented, {} discarded, {} released", counts.created, counts.failed, counts.committed, counts.presented, counts.discarded, counts.released),
            format!("slowest decode reached VASurfaceReady after {} polls", tally.max_polls),
        ],
    )?;
    report.case(
        "display/received-fd-is-the-exported-one",
        tally.identity_equal == all,
        &[format!(
            "{} of {all} received fds have the exported (st_dev, st_ino)",
            tally.identity_equal
        )],
    )?;
    report.case(
        "display/descriptor-carries-the-exported-modifier",
        tally.modifier_unchanged == all,
        &[format!(
            "{} of {all} descriptors carry export_prime's modifier unchanged",
            tally.modifier_unchanged
        )],
    )?;
    let planted = match &tally.planted {
        Some(Ok(refused)) => (
            true,
            format!(
                "refused client-side after frame {PLANT_AFTER}: {refused}; frame {} then presented",
                PLANT_AFTER.saturating_add(1)
            ),
        ),
        Some(Err(why)) => (false, why.clone()),
        None => (false, "the unadvertised pair was never planted".to_owned()),
    };
    report.case("display/unadvertised-pair-refused", planted.0, &[planted.1])?;
    pin_case(report, tally, vendor)
}

/// The per-frame CRC-32 regression pin (D73 pattern).
fn pin_case(report: &mut Report<'_>, tally: &Tally, vendor: &str) -> Result<(), SliceError> {
    let first = tally.crcs.first().copied().unwrap_or_default();
    let last = tally.crcs.last().copied().unwrap_or_default();
    let measured = format!("frame 1 CRC-32 {first:#010x}, frame 60 {last:#010x}");
    match fixture::compare_pins(vendor, &tally.crcs) {
        PinComparison::Matched => {
            report.case("display/frame-crc-regression-pin", true, &[measured])
        }
        PinComparison::Regressed { differing } => report.case(
            "display/frame-crc-regression-pin",
            false,
            &[
                format!("{differing} of 60 frames differ from the pins recorded under this driver"),
                measured,
            ],
        ),
        PinComparison::OtherDriver { differing } => {
            writeln!(
                report.out,
                "NOTE display/frame-crc-regression-pin: pins recorded under {:?}; this run's driver is {vendor:?}; {differing} of 60 differ; re-measure and record them with the driver version (D73)",
                fixture::RECORDED_UNDER
            )?;
            for crc in tally.crcs {
                writeln!(report.out, "     measured {crc:#010x}")?;
            }
            Ok(())
        }
    }
}

/// Decodes the fixture and presents every frame.
fn present_fixture(
    report: &mut Report<'_>,
    decoder: &VaDevice,
    presenter: &mut Presenter,
) -> Result<(), SliceError> {
    let size = (content::FIXTURE_WIDTH, content::FIXTURE_HEIGHT);
    let pipeline = JpegPipeline::new(decoder, size, fixture_frames().saturating_add(1))?;
    planted_zero_case(report, decoder, &pipeline)?;
    let (sender, receiver) = pair(SOCKET_DEADLINES)?;
    let mut frames = Loop {
        decoder,
        pipeline: &pipeline,
        presenter,
        sender,
        receiver,
        send_buffer: LineBuffer::new(),
        receive_buffer: LineBuffer::new(),
        tally: Tally {
            crcs: [0; 60],
            identity_equal: 0,
            modifier_unchanged: 0,
            modifier: None,
            max_polls: 0,
            planted: None,
            frames: 0,
        },
    };
    frames.run()?;
    let tally = frames.tally;
    if let Some(modifier) = tally.modifier {
        report.info(
            "modifier the driver chose",
            format_args!("{modifier:#018x}"),
        )?;
    }
    frame_cases(report, &tally, presenter, decoder.vendor())
}

/// The boundary cases around the surface: an attach before the first
/// `ack_configure` is refused.
fn attach_before_ack_case(
    report: &mut Report<'_>,
    presenter: &Presenter,
) -> Result<(), SliceError> {
    let refused = presenter.try_attach();
    let note = match refused {
        Err(error) => format!("refused before the first ack_configure: {error}"),
        Ok(_) => "an attach before the first ack_configure was admitted".to_owned(),
    };
    report.case(
        "display/attach-before-ack-refused",
        refused.is_err(),
        &[note],
    )
}

/// A presented event that does not arrive fails at its deadline and names
/// the event.
fn unanswered_presentation_case(
    report: &mut Report<'_>,
    presenter: &mut Presenter,
) -> Result<(), SliceError> {
    let outcome = presenter.await_uncommitted_presentation(UNANSWERED_DEADLINE);
    let (passed, note) = match outcome {
        Err(
            missed @ crate::error::WaitError::Deadline {
                event: crate::wayland::WaitEvent::Presented,
            },
        ) => (
            true,
            format!(
                "failed at its {} ms deadline: {missed}",
                UNANSWERED_DEADLINE.as_millis()
            ),
        ),
        Err(other) => (false, other.to_string()),
        Ok(()) => (
            false,
            "a presentation event arrived for a surface that was never committed".to_owned(),
        ),
    };
    report.case("display/presented-deadline", passed, &[note])
}

/// Connects, reports the compositor process and waits for the surface's
/// configure and feedback.
fn connect(report: &mut Report<'_>) -> Result<Presenter, SliceError> {
    let size = (content::FIXTURE_WIDTH, content::FIXTURE_HEIGHT);
    let mut presenter = Presenter::connect(size, DEADLINES)?;
    match presenter.peer().pid {
        Some(pid) => report.info("compositor pid", pid)?,
        None => report.info("compositor pid", "unreported")?,
    }
    presenter.wait_setup()?;
    Ok(presenter)
}

/// The steps of the positive run after the connection.
fn positive(
    report: &mut Report<'_>,
    presenter: &mut Presenter,
) -> Result<Option<String>, SliceError> {
    attach_before_ack_case(report, presenter)?;
    presenter.ack()?;
    let main_device = presenter.main_device().ok_or(PresentError::Feedback {
        what: "no main device",
    })?;
    let node = decode_node(report, main_device)?;
    if !INTEL_DRIVERS.contains(&node.driver.as_str()) {
        return Ok(Some(format!(
            "the compositor's main device {} is driven by {}, not i915 or xe, so the iHD decode the reference profile has is absent",
            node.name, node.driver
        )));
    }
    let Some(decoder) = open_decoder(report, &node)? else {
        return Ok(None);
    };
    reference_case(report, &decoder)?;
    present_fixture(report, &decoder, presenter)?;
    unanswered_presentation_case(report, presenter)?;
    Ok(None)
}

/// Runs a body against a fresh presenter, closing it on every path and
/// mapping a missing capability onto a skip.
fn with_presenter(
    out: &mut dyn Write,
    body: fn(&mut Report<'_>, &mut Presenter) -> Result<Option<String>, SliceError>,
) -> Result<Verdict, SliceError> {
    let mut report = Report { out, failed: 0 };
    writeln!(
        report.out,
        "Display slice (M27, D79): development evidence, client half only; the compositor is the host session's, not P04."
    )?;
    let result = connect(&mut report).and_then(|mut presenter| {
        let outcome = body(&mut report, &mut presenter);
        presenter.close();
        outcome
    });
    match result {
        Ok(Some(reason)) => Ok(Verdict::Skipped(reason)),
        Ok(None) => Ok(report.verdict()),
        Err(error) => skip_reason(&error).map_or(Err(error), |reason| Ok(Verdict::Skipped(reason))),
    }
}

/// The positive run and the negatives inside it.
///
/// # Errors
///
/// Returns the first step that failed before its case could be decided.
pub fn run_display(out: &mut dyn Write) -> Result<Verdict, SliceError> {
    with_presenter(out, positive)
}

/// Negative (a): under the session's `LIBVA_DRIVER_NAME`, the decode node's
/// VA display must be refused before anything is decoded or attached.
fn driver_probe(
    report: &mut Report<'_>,
    presenter: &mut Presenter,
) -> Result<Option<String>, SliceError> {
    let main_device = presenter.main_device().ok_or(PresentError::Feedback {
        what: "no main device",
    })?;
    let root = Path::new(device::SYS_CLASS_DRM);
    let node = device::main_device_node(root, main_device)?;
    let path = device::verify_device_file(&node, Path::new(device::DEV_DRI))?;
    report.info(
        "decode node",
        format_args!("{} (kernel driver {})", path.display(), node.driver),
    )?;
    let variable = std::env::var("LIBVA_DRIVER_NAME").unwrap_or_else(|_| "(unset)".to_owned());
    report.info("LIBVA_DRIVER_NAME in this process", &variable)?;
    let (passed, detail) = match VaDevice::open(&path) {
        Err(refused) => (true, format!("refused before any decode: {refused}")),
        Ok(decoder) => {
            report.info("VA vendor string", decoder.vendor())?;
            match require_ihd(decoder.vendor()) {
                Err(refused) => (true, format!("refused before any decode: {refused}")),
                Ok(()) => (
                    false,
                    "the vendor string names iHD; the driver variable selected nothing else"
                        .to_owned(),
                ),
            }
        }
    };
    report.case("display/driver-not-ihd-refused", passed, &[detail])?;
    Ok(None)
}

/// Negative (a), run by the gate with `LIBVA_DRIVER_NAME=nvidia` in this
/// process's environment only.
///
/// # Errors
///
/// Returns the first step that failed before the case could be decided.
pub fn run_driver_probe(out: &mut dyn Write) -> Result<Verdict, SliceError> {
    with_presenter(out, driver_probe)
}
