// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Measurement fixture surface on gpui driving profiler tracing and AccessKit updates (M28 E28-6).
//!
//! Hosts a layer surface with a seeded node list, enables gpui's profiler frame event tracing,
//! intercepts AccessKit tree update log records, and emits per-frame timing lines.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::{self, Write as _};
use std::io::Write;
use std::rc::Rc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use aegis_scaena::mount::MountCanvas;
use gpui::{
    App, AppContext, Context, FocusHandle, InteractiveElement, IntoElement, ParentElement, Render,
    Role, SharedString, StatefulInteractiveElement, Styled, Window, WindowBounds, WindowOptions,
    div, px,
};

use crate::surface::{Outcome, classify_open_error, viewport, window_options};
use crate::theme;
use crate::timing::{RECORDED_FRAMES, WARMUP_FRAMES};

/// Height of each list item in pixels.
pub const ROW_HEIGHT: f32 = 24.0;

/// Height of the timing fixture window in pixels.
pub const FIXTURE_HEIGHT: f32 = 400.0;

/// Maximum runtime before the fixture aborts with a deadline outcome.
pub const FIXTURE_LIFETIME: Duration = Duration::from_secs(60);

/// Window title assigned to the layer surface.
pub const FIXTURE_TITLE: &str = "Aegis forum shell timing fixture";

static A11Y_UPDATES: Mutex<VecDeque<(Instant, usize)>> = Mutex::new(VecDeque::new());

fn record_update(args_str: &str) {
    let Some(rest) = args_str.strip_prefix("Sending a11y tree update: ") else {
        return;
    };
    let Some(num_str) = rest.strip_suffix(" nodes") else {
        return;
    };
    let Ok(count) = num_str.trim().parse::<usize>() else {
        return;
    };
    let now = Instant::now();
    let Ok(mut lock) = A11Y_UPDATES.lock() else {
        return;
    };
    if lock.len() >= 4096 {
        lock.pop_front();
    }
    lock.push_back((now, count));
}

/// Capacity of the stack buffer one log message is formatted into; the update line is far shorter.
const LINE_CAPACITY: usize = 96;

/// A fixed-size `fmt::Write` sink; a message longer than its capacity is refused, not truncated.
struct LineBuf {
    bytes: [u8; LINE_CAPACITY],
    len: usize,
}

impl Default for LineBuf {
    fn default() -> Self {
        Self {
            bytes: [0; LINE_CAPACITY],
            len: 0,
        }
    }
}

impl LineBuf {
    fn filled(&self) -> &[u8] {
        self.bytes.get(..self.len).unwrap_or_default()
    }
}

impl fmt::Write for LineBuf {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.len.checked_add(text.len()).ok_or(fmt::Error)?;
        let slot = self.bytes.get_mut(self.len..end).ok_or(fmt::Error)?;
        slot.copy_from_slice(text.as_bytes());
        self.len = end;
        Ok(())
    }
}

struct A11yLogger;

impl log::Log for A11yLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.target().starts_with("gpui::window") && metadata.level() <= log::Level::Debug
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        // Formats into a fixed stack buffer: no heap allocation in the frame path.
        let mut line = LineBuf::default();
        if fmt::write(&mut line, *record.args()).is_ok()
            && let Ok(text) = std::str::from_utf8(line.filled())
        {
            record_update(text);
        }
    }

    fn flush(&self) {}
}

static LOGGER: A11yLogger = A11yLogger;

fn init_logger() {
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(log::LevelFilter::Debug);
}

fn get_update_nodes(start: Instant, end: Instant) -> Option<usize> {
    let mut lock = A11Y_UPDATES.lock().ok()?;
    let mut max_nodes = None;
    for &(instant, count) in lock.iter() {
        if instant >= start && instant <= end {
            max_nodes = Some(max_nodes.map_or(count, |m: usize| m.max(count)));
        }
    }
    lock.retain(|&(instant, _)| instant >= start);
    max_nodes
}

struct TimingView {
    labels: Vec<SharedString>,
    collector: Option<gpui::profiler::FrameTimingCollector>,
    frames_drawn: usize,
    out: Rc<RefCell<Box<dyn Write>>>,
    buffer: String,
    first_frame: bool,
    focus_handle: FocusHandle,
    slot: Rc<RefCell<Option<Outcome>>>,
}

/// Builds the row labels once, so a frame clones reference-counted strings instead of formatting.
fn seeded_labels(seeded: usize) -> Vec<SharedString> {
    (0..seeded)
        .map(|i| SharedString::from(format!("Node {i}")))
        .collect()
}

fn build_seeded_list(labels: &[SharedString]) -> gpui::Stateful<gpui::Div> {
    let mut list = div()
        .id("list")
        .role(Role::List)
        .aria_label("Seeded nodes")
        .overflow_y_scroll()
        .h(px(FIXTURE_HEIGHT));

    for (i, label) in labels.iter().enumerate() {
        let row = div()
            .id(("row", i))
            .role(Role::ListItem)
            .h(px(ROW_HEIGHT))
            .aria_label(label.clone());
        list = list.child(row);
    }
    list
}

fn print_first_frame(window: &Window, out: &Rc<RefCell<Box<dyn Write>>>) -> Result<(), Outcome> {
    let mut out_guard = out.borrow_mut();
    if let Some(specs) = window.gpu_specs() {
        let sw = u8::from(specs.is_software_emulated);
        let _ = writeln!(
            *out_guard,
            "GPU {}\t{}\t{}\t{}",
            specs.device_name, specs.driver_name, specs.driver_info, sw
        );
    } else {
        let _ = writeln!(*out_guard, "GPU -");
    }
    let _ = writeln!(*out_guard, "READY");
    if out_guard.flush().is_err() {
        return Err(Outcome::OpenFailed("write error on READY".to_owned()));
    }
    Ok(())
}

fn record_outcome(slot: &RefCell<Option<Outcome>>, outcome: Outcome) {
    let mut borrow = slot.borrow_mut();
    if borrow.is_none() {
        *borrow = Some(outcome);
    }
}

fn emit_frame(
    timing: gpui::profiler::FrameTiming,
    out_guard: &mut Box<dyn Write>,
    buffer: &mut String,
) -> Result<(), Outcome> {
    let draw_ns = u64::try_from(timing.draw_duration().as_nanos()).unwrap_or(u64::MAX);
    let phases_ns = u64::try_from(
        timing
            .phases
            .request_layout
            .as_nanos()
            .saturating_add(timing.phases.prepaint.as_nanos())
            .saturating_add(timing.phases.paint.as_nanos()),
    )
    .unwrap_or(u64::MAX);

    let max_nodes = get_update_nodes(timing.draw_start, timing.draw_end);

    buffer.clear();
    let _ = write!(buffer, "FRAME {draw_ns} {phases_ns} ");
    match max_nodes {
        Some(nodes) => {
            let _ = writeln!(buffer, "{nodes}");
        }
        None => {
            let _ = writeln!(buffer, "-");
        }
    }
    if out_guard
        .write_all(buffer.as_bytes())
        .and_then(|()| out_guard.flush())
        .is_err()
    {
        return Err(Outcome::OpenFailed("write error".to_owned()));
    }
    Ok(())
}

fn process_events(
    events: Vec<gpui::profiler::FrameEvent>,
    out: &Rc<RefCell<Box<dyn Write>>>,
    buffer: &mut String,
    frames_drawn: &mut usize,
    slot: &Rc<RefCell<Option<Outcome>>>,
    window: &mut Window,
) -> Result<(), Outcome> {
    let mut out_guard = out.borrow_mut();
    let target_frames = WARMUP_FRAMES
        .saturating_add(RECORDED_FRAMES)
        .saturating_add(30);
    for event in events {
        if let gpui::profiler::FrameEvent::Draw(timing) = event {
            emit_frame(timing, &mut out_guard, buffer)?;
            *frames_drawn = frames_drawn.saturating_add(1);
            if *frames_drawn >= target_frames {
                let _ = writeln!(*out_guard, "DONE");
                let _ = out_guard.flush();
                record_outcome(slot, viewport(window.viewport_size()));
                window.on_next_frame(|_, cx| cx.quit());
                break;
            }
        }
    }
    Ok(())
}

impl Render for TimingView {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        window.request_animation_frame();

        if self.first_frame {
            self.first_frame = false;
            gpui::profiler::set_trace_enabled(true);
            self.collector = Some(gpui::profiler::FrameTimingCollector::new());
            if let Err(outcome) = print_first_frame(window, &self.out) {
                record_outcome(&self.slot, outcome);
                window.on_next_frame(|_, cx| cx.quit());
                return div();
            }
        } else if let Some(ref mut collector) = self.collector {
            let events = collector.collect_unseen();
            if let Err(outcome) = process_events(
                events,
                &self.out,
                &mut self.buffer,
                &mut self.frames_drawn,
                &self.slot,
                window,
            ) {
                record_outcome(&self.slot, outcome);
                window.on_next_frame(|_, cx| cx.quit());
                return div();
            }
        }

        let button = div()
            .id("button")
            .role(Role::Button)
            .aria_label("Open the forum")
            .track_focus(&self.focus_handle)
            .w(px(200.0))
            .h(px(40.0));

        let list = build_seeded_list(&self.labels);

        let shown_panel = div()
            .id("shown-panel")
            .role(Role::Group)
            .aria_label("Shown panel")
            .w(px(100.0))
            .h(px(20.0));

        div()
            .size_full()
            .bg(theme::surface())
            .child(button)
            .child(list)
            .child(shown_panel)
    }
}

fn window_options_for_timing(canvas: &MountCanvas) -> Result<WindowOptions, Outcome> {
    let mut options =
        window_options(canvas).map_err(|error| Outcome::OpenFailed(format!("{error:#}")))?;
    if let Some(WindowBounds::Windowed(ref mut bounds)) = options.window_bounds {
        bounds.size.height = px(FIXTURE_HEIGHT);
    }
    Ok(options)
}

fn setup_deadline_timer(slot: &Rc<RefCell<Option<Outcome>>>, cx: &mut App) {
    let slot_timer = Rc::clone(slot);
    cx.spawn(async move |cx| {
        cx.background_executor().timer(FIXTURE_LIFETIME).await;
        cx.update(move |cx| {
            record_outcome(&slot_timer, Outcome::Deadline);
            cx.quit();
        });
    })
    .detach();
}

/// Runs the timing fixture event loop on the platform compositor until completion or lifetime.
pub fn run_timing_fixture(
    canvas: &MountCanvas,
    seeded: usize,
    out: impl Write + 'static,
) -> Outcome {
    init_logger();

    let options = match window_options_for_timing(canvas) {
        Ok(opts) => opts,
        Err(outcome) => return outcome,
    };

    let slot = Rc::new(RefCell::new(None));
    let slot_clone = Rc::clone(&slot);
    let out_cell = Rc::new(RefCell::new(Box::new(out) as Box<dyn Write>));
    let out_clone = Rc::clone(&out_cell);
    let platform = gpui_linux::current_platform(false);

    gpui::Application::with_platform(platform).run(move |cx: &mut App| {
        let slot_view = Rc::clone(&slot_clone);
        let out_view = Rc::clone(&out_clone);
        let open_res = cx.open_window(options, |window, cx| {
            window.set_window_title(FIXTURE_TITLE);
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            cx.new(|_| TimingView {
                labels: seeded_labels(seeded),
                collector: None,
                frames_drawn: 0,
                out: out_view,
                buffer: String::with_capacity(128),
                first_frame: true,
                focus_handle,
                slot: slot_view,
            })
        });

        match open_res {
            Err(error) => {
                record_outcome(&slot_clone, classify_open_error(error.as_ref()));
            }
            Ok(_handle) => {
                setup_deadline_timer(&slot_clone, cx);
            }
        }
    });

    slot.borrow_mut()
        .take()
        .unwrap_or_else(|| Outcome::OpenFailed("timing loop ended without outcome".to_owned()))
}
