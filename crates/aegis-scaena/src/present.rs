// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The client half: checked DMA-BUF frames onto a `zwlr_layer_shell_v1`
//! surface through `zwp_linux_dmabuf_v1` (M27 criteria 5, 6 and 9).
//!
//! The protocol objects come from smithay-client-toolkit's re-exported
//! wayland-client and protocol crates. Its own `registry_queue_init` and the
//! state helpers built on it block on the socket with no timeout, so the
//! globals are read with [`Registry::roundtrip`] and every later wait runs
//! through [`dispatch_until`] under its own deadline (HISS-02): the first
//! configure, the surface's DMA-BUF feedback, each `created` or `failed`
//! event and each `presented` or `discarded` event. A missed deadline names
//! the event it waited for.
//!
//! The format and modifier pairs the compositor's feedback advertises for the
//! surface fill a [`FormatTable`] once, and every frame is admitted against it
//! client-side before any request is sent, so a pair the compositor did not
//! advertise is refused with a typed error without resting on the
//! compositor's `invalid_format` conformance. The format table is read with
//! `pread`; nothing is mapped, and the received DMA-BUF is only handed on.
//!
//! The layer surface asks for no keyboard focus (`keyboard_interactivity`
//! none) and is destroyed with every buffer when [`Presenter::close`] runs.
//! A run against the host session's compositor is the client half only
//! (D79): nothing here is evidence that P04 serves either protocol.
//!
//! What is not claimed (D83): the frame loop in this module allocates
//! nothing, but wayland-client allocates for every protocol object it
//! creates, and each frame creates a `zwp_linux_buffer_params_v1`, the
//! `wl_buffer` it announces and a `wp_presentation_feedback`.

use core::time::Duration;
use std::os::fd::{AsFd, OwnedFd};

use rustix::fs::Dev;
use smithay_client_toolkit::reexports::client::backend::ObjectData;
use smithay_client_toolkit::reexports::client::protocol::{wl_buffer, wl_compositor, wl_surface};
use smithay_client_toolkit::reexports::client::{Connection, Dispatch, EventQueue, QueueHandle};
use smithay_client_toolkit::reexports::protocols::wp::linux_dmabuf::zv1::client::{
    zwp_linux_buffer_params_v1, zwp_linux_dmabuf_feedback_v1, zwp_linux_dmabuf_v1,
};
use smithay_client_toolkit::reexports::protocols::wp::presentation_time::client::{
    wp_presentation, wp_presentation_feedback,
};
use smithay_client_toolkit::reexports::protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1, zwlr_layer_surface_v1,
};

use crate::attach::{Checked, FormatTable};
use crate::descriptor::FourCc;
use crate::device::dev_from_wire;
use crate::error::{AttachError, SurfaceError, WaitError};
use crate::surface::{ConfiguredSize, LayerSurfaceModel};
use crate::wayland::{Registry, Wait, WaitEvent, dispatch_until};

/// The most frames one presenter keeps buffers for.
pub const MAX_FRAMES: usize = 64;

/// The largest format table read, in bytes.
pub const MAX_FORMAT_TABLE_BYTES: usize = 1 << 20;

/// The namespace the layer surface is created under.
pub const NAMESPACE: &str = "aegis-scaena-m27";

/// Why a presentation step failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PresentError {
    /// No compositor could be reached.
    #[error("no Wayland compositor could be reached: {reason}")]
    Connect {
        /// Why.
        reason: String,
    },
    /// The compositor does not offer a global at the version needed.
    #[error("the compositor offers no {interface} at version {version} or above")]
    Missing {
        /// The interface.
        interface: &'static str,
        /// The lowest version admitted.
        version: u32,
    },
    /// A wait failed.
    #[error(transparent)]
    Wait(#[from] WaitError),
    /// The surface state machine refused a step.
    #[error(transparent)]
    Surface(#[from] SurfaceError),
    /// The attach checks refused the frame.
    #[error(transparent)]
    Attach(#[from] AttachError),
    /// The feedback carried no main device or no readable format table.
    #[error("the surface feedback is unusable: {what}")]
    Feedback {
        /// What was wrong.
        what: &'static str,
    },
    /// The compositor answered `failed` for the buffer.
    #[error("the compositor answered failed for the DMA-BUF import")]
    BufferFailed,
    /// The compositor discarded the frame instead of presenting it.
    #[error("the compositor discarded the committed frame")]
    Discarded,
    /// More frames than [`MAX_FRAMES`].
    #[error("the presenter holds at most {MAX_FRAMES} buffers")]
    TooManyFrames,
    /// A value does not fit the protocol's integer type.
    #[error("a size does not fit the protocol's integer type")]
    Range,
}

/// Event counters across the whole run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    /// `zwp_linux_buffer_params_v1.created` events.
    pub created: u32,
    /// `zwp_linux_buffer_params_v1.failed` events.
    pub failed: u32,
    /// `wp_presentation_feedback.presented` events.
    pub presented: u32,
    /// `wp_presentation_feedback.discarded` events.
    pub discarded: u32,
    /// `wl_buffer.release` events.
    pub released: u32,
    /// Commits that carried a buffer.
    pub committed: u32,
}

/// What the current frame's events said.
#[derive(Debug, Default)]
struct FrameEvents {
    created: Option<wl_buffer::WlBuffer>,
    failed: bool,
    presented: bool,
    discarded: bool,
}

/// What the surface's DMA-BUF feedback said.
#[derive(Debug, Default)]
struct Feedback {
    table: Vec<(u32, u64)>,
    main_device: Option<Dev>,
    tranche: Vec<u16>,
    formats: FormatTable,
    done: bool,
    unusable: Option<&'static str>,
}

/// The state every event lands in.
#[derive(Debug)]
pub struct PresenterState {
    model: LayerSurfaceModel,
    serial: Option<u32>,
    feedback: Feedback,
    frame: FrameEvents,
    counts: Counts,
}

/// Reads the format table the compositor sent, with `pread`, and splits it
/// into its 16-byte entries: a fourcc, four bytes of padding, a modifier.
/// Nothing is mapped.
///
/// # Errors
///
/// Returns what was wrong: a size that is not a whole number of entries or
/// above [`MAX_FORMAT_TABLE_BYTES`], a failed read or a short one.
pub fn read_format_table(fd: &OwnedFd, size: u32) -> Result<Vec<(u32, u64)>, &'static str> {
    let length = usize::try_from(size).map_err(|_| "the format table's size")?;
    if length > MAX_FORMAT_TABLE_BYTES || length % 16 != 0 {
        return Err("the format table's size");
    }
    let mut bytes = vec![0_u8; length];
    let read = rustix::io::pread(fd, &mut bytes[..], 0).map_err(|_| "the format table read")?;
    if read != length {
        return Err("a short format table read");
    }
    Ok(bytes
        .chunks_exact(16)
        .filter_map(|entry| {
            let format: [u8; 4] = entry.get(..4)?.try_into().ok()?;
            let modifier: [u8; 8] = entry.get(8..16)?.try_into().ok()?;
            Some((u32::from_ne_bytes(format), u64::from_ne_bytes(modifier)))
        })
        .collect())
}

impl Feedback {
    /// Records the pairs the finished tranche names.
    fn close_tranche(&mut self) {
        for index in self.tranche.drain(..) {
            let Some((code, modifier)) = self.table.get(usize::from(index)).copied() else {
                self.unusable = Some("a tranche names an index outside the format table");
                continue;
            };
            let Ok(fourcc) = FourCc::from_code(code) else {
                continue;
            };
            if self.formats.advertise(fourcc, modifier).is_err() {
                self.unusable = Some("more format pairs than the table admits");
            }
        }
    }

    fn event(&mut self, event: zwp_linux_dmabuf_feedback_v1::Event) {
        use zwp_linux_dmabuf_feedback_v1::Event;
        match event {
            Event::FormatTable { fd, size } => match read_format_table(&fd, size) {
                Ok(table) => self.table = table,
                Err(what) => self.unusable = Some(what),
            },
            Event::MainDevice { device } => self.main_device = dev_from_wire(&device),
            Event::TrancheFormats { indices } => self.tranche.extend(
                indices
                    .chunks_exact(2)
                    .filter_map(|pair| Some(u16::from_ne_bytes(pair.try_into().ok()?))),
            ),
            Event::TrancheDone => self.close_tranche(),
            Event::Done => self.done = true,
            _ => {}
        }
    }
}

impl Dispatch<zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1, ()> for PresenterState {
    fn event(
        state: &mut Self,
        _: &zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1,
        event: zwp_linux_dmabuf_feedback_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        state.feedback.event(event);
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for PresenterState {
    fn event(
        state: &mut Self,
        _: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                if state
                    .model
                    .configure(serial, ConfiguredSize { width, height })
                    .is_ok()
                {
                    state.serial = Some(serial);
                }
            }
            zwlr_layer_surface_v1::Event::Closed => state.model.close(),
            _ => {}
        }
    }
}

impl Dispatch<zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1, ()> for PresenterState {
    fn event(
        state: &mut Self,
        _: &zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1,
        event: zwp_linux_buffer_params_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwp_linux_buffer_params_v1::Event::Created { buffer } => {
                state.counts.created = state.counts.created.saturating_add(1);
                state.frame.created = Some(buffer);
            }
            zwp_linux_buffer_params_v1::Event::Failed => {
                state.counts.failed = state.counts.failed.saturating_add(1);
                state.frame.failed = true;
            }
            _ => {}
        }
    }

    /// `created` is the one event of this interface that creates an object.
    fn event_created_child(
        _opcode: u16,
        handle: &QueueHandle<Self>,
    ) -> std::sync::Arc<dyn ObjectData> {
        handle.make_data::<wl_buffer::WlBuffer, _>(())
    }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for PresenterState {
    fn event(
        state: &mut Self,
        _: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            state.counts.released = state.counts.released.saturating_add(1);
        }
    }
}

impl Dispatch<wp_presentation_feedback::WpPresentationFeedback, ()> for PresenterState {
    fn event(
        state: &mut Self,
        _: &wp_presentation_feedback::WpPresentationFeedback,
        event: wp_presentation_feedback::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wp_presentation_feedback::Event::Presented { .. } => {
                state.counts.presented = state.counts.presented.saturating_add(1);
                state.frame.presented = true;
            }
            wp_presentation_feedback::Event::Discarded => {
                state.counts.discarded = state.counts.discarded.saturating_add(1);
                state.frame.discarded = true;
            }
            _ => {}
        }
    }
}

/// Implements `Dispatch` for interfaces whose events this client ignores.
macro_rules! ignore_events {
    ($($interface:ty),* $(,)?) => {
        $(
            impl Dispatch<$interface, ()> for PresenterState {
                fn event(
                    _: &mut Self,
                    _: &$interface,
                    _: <$interface as smithay_client_toolkit::reexports::client::Proxy>::Event,
                    (): &(),
                    _: &Connection,
                    _: &QueueHandle<Self>,
                ) {
                }
            }
        )*
    };
}

ignore_events!(
    wl_compositor::WlCompositor,
    wl_surface::WlSurface,
    zwlr_layer_shell_v1::ZwlrLayerShellV1,
    zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1,
    wp_presentation::WpPresentation,
);

/// The deadlines one presenter runs under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentDeadlines {
    /// The registry roundtrip.
    pub registry: Duration,
    /// The first configure and the surface feedback.
    pub setup: Duration,
    /// Each `created` or `failed` event.
    pub created: Duration,
    /// Each `presented` or `discarded` event.
    pub presented: Duration,
}

/// The deadlines the display slice runs under.
pub const DEADLINES: PresentDeadlines = PresentDeadlines {
    registry: Duration::from_secs(2),
    setup: Duration::from_secs(2),
    created: Duration::from_secs(1),
    presented: Duration::from_secs(1),
};

/// The protocol objects a presenter holds.
#[derive(Debug)]
struct Objects {
    compositor: wl_compositor::WlCompositor,
    layer_shell: zwlr_layer_shell_v1::ZwlrLayerShellV1,
    dmabuf: zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1,
    presentation: wp_presentation::WpPresentation,
}

/// The surface a presenter shows frames on.
#[derive(Debug)]
struct Shown {
    surface: wl_surface::WlSurface,
    layer: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    feedback: zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1,
}

/// Binds `interface` from the registry at `wanted`, or no lower than `least`.
fn bind<I>(
    registry: &Registry,
    handle: &QueueHandle<PresenterState>,
    (interface, least, wanted): (&'static str, u32, u32),
) -> Result<I, PresentError>
where
    I: smithay_client_toolkit::reexports::client::Proxy + 'static,
    PresenterState: Dispatch<I, ()>,
{
    let missing = PresentError::Missing {
        interface,
        version: least,
    };
    let global = registry.find(interface).ok_or(missing.clone())?;
    if global.version < least {
        return Err(missing);
    }
    Ok(registry.proxy().bind::<I, (), PresenterState>(
        global.name,
        global.version.min(wanted),
        handle,
        (),
    ))
}

/// Binds the four globals the slice needs.
fn bind_all(
    registry: &Registry,
    handle: &QueueHandle<PresenterState>,
) -> Result<Objects, PresentError> {
    Ok(Objects {
        compositor: bind(registry, handle, ("wl_compositor", 4, 4))?,
        layer_shell: bind(registry, handle, ("zwlr_layer_shell_v1", 1, 4))?,
        dmabuf: bind(registry, handle, ("zwp_linux_dmabuf_v1", 4, 4))?,
        presentation: bind(registry, handle, ("wp_presentation", 1, 1))?,
    })
}

/// Creates the layer surface: bottom-right, overlay layer, no keyboard focus,
/// the frame's size, committed once without a buffer.
fn show(
    objects: &Objects,
    handle: &QueueHandle<PresenterState>,
    (width, height): (u32, u32),
) -> Shown {
    use zwlr_layer_surface_v1::{Anchor, KeyboardInteractivity};
    let surface = objects.compositor.create_surface(handle, ());
    let feedback = objects.dmabuf.get_surface_feedback(&surface, handle, ());
    let layer = objects.layer_shell.get_layer_surface(
        &surface,
        None,
        zwlr_layer_shell_v1::Layer::Overlay,
        NAMESPACE.to_owned(),
        handle,
        (),
    );
    layer.set_size(width, height);
    layer.set_anchor(Anchor::Bottom | Anchor::Right);
    layer.set_margin(0, 32, 32, 0);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    surface.commit();
    Shown {
        surface,
        layer,
        feedback,
    }
}

/// Who serves the connection: the peer's process id, read with
/// `SO_PEERCRED` on the Wayland socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Peer {
    /// The compositor's process id, if the kernel reported one.
    pub pid: Option<u32>,
}

/// A layer surface on the host compositor that frames are attached to.
#[derive(Debug)]
pub struct Presenter {
    connection: Connection,
    queue: EventQueue<PresenterState>,
    state: PresenterState,
    objects: Objects,
    shown: Shown,
    buffers: [Option<wl_buffer::WlBuffer>; MAX_FRAMES],
    size: (u32, u32),
    deadlines: PresentDeadlines,
}

impl Presenter {
    /// Connects to the compositor `WAYLAND_DISPLAY` names, reads its registry
    /// under a deadline, binds the globals and creates the layer surface.
    /// Nothing is attached; the first configure and the feedback are awaited
    /// by [`Presenter::wait_setup`].
    ///
    /// # Errors
    ///
    /// Returns [`PresentError::Connect`], [`PresentError::Missing`] or the
    /// registry roundtrip's [`WaitError`].
    pub fn connect(size: (u32, u32), deadlines: PresentDeadlines) -> Result<Self, PresentError> {
        let connection = Connection::connect_to_env().map_err(|error| PresentError::Connect {
            reason: error.to_string(),
        })?;
        let registry = Registry::roundtrip(&connection, deadlines.registry)?;
        let queue = connection.new_event_queue::<PresenterState>();
        let handle = queue.handle();
        let objects = bind_all(&registry, &handle)?;
        let shown = show(&objects, &handle, size);
        Ok(Self {
            connection,
            queue,
            state: PresenterState {
                model: LayerSurfaceModel::new(),
                serial: None,
                feedback: Feedback {
                    formats: FormatTable::new(),
                    ..Feedback::default()
                },
                frame: FrameEvents::default(),
                counts: Counts::default(),
            },
            objects,
            shown,
            buffers: core::array::from_fn(|_| None),
            size,
            deadlines,
        })
    }

    /// The compositor's process, read from the connection itself.
    #[must_use]
    pub fn peer(&self) -> Peer {
        let backend = self.connection.backend();
        let pid = rustix::net::sockopt::socket_peercred(backend.poll_fd())
            .ok()
            .and_then(|credentials| u32::try_from(credentials.pid.as_raw_nonzero().get()).ok());
        Peer { pid }
    }

    /// Waits for the layer surface's first configure and the surface's
    /// DMA-BUF feedback, each under its own deadline.
    ///
    /// # Errors
    ///
    /// Returns [`WaitError::Deadline`] naming [`WaitEvent::FirstConfigure`]
    /// or [`WaitEvent::SurfaceFeedback`], and [`PresentError::Feedback`]
    /// when the feedback carries no main device or no usable table.
    pub fn wait_setup(&mut self) -> Result<(), PresentError> {
        let configure = Wait {
            event: WaitEvent::FirstConfigure,
            deadline: self.deadlines.setup,
        };
        dispatch_until(&mut self.queue, &mut self.state, configure, |state| {
            state.serial.is_some()
        })?;
        let feedback = Wait {
            event: WaitEvent::SurfaceFeedback,
            deadline: self.deadlines.setup,
        };
        dispatch_until(&mut self.queue, &mut self.state, feedback, |state| {
            state.feedback.done
        })?;
        if let Some(what) = self.state.feedback.unusable {
            return Err(PresentError::Feedback { what });
        }
        if self.state.feedback.main_device.is_none() {
            return Err(PresentError::Feedback {
                what: "no main device",
            });
        }
        Ok(())
    }

    /// The compositor's main device, from the surface feedback.
    #[must_use]
    pub fn main_device(&self) -> Option<Dev> {
        self.state.feedback.main_device
    }

    /// The pairs the feedback advertised for the surface.
    #[must_use]
    pub fn formats(&self) -> &FormatTable {
        &self.state.feedback.formats
    }

    /// Asks the surface state machine for an attach, as the first frame
    /// would, without sending anything: before [`Presenter::ack`] this is
    /// refused (M27 boundary).
    ///
    /// # Errors
    ///
    /// Returns the model's [`SurfaceError`].
    pub fn try_attach(&self) -> Result<ConfiguredSize, SurfaceError> {
        self.state.model.attach()
    }

    /// Acknowledges the last configure.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceError::NotConfigured`] before any configure.
    pub fn ack(&mut self) -> Result<(), PresentError> {
        let serial = self.state.serial.ok_or(SurfaceError::NotConfigured)?;
        self.state.model.ack(serial)?;
        self.shown.layer.ack_configure(serial);
        Ok(())
    }

    /// The run's event counters.
    #[must_use]
    pub const fn counts(&self) -> Counts {
        self.state.counts
    }

    /// Admits `checked` client-side and, only if every check passes, imports
    /// it with `zwp_linux_buffer_params_v1.create`, waits for `created`,
    /// attaches and commits it with a presentation feedback, and waits for
    /// `presented`.
    ///
    /// # Errors
    ///
    /// Returns [`PresentError::Surface`] before the first acknowledged
    /// configure, [`PresentError::Attach`] for a pair the compositor did not
    /// advertise (both before any request is sent), and
    /// [`PresentError::BufferFailed`], [`PresentError::Discarded`] or a
    /// [`WaitError`] naming the event that missed its deadline.
    pub fn present(&mut self, checked: Checked) -> Result<(), PresentError> {
        let slot = usize::try_from(self.state.counts.committed).map_err(|_| PresentError::Range)?;
        if slot >= MAX_FRAMES {
            return Err(PresentError::TooManyFrames);
        }
        self.state.model.attach()?;
        let frame = *checked.frame();
        self.state
            .feedback
            .formats
            .admit(frame.fourcc, frame.modifier)?;
        let buffer = self.import(checked)?;
        self.commit(&buffer)?;
        if let Some(kept) = self.buffers.get_mut(slot) {
            *kept = Some(buffer);
        }
        Ok(())
    }

    /// Sends the import for `checked` and waits for `created` or `failed`.
    fn import(&mut self, checked: Checked) -> Result<wl_buffer::WlBuffer, PresentError> {
        let frame = *checked.frame();
        let handle = self.queue.handle();
        let params = self.objects.dmabuf.create_params(&handle, ());
        let fd = checked.into_fd();
        let [h0, h1, h2, h3, l0, l1, l2, l3] = frame.modifier.to_be_bytes();
        let (high, low) = (
            u32::from_be_bytes([h0, h1, h2, h3]),
            u32::from_be_bytes([l0, l1, l2, l3]),
        );
        for (index, plane) in (0_u32..).zip(frame.planes.as_slice()) {
            params.add(fd.as_fd(), index, plane.offset, plane.pitch, high, low);
        }
        let width = i32::try_from(frame.width).map_err(|_| PresentError::Range)?;
        let height = i32::try_from(frame.height).map_err(|_| PresentError::Range)?;
        self.state.frame = FrameEvents::default();
        params.create(
            width,
            height,
            frame.fourcc.code(),
            zwp_linux_buffer_params_v1::Flags::empty(),
        );
        let wait = Wait {
            event: WaitEvent::BufferCreated,
            deadline: self.deadlines.created,
        };
        let waited = dispatch_until(&mut self.queue, &mut self.state, wait, |state| {
            state.frame.created.is_some() || state.frame.failed
        });
        params.destroy();
        drop(fd);
        waited?;
        self.state
            .frame
            .created
            .take()
            .ok_or(PresentError::BufferFailed)
    }

    /// Attaches `buffer`, commits it with a presentation feedback and waits
    /// for `presented` or `discarded`.
    fn commit(&mut self, buffer: &wl_buffer::WlBuffer) -> Result<(), PresentError> {
        let handle = self.queue.handle();
        let (width, height) = self.size;
        let width = i32::try_from(width).map_err(|_| PresentError::Range)?;
        let height = i32::try_from(height).map_err(|_| PresentError::Range)?;
        self.shown.surface.attach(Some(buffer), 0, 0);
        self.shown.surface.damage_buffer(0, 0, width, height);
        let _feedback = self
            .objects
            .presentation
            .feedback(&self.shown.surface, &handle, ());
        self.shown.surface.commit();
        self.state.counts.committed = self.state.counts.committed.saturating_add(1);
        let wait = Wait {
            event: WaitEvent::Presented,
            deadline: self.deadlines.presented,
        };
        dispatch_until(&mut self.queue, &mut self.state, wait, |state| {
            state.frame.presented || state.frame.discarded
        })?;
        if self.state.frame.presented {
            Ok(())
        } else {
            Err(PresentError::Discarded)
        }
    }

    /// Asks for a presentation feedback and never commits, so no
    /// `presented` can arrive: the wait must fail at `deadline` and name
    /// [`WaitEvent::Presented`] (M27 boundary).
    ///
    /// # Errors
    ///
    /// Returns the wait's error, which on a conforming compositor is
    /// [`WaitError::Deadline`] naming [`WaitEvent::Presented`].
    pub fn await_uncommitted_presentation(&mut self, deadline: Duration) -> Result<(), WaitError> {
        let handle = self.queue.handle();
        let _feedback = self
            .objects
            .presentation
            .feedback(&self.shown.surface, &handle, ());
        self.state.frame = FrameEvents::default();
        let wait = Wait {
            event: WaitEvent::Presented,
            deadline,
        };
        dispatch_until(&mut self.queue, &mut self.state, wait, |state| {
            state.frame.presented || state.frame.discarded
        })
    }

    /// Destroys the layer surface, the surface and every buffer, and flushes.
    pub fn close(self) {
        self.shown.layer.destroy();
        self.shown.feedback.destroy();
        self.shown.surface.destroy();
        for buffer in self.buffers.into_iter().flatten() {
            buffer.destroy();
        }
        let _ = self.connection.flush();
    }
}
