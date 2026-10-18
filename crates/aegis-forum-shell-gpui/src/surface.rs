// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Surface mapping and first configure outcome for the P05 Forum shell on gpui (M28 E28-2).
//!
//! Maps a [`MountCanvas`] onto gpui's layer-shell window options, connects to
//! the compositor via `gpui_linux`, and observes the first configure event.
//! If the compositor lacks `zwlr_layer_shell_v1`, gpui reports
//! [`LayerShellNotSupportedError`]. If the first configure does not arrive
//! within [`FIRST_CONFIGURE_DEADLINE`], a deadline outcome is recorded.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use aegis_scaena::mount::MountCanvas;
use gpui::layer_shell::LayerShellNotSupportedError;
use gpui::{
    App, AppContext, Bounds, Context, IntoElement, Pixels, Render, Size, Styled, Window,
    WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions, div, point,
    px,
};

use crate::layer::{LayerMapError, layer_shell_options};
use crate::theme;

/// The deadline for the compositor to deliver the first configure event.
pub const FIRST_CONFIGURE_DEADLINE: Duration = Duration::from_secs(5);

/// The requested height of the shell surface in pixels.
pub const SHELL_HEIGHT: f32 = 48.0;

const APP_ID: &str = "aegis-forum-shell";

/// The outcome of attempting to map the shell surface on a compositor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The first configure event arrived with compositor-assigned dimensions.
    Configured {
        /// The width assigned by the compositor.
        width: u32,
        /// The height assigned by the compositor.
        height: u32,
    },
    /// The compositor does not support `zwlr_layer_shell_v1`.
    LayerShellMissing(String),
    /// The first configure event did not arrive before [`FIRST_CONFIGURE_DEADLINE`].
    Deadline,
    /// Opening the window failed with an unexpected error.
    OpenFailed(String),
    /// The mount request line was refused before touching the display.
    Refused(String),
}

/// Builds the gpui [`WindowOptions`] for `canvas`.
///
/// # Errors
///
/// Returns [`LayerMapError`] if the canvas fails validation or cannot be mapped.
pub fn window_options(canvas: &MountCanvas) -> Result<WindowOptions, LayerMapError> {
    let layer_options = layer_shell_options(canvas)?;
    Ok(WindowOptions {
        titlebar: None,
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(0.0), px(0.0)),
            size: Size::new(px(0.0), px(SHELL_HEIGHT)),
        })),
        app_id: Some(APP_ID.to_owned()),
        window_background: WindowBackgroundAppearance::Opaque,
        kind: WindowKind::LayerShell(layer_options),
        ..Default::default()
    })
}

/// Classifies an error from opening a window into an [`Outcome`].
///
/// gpui returns [`LayerShellNotSupportedError`] itself as the error, not
/// wrapped in a context (`gpui_linux` `wayland/window.rs` at the pinned
/// commit), so the check reads the error the caller passes; an error that
/// wraps it is an open failure and its text says so.
#[must_use]
pub fn classify_open_error(error: &(dyn std::error::Error + 'static)) -> Outcome {
    if let Some(inner) = error.downcast_ref::<LayerShellNotSupportedError>() {
        Outcome::LayerShellMissing(format!("LayerShellNotSupportedError: {inner}"))
    } else {
        Outcome::OpenFailed(error.to_string())
    }
}

/// Formats an outcome as an exact reporting line.
#[must_use]
pub fn outcome_line(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Configured { width, height } => format!("OUTCOME configured {width}x{height}"),
        Outcome::LayerShellMissing(msg) => format!("OUTCOME layer-shell-missing: {msg}"),
        Outcome::Deadline => {
            let ms = FIRST_CONFIGURE_DEADLINE.as_millis();
            format!(
                "OUTCOME deadline: zwlr_layer_surface_v1.configure did not arrive within {ms} ms"
            )
        }
        Outcome::OpenFailed(msg) => format!("OUTCOME open-failed: {msg}"),
        Outcome::Refused(msg) => format!("OUTCOME refused: {msg}"),
    }
}

/// Returns the process exit code for an outcome.
#[must_use]
pub const fn exit_code(outcome: &Outcome) -> u8 {
    match outcome {
        Outcome::Configured { .. } => 0,
        Outcome::LayerShellMissing(_) => 4,
        Outcome::Deadline => 5,
        Outcome::OpenFailed(_) => 3,
        Outcome::Refused(_) => 6,
    }
}

/// Decodes a `MOUNT_SHELL_CANVAS` request line into a [`MountCanvas`].
///
/// # Errors
///
/// Returns [`Outcome::Refused`] if the line fails protocol validation.
pub fn decode_canvas(line: &[u8]) -> Result<MountCanvas, Outcome> {
    match aegis_scaena::mount::decode_request(line) {
        Ok(request) => Ok(request.canvas),
        Err(error) => Err(Outcome::Refused(error.to_string())),
    }
}

fn to_pixel_extent(value: f32) -> Result<u32, String> {
    if !value.is_finite() || value < 0.0 || value >= 4_294_967_296.0 {
        return Err(format!("viewport extent {value} is not a pixel extent"));
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "bounds 0.0..4294967296.0 verified"
    )]
    let extent = value as u32;
    Ok(extent)
}

/// Converts a window viewport size into an [`Outcome`].
#[must_use]
pub fn viewport(size: Size<Pixels>) -> Outcome {
    let width = match to_pixel_extent(size.width.as_f32()) {
        Ok(w) => w,
        Err(msg) => return Outcome::OpenFailed(msg),
    };
    let height = match to_pixel_extent(size.height.as_f32()) {
        Ok(h) => h,
        Err(msg) => return Outcome::OpenFailed(msg),
    };
    Outcome::Configured { width, height }
}

struct SurfaceView;

impl Render for SurfaceView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(theme::surface())
    }
}

fn record_outcome(slot: &RefCell<Option<Outcome>>, outcome: Outcome) {
    let mut borrow = slot.borrow_mut();
    if borrow.is_none() {
        *borrow = Some(outcome);
    }
}

fn wire_window(
    handle: &WindowHandle<SurfaceView>,
    slot: &Rc<RefCell<Option<Outcome>>>,
    cx: &mut App,
) {
    let slot_frame = Rc::clone(slot);
    let update_res = handle.update(cx, |_, window, _| {
        window.on_next_frame(move |w, cx| {
            record_outcome(&slot_frame, viewport(w.viewport_size()));
            cx.quit();
        });
    });
    if let Err(error) = update_res {
        record_outcome(slot, Outcome::OpenFailed(format!("{error:#}")));
        quit_from_loop(cx);
        return;
    }
    let slot_timer = Rc::clone(slot);
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(FIRST_CONFIGURE_DEADLINE)
            .await;
        cx.update(move |cx| {
            record_outcome(&slot_timer, Outcome::Deadline);
            cx.quit();
        });
    })
    .detach();
}

/// Quits once the event loop runs. gpui calls the launch callback before the
/// loop starts, and calloop's `EventLoop::run` clears a stop requested earlier.
fn quit_from_loop(cx: &mut App) {
    cx.spawn(async move |cx| cx.update(|cx| cx.quit())).detach();
}

fn setup_surface_app(options: WindowOptions, slot: &Rc<RefCell<Option<Outcome>>>, cx: &mut App) {
    let open_res = cx.open_window(options, |_, cx| cx.new(|_| SurfaceView));
    match open_res {
        Err(error) => {
            record_outcome(slot, classify_open_error(error.as_ref()));
            quit_from_loop(cx);
        }
        Ok(handle) => wire_window(&handle, slot, cx),
    }
}

/// Runs the surface event loop on the platform compositor and returns the outcome.
#[must_use]
pub fn run_surface(canvas: &MountCanvas) -> Outcome {
    let options = match window_options(canvas) {
        Ok(opts) => opts,
        Err(error) => return Outcome::OpenFailed(format!("{error:#}")),
    };
    let slot = Rc::new(RefCell::new(None));
    let slot_clone = Rc::clone(&slot);
    let platform = gpui_linux::current_platform(false);
    gpui::Application::with_platform(platform).run(move |cx: &mut App| {
        setup_surface_app(options, &slot_clone, cx);
    });
    match slot.borrow_mut().take() {
        Some(outcome) => outcome,
        None => Outcome::OpenFailed("the event loop ended without an outcome".to_owned()),
    }
}
