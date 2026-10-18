// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fixture window and test tree for AT-SPI verification (M28 E28-3).
//!
//! Exposes a GPUI layer surface containing an accessible button, a scrollable
//! list of 1,000 seeded items, a shown panel, and a hidden subtree.
//! The fixture supports planted defect variants to test accessibility negative cases.

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;
use std::str::FromStr;
use std::time::Duration;

use aegis_scaena::mount::MountCanvas;
use gpui::{
    A11ySubtreeBuilder, App, AppContext, Bounds, Context, Element, ElementId, FocusHandle,
    GlobalElementId, InspectorElementId, InteractiveElement, IntoElement, LayoutId, ParentElement,
    Pixels, Render, Role, Size, StatefulInteractiveElement, Style, Styled, Window,
    WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions, accesskit, div, point, px,
};

use crate::layer::{LayerMapError, layer_shell_options};
use crate::surface::{Outcome, classify_open_error, viewport};
use crate::theme;

/// Total number of seeded items in the fixture list.
pub const SEEDED_NODES: usize = 1000;

/// Number of list items visible in the viewport without scrolling.
pub const IN_VIEW: usize = 10;

/// Height of each list item in pixels.
pub const ROW_HEIGHT: f32 = 24.0;

/// Height of the fixture window in pixels.
pub const FIXTURE_HEIGHT: f32 = 400.0;

/// Maximum runtime of the fixture process before self-terminating.
pub const FIXTURE_LIFETIME: Duration = Duration::from_secs(90);

/// Window title assigned to the layer surface.
pub const FIXTURE_TITLE: &str = "Aegis forum shell fixture";

/// Initial accessible name of the action button.
pub const BUTTON_LABEL: &str = "Open the forum";

/// Accessible name of the action button after receiving a click.
pub const BUTTON_LABEL_AFTER: &str = "Forum opened";

/// Accessible name of the list container.
pub const LIST_LABEL: &str = "Seeded nodes";

/// Accessible name of the hidden container element.
pub const HIDDEN_LABEL: &str = "Hidden panel";

/// Accessible names of the synthetic children in the hidden subtree.
pub const HIDDEN_CHILD_LABELS: [&str; 2] = ["Hidden child 1", "Hidden child 2"];

/// Accessible name of the visible panel sibling.
pub const SHOWN_LABEL: &str = "Shown panel";

/// Index of the list item that loses its label in [`Variant::Unlabelled`].
pub const UNLABELLED_ROW: usize = 517;

/// Generates the standard accessible label for row `i`.
#[must_use]
pub fn row_label(i: usize) -> String {
    format!("Node {i}")
}

/// Test variants for injecting specific accessibility defects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// Sound implementation with button, 1,000 labelled items, and hidden subtree.
    Sound,
    /// Exposes the button as a container ([`Role::Group`] / AT-SPI Panel).
    ButtonAsContainer,
    /// Omits the `aria_label` from list item 517.
    Unlabelled,
    /// Culls the list to render only the 10 rows in view.
    Culled,
    /// Omits `set_hidden` so the hidden subtree is reported showing.
    HiddenShown,
}

impl Variant {
    /// Returns the kebab-case identifier for this variant.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Sound => "sound",
            Self::ButtonAsContainer => "button-as-container",
            Self::Unlabelled => "unlabelled",
            Self::Culled => "culled",
            Self::HiddenShown => "hidden-shown",
        }
    }
}

impl FromStr for Variant {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sound" => Ok(Self::Sound),
            "button-as-container" => Ok(Self::ButtonAsContainer),
            "unlabelled" => Ok(Self::Unlabelled),
            "culled" => Ok(Self::Culled),
            "hidden-shown" => Ok(Self::HiddenShown),
            other => Err(format!(
                "unknown variant '{other}'; allowed: sound, button-as-container, unlabelled, culled, hidden-shown"
            )),
        }
    }
}

/// Returns the range of list rows rendered for `v`.
#[must_use]
pub fn rendered_rows(v: Variant) -> std::ops::Range<usize> {
    match v {
        Variant::Culled => 0..IN_VIEW,
        _ => 0..SEEDED_NODES,
    }
}

/// Returns whether list row `i` should receive an accessible label in variant `v`.
#[must_use]
pub fn row_is_labelled(v: Variant, i: usize) -> bool {
    !matches!(v, Variant::Unlabelled if i == UNLABELLED_ROW)
}

/// Builds the gpui [`WindowOptions`] for the fixture surface.
///
/// # Errors
///
/// Returns [`LayerMapError`] if the canvas fails validation or mapping.
pub fn window_options(canvas: &MountCanvas) -> Result<WindowOptions, LayerMapError> {
    let layer_options = layer_shell_options(canvas)?;
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

struct HiddenSubtreeElement {
    hide: bool,
}

impl Element for HiddenSubtreeElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(ElementId::Name(HIDDEN_LABEL.into()))
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
        Some(Role::Group)
    }

    fn write_a11y_info(&self, node: &mut accesskit::Node) {
        node.set_label(HIDDEN_LABEL);
    }

    fn a11y_synthetic_children(
        &mut self,
        _prepaint: &mut Self::PrepaintState,
        builder: &mut A11ySubtreeBuilder,
    ) {
        if self.hide {
            builder.parent_node().set_hidden();
        }
        for (i, &child_label) in HIDDEN_CHILD_LABELS.iter().enumerate() {
            let mut child_node = accesskit::Node::new(Role::Group);
            child_node.set_label(child_label);
            let child_id = builder.synthetic_node_id(i);
            let _ = builder.push_child(child_id, child_node);
        }
    }
}

impl IntoElement for HiddenSubtreeElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

struct FixtureView {
    variant: Variant,
    button_label: String,
    focus_handle: FocusHandle,
    out: Rc<RefCell<Box<dyn Write>>>,
}

impl Render for FixtureView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let btn_role = if self.variant == Variant::ButtonAsContainer {
            Role::Group
        } else {
            Role::Button
        };
        let out_click = Rc::clone(&self.out);
        let button = div()
            .id("button")
            .role(btn_role)
            .aria_label(self.button_label.clone())
            .track_focus(&self.focus_handle)
            .w(px(200.0))
            .h(px(40.0))
            .on_click(cx.listener(move |this, _, _window, cx| {
                BUTTON_LABEL_AFTER.clone_into(&mut this.button_label);
                let mut out = out_click.borrow_mut();
                let _ = writeln!(*out, "EVENT click");
                let _ = out.flush();
                cx.notify();
            }));

        let mut list = div()
            .id("list")
            .role(Role::List)
            .aria_label(LIST_LABEL)
            .overflow_y_scroll()
            .h(px(FIXTURE_HEIGHT));

        for i in rendered_rows(self.variant) {
            let mut row = div().id(("row", i)).role(Role::ListItem).h(px(ROW_HEIGHT));
            if row_is_labelled(self.variant, i) {
                row = row.aria_label(row_label(i));
            }
            list = list.child(row);
        }

        let shown_panel = div()
            .id("shown-panel")
            .role(Role::Group)
            .aria_label(SHOWN_LABEL)
            .w(px(100.0))
            .h(px(20.0));

        let hidden_elem = HiddenSubtreeElement {
            hide: self.variant != Variant::HiddenShown,
        };

        div()
            .size_full()
            .bg(theme::surface())
            .child(button)
            .child(list)
            .child(shown_panel)
            .child(hidden_elem)
    }
}

fn record_outcome(slot: &RefCell<Option<Outcome>>, outcome: Outcome) {
    let mut borrow = slot.borrow_mut();
    if borrow.is_none() {
        *borrow = Some(outcome);
    }
}

fn wire_fixture_window(
    handle: &gpui::WindowHandle<FixtureView>,
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

/// Runs the fixture surface event loop on the platform compositor until lifetime expiry.
pub fn run_fixture(canvas: &MountCanvas, variant: Variant, out: impl Write + 'static) -> Outcome {
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
        let out_view = Rc::clone(&out_clone);
        let open_res = cx.open_window(options, |window, cx| {
            window.set_window_title(FIXTURE_TITLE);
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            cx.new(|_| FixtureView {
                variant,
                button_label: BUTTON_LABEL.to_owned(),
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

    slot.borrow_mut()
        .take()
        .unwrap_or_else(|| Outcome::OpenFailed("fixture loop ended without an outcome".to_owned()))
}
