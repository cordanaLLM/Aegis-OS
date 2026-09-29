// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The keyboard-only walk and the pointer path it is compared with (E16-3,
//! REQ-P05-09, REQ-P05-10).
//!
//! [`keyboard_walk`] drives a [`FocusModel`] over a canvas with key presses
//! only. It Tabs into the canvas, visits every node in focus order with the
//! Next key, checks that the camera brought each one into view, and presses
//! every action's binding once. On a node with an instrument it opens every
//! nested level with Enter, presses every level's actions, climbs back out
//! with Escape and counts the presses, and then, from each level in turn,
//! checks that Tab leaves the canvas for the next shell region and Shift+Tab
//! comes back to the node. It ends by Tabbing out of the canvas.
//!
//! It fails, naming the target, on a node it does not reach, a node the
//! camera leaves out of view, an action with no keyboard binding, a binding
//! that runs nothing, a level that swallows Tab, and an Escape count that is
//! not the nesting depth.
//!
//! [`pointer_walk`] invokes every pointer-reachable action once, as clicks
//! would. Each walk invokes every action exactly once, so a canvas walked by
//! keyboard and a copy walked by pointer end in the same
//! [`crate::canvas::Canvas::action_states`].

use thiserror::Error;

use crate::canvas::registry::{Binding, NodeAction, NodeKey};
use crate::canvas::{Canvas, CanvasError, Target};
use crate::focus::{Focus, FocusError, FocusModel, Key, KeyOutcome, Region};

/// Why the keyboard walk failed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WalkError {
    /// A key press was refused by the model.
    #[error(transparent)]
    Focus(#[from] FocusError),
    /// The canvas refused an operation.
    #[error(transparent)]
    Canvas(#[from] CanvasError),
    /// Focus was not where the walk expected it.
    #[error("expected focus on {expected:?}, found it on {found:?}")]
    Unreached {
        /// Where focus should have been.
        expected: Focus,
        /// Where it was.
        found: Focus,
    },
    /// The camera left a focused node out of view.
    #[error("canvas node {key} has focus but is not in view")]
    NotRevealed {
        /// The node concerned.
        key: u32,
    },
    /// An action has no keyboard binding: only a pointer reaches it.
    #[error("{target:?} offers {action:?} to the pointer only")]
    PointerOnly {
        /// The node or level carrying the action.
        target: Target,
        /// The action.
        action: crate::canvas::registry::ActionKind,
    },
    /// A binding was pressed and did not run its action.
    #[error("{target:?}: the binding for {action:?} did not run it")]
    NotInvoked {
        /// The node or level carrying the action.
        target: Target,
        /// The action.
        action: crate::canvas::registry::ActionKind,
    },
    /// Tab or Shift+Tab did not leave the canvas from an instrument level.
    #[error("{target:?} traps focus: Tab did not leave the canvas")]
    Trapped {
        /// The level that kept focus.
        target: Target,
    },
}

/// What the keyboard walk did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WalkReport {
    /// Nodes focused, in focus order.
    pub nodes_reached: usize,
    /// Actions run by a key press.
    pub actions_invoked: usize,
    /// Instrument levels opened with Enter.
    pub levels_opened: usize,
    /// Escape presses, each returning focus to its owner.
    pub escapes: usize,
    /// Levels Tab left the canvas from.
    pub tab_exits: usize,
    /// Focus moves after which the camera had moved.
    pub camera_moves: usize,
}

/// Presses `key` and requires focus to land on `expected`.
fn press_to(
    model: &mut FocusModel,
    canvas: &mut Canvas,
    key: Key,
    expected: Focus,
) -> Result<(), WalkError> {
    model.press(canvas, key)?;
    if model.focus() == expected {
        return Ok(());
    }
    if let (Focus::Target(target @ Target::Level(..)), Key::Tab | Key::ShiftTab) =
        (model.focus(), key)
    {
        return Err(WalkError::Trapped { target });
    }
    Err(WalkError::Unreached {
        expected,
        found: model.focus(),
    })
}

/// The key a binding is pressed with.
const fn key_for(binding: Binding) -> Key {
    match binding {
        Binding::Space => Key::Space,
        Binding::Ctrl(letter) => Key::Ctrl(letter),
    }
}

/// Presses every action's binding on the focused `target` once.
fn invoke_all(
    model: &mut FocusModel,
    canvas: &mut Canvas,
    target: Target,
    actions: &[NodeAction],
) -> Result<usize, WalkError> {
    for action in actions {
        let binding = action.keyboard.ok_or(WalkError::PointerOnly {
            target,
            action: action.kind,
        })?;
        let outcome = model.press(canvas, key_for(binding))?;
        if outcome != KeyOutcome::Invoked(target, action.kind) {
            return Err(WalkError::NotInvoked {
                target,
                action: action.kind,
            });
        }
    }
    Ok(actions.len())
}

/// The actions of `target`, copied out so the canvas can be borrowed again.
fn actions_of(canvas: &Canvas, target: Target) -> Vec<NodeAction> {
    let registry = canvas.registry();
    match target {
        Target::Node(key) => registry.get(key).map(|node| node.actions.clone()),
        Target::Level(key, depth) => registry
            .get(key)
            .and_then(|node| node.levels.get(depth))
            .map(|level| level.actions.clone()),
    }
    .unwrap_or_default()
}

/// Opens every level of `key`'s instrument, runs each level's actions, and
/// climbs back out with exactly `depth` Escape presses.
fn descend_and_escape(
    model: &mut FocusModel,
    canvas: &mut Canvas,
    key: NodeKey,
    depth: usize,
    report: &mut WalkReport,
) -> Result<(), WalkError> {
    for level in 0..depth {
        let target = Target::Level(key, level);
        press_to(model, canvas, Key::Enter, Focus::Target(target))?;
        report.levels_opened = report.levels_opened.saturating_add(1);
        let actions = actions_of(canvas, target);
        let invoked = invoke_all(model, canvas, target, &actions)?;
        report.actions_invoked = report.actions_invoked.saturating_add(invoked);
    }
    for level in (0..depth).rev() {
        let owner = match level.checked_sub(1) {
            Some(outer) => Target::Level(key, outer),
            None => Target::Node(key),
        };
        press_to(model, canvas, Key::Escape, Focus::Target(owner))?;
        report.escapes = report.escapes.saturating_add(1);
    }
    Ok(())
}

/// From each level of `key`'s instrument, checks that Tab reaches the next
/// shell region and Shift+Tab returns to the node.
fn tab_out_of_each_level(
    model: &mut FocusModel,
    canvas: &mut Canvas,
    key: NodeKey,
    depth: usize,
    report: &mut WalkReport,
) -> Result<(), WalkError> {
    let node = Focus::Target(Target::Node(key));
    for level in 0..depth {
        for opened in 0..=level {
            press_to(
                model,
                canvas,
                Key::Enter,
                Focus::Target(Target::Level(key, opened)),
            )?;
        }
        press_to(
            model,
            canvas,
            Key::Tab,
            Focus::Region(Region::Canvas.next()),
        )?;
        report.tab_exits = report.tab_exits.saturating_add(1);
        press_to(model, canvas, Key::ShiftTab, node)?;
    }
    Ok(())
}

/// Visits the focused node `key`: in view, every action, its instrument.
fn visit(
    model: &mut FocusModel,
    canvas: &mut Canvas,
    key: NodeKey,
    report: &mut WalkReport,
) -> Result<(), WalkError> {
    let expected = Focus::Target(Target::Node(key));
    if model.focus() != expected {
        return Err(WalkError::Unreached {
            expected,
            found: model.focus(),
        });
    }
    if !canvas.fully_in_view(key)? {
        return Err(WalkError::NotRevealed { key: key.0 });
    }
    report.nodes_reached = report.nodes_reached.saturating_add(1);
    let actions = actions_of(canvas, Target::Node(key));
    let invoked = invoke_all(model, canvas, Target::Node(key), &actions)?;
    report.actions_invoked = report.actions_invoked.saturating_add(invoked);
    let depth = canvas
        .registry()
        .get(key)
        .map_or(0, |node| node.levels.len());
    descend_and_escape(model, canvas, key, depth, report)?;
    tab_out_of_each_level(model, canvas, key, depth, report)
}

/// Walks the whole canvas with the keyboard only; see the module
/// documentation for what is checked.
///
/// # Errors
///
/// Returns the first [`WalkError`], naming the node or level concerned.
pub fn keyboard_walk(canvas: &mut Canvas) -> Result<WalkReport, WalkError> {
    let mut model = FocusModel::new();
    let mut report = WalkReport::default();
    let first = canvas.registry().at(0).map(|node| node.key);
    let landing = first.map_or(Focus::Region(Region::Canvas), |key| {
        Focus::Target(Target::Node(key))
    });
    press_to(&mut model, canvas, Key::Tab, landing)?;
    let keys: Vec<NodeKey> = canvas
        .registry()
        .nodes()
        .iter()
        .map(|node| node.key)
        .collect();
    for (position, key) in keys.iter().enumerate() {
        visit(&mut model, canvas, *key, &mut report)?;
        let Some(next) = position.checked_add(1).and_then(|index| keys.get(index)) else {
            continue;
        };
        let before = *canvas.camera();
        press_to(
            &mut model,
            canvas,
            Key::Next,
            Focus::Target(Target::Node(*next)),
        )?;
        if *canvas.camera() != before {
            report.camera_moves = report.camera_moves.saturating_add(1);
        }
    }
    press_to(
        &mut model,
        canvas,
        Key::Tab,
        Focus::Region(Region::Canvas.next()),
    )?;
    Ok(report)
}

/// Invokes every pointer-reachable action on the canvas once, as clicks
/// would, and returns how many ran.
///
/// # Errors
///
/// Propagates [`Canvas::apply`].
pub fn pointer_walk(canvas: &mut Canvas) -> Result<usize, CanvasError> {
    let mut invoked = 0_usize;
    let keys: Vec<NodeKey> = canvas
        .registry()
        .nodes()
        .iter()
        .map(|node| node.key)
        .collect();
    for key in keys {
        let depth = canvas
            .registry()
            .get(key)
            .map_or(0, |node| node.levels.len());
        let targets = core::iter::once(Target::Node(key))
            .chain((0..depth).map(|level| Target::Level(key, level)));
        for target in targets {
            for action in actions_of(canvas, target)
                .iter()
                .filter(|action| action.pointer)
            {
                canvas.apply(target, action.kind)?;
                invoked = invoked.saturating_add(1);
            }
        }
    }
    Ok(invoked)
}
