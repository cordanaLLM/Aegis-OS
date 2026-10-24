// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! M30 E30-2 part A (REQ-P05-14): pointer alternatives and target size model.
//!
//! Positive:
//! - default operation specifications pass `check_operations`;
//! - for every operation (`Pan`, `Zoom`, `NodeMove`, `NodeResize`, `HandleRotate`), drag of N
//!   steps, N clicks of the click-only control, and N key presses of the shortcut command end
//!   in identical canvas state;
//! - node-bound commands with no focused node return `KeyOutcome::Ignored`;
//! - pointer release on node target activates Select once;
//! - pointer press on click control alone changes nothing; release on it activates click once;
//! - pointer press on click control and release off target aborts with zero side effects.
//!
//! Negative:
//! - an operation spec with drag=true and no click controls fails as `DragOnly`;
//! - an operation spec with drag=true and no keyboard commands fails as `NoKeyboard`;
//! - removing the rotate letter from Shortcuts refuses `check_operations` as `NoKeyboard`;
//! - a pinned node refuses move and resize with Pinned;
//! - a node forced below 24 CSS px with pointer-inert Forbidden reports Defect.
//!
//! Boundary:
//! - at camera `MIN_SCALE` (0.125) on the seeded canvas, every node is Ok or Inert-with-equivalent,
//!   none is Defect;
//! - a node at exactly 24.0 CSS px is Ok and hit-testable, while at 23.9 CSS px it is Inert
//!   and skipped by hit-test;
//! - press on node, move off target and release aborts with zero side effect on canvas state;
//! - handle rotation wraps correctly: 345 + 15 -> 0, and 0 - 15 -> 345.

mod common;

use aegis_forum_shell::canvas::camera::{Camera, MIN_SCALE};
use aegis_forum_shell::canvas::geometry::{Viewport, WorldRect};
use aegis_forum_shell::canvas::registry::{
    ActionKind, ActionState, Binding, CanvasNode, InstrumentLevel, LevelKind, NodeAction, NodeKey,
    NodeKind, TabPolicy,
};
use aegis_forum_shell::canvas::{Canvas, CanvasError, InertPolicy, Target, seed};
use aegis_forum_shell::focus::{Command, FocusModel, Key, KeyOutcome, Shortcuts};
use aegis_forum_shell::pointer::{
    ActivationOutcome, ClickControl, ClickControlKind, DragDelta, FLOAT_TOLERANCE, GRID_STEP,
    MIN_TARGET_CSS_PX, Operation, OperationSpec, PointerError, PointerTracker, PressTarget,
    RESIZE_STEP, ROTATE_STEP_DEGREES, TargetStatus, ZOOM_FACTOR, check_operations, click,
    default_operation_specs, drag, target_report,
};
use common::Fallible;

/// Builds a test canvas with one node with given origin and dimensions.
fn single_node_canvas(x: f64, y: f64, w: f64, h: f64) -> Fallible<(Canvas, NodeKey)> {
    let vp = Viewport::new(500.0, 500.0)?;
    let cam = Camera::new((250.0, 250.0), 1.0, vp)?;
    let mut canvas = Canvas::new(WorldRect::new(-1000.0, -1000.0, 2000.0, 2000.0)?, cam);
    let key = NodeKey(1);
    canvas.push(CanvasNode {
        key,
        label: "Target Node".to_owned(),
        kind: NodeKind::Application,
        rect: WorldRect::from_origin_size(x, y, w, h)?,
        actions: vec![NodeAction::both(ActionKind::Select, Binding::Space)],
        levels: Vec::new(),
        state: ActionState::default(),
    })?;
    Ok((canvas, key))
}

/// Builds a test canvas with an instrumented node.
fn instrumented_node_canvas() -> Fallible<(Canvas, Target)> {
    let vp = Viewport::new(500.0, 500.0)?;
    let cam = Camera::new((250.0, 250.0), 1.0, vp)?;
    let mut canvas = Canvas::new(WorldRect::new(-1000.0, -1000.0, 2000.0, 2000.0)?, cam);
    let key = NodeKey(1);
    let level = InstrumentLevel {
        label: "Rotatable Dial".to_owned(),
        kind: LevelKind::Instrument,
        actions: vec![NodeAction::both(ActionKind::Increment, Binding::Ctrl('t'))],
        tab: TabPolicy::Leaves,
        state: ActionState::default(),
    };
    canvas.push(CanvasNode {
        key,
        label: "Instrumented Node".to_owned(),
        kind: NodeKind::Application,
        rect: WorldRect::from_origin_size(200.0, 200.0, 100.0, 100.0)?,
        actions: vec![NodeAction::both(ActionKind::Select, Binding::Space)],
        levels: vec![level],
        state: ActionState::default(),
    })?;
    Ok((canvas, Target::Level(key, 0)))
}

// --- Positive -------------------------------------------------------------

/// Positive: the default operation specifications for the shell pass validation.
#[test]
fn positive_default_specs_pass() -> Fallible {
    let specs = default_operation_specs();
    let shortcuts = Shortcuts::default();
    assert_eq!(specs.len(), 5);
    check_operations(&specs, &shortcuts)?;
    Ok(())
}

/// Positive: pan drag of N steps equals N clicks of click control and N keyboard presses.
#[test]
fn positive_drag_click_keyboard_equality_pan() -> Fallible {
    let mut c_click = seed::grid(2, 2)?;
    let mut c_drag = c_click.clone();
    let mut c_key = c_click.clone();
    let mut model = FocusModel::new();

    for _ in 0..3 {
        click(&mut c_click, ClickControl::PanRight)?;
        drag(
            &mut c_drag,
            Operation::Pan,
            None,
            DragDelta::Offset(GRID_STEP, 0.0),
        )?;
        assert_eq!(
            model.press(&mut c_key, Key::Char('l'))?,
            KeyOutcome::Command(Command::PanRight)
        );
    }

    let diff_x = (c_click.camera().center().0 - c_drag.camera().center().0).abs();
    let diff_y = (c_click.camera().center().1 - c_drag.camera().center().1).abs();
    assert!(diff_x < FLOAT_TOLERANCE, "center x diff {diff_x}");
    assert!(diff_y < FLOAT_TOLERANCE, "center y diff {diff_y}");
    assert_eq!(c_click.camera().scale(), c_drag.camera().scale());

    let key_diff_x = (c_key.camera().center().0 - c_drag.camera().center().0).abs();
    let key_diff_y = (c_key.camera().center().1 - c_drag.camera().center().1).abs();
    assert!(
        key_diff_x < FLOAT_TOLERANCE,
        "key center x diff {key_diff_x}"
    );
    assert!(
        key_diff_y < FLOAT_TOLERANCE,
        "key center y diff {key_diff_y}"
    );
    assert_eq!(c_key.camera().scale(), c_drag.camera().scale());
    Ok(())
}

/// Positive: zoom drag of N steps equals N clicks of click control and N keyboard presses.
#[test]
fn positive_drag_click_keyboard_equality_zoom() -> Fallible {
    let mut c_click = seed::grid(2, 2)?;
    let mut c_drag = c_click.clone();
    let mut c_key = c_click.clone();
    let mut model = FocusModel::new();

    for _ in 0..3 {
        click(&mut c_click, ClickControl::ZoomIn)?;
        drag(
            &mut c_drag,
            Operation::Zoom,
            None,
            DragDelta::Factor(ZOOM_FACTOR),
        )?;
        assert_eq!(
            model.press(&mut c_key, Key::Char('i'))?,
            KeyOutcome::Command(Command::ZoomIn)
        );
    }

    let diff_scale = (c_click.camera().scale() - c_drag.camera().scale()).abs();
    assert!(diff_scale < FLOAT_TOLERANCE, "scale diff {diff_scale}");
    assert_eq!(c_click.camera().center(), c_drag.camera().center());

    let key_diff_scale = (c_key.camera().scale() - c_drag.camera().scale()).abs();
    assert!(
        key_diff_scale < FLOAT_TOLERANCE,
        "key scale diff {key_diff_scale}"
    );
    assert_eq!(c_key.camera().center(), c_drag.camera().center());
    Ok(())
}

/// Positive: node move drag of N steps equals N clicks and N keyboard presses.
#[test]
fn positive_drag_click_keyboard_equality_node_move() -> Fallible {
    let mut c_click = seed::grid(2, 2)?;
    let mut c_drag = c_click.clone();
    let mut c_key = c_click.clone();
    let key = NodeKey(0);
    let mut model = FocusModel::new();
    model.focus_target(&mut c_key, Target::Node(key))?;

    for _ in 0..3 {
        click(&mut c_click, ClickControl::MoveRight(key))?;
        drag(
            &mut c_drag,
            Operation::NodeMove,
            Some(Target::Node(key)),
            DragDelta::Offset(GRID_STEP, 0.0),
        )?;
        assert_eq!(
            model.press(&mut c_key, Key::Char('d'))?,
            KeyOutcome::Command(Command::MoveRight)
        );
    }

    let rect_click = c_click.registry().get(key).ok_or("missing key")?.rect;
    let rect_drag = c_drag.registry().get(key).ok_or("missing key")?.rect;
    let rect_key = c_key.registry().get(key).ok_or("missing key")?.rect;
    assert_eq!(rect_click, rect_drag);
    assert_eq!(rect_key, rect_drag);
    Ok(())
}

/// Positive: node resize drag of N steps equals N clicks and N keyboard presses.
#[test]
fn positive_drag_click_keyboard_equality_node_resize() -> Fallible {
    let mut c_click = seed::grid(2, 2)?;
    let mut c_drag = c_click.clone();
    let mut c_key = c_click.clone();
    let key = NodeKey(0);
    let mut model = FocusModel::new();
    model.focus_target(&mut c_key, Target::Node(key))?;

    for _ in 0..3 {
        click(&mut c_click, ClickControl::ResizeGrow(key))?;
        drag(
            &mut c_drag,
            Operation::NodeResize,
            Some(Target::Node(key)),
            DragDelta::Offset(RESIZE_STEP, RESIZE_STEP),
        )?;
        assert_eq!(
            model.press(&mut c_key, Key::Char('g'))?,
            KeyOutcome::Command(Command::ResizeGrow)
        );
    }

    let rect_click = c_click.registry().get(key).ok_or("missing key")?.rect;
    let rect_drag = c_drag.registry().get(key).ok_or("missing key")?.rect;
    let rect_key = c_key.registry().get(key).ok_or("missing key")?.rect;
    assert_eq!(rect_click, rect_drag);
    assert_eq!(rect_key, rect_drag);
    Ok(())
}

/// Positive: handle rotate drag of N steps equals N clicks and N keyboard presses.
#[test]
fn positive_drag_click_keyboard_equality_handle_rotate() -> Fallible {
    let (mut c_click, target) = instrumented_node_canvas()?;
    let mut c_drag = c_click.clone();
    let mut c_key = c_click.clone();
    let mut model = FocusModel::new();
    model.focus_target(&mut c_key, target)?;

    for _ in 0..3 {
        click(&mut c_click, ClickControl::RotateCw(target))?;
        drag(
            &mut c_drag,
            Operation::HandleRotate,
            Some(target),
            DragDelta::Degrees(ROTATE_STEP_DEGREES),
        )?;
        assert_eq!(
            model.press(&mut c_key, Key::Char('r'))?,
            KeyOutcome::Command(Command::RotateCw)
        );
    }

    assert_eq!(c_click.action_states(), c_drag.action_states());
    assert_eq!(c_key.action_states(), c_drag.action_states());
    let states = c_click.action_states();
    let level_state = states
        .first()
        .and_then(|(_, _, levels)| levels.first())
        .ok_or("missing level state")?;
    assert_eq!(level_state.angle, 3 * ROTATE_STEP_DEGREES);
    Ok(())
}

/// Positive: node-bound commands with no focused node return `KeyOutcome::Ignored`.
#[test]
fn positive_node_bound_command_with_no_focused_node_ignored() -> Fallible {
    let mut canvas = seed::grid(2, 2)?;
    let mut model = FocusModel::new();
    assert_eq!(
        model.press(&mut canvas, Key::Char('d'))?,
        KeyOutcome::Ignored
    );
    assert_eq!(
        model.press(&mut canvas, Key::Char('g'))?,
        KeyOutcome::Ignored
    );
    assert_eq!(
        model.press(&mut canvas, Key::Char('r'))?,
        KeyOutcome::Ignored
    );
    Ok(())
}

/// Positive: single pointer release on node target fires activation exactly once.
#[test]
fn positive_release_on_target_activates_once() -> Fallible {
    let (mut canvas, key) = single_node_canvas(200.0, 200.0, 100.0, 100.0)?;
    let target = Target::Node(key);
    let mut tracker = PointerTracker::new();

    let node_before = canvas.registry().get(key).ok_or("node missing")?;
    assert!(!node_before.state.selected);

    tracker.press(PressTarget::Node(target), (250.0, 250.0));
    assert!(tracker.is_pressed());
    let node_after_down = canvas.registry().get(key).ok_or("node missing")?;
    assert!(!node_after_down.state.selected, "down must not activate");

    tracker.move_to((252.0, 251.0));
    assert_eq!(tracker.moves().len(), 1);

    let outcome = tracker.release(&mut canvas, (252.0, 251.0), None)?;
    assert_eq!(
        outcome,
        ActivationOutcome::Activated(PressTarget::Node(target))
    );
    assert!(!tracker.is_pressed());

    let node_after = canvas.registry().get(key).ok_or("node missing")?;
    assert!(node_after.state.selected, "release must activate");
    Ok(())
}

/// Positive: control press alone never changes state; press + release on it activates click once.
#[test]
fn positive_control_press_and_release_activates_once() -> Fallible {
    let mut canvas = seed::grid(2, 2)?;
    let before_cam = *canvas.camera();
    let mut tracker = PointerTracker::new();
    let ctrl_target = PressTarget::Control(ClickControl::PanRight);

    tracker.press(ctrl_target, (10.0, 10.0));
    assert!(tracker.is_pressed());
    assert_eq!(
        canvas.camera(),
        &before_cam,
        "press alone never changes state"
    );

    let outcome = tracker.release(&mut canvas, (10.0, 10.0), Some(ctrl_target))?;
    assert_eq!(outcome, ActivationOutcome::Activated(ctrl_target));
    assert!(!tracker.is_pressed());
    assert_eq!(
        canvas.camera().center().0,
        before_cam.center().0 + GRID_STEP
    );
    Ok(())
}

/// Positive: control press + release off target aborts with zero canvas side effects.
#[test]
fn positive_control_press_release_off_target_aborts() -> Fallible {
    let mut canvas = seed::grid(2, 2)?;
    let before_cam = *canvas.camera();
    let before_states = canvas.action_states();
    let before_rects: Vec<_> = canvas.registry().nodes().iter().map(|n| n.rect).collect();
    let mut tracker = PointerTracker::new();
    let ctrl_target = PressTarget::Control(ClickControl::PanRight);

    tracker.press(ctrl_target, (10.0, 10.0));
    let outcome = tracker.release(&mut canvas, (50.0, 50.0), None)?;
    assert_eq!(outcome, ActivationOutcome::Aborted);
    assert_eq!(canvas.camera(), &before_cam);
    assert_eq!(canvas.action_states(), before_states);
    let after_rects: Vec<_> = canvas.registry().nodes().iter().map(|n| n.rect).collect();
    assert_eq!(after_rects, before_rects);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: an operation specification with drag=true and no click controls fails.
#[test]
fn negative_handle_rotate_spec_drag_only_refused() {
    let spec = OperationSpec {
        op: Operation::HandleRotate,
        drag: true,
        click_controls: Vec::new(),
        keyboard: vec![Command::RotateCw, Command::RotateCcw],
    };
    let shortcuts = Shortcuts::default();
    assert_eq!(
        check_operations(&[spec], &shortcuts),
        Err(PointerError::DragOnly {
            op: Operation::HandleRotate,
        })
    );
}

/// Negative: an operation specification with drag=true and no keyboard commands fails.
#[test]
fn negative_spec_no_keyboard_refused() {
    let spec = OperationSpec {
        op: Operation::Pan,
        drag: true,
        click_controls: vec![ClickControlKind::PanLeft],
        keyboard: Vec::new(),
    };
    let shortcuts = Shortcuts::default();
    assert_eq!(
        check_operations(&[spec], &shortcuts),
        Err(PointerError::NoKeyboard { op: Operation::Pan })
    );
}

/// Negative: removing the rotate letter from `Shortcuts` refuses `check_operations` with `NoKeyboard`.
#[test]
fn negative_remove_rotate_letter_from_shortcuts_refuses_no_keyboard() {
    let specs = default_operation_specs();
    let mut shortcuts = Shortcuts::default();
    shortcuts.unbind(Command::RotateCw);
    assert_eq!(
        check_operations(&specs, &shortcuts),
        Err(PointerError::NoKeyboard {
            op: Operation::HandleRotate,
        })
    );
}

/// Negative: moving or resizing a pinned node is refused with Pinned error.
#[test]
fn negative_pinned_node_move_and_resize_refused() -> Fallible {
    let (mut canvas, key) = single_node_canvas(200.0, 200.0, 100.0, 100.0)?;
    canvas.apply(Target::Node(key), ActionKind::Pin)?;
    let node = canvas.registry().get(key).ok_or("node missing")?;
    assert!(node.state.pinned);

    let err_move = canvas.move_node(key, GRID_STEP, 0.0);
    assert_eq!(err_move, Err(CanvasError::Pinned { key: key.0 }));

    let err_resize = canvas.resize_node(key, RESIZE_STEP, RESIZE_STEP);
    assert_eq!(err_resize, Err(CanvasError::Pinned { key: key.0 }));

    let err_click = click(&mut canvas, ClickControl::MoveRight(key));
    assert_eq!(
        err_click,
        Err(PointerError::Canvas(CanvasError::Pinned { key: key.0 }))
    );
    Ok(())
}

/// Negative: a node below 24 CSS px with inert policy Forbidden reports Defect.
#[test]
fn negative_node_below_24px_inert_forbidden_reports_defect() -> Fallible {
    let (mut canvas, key) = single_node_canvas(200.0, 200.0, 15.0, 15.0)?;
    canvas.set_inert_policy(InertPolicy::Forbidden);

    let report = target_report(&canvas);
    assert_eq!(report.len(), 1);
    assert_eq!(report, vec![(key, TargetStatus::Defect)]);
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: at `MIN_SCALE` (0.125), all seeded nodes are Ok or Inert-with-equivalent, none Defect.
#[test]
fn boundary_min_scale_seeded_canvas_report() -> Fallible {
    let mut canvas = seed::seeded()?;
    canvas.camera_mut().zoom_by(MIN_SCALE);
    assert_eq!(canvas.camera().scale(), MIN_SCALE);

    let report = target_report(&canvas);
    assert_eq!(report.len(), 1000);
    for (key, status) in report {
        match status {
            TargetStatus::Ok | TargetStatus::Inert { equivalent: true } => {}
            _ => return Err(format!("node {key:?} reported defect {status:?}").into()),
        }
    }
    Ok(())
}

/// Boundary: node exactly 24.0 CSS px is Ok, while 23.9 CSS px is Inert and hit-test skips it.
#[test]
fn boundary_exact_24_vs_23_9_css_px() -> Fallible {
    let vp = Viewport::new(500.0, 500.0)?;
    let cam = Camera::new((250.0, 250.0), 1.0, vp)?;
    let mut canvas = Canvas::new(WorldRect::new(-1000.0, -1000.0, 2000.0, 2000.0)?, cam);

    let key_ok = NodeKey(1);
    canvas.push(CanvasNode {
        key: key_ok,
        label: "Node Exactly 24".to_owned(),
        kind: NodeKind::Application,
        rect: WorldRect::from_origin_size(100.0, 100.0, MIN_TARGET_CSS_PX, MIN_TARGET_CSS_PX)?,
        actions: vec![NodeAction::both(ActionKind::Select, Binding::Space)],
        levels: Vec::new(),
        state: ActionState::default(),
    })?;

    let key_inert = NodeKey(2);
    canvas.push(CanvasNode {
        key: key_inert,
        label: "Node 23.9".to_owned(),
        kind: NodeKind::Application,
        rect: WorldRect::from_origin_size(300.0, 300.0, 23.9, 23.9)?,
        actions: vec![NodeAction::both(ActionKind::Select, Binding::Space)],
        levels: Vec::new(),
        state: ActionState::default(),
    })?;

    let report = target_report(&canvas);
    assert_eq!(
        report,
        vec![
            (key_ok, TargetStatus::Ok),
            (key_inert, TargetStatus::Inert { equivalent: true }),
        ]
    );

    // Hit-testing key_ok center succeeds.
    let hit_ok = canvas.hit_test((112.0, 112.0));
    assert_eq!(hit_ok, Some(Target::Node(key_ok)));

    // Hit-testing key_inert center skips the node because it is inert.
    let hit_inert = canvas.hit_test((311.95, 311.95));
    assert_eq!(hit_inert, None, "hit test must skip inert nodes");
    Ok(())
}

/// Boundary: pointer press on node, move off target, and release aborts with zero canvas side effects.
#[test]
fn boundary_press_move_off_target_release_aborts() -> Fallible {
    let (mut canvas, key) = single_node_canvas(200.0, 200.0, 100.0, 100.0)?;
    let target = Target::Node(key);
    let before_states = canvas.action_states();
    let mut tracker = PointerTracker::new();

    tracker.press(PressTarget::Node(target), (250.0, 250.0));
    tracker.move_to((400.0, 400.0));

    let outcome = tracker.release(&mut canvas, (400.0, 400.0), None)?;
    assert_eq!(outcome, ActivationOutcome::Aborted);
    assert_eq!(canvas.action_states(), before_states);

    let node = canvas.registry().get(key).ok_or("node missing")?;
    assert!(!node.state.selected, "aborted gesture must not select");
    Ok(())
}

/// Boundary: handle rotation wraps around 0..360 boundaries.
#[test]
fn boundary_rotate_wrapping() -> Fallible {
    let (mut canvas, target) = instrumented_node_canvas()?;

    // Rotate by 345 degrees.
    canvas.rotate_handle(target, 345)?;
    let states = canvas.action_states();
    let angle = states
        .first()
        .and_then(|(_, _, levels)| levels.first())
        .map(|s| s.angle)
        .ok_or("missing level state")?;
    assert_eq!(angle, 345);

    // 345 + 15 wraps to 0.
    canvas.rotate_handle(target, 15)?;
    let states = canvas.action_states();
    let angle = states
        .first()
        .and_then(|(_, _, levels)| levels.first())
        .map(|s| s.angle)
        .ok_or("missing level state")?;
    assert_eq!(angle, 0);

    // 0 - 15 wraps to 345.
    canvas.rotate_handle(target, -15)?;
    let states = canvas.action_states();
    let angle = states
        .first()
        .and_then(|(_, _, levels)| levels.first())
        .map(|s| s.angle)
        .ok_or("missing level state")?;
    assert_eq!(angle, 345);
    Ok(())
}
