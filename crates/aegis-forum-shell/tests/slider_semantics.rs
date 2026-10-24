// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! M30 E30-1 part A (REQ-P05-12): slider semantics for instrument handles.
//!
//! Positive: a slidable instrument level exports `Role::Slider`, its label,
//! value, minimum -8 and maximum 8; an Increment action advances the value
//! by one on re-export. A level without increment or decrement actions
//! remains a `Role::Group`.
//! Negative: a slider with its role stripped fails the tree check as `NoRole`;
//! a slider missing its maximum numeric value fails with `MissingNumericValue`.
//! Boundary: values at minimum -8 and maximum 8 pass; Increment at 8 stays 8;
//! Decrement at -8 stays -8; an out-of-range value (such as 9) fails with
//! `OutOfRange`.

mod common;

use accesskit::{Node, NodeId, Role, TreeUpdate};
use aegis_forum_shell::a11y::{TreeError, check_tree, export, level_id};
use aegis_forum_shell::canvas::geometry::WorldRect;
use aegis_forum_shell::canvas::registry::{
    ActionKind, ActionState, Binding, CanvasNode, InstrumentLevel, LevelKind, MAX_VALUE,
    NodeAction, NodeKey, NodeKind, TabPolicy,
};
use aegis_forum_shell::canvas::{Canvas, Target, seed};
use aegis_forum_shell::focus::Key;
use aegis_forum_shell::state::ShellState;

use common::Fallible;

/// Finds a node in `update` by `id`.
fn find_node(update: &TreeUpdate, id: NodeId) -> Fallible<&Node> {
    update
        .nodes
        .iter()
        .find(|(candidate, _)| *candidate == id)
        .map(|(_, node)| node)
        .ok_or_else(|| format!("node {id:?} missing from tree update").into())
}

/// Finds a mutable node in `update` by `id`.
fn find_node_mut(update: &mut TreeUpdate, id: NodeId) -> Fallible<&mut Node> {
    update
        .nodes
        .iter_mut()
        .find(|(candidate, _)| *candidate == id)
        .map(|(_, node)| node)
        .ok_or_else(|| format!("node {id:?} missing from tree update").into())
}

/// Opens the first instrument level on node 99 of a seeded shell.
fn open_seeded_slider() -> Fallible<(ShellState, NodeId)> {
    let mut state = ShellState::new(seed::seeded()?);
    state.focus.focus_node(&mut state.canvas, NodeKey(99))?;
    state.focus.press(&mut state.canvas, Key::Enter)?;
    let id = level_id(NodeKey(99), 0);
    Ok((state, id))
}

/// A constructed shell with a single node carrying `level`.
fn constructed_shell(level: InstrumentLevel) -> Fallible<(ShellState, NodeId)> {
    let mut canvas = Canvas::new(
        WorldRect::from_origin_size(0.0, 0.0, 1000.0, 1000.0)?,
        seed::seed_camera()?,
    );
    canvas.push(CanvasNode {
        key: NodeKey(0),
        label: "Constructed Node".to_owned(),
        kind: NodeKind::Application,
        rect: WorldRect::from_origin_size(10.0, 10.0, 100.0, 50.0)?,
        actions: vec![NodeAction::both(ActionKind::Select, Binding::Space)],
        levels: vec![level],
        state: ActionState::default(),
    })?;
    let mut state = ShellState::new(canvas);
    state.focus.focus_node(&mut state.canvas, NodeKey(0))?;
    state.focus.press(&mut state.canvas, Key::Enter)?;
    let id = level_id(NodeKey(0), 0);
    Ok((state, id))
}

// --- Positive -------------------------------------------------------------

/// Positive: a seeded slidable level exports `Role::Slider`, its label,
/// value 0, min -8, max 8 and step 1.0; Increment via `ActionState::apply`
/// advances value to 1 and re-export presents value+1.
#[test]
fn seeded_slidable_level_exports_slider_and_notifies_increment() -> Fallible {
    let (mut state, id) = open_seeded_slider()?;
    let update = export(&state);
    check_tree(&update)?;
    let node = find_node(&update, id)?;

    assert_eq!(node.role(), Role::Slider);
    assert_eq!(node.label(), Some("Settings instrument for node 100"));
    assert_eq!(node.role_description(), Some("instrument"));
    assert_eq!(node.numeric_value(), Some(0.0));
    assert_eq!(node.min_numeric_value(), Some(f64::from(-MAX_VALUE)));
    assert_eq!(node.max_numeric_value(), Some(f64::from(MAX_VALUE)));
    assert_eq!(node.numeric_value_step(), Some(1.0));

    state
        .canvas
        .apply(Target::Level(NodeKey(99), 0), ActionKind::Increment)?;
    let update_after = export(&state);
    check_tree(&update_after)?;
    let node_after = find_node(&update_after, id)?;
    assert_eq!(node_after.numeric_value(), Some(1.0));

    Ok(())
}

/// Positive: a constructed slidable level exports `Role::Slider`, its label,
/// and configured state.
#[test]
fn constructed_slidable_level_exports_slider() -> Fallible {
    let slidable_level = InstrumentLevel {
        label: "Volume slider".to_owned(),
        kind: LevelKind::Instrument,
        actions: vec![
            NodeAction::both(ActionKind::Increment, Binding::Ctrl('=')),
            NodeAction::both(ActionKind::Decrement, Binding::Ctrl('-')),
        ],
        tab: TabPolicy::Leaves,
        state: ActionState {
            selected: false,
            pinned: false,
            value: 3,
        },
    };
    let (state, id) = constructed_shell(slidable_level)?;
    let update = export(&state);
    check_tree(&update)?;
    let node = find_node(&update, id)?;

    assert_eq!(node.role(), Role::Slider);
    assert_eq!(node.label(), Some("Volume slider"));
    assert_eq!(node.role_description(), Some("instrument"));
    assert_eq!(node.numeric_value(), Some(3.0));
    assert_eq!(node.min_numeric_value(), Some(f64::from(-MAX_VALUE)));
    assert_eq!(node.max_numeric_value(), Some(f64::from(MAX_VALUE)));

    Ok(())
}

/// Positive: a surrogate level inside an instrument is also slidable and
/// exports `Role::Slider` with role description "surrogate".
#[test]
fn surrogate_slidable_level_exports_slider() -> Fallible {
    let (mut state, _) = open_seeded_slider()?;
    state.focus.press(&mut state.canvas, Key::Enter)?;
    let surrogate_id = level_id(NodeKey(99), 1);
    let update = export(&state);
    check_tree(&update)?;
    let node = find_node(&update, surrogate_id)?;

    assert_eq!(node.role(), Role::Slider);
    assert_eq!(node.role_description(), Some("surrogate"));
    assert_eq!(node.numeric_value(), Some(0.0));
    assert_eq!(node.min_numeric_value(), Some(f64::from(-MAX_VALUE)));
    assert_eq!(node.max_numeric_value(), Some(f64::from(MAX_VALUE)));

    Ok(())
}

/// Positive: an instrument level without increment or decrement actions
/// remains `Role::Group` with no numeric range.
#[test]
fn level_without_increment_or_decrement_remains_group() -> Fallible {
    let group_level = InstrumentLevel {
        label: "Read-only summary".to_owned(),
        kind: LevelKind::Instrument,
        actions: vec![NodeAction::both(ActionKind::Select, Binding::Space)],
        tab: TabPolicy::Leaves,
        state: ActionState::default(),
    };
    let (state, id) = constructed_shell(group_level)?;
    let update = export(&state);
    check_tree(&update)?;
    let node = find_node(&update, id)?;

    assert_eq!(node.role(), Role::Group);
    assert_eq!(node.numeric_value(), None);
    assert_eq!(node.min_numeric_value(), None);
    assert_eq!(node.max_numeric_value(), None);

    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a slider whose role is stripped fails the tree check as `NoRole`.
#[test]
fn slider_with_stripped_role_fails_as_no_role() -> Fallible {
    let (state, id) = open_seeded_slider()?;
    let mut update = export(&state);
    let node = find_node_mut(&mut update, id)?;
    node.set_role(Role::Unknown);

    assert_eq!(
        check_tree(&update),
        Err(TreeError::NoRole {
            id,
            role: Role::Unknown,
        })
    );
    Ok(())
}

/// Negative: a slider missing its max numeric value fails with `MissingNumericValue`.
#[test]
fn slider_missing_max_fails() -> Fallible {
    let (state, id) = open_seeded_slider()?;
    let mut update = export(&state);
    let node = find_node_mut(&mut update, id)?;
    node.clear_max_numeric_value();

    assert_eq!(
        check_tree(&update),
        Err(TreeError::MissingNumericValue {
            id,
            role: Role::Slider,
        })
    );
    Ok(())
}

/// Negative: a slider missing min or current value also fails with `MissingNumericValue`.
#[test]
fn slider_missing_min_or_value_fails() -> Fallible {
    let (state, id) = open_seeded_slider()?;

    let mut update_min = export(&state);
    find_node_mut(&mut update_min, id)?.clear_min_numeric_value();
    assert_eq!(
        check_tree(&update_min),
        Err(TreeError::MissingNumericValue {
            id,
            role: Role::Slider,
        })
    );

    let mut update_val = export(&state);
    find_node_mut(&mut update_val, id)?.clear_numeric_value();
    assert_eq!(
        check_tree(&update_val),
        Err(TreeError::MissingNumericValue {
            id,
            role: Role::Slider,
        })
    );

    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: values at minimum -8 and maximum 8 pass; Increment at 8 stays 8;
/// Decrement at -8 stays -8.
#[test]
fn boundary_values_and_clamping_hold() -> Fallible {
    let (mut state, id) = open_seeded_slider()?;

    // Drive value to max (8)
    for _ in 0..10 {
        state
            .canvas
            .apply(Target::Level(NodeKey(99), 0), ActionKind::Increment)?;
    }
    let update_max = export(&state);
    check_tree(&update_max)?;
    assert_eq!(
        find_node(&update_max, id)?.numeric_value(),
        Some(f64::from(MAX_VALUE))
    );

    // Increment at 8 stays 8
    state
        .canvas
        .apply(Target::Level(NodeKey(99), 0), ActionKind::Increment)?;
    let update_clamped_max = export(&state);
    check_tree(&update_clamped_max)?;
    assert_eq!(
        find_node(&update_clamped_max, id)?.numeric_value(),
        Some(f64::from(MAX_VALUE))
    );

    // Drive value to min (-8)
    for _ in 0..20 {
        state
            .canvas
            .apply(Target::Level(NodeKey(99), 0), ActionKind::Decrement)?;
    }
    let update_min = export(&state);
    check_tree(&update_min)?;
    assert_eq!(
        find_node(&update_min, id)?.numeric_value(),
        Some(f64::from(-MAX_VALUE))
    );

    // Decrement at -8 stays -8
    state
        .canvas
        .apply(Target::Level(NodeKey(99), 0), ActionKind::Decrement)?;
    let update_clamped_min = export(&state);
    check_tree(&update_clamped_min)?;
    assert_eq!(
        find_node(&update_clamped_min, id)?.numeric_value(),
        Some(f64::from(-MAX_VALUE))
    );

    Ok(())
}

/// Boundary: forcing a value outside [min, max] (e.g. 9 or -9) fails with `OutOfRange`;
/// an inverted range (min > max) fails with `InvertedRange`.
#[test]
fn boundary_forced_out_of_range_and_inverted_range_fail() -> Fallible {
    let (state, id) = open_seeded_slider()?;

    // Value 9 forced -> out-of-range error
    let mut update_high = export(&state);
    find_node_mut(&mut update_high, id)?.set_numeric_value(9.0);
    assert_eq!(
        check_tree(&update_high),
        Err(TreeError::OutOfRange {
            id,
            role: Role::Slider,
            value: 9.0,
            min: f64::from(-MAX_VALUE),
            max: f64::from(MAX_VALUE),
        })
    );

    // Value -9 forced -> out-of-range error
    let mut update_low = export(&state);
    find_node_mut(&mut update_low, id)?.set_numeric_value(-9.0);
    assert_eq!(
        check_tree(&update_low),
        Err(TreeError::OutOfRange {
            id,
            role: Role::Slider,
            value: -9.0,
            min: f64::from(-MAX_VALUE),
            max: f64::from(MAX_VALUE),
        })
    );

    // Inverted range: min > max
    let mut update_inverted = export(&state);
    let node = find_node_mut(&mut update_inverted, id)?;
    node.set_min_numeric_value(10.0);
    node.set_max_numeric_value(5.0);
    assert_eq!(
        check_tree(&update_inverted),
        Err(TreeError::InvertedRange {
            id,
            role: Role::Slider,
            min: 10.0,
            max: 5.0,
        })
    );

    Ok(())
}
