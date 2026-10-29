// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `KWin` Wayland fake input key injection client (M30 E30-1).
//!
//! Connects to `KWin`'s compositor socket, checks for the `org_kde_kwin_fake_input`
//! global advertisement, binds it, and injects evdev keystrokes under a deadline (HISS-02).

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use wayland_client::protocol::wl_registry;
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols_plasma::fake_input::client::org_kde_kwin_fake_input::OrgKdeKwinFakeInput;

/// Evdev keycode for Left Control.
pub const KEY_LEFTCTRL: u32 = 29;

/// Evdev keycode for Equal / Plus key (`=`).
pub const KEY_EQUAL: u32 = 13;

/// Evdev keycode for Minus / Underscore key (`-`).
pub const KEY_MINUS: u32 = 12;

/// Key released state for Wayland keyboard events.
pub const KEY_STATE_RELEASED: u32 = 0;

/// Key pressed state for Wayland keyboard events.
pub const KEY_STATE_PRESSED: u32 = 1;

/// Default deadline for Wayland fake input key injection (HISS-02).
pub const INJECT_DEADLINE: Duration = Duration::from_secs(3);

const STEP_REGISTRY: u8 = 1;
const STEP_INJECT: u8 = 2;

fn step_name(code: u8) -> &'static str {
    match code {
        STEP_INJECT => "inject",
        _ => "registry",
    }
}

/// Slider action key to inject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderAction {
    /// Increment slider value (Ctrl+=).
    Increment,
    /// Decrement slider value (Ctrl+-).
    Decrement,
}

/// Errors occurring during Wayland fake input key injection.
#[derive(Debug, thiserror::Error)]
pub enum InjectError {
    /// Wayland connection failure.
    #[error("Wayland connection failed: {0}")]
    Connect(String),
    /// Compositor did not advertise `org_kde_kwin_fake_input`.
    #[error("compositor does not advertise the org_kde_kwin_fake_input global to this client")]
    GlobalNotAdvertised,
    /// Wayland dispatch error.
    #[error("Wayland dispatch error: {0}")]
    Dispatch(String),
    /// Wayland operation exceeded deadline at given step.
    #[error("Wayland key injection timed out at step: {step}")]
    Deadline {
        /// Step in progress when deadline expired (`"registry"` or `"inject"`).
        step: &'static str,
    },
}

#[derive(Default)]
struct FakeInputState {
    fake_input: Option<OrgKdeKwinFakeInput>,
    global_seen: bool,
}

impl Dispatch<wl_registry::WlRegistry, ()> for FakeInputState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
            && interface == "org_kde_kwin_fake_input"
        {
            state.global_seen = true;
            let bind_version = version.min(5);
            let fake_input = registry.bind::<OrgKdeKwinFakeInput, _, _>(name, bind_version, qh, ());
            state.fake_input = Some(fake_input);
        }
    }
}

impl Dispatch<OrgKdeKwinFakeInput, ()> for FakeInputState {
    fn event(
        _: &mut Self,
        _: &OrgKdeKwinFakeInput,
        _: <OrgKdeKwinFakeInput as wayland_client::Proxy>::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

fn emit_keystrokes(fake_input: &OrgKdeKwinFakeInput, target_key: u32) {
    fake_input.keyboard_key(KEY_LEFTCTRL, KEY_STATE_PRESSED);
    fake_input.keyboard_key(target_key, KEY_STATE_PRESSED);
    fake_input.keyboard_key(target_key, KEY_STATE_RELEASED);
    fake_input.keyboard_key(KEY_LEFTCTRL, KEY_STATE_RELEASED);
}

fn inject_worker(
    conn: &Connection,
    action: SliderAction,
    step_tracker: &AtomicU8,
) -> Result<(), InjectError> {
    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();
    let display = conn.display();
    let _registry = display.get_registry(&qh, ());

    let mut state = FakeInputState::default();
    step_tracker.store(STEP_REGISTRY, Ordering::SeqCst);
    event_queue
        .roundtrip(&mut state)
        .map_err(|e| InjectError::Dispatch(e.to_string()))?;

    if !state.global_seen {
        return Err(InjectError::GlobalNotAdvertised);
    }
    let fake_input = state
        .fake_input
        .clone()
        .ok_or(InjectError::GlobalNotAdvertised)?;

    fake_input.authenticate(
        "aegis-readback".to_owned(),
        "slider verification".to_owned(),
    );

    let target_key = match action {
        SliderAction::Increment => KEY_EQUAL,
        SliderAction::Decrement => KEY_MINUS,
    };
    emit_keystrokes(&fake_input, target_key);

    step_tracker.store(STEP_INJECT, Ordering::SeqCst);
    event_queue
        .roundtrip(&mut state)
        .map_err(|e| InjectError::Dispatch(e.to_string()))?;

    Ok(())
}

/// Injects slider action keystrokes on the provided Wayland connection under a scalar deadline.
///
/// Runs the blocking Wayland event loop on a dedicated worker thread bounded by `deadline`
/// via an mpsc channel (HISS-02). If the compositor fails to answer before the deadline expires,
/// this returns [`InjectError::Deadline`] immediately; the worker thread is left detached
/// to terminate when the underlying socket or process terminates.
///
/// # Errors
///
/// Returns [`InjectError::Deadline`] if the deadline expires at any step,
/// [`InjectError::GlobalNotAdvertised`] if `KWin` does not advertise `org_kde_kwin_fake_input`,
/// or [`InjectError::Dispatch`] on protocol/thread failures.
pub fn inject_slider_action_on_connection(
    conn: Connection,
    action: SliderAction,
    deadline: Duration,
) -> Result<(), InjectError> {
    let step_tracker = Arc::new(AtomicU8::new(STEP_REGISTRY));
    let step_clone = Arc::clone(&step_tracker);
    let (tx, rx) = mpsc::channel();

    let thread_handle = std::thread::Builder::new()
        .name("wayland-fake-input".to_owned())
        .spawn(move || {
            let res = inject_worker(&conn, action, &step_clone);
            let _ = tx.send(res);
        })
        .map_err(|e| InjectError::Dispatch(format!("failed to spawn worker thread: {e}")))?;

    match rx.recv_timeout(deadline) {
        Ok(res) => {
            let _ = thread_handle.join();
            res
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            let step = step_name(step_tracker.load(Ordering::SeqCst));
            Err(InjectError::Deadline { step })
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(InjectError::Dispatch(
            "injection worker exited prematurely".to_owned(),
        )),
    }
}

/// Connects to Wayland compositor from environment and injects slider keystrokes under `deadline`.
///
/// # Errors
///
/// Returns [`InjectError::Connect`] if connection fails, or errors from [`inject_slider_action_on_connection`].
pub fn inject_slider_action_timeout(
    action: SliderAction,
    deadline: Duration,
) -> Result<(), InjectError> {
    let conn = Connection::connect_to_env().map_err(|e| InjectError::Connect(e.to_string()))?;
    inject_slider_action_on_connection(conn, action, deadline)
}

/// Connects to Wayland compositor and injects the specified slider key combination under [`INJECT_DEADLINE`].
///
/// # Errors
///
/// Returns [`InjectError::Connect`] if connection fails, [`InjectError::GlobalNotAdvertised`]
/// if `KWin` does not offer `org_kde_kwin_fake_input`, [`InjectError::Deadline`] on timeout,
/// or [`InjectError::Dispatch`] on protocol errors.
pub fn inject_slider_action(action: SliderAction) -> Result<(), InjectError> {
    inject_slider_action_timeout(action, INJECT_DEADLINE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream;

    #[test]
    fn deadline_triggered_on_silent_compositor() -> Result<(), Box<dyn std::error::Error>> {
        let (server, client) = UnixStream::pair()?;
        let conn = Connection::from_socket(client)?;
        let res = inject_slider_action_on_connection(
            conn,
            SliderAction::Increment,
            Duration::from_millis(50),
        );
        drop(server);
        match res {
            Err(InjectError::Deadline { step: "registry" }) => Ok(()),
            other => {
                Err(format!("expected InjectError::Deadline at registry, got {other:?}").into())
            }
        }
    }
}
