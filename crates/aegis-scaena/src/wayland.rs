// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Waiting on the Wayland connection, every wait under its own deadline
//! (HISS-02).
//!
//! wayland-client's `roundtrip` and `blocking_dispatch`, and
//! smithay-client-toolkit's `registry_queue_init` built on them, block on the
//! socket with no timeout, so a compositor that accepts the connection and
//! never answers would hang the client. [`dispatch_until`] replaces them: it
//! dispatches what is queued, flushes, and then polls the connection fd for
//! the time that is left, in a loop bounded by [`MAX_WAIT_ROUNDS`], until a
//! predicate over the caller's state holds. A missed deadline fails with
//! [`WaitError::Deadline`] naming the [`WaitEvent`] it waited for.
//!
//! [`Registry::roundtrip`] is the first wait of every session: it asks for the
//! registry, sends `wl_display.sync`, and waits for the callback's `done`, by
//! which point the compositor has announced every global.

use core::fmt;
use core::time::Duration;
use std::io::ErrorKind;
use std::os::fd::BorrowedFd;
use std::time::Instant;

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use smithay_client_toolkit::reexports::client::backend::WaylandError;
use smithay_client_toolkit::reexports::client::protocol::{wl_callback, wl_registry};
use smithay_client_toolkit::reexports::client::{
    Connection, Dispatch, DispatchError, EventQueue, QueueHandle,
};

use crate::error::WaitError;

/// The most dispatch-and-poll rounds one wait makes before it gives up.
pub const MAX_WAIT_ROUNDS: u32 = 4_096;

/// The most globals a registry records.
pub const MAX_GLOBALS: usize = 256;

/// What a wait was waiting for; a missed deadline names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WaitEvent {
    /// `wl_callback.done` for the `wl_display.sync` after `get_registry`.
    RegistryRoundtrip,
    /// The layer surface's first `configure`.
    FirstConfigure,
    /// The surface's `zwp_linux_dmabuf_feedback_v1.done`.
    SurfaceFeedback,
    /// `zwp_linux_buffer_params_v1.created` or `failed`.
    BufferCreated,
    /// `wp_presentation_feedback.presented` or `discarded`.
    Presented,
}

impl fmt::Display for WaitEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::RegistryRoundtrip => "the registry roundtrip (wl_callback.done)",
            Self::FirstConfigure => "the layer surface's first configure",
            Self::SurfaceFeedback => "the surface's zwp_linux_dmabuf_feedback_v1 done",
            Self::BufferCreated => "zwp_linux_buffer_params_v1 created or failed",
            Self::Presented => "wp_presentation_feedback presented or discarded",
        })
    }
}

/// One wait: what it waits for and for how long.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wait {
    /// What is awaited.
    pub event: WaitEvent,
    /// How long it may take, from the start of the wait.
    pub deadline: Duration,
}

/// Dispatches what is queued, mapping a failure onto the event awaited.
fn dispatch<S>(
    queue: &mut EventQueue<S>,
    state: &mut S,
    event: WaitEvent,
) -> Result<(), WaitError> {
    match queue.dispatch_pending(state) {
        Ok(_) => Ok(()),
        Err(DispatchError::Backend(WaylandError::Protocol(_))) => {
            Err(WaitError::Protocol { event })
        }
        Err(_) => Err(WaitError::Connection { event }),
    }
}

/// Flushes queued requests; a full socket buffer is retried next round.
fn flush<S>(queue: &EventQueue<S>, event: WaitEvent) -> Result<(), WaitError> {
    match queue.flush() {
        Ok(()) => Ok(()),
        Err(WaylandError::Io(error)) if error.kind() == ErrorKind::WouldBlock => Ok(()),
        Err(WaylandError::Protocol(_)) => Err(WaitError::Protocol { event }),
        Err(WaylandError::Io(_)) => Err(WaitError::Connection { event }),
    }
}

/// Polls `fd` for input for at most `remaining`; `false` when nothing came.
fn readable(fd: BorrowedFd<'_>, remaining: Duration, event: WaitEvent) -> Result<bool, WaitError> {
    let timeout = Timespec::try_from(remaining).map_err(|_| WaitError::Deadline { event })?;
    let mut fds = [PollFd::from_borrowed_fd(fd, PollFlags::IN)];
    match poll(&mut fds, Some(&timeout)) {
        Ok(ready) => Ok(ready > 0),
        Err(rustix::io::Errno::INTR) => Ok(true),
        Err(_) => Err(WaitError::Connection { event }),
    }
}

/// Reads what the compositor sent, after [`readable`] said something came.
fn read<S>(queue: &EventQueue<S>, remaining: Duration, event: WaitEvent) -> Result<(), WaitError> {
    let Some(guard) = queue.prepare_read() else {
        return Ok(());
    };
    if !readable(guard.connection_fd(), remaining, event)? {
        return Err(WaitError::Deadline { event });
    }
    match guard.read() {
        Ok(_) => Ok(()),
        Err(WaylandError::Io(error)) if error.kind() == ErrorKind::WouldBlock => Ok(()),
        Err(WaylandError::Protocol(_)) => Err(WaitError::Protocol { event }),
        Err(WaylandError::Io(_)) => Err(WaitError::Connection { event }),
    }
}

/// Dispatches events until `done` holds for the state, under `wait`'s
/// deadline.
///
/// # Errors
///
/// Returns [`WaitError::Deadline`] naming the event when the deadline passes
/// or the round bound is spent, [`WaitError::Protocol`] for a protocol error
/// and [`WaitError::Connection`] when the connection fails or closes.
pub fn dispatch_until<S>(
    queue: &mut EventQueue<S>,
    state: &mut S,
    wait: Wait,
    done: impl Fn(&S) -> bool,
) -> Result<(), WaitError> {
    let started = Instant::now();
    let missed = WaitError::Deadline { event: wait.event };
    for _ in 0..MAX_WAIT_ROUNDS {
        dispatch(queue, state, wait.event)?;
        if done(state) {
            return Ok(());
        }
        flush(queue, wait.event)?;
        let remaining = wait
            .deadline
            .checked_sub(started.elapsed())
            .filter(|left| !left.is_zero())
            .ok_or(missed)?;
        read(queue, remaining, wait.event)?;
    }
    Err(missed)
}

/// One global the compositor announced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Global {
    /// The registry name to bind it by.
    pub name: u32,
    /// The interface.
    pub interface: String,
    /// The highest version offered.
    pub version: u32,
}

/// What the registry roundtrip collects.
#[derive(Debug, Default)]
pub struct RegistryState {
    globals: Vec<Global>,
    synced: bool,
}

impl Dispatch<wl_registry::WlRegistry, ()> for RegistryState {
    fn event(
        state: &mut Self,
        _: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if state.globals.len() < MAX_GLOBALS => state.globals.push(Global {
                name,
                interface,
                version,
            }),
            wl_registry::Event::GlobalRemove { name } => {
                state.globals.retain(|global| global.name != name);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_callback::WlCallback, ()> for RegistryState {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        event: wl_callback::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { .. } = event {
            state.synced = true;
        }
    }
}

/// The compositor's registry, read once under a deadline.
#[derive(Debug)]
pub struct Registry {
    proxy: wl_registry::WlRegistry,
    queue: EventQueue<RegistryState>,
    state: RegistryState,
}

impl Registry {
    /// Asks `connection` for its registry and waits, under `deadline`, for
    /// the `wl_display.sync` sent after it to complete.
    ///
    /// # Errors
    ///
    /// Returns [`WaitError::Deadline`] naming
    /// [`WaitEvent::RegistryRoundtrip`] when a compositor accepts the
    /// connection and never answers, and the other [`WaitError`]s from
    /// [`dispatch_until`].
    pub fn roundtrip(connection: &Connection, deadline: Duration) -> Result<Self, WaitError> {
        let mut queue = connection.new_event_queue();
        let handle = queue.handle();
        let display = connection.display();
        let proxy = display.get_registry(&handle, ());
        let _sync = display.sync(&handle, ());
        let mut state = RegistryState::default();
        let wait = Wait {
            event: WaitEvent::RegistryRoundtrip,
            deadline,
        };
        dispatch_until(&mut queue, &mut state, wait, |state| state.synced)?;
        Ok(Self {
            proxy,
            queue,
            state,
        })
    }

    /// Every global announced before the roundtrip completed.
    #[must_use]
    pub fn globals(&self) -> &[Global] {
        &self.state.globals
    }

    /// The global announcing `interface`, if one did.
    #[must_use]
    pub fn find(&self, interface: &str) -> Option<&Global> {
        self.state
            .globals
            .iter()
            .find(|global| global.interface == interface)
    }

    /// The registry proxy, to bind globals with.
    #[must_use]
    pub const fn proxy(&self) -> &wl_registry::WlRegistry {
        &self.proxy
    }

    /// Dispatches registry events that arrived since, without waiting.
    ///
    /// # Errors
    ///
    /// As [`dispatch_until`]'s dispatch step.
    pub fn refresh(&mut self) -> Result<(), WaitError> {
        dispatch(
            &mut self.queue,
            &mut self.state,
            WaitEvent::RegistryRoundtrip,
        )
    }
}
