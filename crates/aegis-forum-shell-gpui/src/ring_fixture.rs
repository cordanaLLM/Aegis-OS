// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fixture window for focus-ring screen-space verification (M28 E28-4).
//!
//! Exposes a GPUI layer surface containing a focused node and its surrounding focus ring,
//! supporting variants for min, unit, max scale, layer-space defect, occluding panels,
//! and boundary width test cases.

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;
use std::str::FromStr;
use std::time::Duration;

use aegis_concordia::{Rgb, tokens};
use aegis_forum_shell::canvas::camera::{Camera, MAX_SCALE, MIN_SCALE, ScreenRect};
use aegis_forum_shell::canvas::geometry::{Viewport, WorldRect};
use aegis_scaena::mount::MountCanvas;
use gpui::{
    App, AppContext, Bounds, Context, FocusHandle, InteractiveElement, IntoElement, ParentElement,
    Render, Size, Styled, Window, WindowBackgroundAppearance, WindowBounds, WindowKind,
    WindowOptions, div, point, px,
};

use crate::layer::{LayerMapError, layer_shell_options};
use crate::ring::{RingGeometry, layer_space_ring, ring_line, screen_space_ring};
use crate::surface::{Outcome, classify_open_error, viewport};
use crate::theme;

/// Height of the ring fixture window in pixels.
pub const RING_FIXTURE_HEIGHT: f32 = 320.0;

/// Maximum runtime of the ring fixture process before self-terminating.
pub const RING_FIXTURE_LIFETIME: Duration = Duration::from_secs(60);

/// A screen point on plain fixture surface in every variant, clear of every node and panel.
pub const SURFACE_PROBE: (i32, i32) = (4, 4);

/// Window title assigned to the layer surface.
pub const RING_FIXTURE_TITLE: &str = "Aegis forum shell ring fixture";

/// Focus ring test variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingVariant {
    /// Screen-space ring at minimum camera zoom (0.125).
    Min,
    /// Screen-space ring at unit camera zoom (1.0).
    Unit,
    /// Screen-space ring at maximum camera zoom (8.0).
    Max,
    /// Planted defect: layer-space ring drawn at camera scale 0.5 (width 1.5 px).
    ScaledLayer,
    /// Planted defect: focused node and ring fully occluded by a raised panel.
    UnderPanel,
    /// Boundary case: ring partially occluded on left; other sides remain visible.
    PartlyCovered,
    /// Boundary case: ring stroke width explicitly set to 2.0 px (passes).
    Width2,
    /// Boundary case: ring stroke width explicitly set to 1.9 px (fails).
    Width19,
}

impl RingVariant {
    /// Every variant, in the order the gate runs them.
    pub const ALL: [Self; 8] = [
        Self::Min,
        Self::Unit,
        Self::Max,
        Self::PartlyCovered,
        Self::Width2,
        Self::ScaledLayer,
        Self::UnderPanel,
        Self::Width19,
    ];

    /// Returns the kebab-case identifier for this variant.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Min => "min",
            Self::Unit => "unit",
            Self::Max => "max",
            Self::ScaledLayer => "scaled-layer",
            Self::UnderPanel => "under-panel",
            Self::PartlyCovered => "partly-covered",
            Self::Width2 => "width-2-0",
            Self::Width19 => "width-1-9",
        }
    }

    /// Returns the standard verification case name for this variant.
    #[must_use]
    pub const fn case_name(&self) -> &'static str {
        match self {
            Self::Min => "ring/min",
            Self::Unit => "ring/unit",
            Self::Max => "ring/max",
            Self::ScaledLayer => "ring/scaled-layer",
            Self::UnderPanel => "ring/under-panel",
            Self::PartlyCovered => "ring/partly-covered",
            Self::Width2 => "ring/width-2-0",
            Self::Width19 => "ring/width-1-9",
        }
    }
}

impl FromStr for RingVariant {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "min" => Ok(Self::Min),
            "unit" => Ok(Self::Unit),
            "max" => Ok(Self::Max),
            "scaled-layer" => Ok(Self::ScaledLayer),
            "under-panel" => Ok(Self::UnderPanel),
            "partly-covered" => Ok(Self::PartlyCovered),
            "width-2-0" => Ok(Self::Width2),
            "width-1-9" => Ok(Self::Width19),
            other => Err(format!(
                "unknown variant '{other}'; allowed: min, unit, max, scaled-layer, under-panel, partly-covered, width-2-0, width-1-9"
            )),
        }
    }
}

/// Returns the camera scale factor for `variant`.
#[must_use]
pub const fn scale(v: RingVariant) -> f64 {
    match v {
        RingVariant::Min => MIN_SCALE,
        RingVariant::Max => MAX_SCALE,
        RingVariant::ScaledLayer => 0.5,
        RingVariant::Unit
        | RingVariant::UnderPanel
        | RingVariant::PartlyCovered
        | RingVariant::Width2
        | RingVariant::Width19 => 1.0,
    }
}

fn fixture_camera_and_node(v: RingVariant) -> Option<(Camera, WorldRect)> {
    let vp = Viewport::new(1280.0, f64::from(RING_FIXTURE_HEIGHT)).ok()?;
    let cam = Camera::new((0.0, 0.0), scale(v), vp).ok()?;
    let node = WorldRect::new(-6.0, -2.0, 6.0, 2.0).ok()?;
    Some((cam, node))
}

/// Computes the expected [`RingGeometry`] for `variant`.
#[must_use]
pub fn geometry(v: RingVariant) -> RingGeometry {
    let Some((camera, node)) = fixture_camera_and_node(v) else {
        return RingGeometry {
            node: ScreenRect {
                x0: 0.0,
                y0: 0.0,
                x1: 0.0,
                y1: 0.0,
            },
            width: 0.0,
            offset: 0.0,
        };
    };

    match v {
        RingVariant::ScaledLayer => layer_space_ring(&camera, &node),
        RingVariant::Width2 => {
            let mut g = screen_space_ring(&camera, &node);
            g.width = 2.0;
            g
        }
        RingVariant::Width19 => {
            let mut g = screen_space_ring(&camera, &node);
            g.width = 1.9;
            g
        }
        RingVariant::Min
        | RingVariant::Unit
        | RingVariant::Max
        | RingVariant::UnderPanel
        | RingVariant::PartlyCovered => screen_space_ring(&camera, &node),
    }
}

/// Returns the occluding panel screen rectangle for `variant` if any.
#[must_use]
pub fn panel(v: RingVariant) -> Option<ScreenRect> {
    match v {
        RingVariant::UnderPanel => {
            let outer = geometry(v).outer();
            Some(ScreenRect {
                x0: outer.x0 - 4.0,
                y0: outer.y0 - 4.0,
                x1: outer.x1 + 4.0,
                y1: outer.y1 + 4.0,
            })
        }
        RingVariant::PartlyCovered => {
            let geom = geometry(v);
            let outer = geom.outer();
            let node = geom.node;
            let node_width = node.x1 - node.x0;
            Some(ScreenRect {
                x0: outer.x0 - 4.0,
                y0: outer.y0,
                x1: node.x0 + 0.25 * node_width,
                y1: outer.y1,
            })
        }
        _ => None,
    }
}

/// Builds the gpui [`WindowOptions`] for the ring fixture surface.
///
/// # Errors
///
/// Returns [`LayerMapError`] if canvas mapping fails.
pub fn window_options(canvas: &MountCanvas) -> Result<WindowOptions, LayerMapError> {
    let layer_options = layer_shell_options(canvas)?;
    Ok(WindowOptions {
        titlebar: None,
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(0.0), px(0.0)),
            size: Size::new(px(0.0), px(RING_FIXTURE_HEIGHT)),
        })),
        app_id: Some("aegis-forum-shell".to_owned()),
        window_background: WindowBackgroundAppearance::Opaque,
        kind: WindowKind::LayerShell(layer_options),
        ..Default::default()
    })
}

struct RingFixtureView {
    variant: RingVariant,
    focus_handle: FocusHandle,
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "coordinate conversion to f32 for gpui"
)]
fn build_node_div(node: ScreenRect, focus_handle: &FocusHandle) -> impl IntoElement {
    let (left, top) = (node.x0 as f32, node.y0 as f32);
    let (width, height) = ((node.x1 - node.x0) as f32, (node.y1 - node.y0) as f32);

    div()
        .id("focused-node")
        .absolute()
        .left(px(left))
        .top(px(top))
        .w(px(width))
        .h(px(height))
        .track_focus(focus_handle)
}

/// Whole device pixels and colour coverage that paint a ring stroke `width` px wide.
///
/// gpui snaps border widths to device pixels, so a fractional width is painted as
/// `ceil(width)` whole pixels whose colour lies `width / ceil(width)` of the way from
/// the surface token to the ring token; the coverage summed across the stroke is then
/// `width`. Returns `None` for a width that is not finite or not positive.
#[must_use]
pub fn stroke(width: f64) -> Option<(f64, f64)> {
    if !width.is_finite() || width <= 0.0 {
        return None;
    }
    let whole = width.ceil();
    Some((whole, width / whole))
}

/// The ring token blended over the surface token at `coverage`, clamped to `0.0..=1.0`.
#[must_use]
pub fn stroke_color(coverage: f64) -> Rgb {
    let c = coverage.clamp(0.0, 1.0);
    let (s, r) = (tokens::COLOR_SURFACE, tokens::COLOR_FOCUS_RING);
    Rgb {
        red: mix(s.red, r.red, c),
        green: mix(s.green, r.green, c),
        blue: mix(s.blue, r.blue, c),
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "value rounded and clamped to 0..=255"
)]
fn mix(surface: u8, ring: u8, c: f64) -> u8 {
    let v = f64::from(surface) + c * (f64::from(ring) - f64::from(surface));
    v.round().clamp(0.0, 255.0) as u8
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "coordinate conversion to f32 for gpui"
)]
fn build_ring_div(outer: ScreenRect, ring_w: f64) -> impl IntoElement {
    let (left, top) = (outer.x0 as f32, outer.y0 as f32);
    let (width, height) = ((outer.x1 - outer.x0) as f32, (outer.y1 - outer.y0) as f32);
    let (whole, coverage) = stroke(ring_w).unwrap_or((0.0, 0.0));

    div()
        .id("focus-ring")
        .absolute()
        .left(px(left))
        .top(px(top))
        .w(px(width))
        .h(px(height))
        .border(px(whole as f32))
        .border_color(theme::color(stroke_color(coverage)))
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "coordinate conversion to f32 for gpui"
)]
fn build_panel_div(p: ScreenRect) -> impl IntoElement {
    let (left, top) = (p.x0 as f32, p.y0 as f32);
    let (width, height) = ((p.x1 - p.x0) as f32, (p.y1 - p.y0) as f32);

    div()
        .id("occluding-panel")
        .absolute()
        .left(px(left))
        .top(px(top))
        .w(px(width))
        .h(px(height))
        .bg(theme::surface_raised())
}

impl Render for RingFixtureView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let geom = geometry(self.variant);
        let node_div = build_node_div(geom.node, &self.focus_handle);
        let ring_div = build_ring_div(geom.outer(), geom.width);

        let mut root = div()
            .size_full()
            .bg(theme::surface())
            .child(node_div)
            .child(ring_div);

        if let Some(p) = panel(self.variant) {
            root = root.child(build_panel_div(p));
        }

        root
    }
}

fn record_outcome(slot: &RefCell<Option<Outcome>>, outcome: Outcome) {
    let mut borrow = slot.borrow_mut();
    if borrow.is_none() {
        *borrow = Some(outcome);
    }
}

fn wire_ring_window(
    handle: &gpui::WindowHandle<RingFixtureView>,
    slot: &Rc<RefCell<Option<Outcome>>>,
    out: &Rc<RefCell<Box<dyn Write>>>,
    variant: RingVariant,
    focus_handle: FocusHandle,
    cx: &mut App,
) {
    let slot_frame = Rc::clone(slot);
    let out_frame = Rc::clone(out);
    let update_res = handle.update(cx, |_, window, _| {
        // Next-frame callbacks run before that frame draws: focus here, so this frame
        // paints the ring, and announce READY one frame later, once the compositor's
        // frame callback for the committed ring has started the next frame.
        window.on_next_frame(move |w, cx| {
            w.focus(&focus_handle, cx);
            w.on_next_frame(move |w, _| {
                let mut writer = out_frame.borrow_mut();
                let _ = writeln!(*writer, "{}", ring_line(&geometry(variant)));
                let _ = writeln!(*writer, "READY");
                let _ = writer.flush();
                record_outcome(&slot_frame, viewport(w.viewport_size()));
            });
        });
    });
    if let Err(error) = update_res {
        record_outcome(slot, Outcome::OpenFailed(format!("{error:#}")));
        return;
    }

    let slot_timer = Rc::clone(slot);
    cx.spawn(async move |cx| {
        cx.background_executor().timer(RING_FIXTURE_LIFETIME).await;
        cx.update(move |cx| {
            record_outcome(&slot_timer, Outcome::Deadline);
            cx.quit();
        });
    })
    .detach();
}

/// Runs the ring fixture surface event loop on the platform compositor until lifetime expiry.
pub fn run_ring_fixture(
    canvas: &MountCanvas,
    variant: RingVariant,
    out: impl Write + 'static,
) -> Outcome {
    let options = match window_options(canvas) {
        Ok(opts) => opts,
        Err(error) => return Outcome::OpenFailed(format!("{error:#}")),
    };
    let slot = Rc::new(RefCell::new(None));
    let slot_clone = Rc::clone(&slot);
    let out_cell = Rc::new(RefCell::new(Box::new(out) as Box<dyn Write>));
    let out_clone = Rc::clone(&out_cell);
    let platform = gpui_linux::current_platform(false);

    gpui::Application::with_platform(platform).run(move |cx: &mut App| {
        let open_res = cx.open_window(options, |window, cx| {
            window.set_window_title(RING_FIXTURE_TITLE);
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            cx.new(|_| RingFixtureView {
                variant,
                focus_handle: focus_handle.clone(),
            })
        });
        match open_res {
            Err(error) => {
                record_outcome(&slot_clone, classify_open_error(error.as_ref()));
            }
            Ok(handle) => {
                let focus = handle
                    .read(cx)
                    .map_or_else(|_| cx.focus_handle(), |v| v.focus_handle.clone());
                wire_ring_window(&handle, &slot_clone, &out_clone, variant, focus, cx);
            }
        }
    });

    slot.borrow_mut()
        .take()
        .unwrap_or_else(|| Outcome::OpenFailed("fixture loop ended without an outcome".to_owned()))
}
