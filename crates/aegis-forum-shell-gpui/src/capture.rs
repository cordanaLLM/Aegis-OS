// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `KWin` `org.kde.KWin.ScreenShot2` capture client over D-Bus (M28 E28-4).
//!
//! Connects to `KWin`'s screenshot interface on the private session bus, passes a
//! pipe write end via Unix file descriptor passing, captures raw pixel data
//! under an explicit deadline, and decodes the result into an [`Image`].

use std::collections::HashMap;
use std::time::Duration;

use futures_lite::AsyncReadExt;
use thiserror::Error;
use zbus::zvariant::{Fd, OwnedValue, Value};

use crate::pixels::{Image, PixelError, from_raw};

/// Maximum duration permitted for an individual capture D-Bus roundtrip or pipe read.
pub const CAPTURE_DEADLINE: Duration = Duration::from_secs(5);

/// Errors arising during `KWin` screen capture operations.
#[derive(Debug, Error)]
pub enum CaptureError {
    /// A D-Bus method call or pipe read exceeded its allotted deadline.
    #[error("capture operation '{call}' timed out after {ms} ms")]
    Deadline {
        /// Name of the operation that timed out.
        call: String,
        /// Configured deadline duration in milliseconds.
        ms: u128,
    },
    /// An underlying D-Bus protocol or transport failure occurred.
    #[error("D-Bus error: {0}")]
    Bus(String),
    /// An unsupported `QImage` format was returned.
    #[error("unsupported QImage format: {0}")]
    Format(u32),
    /// The compositor returned a scale factor other than 1.0.
    #[error("unsupported compositor scale: {0}")]
    Scale(f64),
    /// The capture metadata or byte buffer had an invalid size.
    #[error("invalid image size: {0}")]
    Size(String),
    /// An I/O error occurred during pipe handling.
    #[error("I/O error: {0}")]
    Io(String),
    /// The compositor never showed the fixture surface at the probe point.
    #[error("fixture surface not composited: {0}")]
    NotReady(String),
    /// Raw pixel parsing failed.
    #[error(transparent)]
    Pixel(#[from] PixelError),
}

#[allow(clippy::too_many_arguments)]
#[zbus::proxy(
    interface = "org.kde.KWin.ScreenShot2",
    default_service = "org.kde.KWin",
    default_path = "/org/kde/KWin/ScreenShot2"
)]
trait ScreenShot2 {
    #[allow(clippy::too_many_arguments)]
    fn capture_area(
        &self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        options: HashMap<&str, Value<'_>>,
        pipe: Fd<'_>,
    ) -> zbus::Result<HashMap<String, OwnedValue>>;
}

async fn with_deadline<T, E: std::fmt::Display>(
    what: &str,
    d: Duration,
    fut: impl std::future::Future<Output = Result<T, E>>,
) -> Result<T, CaptureError> {
    if d.is_zero() {
        return Err(CaptureError::Deadline {
            call: what.to_owned(),
            ms: 0,
        });
    }
    let call_name = what.to_owned();
    let ms = d.as_millis();
    let timer = async {
        async_io::Timer::after(d).await;
        Err(CaptureError::Deadline {
            call: call_name,
            ms,
        })
    };
    let work = async { fut.await.map_err(|err| CaptureError::Bus(err.to_string())) };
    futures_lite::future::or(work, timer).await
}

fn extract_u32(val: &OwnedValue) -> Option<u32> {
    match &**val {
        Value::U32(v) => Some(*v),
        Value::I32(v) => u32::try_from(*v).ok(),
        Value::U64(v) => u32::try_from(*v).ok(),
        Value::I64(v) => u32::try_from(*v).ok(),
        _ => None,
    }
}

fn extract_f64(val: &OwnedValue) -> Option<f64> {
    match &**val {
        Value::F64(v) => Some(*v),
        Value::U32(v) => Some(f64::from(*v)),
        Value::I32(v) => Some(f64::from(*v)),
        _ => None,
    }
}

fn extract_str(val: &OwnedValue) -> Option<&str> {
    match &**val {
        Value::Str(s) => Some(s.as_str()),
        _ => None,
    }
}

fn parse_reply_params(
    reply: &HashMap<String, OwnedValue>,
) -> Result<(u32, u32, u32, u32), CaptureError> {
    let img_type = reply.get("type").and_then(extract_str).unwrap_or("unknown");
    if img_type != "raw" {
        return Err(CaptureError::Size(format!(
            "expected capture type 'raw', got '{img_type}'"
        )));
    }

    let format = reply
        .get("format")
        .and_then(extract_u32)
        .ok_or_else(|| CaptureError::Size("missing or invalid 'format' in reply".to_owned()))?;

    let width = reply
        .get("width")
        .and_then(extract_u32)
        .ok_or_else(|| CaptureError::Size("missing or invalid 'width' in reply".to_owned()))?;

    let height = reply
        .get("height")
        .and_then(extract_u32)
        .ok_or_else(|| CaptureError::Size("missing or invalid 'height' in reply".to_owned()))?;

    let stride = reply
        .get("stride")
        .and_then(extract_u32)
        .ok_or_else(|| CaptureError::Size("missing or invalid 'stride' in reply".to_owned()))?;

    let scale = reply.get("scale").and_then(extract_f64).unwrap_or(1.0);
    if (scale - 1.0).abs() > 0.001 {
        return Err(CaptureError::Scale(scale));
    }

    Ok((format, width, height, stride))
}

async fn read_pipe_bytes(
    reader: std::io::PipeReader,
    max_bytes: u64,
) -> Result<Vec<u8>, CaptureError> {
    let async_reader = async_io::Async::new(reader).map_err(|e| CaptureError::Io(e.to_string()))?;

    let mut buf = Vec::new();
    let max_read = max_bytes.saturating_add(1);
    let mut take_reader = async_reader.take(max_read);

    let read_fut = take_reader.read_to_end(&mut buf);
    with_deadline("pipe_read", CAPTURE_DEADLINE, read_fut).await?;

    let max_bytes_usize = usize::try_from(max_bytes)
        .map_err(|_| CaptureError::Size("buffer size overflow".to_owned()))?;
    if buf.len() > max_bytes_usize {
        return Err(CaptureError::Size(format!(
            "pipe read {} bytes, expected at most {}",
            buf.len(),
            max_bytes
        )));
    }

    Ok(buf)
}

/// Probes up to `attempts` times, `interval` apart, until `probe` reports ready.
///
/// Returns `Ok(false)` when every attempt reported not ready.
///
/// # Errors
///
/// Returns the first error `probe` returns; no further attempt is made.
pub async fn poll_ready<F, Fut>(
    attempts: u32,
    interval: Duration,
    mut probe: F,
) -> Result<bool, CaptureError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<bool, CaptureError>>,
{
    for attempt in 0..attempts {
        if attempt > 0 {
            async_io::Timer::after(interval).await;
        }
        if probe().await? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Attempts [`wait_for_pixel`] makes before it reports the surface missing.
pub const READY_ATTEMPTS: u32 = 50;

/// Interval between [`wait_for_pixel`] attempts.
pub const READY_INTERVAL: Duration = Duration::from_millis(20);

/// Waits until the composited pixel at `point` shows `want`.
///
/// The fixture's first frame reaching the client says nothing about the compositor
/// having composited it, so the readback captures one pixel of plain fixture surface
/// until it appears before measuring the ring.
///
/// # Errors
///
/// Returns [`CaptureError::NotReady`] when the pixel never matches, or a capture error.
pub async fn wait_for_pixel(
    conn: &zbus::Connection,
    point: (i32, i32),
    want: [u8; 3],
) -> Result<(), CaptureError> {
    let probe = || async move {
        let img = capture_area(conn, (point.0, point.1, 1, 1)).await?;
        Ok(img.pixel(0, 0) == Some(want))
    };
    if poll_ready(READY_ATTEMPTS, READY_INTERVAL, probe).await? {
        return Ok(());
    }
    Err(CaptureError::NotReady(format!(
        "pixel {point:?} never showed {want:?} in {READY_ATTEMPTS} attempts"
    )))
}

/// Connects to the session bus named by `DBUS_SESSION_BUS_ADDRESS` under
/// [`CAPTURE_DEADLINE`].
///
/// # Errors
///
/// Returns [`CaptureError::Deadline`] or [`CaptureError::Bus`].
pub async fn session_connection() -> Result<zbus::Connection, CaptureError> {
    with_deadline("session_bus", CAPTURE_DEADLINE, zbus::Connection::session()).await
}

/// Captures a screen area from `KWin` over D-Bus into an [`Image`].
///
/// # Errors
///
/// Returns [`CaptureError`] on timeout, protocol failure, unsupported image format,
/// scale != 1.0, or pipe read truncation.
pub async fn capture_area(
    conn: &zbus::Connection,
    rect: (i32, i32, i32, i32),
) -> Result<Image, CaptureError> {
    let proxy = ScreenShot2Proxy::builder(conn)
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
        .map_err(|e| CaptureError::Bus(e.to_string()))?;

    let (reader, writer) = std::io::pipe().map_err(|e| CaptureError::Io(e.to_string()))?;
    let fd = Fd::from(&writer);

    let mut options = HashMap::new();
    options.insert("include-cursor", Value::from(false));
    options.insert("native-resolution", Value::from(true));

    // KWin 6.8 declares `CaptureArea(i x, i y, u width, u height, a{sv}, h)`.
    let w = u32::try_from(rect.2).map_err(|_| CaptureError::Size(format!("width {}", rect.2)))?;
    let h = u32::try_from(rect.3).map_err(|_| CaptureError::Size(format!("height {}", rect.3)))?;
    let capture_fut = proxy.capture_area(rect.0, rect.1, w, h, options, fd);
    let reply = with_deadline("CaptureArea", CAPTURE_DEADLINE, capture_fut).await?;

    // Drop our write descriptor so reader sees EOF when KWin finishes writing
    drop(writer);

    let (format, width, height, stride) = parse_reply_params(&reply)?;
    let max_bytes = u64::from(stride).saturating_mul(u64::from(height));
    let buf = read_pipe_bytes(reader, max_bytes).await?;

    from_raw(&buf, width, height, stride, format).map_err(CaptureError::from)
}
