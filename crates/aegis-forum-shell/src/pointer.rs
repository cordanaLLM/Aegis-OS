// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Pointer alternatives and target size model (M30 E30-2, REQ-P05-14).
//!
//! While REQ-P05-14 lists lens drag among canvas operations, the current
//! canvas model has no lens entity or lens drag operation; no lens is
//! invented here.
//!
//! This module provides:
//! - [`Operation`]: canvas pointer operations (`Pan`, `Zoom`, `NodeMove`, `NodeResize`, `HandleRotate`).
//! - [`OperationSpec`] and [`check_operations`]: verifying that every drag operation offers
//!   click-only step controls and keyboard alternatives.
//! - [`drag`] and [`click`]: executing operations via drag or click-only controls, ensuring
//!   state equivalence.
//! - [`PointerTracker`]: single-pointer activation state machine firing only on release
//!   on-target, aborting on release off-target.
//! - [`target_report`]: auditing node screen bounds against [`MIN_TARGET_CSS_PX`] (24 CSS px),
//!   marking smaller targets as pointer-inert when an equivalent keyboard control exists.

use thiserror::Error;

use crate::canvas::registry::{ActionKind, CanvasNode, NodeKey};
use crate::canvas::{Canvas, CanvasError, InertPolicy, Target};
use crate::focus::{Command, Shortcuts};

/// Minimum target size in CSS pixels (REQ-P05-14).
pub const MIN_TARGET_CSS_PX: f64 = 24.0;

/// Default grid step in canvas world units for moving nodes and panning.
pub const GRID_STEP: f64 = 20.0;

/// Default resize step in canvas world units.
pub const RESIZE_STEP: f64 = 10.0;

/// Default minimum node edge in canvas world units.
pub const MIN_NODE_SIZE: f64 = 10.0;

/// Whole-step angle delta in degrees for handle rotation.
pub const ROTATE_STEP_DEGREES: i16 = 15;

/// Multiplicative zoom factor for one zoom step.
pub const ZOOM_FACTOR: f64 = 2.0;

/// Maximum tracked pointer move positions.
pub const MAX_TRACKED_MOVES: usize = 64;

/// Tolerance for floating-point coordinate comparisons.
pub const FLOAT_TOLERANCE: f64 = 1e-6;

/// Canvas operations that may involve pointer interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Operation {
    /// Panning the camera across canvas world space.
    Pan,
    /// Zooming the camera scale in or out.
    Zoom,
    /// Moving a canvas node.
    NodeMove,
    /// Resizing a canvas node.
    NodeResize,
    /// Rotating an instrument handle.
    HandleRotate,
}

/// Descriptors of click-only step controls for canvas operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClickControlKind {
    /// Step pan left.
    PanLeft,
    /// Step pan right.
    PanRight,
    /// Step pan up.
    PanUp,
    /// Step pan down.
    PanDown,
    /// Step zoom in.
    ZoomIn,
    /// Step zoom out.
    ZoomOut,
    /// Step move left.
    MoveLeft,
    /// Step move right.
    MoveRight,
    /// Step move up.
    MoveUp,
    /// Step move down.
    MoveDown,
    /// Step resize larger (grow).
    ResizeGrow,
    /// Step resize smaller (shrink).
    ResizeShrink,
    /// Step rotate clockwise.
    RotateCw,
    /// Step rotate counter-clockwise.
    RotateCcw,
}

/// A specific click control invocation targeting the canvas or a node/handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickControl {
    /// Step pan left.
    PanLeft,
    /// Step pan right.
    PanRight,
    /// Step pan up.
    PanUp,
    /// Step pan down.
    PanDown,
    /// Step zoom in.
    ZoomIn,
    /// Step zoom out.
    ZoomOut,
    /// Step move target node left.
    MoveLeft(NodeKey),
    /// Step move target node right.
    MoveRight(NodeKey),
    /// Step move target node up.
    MoveUp(NodeKey),
    /// Step move target node down.
    MoveDown(NodeKey),
    /// Step resize target node larger.
    ResizeGrow(NodeKey),
    /// Step resize target node smaller.
    ResizeShrink(NodeKey),
    /// Step rotate handle clockwise.
    RotateCw(Target),
    /// Step rotate handle counter-clockwise.
    RotateCcw(Target),
}

impl ClickControl {
    /// Returns the kind of this click control.
    #[must_use]
    pub const fn kind(&self) -> ClickControlKind {
        match self {
            Self::PanLeft => ClickControlKind::PanLeft,
            Self::PanRight => ClickControlKind::PanRight,
            Self::PanUp => ClickControlKind::PanUp,
            Self::PanDown => ClickControlKind::PanDown,
            Self::ZoomIn => ClickControlKind::ZoomIn,
            Self::ZoomOut => ClickControlKind::ZoomOut,
            Self::MoveLeft(_) => ClickControlKind::MoveLeft,
            Self::MoveRight(_) => ClickControlKind::MoveRight,
            Self::MoveUp(_) => ClickControlKind::MoveUp,
            Self::MoveDown(_) => ClickControlKind::MoveDown,
            Self::ResizeGrow(_) => ClickControlKind::ResizeGrow,
            Self::ResizeShrink(_) => ClickControlKind::ResizeShrink,
            Self::RotateCw(_) => ClickControlKind::RotateCw,
            Self::RotateCcw(_) => ClickControlKind::RotateCcw,
        }
    }
}

/// Specification of interaction alternatives for an operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationSpec {
    /// The operation concerned.
    pub op: Operation,
    /// Whether the operation supports drag/path input.
    pub drag: bool,
    /// Available click-only step controls.
    pub click_controls: Vec<ClickControlKind>,
    /// Available keyboard commands.
    pub keyboard: Vec<Command>,
}

/// Errors arising from pointer alternatives, specifications, or tracking.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PointerError {
    /// An operation supports drag but provides no click-only alternatives.
    #[error("operation {op:?} supports drag but provides no click alternatives")]
    DragOnly {
        /// The operation concerned.
        op: Operation,
    },
    /// An operation provides no keyboard alternative.
    #[error("operation {op:?} has no keyboard alternative")]
    NoKeyboard {
        /// The operation concerned.
        op: Operation,
    },
    /// Target required for operation is missing.
    #[error("operation {op:?} requires a target")]
    MissingTarget {
        /// The operation concerned.
        op: Operation,
    },
    /// Drag delta does not match the operation type.
    #[error("invalid drag delta for operation {op:?}")]
    InvalidDelta {
        /// The operation concerned.
        op: Operation,
    },
    /// Underlying canvas operation failed.
    #[error(transparent)]
    Canvas(#[from] CanvasError),
}

/// Verifies that every drag operation provides click-only controls and a keyboard alternative.
///
/// # Errors
///
/// Returns [`PointerError::DragOnly`] if `drag` is true but `click_controls` is empty,
/// or [`PointerError::NoKeyboard`] if `drag` is true and either `keyboard` is empty
/// or any listed command has no letter bound in `shortcuts`.
pub fn check_operations(
    specs: &[OperationSpec],
    shortcuts: &Shortcuts,
) -> Result<(), PointerError> {
    for spec in specs {
        if spec.drag {
            if spec.click_controls.is_empty() {
                return Err(PointerError::DragOnly { op: spec.op });
            }
            let unbound = spec
                .keyboard
                .iter()
                .any(|cmd| shortcuts.binding(*cmd).is_none());
            if spec.keyboard.is_empty() || unbound {
                return Err(PointerError::NoKeyboard { op: spec.op });
            }
        }
    }
    Ok(())
}

/// The default operation specifications for the forum shell canvas.
#[must_use]
pub fn default_operation_specs() -> Vec<OperationSpec> {
    vec![
        OperationSpec {
            op: Operation::Pan,
            drag: true,
            click_controls: vec![
                ClickControlKind::PanLeft,
                ClickControlKind::PanRight,
                ClickControlKind::PanUp,
                ClickControlKind::PanDown,
            ],
            keyboard: vec![
                Command::PanLeft,
                Command::PanRight,
                Command::PanUp,
                Command::PanDown,
            ],
        },
        OperationSpec {
            op: Operation::Zoom,
            drag: true,
            click_controls: vec![ClickControlKind::ZoomIn, ClickControlKind::ZoomOut],
            keyboard: vec![Command::ZoomIn, Command::ZoomOut],
        },
        OperationSpec {
            op: Operation::NodeMove,
            drag: true,
            click_controls: vec![
                ClickControlKind::MoveLeft,
                ClickControlKind::MoveRight,
                ClickControlKind::MoveUp,
                ClickControlKind::MoveDown,
            ],
            keyboard: vec![
                Command::MoveLeft,
                Command::MoveRight,
                Command::MoveUp,
                Command::MoveDown,
            ],
        },
        OperationSpec {
            op: Operation::NodeResize,
            drag: true,
            click_controls: vec![ClickControlKind::ResizeGrow, ClickControlKind::ResizeShrink],
            keyboard: vec![Command::ResizeGrow, Command::ResizeShrink],
        },
        OperationSpec {
            op: Operation::HandleRotate,
            drag: true,
            click_controls: vec![ClickControlKind::RotateCw, ClickControlKind::RotateCcw],
            keyboard: vec![Command::RotateCw, Command::RotateCcw],
        },
    ]
}

/// Drag displacement or magnitude for an operation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DragDelta {
    /// 2D offset in world units `(dx, dy)`.
    Offset(f64, f64),
    /// Multiplicative scale factor.
    Factor(f64),
    /// Angular delta in degrees.
    Degrees(i16),
}

/// Executes a drag operation on the canvas.
///
/// # Errors
///
/// Returns [`PointerError::MissingTarget`], [`PointerError::InvalidDelta`],
/// or propagates canvas errors.
pub fn drag(
    canvas: &mut Canvas,
    op: Operation,
    target: Option<Target>,
    delta: DragDelta,
) -> Result<(), PointerError> {
    match (op, delta) {
        (Operation::Pan, DragDelta::Offset(dx, dy)) => {
            canvas.pan(dx, dy);
            Ok(())
        }
        (Operation::Zoom, DragDelta::Factor(factor)) => {
            canvas.zoom(factor);
            Ok(())
        }
        (Operation::NodeMove, DragDelta::Offset(dx, dy)) => {
            let key = target_node_key(op, target)?;
            Ok(canvas.move_node(key, dx, dy)?)
        }
        (Operation::NodeResize, DragDelta::Offset(dw, dh)) => {
            let key = target_node_key(op, target)?;
            Ok(canvas.resize_node(key, dw, dh)?)
        }
        (Operation::HandleRotate, DragDelta::Degrees(deg)) => {
            let tgt = target.ok_or(PointerError::MissingTarget { op })?;
            Ok(canvas.rotate_handle(tgt, deg)?)
        }
        _ => Err(PointerError::InvalidDelta { op }),
    }
}

fn target_node_key(op: Operation, target: Option<Target>) -> Result<NodeKey, PointerError> {
    match target {
        Some(Target::Node(key)) => Ok(key),
        _ => Err(PointerError::MissingTarget { op }),
    }
}

fn click_pan(canvas: &mut Canvas, control: ClickControl) {
    match control {
        ClickControl::PanLeft => canvas.pan(-GRID_STEP, 0.0),
        ClickControl::PanRight => canvas.pan(GRID_STEP, 0.0),
        ClickControl::PanUp => canvas.pan(0.0, -GRID_STEP),
        ClickControl::PanDown => canvas.pan(0.0, GRID_STEP),
        _ => {}
    }
}

fn click_zoom(canvas: &mut Canvas, control: ClickControl) {
    match control {
        ClickControl::ZoomIn => canvas.zoom(ZOOM_FACTOR),
        ClickControl::ZoomOut => canvas.zoom(1.0 / ZOOM_FACTOR),
        _ => {}
    }
}

fn click_move(canvas: &mut Canvas, control: ClickControl) -> Result<(), CanvasError> {
    match control {
        ClickControl::MoveLeft(key) => canvas.move_node(key, -GRID_STEP, 0.0),
        ClickControl::MoveRight(key) => canvas.move_node(key, GRID_STEP, 0.0),
        ClickControl::MoveUp(key) => canvas.move_node(key, 0.0, -GRID_STEP),
        ClickControl::MoveDown(key) => canvas.move_node(key, 0.0, GRID_STEP),
        _ => Ok(()),
    }
}

fn click_resize_or_rotate(canvas: &mut Canvas, control: ClickControl) -> Result<(), CanvasError> {
    match control {
        ClickControl::ResizeGrow(key) => canvas.resize_node(key, RESIZE_STEP, RESIZE_STEP),
        ClickControl::ResizeShrink(key) => canvas.resize_node(key, -RESIZE_STEP, -RESIZE_STEP),
        ClickControl::RotateCw(tgt) => canvas.rotate_handle(tgt, ROTATE_STEP_DEGREES),
        ClickControl::RotateCcw(tgt) => canvas.rotate_handle(tgt, -ROTATE_STEP_DEGREES),
        _ => Ok(()),
    }
}

/// Executes a click-only step control on the canvas.
///
/// # Errors
///
/// Propagates canvas errors such as moving a pinned node.
pub fn click(canvas: &mut Canvas, control: ClickControl) -> Result<(), PointerError> {
    match control {
        ClickControl::PanLeft
        | ClickControl::PanRight
        | ClickControl::PanUp
        | ClickControl::PanDown => {
            click_pan(canvas, control);
            Ok(())
        }
        ClickControl::ZoomIn | ClickControl::ZoomOut => {
            click_zoom(canvas, control);
            Ok(())
        }
        ClickControl::MoveLeft(_)
        | ClickControl::MoveRight(_)
        | ClickControl::MoveUp(_)
        | ClickControl::MoveDown(_) => Ok(click_move(canvas, control)?),
        ClickControl::ResizeGrow(_)
        | ClickControl::ResizeShrink(_)
        | ClickControl::RotateCw(_)
        | ClickControl::RotateCcw(_) => Ok(click_resize_or_rotate(canvas, control)?),
    }
}

/// What a pointer press targets: a canvas node/level or a click-only control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressTarget {
    /// A canvas node or instrument level.
    Node(Target),
    /// A click-only step control button.
    Control(ClickControl),
}

/// Single-pointer gesture activation tracker (REQ-P05-14).
///
/// Single-pointer activation completes on release only when hit-testing the
/// pressed target. Releasing off target aborts with zero state change.
/// Down-event never activates.
#[derive(Debug, Clone, Default)]
pub struct PointerTracker {
    pressed: Option<(PressTarget, (f64, f64))>,
    moves: Vec<(f64, f64)>,
}

/// Outcome of releasing a pointer gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationOutcome {
    /// No gesture was active.
    Ignored,
    /// Gesture aborted because release occurred off the pressed target.
    Aborted,
    /// Action activated on release on target.
    Activated(PressTarget),
}

impl PointerTracker {
    /// Creates an idle tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pressed: None,
            moves: Vec::new(),
        }
    }

    /// Records pointer down at `pos` targeting `target`. Down never activates.
    pub fn press(&mut self, target: PressTarget, pos: (f64, f64)) {
        self.pressed = Some((target, pos));
        self.moves.clear();
    }

    /// Records a pointer move, bounded by [`MAX_TRACKED_MOVES`].
    pub fn move_to(&mut self, pos: (f64, f64)) {
        if self.pressed.is_some() && self.moves.len() < MAX_TRACKED_MOVES {
            self.moves.push(pos);
        }
    }

    /// Finishes gesture on pointer up.
    ///
    /// If `released_on` is `Some(target)`, the caller has hit-tested the release
    /// position (required for click-only controls, and optional for nodes).
    /// If `released_on` is `None`, hit-testing defaults to `canvas.hit_test(pos)`.
    ///
    /// Activation executes only if release matches the pressed target:
    /// - for controls: release on the identical control executes [`click`];
    /// - for nodes: release on the pressed node or its instrument executes [`ActionKind::Select`].
    ///
    /// Releasing elsewhere aborts with zero state change on the canvas.
    ///
    /// # Errors
    ///
    /// Propagates canvas or click errors if activation fails.
    pub fn release(
        &mut self,
        canvas: &mut Canvas,
        pos: (f64, f64),
        released_on: Option<PressTarget>,
    ) -> Result<ActivationOutcome, PointerError> {
        let Some((target, _start)) = self.pressed.take() else {
            return Ok(ActivationOutcome::Ignored);
        };
        self.moves.clear();
        let hit = released_on.or_else(|| canvas.hit_test(pos).map(PressTarget::Node));
        if !is_matching_target(hit, target) {
            return Ok(ActivationOutcome::Aborted);
        }
        match target {
            PressTarget::Node(t) => {
                canvas.apply(t, ActionKind::Select)?;
            }
            PressTarget::Control(control) => {
                click(canvas, control)?;
            }
        }
        Ok(ActivationOutcome::Activated(target))
    }

    /// Whether a pointer press is currently tracked.
    #[must_use]
    pub const fn is_pressed(&self) -> bool {
        self.pressed.is_some()
    }

    /// The tracked move positions.
    #[must_use]
    pub fn moves(&self) -> &[(f64, f64)] {
        &self.moves
    }
}

fn is_matching_target(hit: Option<PressTarget>, pressed: PressTarget) -> bool {
    match (hit, pressed) {
        (Some(PressTarget::Control(c1)), PressTarget::Control(c2)) => c1 == c2,
        (
            Some(PressTarget::Node(Target::Node(k1) | Target::Level(k1, _))),
            PressTarget::Node(Target::Node(k2) | Target::Level(k2, _)),
        ) => k1 == k2,
        _ => false,
    }
}

/// Status of a canvas node's pointer target size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetStatus {
    /// Target meets or exceeds [`MIN_TARGET_CSS_PX`].
    Ok,
    /// Target is below [`MIN_TARGET_CSS_PX`] but pointer-inert with keyboard equivalent.
    Inert {
        /// Whether an equivalent keyboard control exists.
        equivalent: bool,
    },
    /// Target is below [`MIN_TARGET_CSS_PX`] with inert forbidden or no equivalent.
    Defect,
}

/// Generates a report of pointer target sizes for every node on the canvas.
#[must_use]
pub fn target_report(canvas: &Canvas) -> Vec<(NodeKey, TargetStatus)> {
    let mut report = Vec::with_capacity(canvas.registry().len());
    for node in canvas.registry().nodes() {
        let screen = canvas.camera().to_screen(&node.rect);
        let width = screen.x1 - screen.x0;
        let height = screen.y1 - screen.y0;
        let status = if width >= MIN_TARGET_CSS_PX - FLOAT_TOLERANCE
            && height >= MIN_TARGET_CSS_PX - FLOAT_TOLERANCE
        {
            TargetStatus::Ok
        } else if canvas.inert_policy() == InertPolicy::Forbidden {
            TargetStatus::Defect
        } else {
            let equivalent = has_keyboard_equivalent(canvas, node);
            if equivalent {
                TargetStatus::Inert { equivalent: true }
            } else {
                TargetStatus::Defect
            }
        };
        report.push((node.key, status));
    }
    report
}

/// Checks whether `node` has an equivalent keyboard alternative in the canvas model.
fn has_keyboard_equivalent(canvas: &Canvas, node: &CanvasNode) -> bool {
    if canvas.registry().position(node.key).is_none() {
        return false;
    }
    if !node.actions.is_empty() && node.actions.iter().all(|a| a.keyboard.is_none()) {
        return false;
    }
    true
}
