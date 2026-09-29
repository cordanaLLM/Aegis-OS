// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E16-3's keyboard half (REQ-P05-09, REQ-P05-10): a keyboard-only walk over
//! the model.
//!
//! Positive: the walk reaches every node and invokes every action, ending in
//! the same state as the pointer path; from each instrument Escape restores
//! focus to its owner and Tab reaches the next shell region. Negative: an
//! instrument with only a pointer action fails the walk, a transcluded
//! fragment that swallows Tab fails, a binding that runs another action
//! fails, and a node the camera cannot bring into view fails. Boundary:
//! Escape from the deepest nested instrument returns focus to canvas level
//! in exactly that many presses, a disabled single-letter shortcut no longer
//! fires, and a node exactly as wide as the viewport shows at the smallest
//! scale is revealed while one a unit wider is not.

mod common;

use aegis_forum_shell::canvas::camera::MIN_SCALE;
use aegis_forum_shell::canvas::geometry::WorldRect;
use aegis_forum_shell::canvas::registry::{
    ActionKind, Binding, CanvasNode, InstrumentLevel, NodeAction, NodeKey, TabPolicy,
};
use aegis_forum_shell::canvas::{Canvas, Target, seed};
use aegis_forum_shell::focus::{Command, Focus, FocusModel, Key, KeyOutcome, Region};
use aegis_forum_shell::walk::{WalkError, keyboard_walk, pointer_walk};

use common::Fallible;

/// A 10 by 20 seeded canvas: 200 nodes, two of them with a three-level
/// instrument (nodes 100 and 200).
fn small() -> Fallible<Canvas> {
    Ok(seed::grid(10, 20)?)
}

/// The node the planted defects are placed on: the 43rd in focus order, in
/// the fifth row, which the walk reaches only after the camera has moved.
const PLANTED: NodeKey = NodeKey(42);

/// The small canvas with `edit` applied to the node under `key`, through a
/// rebuilt canvas, since the registry is append-only. The cull index covers
/// room to the right of the grid for a widened node.
fn with_node(key: NodeKey, edit: impl Fn(&mut CanvasNode) -> Fallible) -> Fallible<Canvas> {
    let source = small()?;
    let mut canvas = Canvas::new(
        WorldRect::new(-200.0, -200.0, 12_200.0, 4_200.0)?,
        *source.camera(),
    );
    for node in source.registry().nodes() {
        let mut node = node.clone();
        if node.key == key {
            edit(&mut node)?;
        }
        canvas.push(node)?;
    }
    Ok(canvas)
}

/// Replaces node 100's level `depth` with `edit` applied.
fn edited(depth: usize, edit: impl Fn(&mut InstrumentLevel)) -> Fallible<Canvas> {
    with_node(NodeKey(99), |node| {
        let level = node
            .levels
            .get_mut(depth)
            .ok_or("node 100 has no such level")?;
        edit(level);
        Ok(())
    })
}

/// The small canvas with the planted node widened to `width` canvas units.
fn widened(width: f64) -> Fallible<Canvas> {
    with_node(PLANTED, |node| {
        node.rect = WorldRect::from_origin_size(node.rect.x0(), node.rect.y0(), width, 100.0)?;
        Ok(())
    })
}

/// The widest node the seeded viewport shows whole: its width at
/// [`MIN_SCALE`], in canvas units.
fn widest_revealable() -> f64 {
    seed::SEED_VIEWPORT.0 / MIN_SCALE
}

/// Presses `key` and returns where focus is after it.
fn press(model: &mut FocusModel, canvas: &mut Canvas, key: Key) -> Fallible<Focus> {
    model.press(canvas, key)?;
    Ok(model.focus())
}

// --- Positive -------------------------------------------------------------

/// Positive: the walk reaches every node and invokes every action, and ends
/// in the same state as the pointer path.
#[test]
fn the_keyboard_walk_matches_the_pointer_path() -> Fallible {
    let mut by_keyboard = small()?;
    let mut by_pointer = small()?;
    let report = keyboard_walk(&mut by_keyboard)?;
    let clicks = pointer_walk(&mut by_pointer)?;
    assert_eq!(report.nodes_reached, 200);
    assert_eq!(report.actions_invoked, clicks);
    assert_eq!(
        clicks,
        200 * 2 + 2 * 3 * 2,
        "two per node, two per instrument level"
    );
    assert_eq!(report.levels_opened, 6);
    assert_eq!(report.escapes, 6);
    assert_eq!(report.tab_exits, 6);
    assert!(
        report.camera_moves > 0,
        "the camera followed focus off the first view"
    );
    assert_eq!(by_keyboard.action_states(), by_pointer.action_states());
    assert_ne!(
        by_keyboard.action_states(),
        small()?.action_states(),
        "the walk changed state"
    );
    Ok(())
}

/// Positive: the walk also covers the 1,000-node seeded canvas.
#[test]
fn the_keyboard_walk_covers_the_seeded_canvas() -> Fallible {
    let mut canvas = seed::seeded()?;
    let report = keyboard_walk(&mut canvas)?;
    assert_eq!(report.nodes_reached, 1_000);
    assert_eq!(report.levels_opened, 30, "ten nodes with three levels each");
    Ok(())
}

/// Positive: from each instrument level Escape restores focus to its owner,
/// and Tab reaches the next shell region, from which Shift+Tab comes back.
#[test]
fn escape_and_tab_leave_every_instrument_level() -> Fallible {
    let mut canvas = small()?;
    let mut model = FocusModel::new();
    let node = NodeKey(99);
    model.focus_node(&mut canvas, node)?;
    for depth in 0..3 {
        for opened in 0..=depth {
            press(&mut model, &mut canvas, Key::Enter)?;
            assert_eq!(model.focus(), Focus::Target(Target::Level(node, opened)));
        }
        let owner = depth
            .checked_sub(1)
            .map_or(Target::Node(node), |outer| Target::Level(node, outer));
        assert_eq!(
            press(&mut model, &mut canvas, Key::Escape)?,
            Focus::Target(owner)
        );
        model.focus_node(&mut canvas, node)?;
        for _ in 0..=depth {
            press(&mut model, &mut canvas, Key::Enter)?;
        }
        assert_eq!(
            press(&mut model, &mut canvas, Key::Tab)?,
            Focus::Region(Region::Decisions)
        );
        assert_eq!(
            press(&mut model, &mut canvas, Key::ShiftTab)?,
            Focus::Target(Target::Node(node))
        );
    }
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: an instrument level whose action only a pointer reaches fails
/// the walk, and the failure names the level and the action.
#[test]
fn an_instrument_with_a_pointer_only_action_fails_the_walk() -> Fallible {
    let mut canvas = edited(1, |level| {
        level
            .actions
            .push(NodeAction::pointer_only(ActionKind::Pin));
    })?;
    let refused = keyboard_walk(&mut canvas);
    assert_eq!(
        refused,
        Err(WalkError::PointerOnly {
            target: Target::Level(NodeKey(99), 1),
            action: ActionKind::Pin
        })
    );
    let message = refused
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    assert!(
        message.contains("Level(NodeKey(99), 1)") && message.contains("Pin"),
        "{message}"
    );
    Ok(())
}

/// Negative: a transcluded fragment that swallows Tab fails the walk as a
/// focus trap, naming the fragment.
#[test]
fn a_fragment_that_swallows_tab_fails_the_walk() -> Fallible {
    let mut canvas = edited(2, |level| level.tab = TabPolicy::Swallows)?;
    assert_eq!(
        keyboard_walk(&mut canvas),
        Err(WalkError::Trapped {
            target: Target::Level(NodeKey(99), 2)
        })
    );
    Ok(())
}

/// Negative: a keyboard-only action (no pointer) makes the two paths end in
/// different states, which the comparison catches.
#[test]
fn a_keyboard_only_action_breaks_the_state_comparison() -> Fallible {
    let mut by_keyboard = edited(0, |level| {
        if let Some(action) = level.actions.first_mut() {
            action.pointer = false;
        }
    })?;
    let mut by_pointer = by_keyboard.clone();
    keyboard_walk(&mut by_keyboard)?;
    pointer_walk(&mut by_pointer)?;
    assert_ne!(by_keyboard.action_states(), by_pointer.action_states());
    Ok(())
}

/// Negative: a node whose second action shares the first one's binding runs
/// the first action again when the walk presses it, and the walk fails,
/// naming the node and the action the binding never ran.
#[test]
fn a_binding_that_runs_another_action_fails_the_walk() -> Fallible {
    let mut canvas = with_node(PLANTED, |node| {
        node.actions = vec![
            NodeAction::both(ActionKind::Select, Binding::Space),
            NodeAction::both(ActionKind::Pin, Binding::Space),
        ];
        Ok(())
    })?;
    let refused = keyboard_walk(&mut canvas);
    assert_eq!(
        refused,
        Err(WalkError::NotInvoked {
            target: Target::Node(PLANTED),
            action: ActionKind::Pin
        })
    );
    let message = refused
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    assert!(
        message.contains("Node(NodeKey(42))") && message.contains("Pin"),
        "{message}"
    );
    Ok(())
}

/// Negative: a node wider than the viewport shows at the smallest camera
/// scale cannot be brought into view whole, and the walk fails there,
/// naming the node, after the camera zoomed out as far as it goes.
#[test]
fn a_node_the_camera_cannot_reveal_fails_the_walk() -> Fallible {
    let mut canvas = widened(widest_revealable() + 1.0)?;
    let refused = keyboard_walk(&mut canvas);
    assert_eq!(refused, Err(WalkError::NotRevealed { key: PLANTED.0 }));
    assert!(
        (canvas.camera().scale() - MIN_SCALE).abs() < f64::EPSILON,
        "the camera zoomed out to its limit first"
    );
    assert!(!canvas.fully_in_view(PLANTED)?);
    let message = refused
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    assert!(message.contains("canvas node 42"), "{message}");
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: from the deepest level, three deep, focus is back at canvas
/// level after exactly three Escape presses and not after two; a fourth
/// changes nothing.
#[test]
fn escape_from_the_deepest_level_takes_exactly_that_many_presses() -> Fallible {
    let mut canvas = small()?;
    let mut model = FocusModel::new();
    let node = NodeKey(99);
    model.focus_node(&mut canvas, node)?;
    for _ in 0..3 {
        press(&mut model, &mut canvas, Key::Enter)?;
    }
    assert_eq!(model.focus(), Focus::Target(Target::Level(node, 2)));
    assert_eq!(
        press(&mut model, &mut canvas, Key::Enter)?,
        Focus::Target(Target::Level(node, 2)),
        "no fourth level"
    );
    press(&mut model, &mut canvas, Key::Escape)?;
    assert_eq!(
        press(&mut model, &mut canvas, Key::Escape)?,
        Focus::Target(Target::Level(node, 0)),
        "two presses"
    );
    assert_eq!(
        press(&mut model, &mut canvas, Key::Escape)?,
        Focus::Target(Target::Node(node)),
        "three presses"
    );
    assert_eq!(model.press(&mut canvas, Key::Escape)?, KeyOutcome::Ignored);
    assert_eq!(model.focus(), Focus::Target(Target::Node(node)));
    Ok(())
}

/// Boundary: a single-letter shortcut fires while enabled, no longer fires
/// once disabled, fires again when re-enabled, and fires only under its new
/// letter once remapped.
#[test]
fn a_disabled_single_letter_shortcut_no_longer_fires() -> Fallible {
    let mut canvas = seed::seeded()?;
    let mut model = FocusModel::new();
    model.focus_node(&mut canvas, NodeKey(0))?;
    let scale = canvas.camera().scale();
    assert_eq!(
        model.press(&mut canvas, Key::Char('i'))?,
        KeyOutcome::Command(Command::ZoomIn)
    );
    assert!(canvas.camera().scale() > scale);
    model.shortcuts_mut().set_enabled(Command::ZoomIn, false);
    let zoomed = canvas.camera().scale();
    assert_eq!(
        model.press(&mut canvas, Key::Char('i'))?,
        KeyOutcome::Ignored
    );
    assert!(
        (canvas.camera().scale() - zoomed).abs() < f64::EPSILON,
        "a disabled shortcut changes nothing"
    );
    model.shortcuts_mut().set_enabled(Command::ZoomIn, true);
    model.shortcuts_mut().remap(Command::ZoomIn, 'z')?;
    assert_eq!(
        model.press(&mut canvas, Key::Char('i'))?,
        KeyOutcome::Ignored
    );
    assert_eq!(
        model.press(&mut canvas, Key::Char('z'))?,
        KeyOutcome::Command(Command::ZoomIn)
    );
    assert!(
        model.shortcuts_mut().remap(Command::ZoomOut, 'z').is_err(),
        "z is taken"
    );
    assert!(
        model.shortcuts_mut().remap(Command::ZoomOut, '1').is_err(),
        "not a letter"
    );
    Ok(())
}

/// Boundary: a node exactly as wide as the viewport shows at the smallest
/// scale is revealed whole, and the walk over it completes.
#[test]
fn a_node_exactly_as_wide_as_the_zoomed_out_view_is_revealed() -> Fallible {
    let mut canvas = widened(widest_revealable())?;
    let report = keyboard_walk(&mut canvas)?;
    assert_eq!(report.nodes_reached, 200);
    assert!(
        (canvas.camera().scale() - MIN_SCALE).abs() < f64::EPSILON,
        "revealing the node took the smallest scale"
    );
    Ok(())
}

/// Boundary: an empty canvas is walked by landing on the canvas region
/// itself and Tabbing on.
#[test]
fn the_walk_over_an_empty_canvas_lands_on_the_region() -> Fallible {
    let mut canvas = seed::empty()?;
    let report = keyboard_walk(&mut canvas)?;
    assert_eq!(report.nodes_reached, 0);
    let mut model = FocusModel::new();
    assert_eq!(
        press(&mut model, &mut canvas, Key::Tab)?,
        Focus::Region(Region::Canvas)
    );
    assert_eq!(
        press(&mut model, &mut canvas, Key::Tab)?,
        Focus::Region(Region::Decisions)
    );
    assert_eq!(
        press(&mut model, &mut canvas, Key::Tab)?,
        Focus::Region(Region::StatusBar)
    );
    Ok(())
}
