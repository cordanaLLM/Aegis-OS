// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Subtitle timing presentation run on the reference profile (milestone M32,
//! REQ-P17-08, E32-1).
//!
//! Evaluates presentation synchronization against `wp_presentation` timestamps
//! for a 90-frame Motion-JPEG video with synchronized audio and `SubRip` subtitle
//! cues. Paced at 30 fps, each frame's presentation timestamp is recorded, and
//! subtitle presentation offsets are judged against the 100 ms
//! [`SYNC_BOUND`](crate::subtitle::SYNC_BOUND) tolerance bound.

use core::time::Duration;
use std::io::Write;
use std::os::fd::AsFd;
use std::time::Instant;

use crate::decode::{JpegPipeline, READY_WAIT, VaDevice};
use crate::descriptor::{CorrelationId, DecodedFrame, FourCc, Planes};
use crate::error::DescriptorError;
use crate::jpeg;
use crate::line::{LineBuffer, Request};
use crate::present::{PresentError, Presenter};
use crate::slice::{
    INTEL_DRIVERS, Report, SliceError, Verdict, decode_node, open_decoder, with_presenter,
};
use crate::subtitle::{self, Cue, Offset};
use crate::transport::{Deadlines, FrameReceiver, FrameSender, pair};

/// The committed Motion-JPEG subtitle fixture clip (90 frames, 256x64, 4:2:0).
pub const SUBTITLE_MJPEG: &[u8] = include_bytes!("../fixtures/subtitle-clip-90.mjpeg");

/// The committed `SubRip` subtitle track (3 cues aligned to whole frames).
pub const SUBTITLE_SRT: &str = include_str!("../fixtures/subtitle-clip-90.srt");

/// Number of frames in the subtitle clip.
pub const SUBTITLE_FRAMES: usize = 90;

/// The frame pixel dimensions (width, height) for the subtitle clip.
const SUBTITLE_SIZE: (u32, u32) = (256, 64);

/// The socket pair deadlines for subtitle frame transport.
const SOCKET_DEADLINES: Deadlines = Deadlines {
    receive: Duration::from_secs(1),
    send: Duration::from_secs(1),
};

/// The banner header line for M32 subtitle slice.
pub const M32_HEADER: &str = "Display slice (M32, D79): development evidence, client half only; subtitle text is not rasterised: each subtitle's presentation time is the wp_presentation timestamp of the frame its start timecode falls on.";

/// The target time offset from `t0` for frame `frame_index` at 30 fps.
#[must_use]
pub fn pacing_target_duration(frame_index: usize) -> Duration {
    let nanos = u64::try_from(frame_index)
        .unwrap_or(u64::MAX)
        .saturating_mul(1_000_000_000)
        .checked_div(30)
        .unwrap_or(0);
    Duration::from_nanos(nanos)
}

/// The 0-based frame index corresponding to `start` at 30 fps.
#[must_use]
pub fn cue_frame_index(start: Duration) -> usize {
    let start_ms = start.as_millis();
    usize::try_from(start_ms.saturating_mul(30) / 1000).unwrap_or(usize::MAX)
}

/// `m32-subtitle-NN`, written into a stack buffer.
///
/// # Errors
///
/// Returns [`DescriptorError`] if the correlation id cannot be parsed.
pub fn correlation_id(number: usize) -> Result<CorrelationId, DescriptorError> {
    let mut text = *b"m32-subtitle-00";
    let tens = number.checked_div(10).and_then(|tens| tens.checked_rem(10));
    let ones = number.checked_rem(10);
    let tens = tens.and_then(|digit| u8::try_from(digit).ok()).unwrap_or(0);
    let ones = ones.and_then(|digit| u8::try_from(digit).ok()).unwrap_or(0);
    if let (Some(high), Some(low)) = (text.get(13).copied(), text.get(14).copied()) {
        let digits = [high.saturating_add(tens), low.saturating_add(ones)];
        if let Some(slot) = text.get_mut(13..15) {
            slot.copy_from_slice(&digits);
        }
    }
    CorrelationId::parse(&text)
}

/// Computes per-frame presentation offsets relative to frame 0.
#[must_use]
pub fn compute_offsets(
    timestamps: &[Option<Duration>; SUBTITLE_FRAMES],
) -> [Option<Duration>; SUBTITLE_FRAMES] {
    let mut offsets = [None; SUBTITLE_FRAMES];
    let Some(base) = timestamps.first().copied().flatten() else {
        return offsets;
    };
    for (i, opt_p) in timestamps.iter().enumerate() {
        if let (Some(p), Some(slot)) = (*opt_p, offsets.get_mut(i)) {
            *slot = p.checked_sub(base);
        }
    }
    offsets
}

/// Forwarding bridge that passes decoded frames across the socket pair.
struct SocketBridge<'a> {
    sender: &'a mut FrameSender,
    receiver: &'a mut FrameReceiver,
    send_buffer: &'a mut LineBuffer,
    receive_buffer: &'a mut LineBuffer,
}

impl SocketBridge<'_> {
    fn cross(
        &mut self,
        id: u64,
        frame: DecodedFrame,
        fd: std::os::fd::BorrowedFd<'_>,
    ) -> Result<crate::attach::Checked, SliceError> {
        let request = Request { id, frame };
        self.sender.send_request(&request, fd, self.send_buffer)?;
        let (received, received_fd) = self.receiver.receive_request(self.receive_buffer)?;
        if received.id != id {
            return Err(SliceError::Refused(format!(
                "request {id} arrived as {}",
                received.id
            )));
        }
        Ok(crate::attach::check(received.frame, received_fd)?)
    }
}

/// Paces presentation to 30 fps against the baseline start instant.
fn pace_frame(index: usize, t0: &mut Option<Instant>, max_slip: &mut Duration) {
    if index == 0 {
        *t0 = Some(Instant::now());
        return;
    }
    let Some(base) = *t0 else {
        return;
    };
    let Some(target) = base.checked_add(pacing_target_duration(index)) else {
        return;
    };
    let now = Instant::now();
    if target > now {
        let remaining = target.duration_since(now).min(Duration::from_millis(100));
        std::thread::sleep(remaining);
    }
    let after = Instant::now();
    if after > target {
        let slip = after.duration_since(target);
        *max_slip = (*max_slip).max(slip);
    }
}

/// Evaluates presented subtitle cue timing against the synchronization bound.
fn evaluate_cues(
    report: &mut Report<'_>,
    cues: &[Cue],
    offsets: &[Option<Duration>; SUBTITLE_FRAMES],
) -> Result<(), SliceError> {
    for cue in cues {
        let case_name = format!("subtitle/cue-{}-within-100ms", cue.index);
        let frame = cue_frame_index(cue.start);
        if let Some(offset) = offsets.get(frame).copied().flatten() {
            let passed = subtitle::judge(cue.start, offset).is_ok();
            let sign = if offset >= cue.start { "+" } else { "-" };
            let diff_ms = if offset >= cue.start {
                offset.saturating_sub(cue.start).as_millis()
            } else {
                cue.start.saturating_sub(offset).as_millis()
            };
            let note = format!(
                "timecode {} ms, presented offset {} ms (difference {sign}{} ms)",
                cue.start.as_millis(),
                offset.as_millis(),
                diff_ms,
            );
            report.case(&case_name, passed, &[note])?;
        } else {
            let note = format!("frame {frame} presentation timestamp missing");
            report.case(&case_name, false, &[note])?;
        }
    }
    Ok(())
}

/// Evaluates the planted 150 ms late cue case to verify it is refused.
fn evaluate_planted_case(
    report: &mut Report<'_>,
    cues: &[Cue],
    offsets: &[Option<Duration>; SUBTITLE_FRAMES],
) -> Result<(), SliceError> {
    let case_name = "subtitle/planted-150ms-late-refused";
    let Some(target_cue) = cues.iter().find(|c| c.index == 2).or_else(|| cues.get(1)) else {
        return report.case(
            case_name,
            false,
            &["cue 2 missing from subtitles".to_owned()],
        );
    };
    let frame2 = cue_frame_index(target_cue.start);
    let Some(target_offset) = offsets.get(frame2).copied().flatten() else {
        return report.case(
            case_name,
            false,
            &[format!("frame {frame2} presentation timestamp missing")],
        );
    };
    let planted = target_offset.saturating_add(Duration::from_millis(150));
    let outcome = subtitle::judge(target_cue.start, planted);
    let (passed, note) = match outcome {
        Err(Offset { late: true, offset }) => (
            true,
            format!(
                "refused 150 ms late presentation: offset of {} ms is outside sync bound (late: true)",
                offset.as_millis()
            ),
        ),
        Err(Offset {
            late: false,
            offset,
        }) => (
            false,
            format!(
                "offset {} ms was not recognized as late",
                offset.as_millis()
            ),
        ),
        Ok(offset) => (
            false,
            format!(
                "planted late offset of {} ms was admitted",
                offset.as_millis()
            ),
        ),
    };
    report.case(case_name, passed, &[note])
}

/// Records frame count, pacing slip statistics, and evaluated cue cases in the report.
fn record_subtitle_results(
    report: &mut Report<'_>,
    cues: &[Cue],
    timestamps: &[Option<Duration>; SUBTITLE_FRAMES],
    count: u32,
    max_slip: Duration,
) -> Result<(), SliceError> {
    report.info("frames presented", count)?;
    report.info(
        "max per-frame pacing slip",
        format_args!("{} ms", max_slip.as_millis()),
    )?;
    let offsets = compute_offsets(timestamps);
    evaluate_cues(report, cues, &offsets)?;
    evaluate_planted_case(report, cues, &offsets)
}

/// Decodes, transports, and presents the subtitle clip frames while recording timing.
fn present_subtitles(
    report: &mut Report<'_>,
    decoder: &VaDevice,
    presenter: &mut Presenter,
) -> Result<(), SliceError> {
    let cues = subtitle::parse_srt(SUBTITLE_SRT)
        .map_err(|err| SliceError::Refused(format!("SRT parse error: {err}")))?;
    let pipeline = JpegPipeline::new(decoder, SUBTITLE_SIZE, SUBTITLE_FRAMES.saturating_add(1))?;
    let (mut sender, mut receiver) = pair(SOCKET_DEADLINES)?;
    let mut send_buf = LineBuffer::new();
    let mut recv_buf = LineBuffer::new();
    let mut rest = SUBTITLE_MJPEG;
    let mut timestamps: [Option<Duration>; SUBTITLE_FRAMES] = [None; SUBTITLE_FRAMES];
    let mut t0: Option<Instant> = None;
    let mut max_slip = Duration::ZERO;
    let mut count = 0_u32;
    let mut bridge = SocketBridge {
        sender: &mut sender,
        receiver: &mut receiver,
        send_buffer: &mut send_buf,
        receive_buffer: &mut recv_buf,
    };

    for index in 0..SUBTITLE_FRAMES {
        let (frame, used) = jpeg::parse(rest).map_err(|error| SliceError::Jpeg {
            frame: index.saturating_add(1),
            error,
        })?;
        rest = rest.get(used..).unwrap_or_default();
        let number = index.saturating_add(1);
        pipeline.decode(index, &frame, READY_WAIT)?;
        let exported = pipeline.export(index)?;
        let planes = Planes::new(&exported.planes)?;
        let id = correlation_id(number)?;
        let desc = DecodedFrame::new(id, FourCc::NV12, exported.modifier, SUBTITLE_SIZE, planes)?;
        let checked = bridge.cross(
            u64::try_from(number).unwrap_or(0),
            desc,
            exported.fd.as_fd(),
        )?;
        drop(exported.fd);
        pace_frame(index, &mut t0, &mut max_slip);
        presenter.present(checked)?;
        count = count.saturating_add(1);
        if let Some(slot) = timestamps.get_mut(index) {
            *slot = presenter.last_presented_at();
        }
    }
    record_subtitle_results(report, &cues, &timestamps, count, max_slip)
}

/// Verifies the compositor driver and runs positive subtitle presentation.
fn positive_subtitles(
    report: &mut Report<'_>,
    presenter: &mut Presenter,
) -> Result<Option<String>, SliceError> {
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
    present_subtitles(report, &decoder, presenter)?;
    Ok(None)
}

/// The subtitle presentation run on the reference profile (milestone M32).
///
/// # Errors
///
/// Returns the first step that failed before its cases were decided.
pub fn run_subtitles(out: &mut dyn Write) -> Result<Verdict, SliceError> {
    with_presenter(out, M32_HEADER, positive_subtitles)
}
