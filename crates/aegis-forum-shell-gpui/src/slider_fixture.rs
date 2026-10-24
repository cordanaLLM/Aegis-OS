// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fixture window for AT-SPI slider semantics verification (M30 E30-1).
//!
//! Exposes a GPUI layer surface containing the seeded Forum shell state with
//! node 99's slidable instrument open, and converts its AccessKit tree for GPUI.

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;
use std::str::FromStr;
use std::time::Duration;

use aegis_forum_shell::a11y::{export, level_id};
use aegis_forum_shell::canvas::registry::{ActionKind, NodeKey};
use aegis_forum_shell::canvas::{Target, seed};
use aegis_forum_shell::focus::Key;
use aegis_forum_shell::state::ShellState;
use aegis_scaena::mount::MountCanvas;
use gpui::{
    A11ySubtreeBuilder, App, AppContext, Bounds, Context, Element, ElementId, FocusHandle,
    GlobalElementId, InspectorElementId, InteractiveElement, IntoElement, KeyDownEvent, LayoutId,
    ParentElement, Pixels, Render, Role, Size, Style, Styled, Window, WindowBackgroundAppearance,
    WindowBounds, WindowKind, WindowOptions, div, point, px,
};

use crate::convert::to_gpui;
use crate::layer::{LayerMapError, layer_shell_options};
use crate::surface::{Outcome, classify_open_error, viewport};
use crate::theme;

/// Height of the slider fixture window in pixels.
pub const FIXTURE_HEIGHT: f32 = 400.0;

/// Maximum runtime of the fixture process before self-terminating.
pub const FIXTURE_LIFETIME: Duration = Duration::from_secs(90);

/// Window title assigned to the layer surface.
pub const SLIDER_FIXTURE_TITLE: &str = "Aegis forum shell slider fixture";

/// Target node index for the instrument slider fixture.
pub const TARGET_NODE_INDEX: u32 = 99;

/// Test variants for slider verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderVariant {
    /// Sound slider starting at value 0.
    Sound,
    /// Planted defect: slider node exported with `Role::Unknown`.
    NoRole,
    /// Slider starting at boundary minimum value -8.
    AtMin,
    /// Slider starting at boundary maximum value 8.
    AtMax,
}

impl SliderVariant {
    /// Returns the kebab-case identifier for this variant.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Sound => "sound",
            Self::NoRole => "no-role",
            Self::AtMin => "at-min",
            Self::AtMax => "at-max",
        }
    }
}

impl FromStr for SliderVariant {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sound" => Ok(Self::Sound),
            "no-role" => Ok(Self::NoRole),
            "at-min" => Ok(Self::AtMin),
            "at-max" => Ok(Self::AtMax),
            other => Err(format!(
                "unknown slider variant '{other}'; allowed: sound, no-role, at-min, at-max"
            )),
        }
    }
}

/// Builds the gpui [`WindowOptions`] for the slider fixture surface.
///
/// # Errors
///
/// Returns [`LayerMapError`] if the canvas fails validation or mapping.
pub fn window_options(canvas: &MountCanvas) -> Result<WindowOptions, LayerMapError> {
    let mut layer_options = layer_shell_options(canvas)?;
    layer_options.keyboard_interactivity = gpui::layer_shell::KeyboardInteractivity::Exclusive;
    Ok(WindowOptions {
        titlebar: None,
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(0.0), px(0.0)),
            size: Size::new(px(0.0), px(FIXTURE_HEIGHT)),
        })),
        app_id: Some("aegis-forum-shell".to_owned()),
        window_background: WindowBackgroundAppearance::Opaque,
        kind: WindowKind::LayerShell(layer_options),
        ..Default::default()
    })
}

fn open_seeded_slider(variant: SliderVariant) -> Result<ShellState, String> {
    let canvas = seed::seeded().map_err(|e| format!("seed failed: {e}"))?;
    let mut state = ShellState::new(canvas);
    state
        .focus
        .focus_node(&mut state.canvas, NodeKey(TARGET_NODE_INDEX))
        .map_err(|e| format!("focus node {TARGET_NODE_INDEX} failed: {e}"))?;
    state
        .focus
        .press(&mut state.canvas, Key::Enter)
        .map_err(|e| format!("press enter failed: {e}"))?;

    let target = Target::Level(NodeKey(TARGET_NODE_INDEX), 0);
    match variant {
        SliderVariant::AtMin => {
            for _ in 0..8 {
                state
                    .canvas
                    .apply(target, ActionKind::Decrement)
                    .map_err(|e| format!("decrement failed: {e}"))?;
            }
        }
        SliderVariant::AtMax => {
            for _ in 0..8 {
                state
                    .canvas
                    .apply(target, ActionKind::Increment)
                    .map_err(|e| format!("increment failed: {e}"))?;
            }
        }
        SliderVariant::Sound | SliderVariant::NoRole => {}
    }
    Ok(state)
}

fn current_slider_value(state: &ShellState) -> i8 {
    for node in state.canvas.registry().nodes() {
        if node.key == NodeKey(TARGET_NODE_INDEX)
            && let Some(level) = node.levels.first()
        {
            return level.state.value;
        }
    }
    0
}

fn export_gpui_tree(
    state: &ShellState,
    variant: SliderVariant,
) -> Result<
    (
        gpui::accesskit::NodeId,
        Vec<(gpui::accesskit::NodeId, gpui::accesskit::Node)>,
    ),
    String,
> {
    let mut update = export(state);
    let slider_id = level_id(NodeKey(TARGET_NODE_INDEX), 0);
    for (id, node) in &mut update.nodes {
        if *id == slider_id {
            node.clear_role_description();
            if variant == SliderVariant::NoRole {
                node.set_role(accesskit::Role::Unknown);
            }
        }
    }
    let converted = to_gpui(&update).map_err(|e| format!("tree conversion failed: {e}"))?;
    let root_id = converted.tree.as_ref().map_or(converted.focus, |t| t.root);
    Ok((root_id, converted.nodes))
}

struct SliderA11yElement {
    nodes: Vec<(gpui::accesskit::NodeId, gpui::accesskit::Node)>,
    root_id: gpui::accesskit::NodeId,
}

impl Element for SliderA11yElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(ElementId::Name("slider-tree-root".into()))
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        (
            window.request_layout(
                Style {
                    display: gpui::Display::Block,
                    ..Default::default()
                },
                None,
                cx,
            ),
            (),
        )
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn a11y_role(&self) -> Option<Role> {
        Some(Role::GenericContainer)
    }

    fn write_a11y_info(&self, node: &mut gpui::accesskit::Node) {
        node.set_label("Shell Tree");
    }

    fn a11y_synthetic_children(
        &mut self,
        _prepaint: &mut Self::PrepaintState,
        builder: &mut A11ySubtreeBuilder,
    ) {
        for (id, node) in &self.nodes {
            let _ = builder.push_child(*id, node.clone());
        }
        builder.parent_node().set_children(vec![self.root_id]);
    }
}

impl IntoElement for SliderA11yElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

struct SliderFixtureView {
    state: ShellState,
    variant: SliderVariant,
    tree_nodes: Vec<(gpui::accesskit::NodeId, gpui::accesskit::Node)>,
    root_id: gpui::accesskit::NodeId,
    focus_handle: FocusHandle,
    out: Rc<RefCell<Box<dyn Write>>>,
}

impl SliderFixtureView {
    fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let is_ctrl = event.keystroke.modifiers.control;
        let key = event.keystroke.key.as_str();
        let action = if is_ctrl && (key == "=" || key == "+") {
            Some(ActionKind::Increment)
        } else if is_ctrl && (key == "-" || key == "_") {
            Some(ActionKind::Decrement)
        } else {
            None
        };
        let Some(kind) = action else {
            return;
        };
        let target = Target::Level(NodeKey(TARGET_NODE_INDEX), 0);
        if self.state.canvas.apply(target, kind).is_err() {
            return;
        }
        if let Ok((root, nodes)) = export_gpui_tree(&self.state, self.variant) {
            self.root_id = root;
            self.tree_nodes = nodes;
        }
        let val = current_slider_value(&self.state);
        let mut writer = self.out.borrow_mut();
        let _ = writeln!(*writer, "EVENT value {val}");
        let _ = writer.flush();
        cx.notify();
    }
}

impl Render for SliderFixtureView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tree_elem = SliderA11yElement {
            nodes: self.tree_nodes.clone(),
            root_id: self.root_id,
        };
        let val = current_slider_value(&self.state);
        div()
            .id("slider-surface")
            .track_focus(&self.focus_handle)
            .key_context("SliderSurface")
            .size_full()
            .bg(theme::surface())
            .on_key_down(cx.listener(|this, event, _win, cx| {
                this.handle_key(event, cx);
            }))
            .child(tree_elem)
            .child(
                div()
                    .p(px(16.0))
                    .child(format!("Settings instrument for node 100: {val}")),
            )
    }
}

fn record_outcome(slot: &RefCell<Option<Outcome>>, outcome: Outcome) {
    let mut borrow = slot.borrow_mut();
    if borrow.is_none() {
        *borrow = Some(outcome);
    }
}

fn wire_fixture_window(
    handle: &gpui::WindowHandle<SliderFixtureView>,
    slot: &Rc<RefCell<Option<Outcome>>>,
    out: &Rc<RefCell<Box<dyn Write>>>,
    focus_handle: FocusHandle,
    cx: &mut App,
) {
    let slot_frame = Rc::clone(slot);
    let out_frame = Rc::clone(out);
    let update_res = handle.update(cx, |_, window, _| {
        window.on_next_frame(move |w, cx| {
            w.focus(&focus_handle, cx);
            let mut writer = out_frame.borrow_mut();
            let _ = writeln!(*writer, "READY");
            let _ = writer.flush();
            record_outcome(&slot_frame, viewport(w.viewport_size()));
        });
    });
    if let Err(error) = update_res {
        record_outcome(slot, Outcome::OpenFailed(format!("{error:#}")));
        return;
    }

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

/// Runs the slider fixture surface event loop on the platform compositor until lifetime expiry.
pub fn run_slider_fixture(
    canvas: &MountCanvas,
    variant: SliderVariant,
    out: impl Write + 'static,
) -> Outcome {
    let options = match window_options(canvas) {
        Ok(opts) => opts,
        Err(error) => return Outcome::OpenFailed(format!("{error:#}")),
    };
    let state = match open_seeded_slider(variant) {
        Ok(s) => s,
        Err(error) => return Outcome::OpenFailed(error),
    };
    let (root_id, tree_nodes) = match export_gpui_tree(&state, variant) {
        Ok(pair) => pair,
        Err(error) => return Outcome::OpenFailed(error),
    };

    let slot = Rc::new(RefCell::new(None));
    let slot_clone = Rc::clone(&slot);
    let out_cell = Rc::new(RefCell::new(Box::new(out) as Box<dyn Write>));
    let out_clone = Rc::clone(&out_cell);
    let platform = gpui_linux::current_platform(false);

    gpui::Application::with_platform(platform).run(move |cx: &mut App| {
        let out_view = Rc::clone(&out_clone);
        let open_res = cx.open_window(options, |window, cx| {
            window.set_window_title(SLIDER_FIXTURE_TITLE);
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            cx.new(|_| SliderFixtureView {
                state,
                variant,
                tree_nodes,
                root_id,
                focus_handle: focus_handle.clone(),
                out: out_view,
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
                wire_fixture_window(&handle, &slot_clone, &out_clone, focus, cx);
            }
        }
    });

    slot.borrow_mut().take().unwrap_or_else(|| {
        Outcome::OpenFailed("slider fixture loop ended without an outcome".to_owned())
    })
}
