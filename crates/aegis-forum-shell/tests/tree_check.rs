// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E16-4: the Forum Shell accessibility tree check (REQ-P12-06 as D101
//! re-maps it, REQ-P05-08).
//!
//! Positive: the default state and the seeded canvas export a `TreeUpdate`
//! in which every node has a role and a non-empty name. Negative: a planted
//! unlabelled node fails the check, and the failure names it. Boundary: the
//! empty canvas exports its labelled empty state and passes.
//!
//! The check is narrower than the axe-core scan it replaces (ADR-0004): it
//! asserts a role and a name on every node and the labelled empty state, not
//! the WCAG 2.2 AA rule set.

mod common;

use accesskit::{Node, NodeId, Role, TreeUpdate};
use aegis_forum_shell::a11y::{
    CANVAS_ID, EMPTY_STATE_ID, EMPTY_STATE_NAME, ROOT_ID, TreeError, check_tree, describe, export,
    node_id,
};
use aegis_forum_shell::canvas::registry::{
    ActionState, CanvasNode, NodeKey, NodeKind, RegistryError,
};
use aegis_forum_shell::canvas::{Target, seed};
use aegis_forum_shell::consumers::Inbound;
use aegis_forum_shell::focus::{Focus, Key};
use aegis_forum_shell::state::ShellState;

use common::{Fallible, decision_request, telemetry};

/// The planted node's tree id: canvas node 4242, which the model never held.
fn planted_id() -> NodeId {
    node_id(NodeKey(4_242))
}

/// A seeded shell with a pending decision, telemetry, and focus three levels
/// deep in node 100's instrument, so every kind of node is exported.
fn busy_shell() -> Fallible<ShellState> {
    let mut state = ShellState::new(seed::seeded()?);
    state.apply(Inbound::Decision(Box::new(decision_request(
        "request-0001",
    )?)))?;
    state.apply(Inbound::Telemetry(telemetry(14.2, 0.6)?))?;
    state.focus.focus_node(&mut state.canvas, NodeKey(99))?;
    for _ in 0..3 {
        state.focus.press(&mut state.canvas, Key::Enter)?;
    }
    assert_eq!(
        state.focus.focus(),
        Focus::Target(Target::Level(NodeKey(99), 2))
    );
    Ok(state)
}

/// Adds `planted` as a child of the canvas in `update`.
fn plant(update: &mut TreeUpdate, planted: Node) {
    for (id, node) in &mut update.nodes {
        if *id == CANVAS_ID {
            node.push_child(planted_id());
        }
    }
    update.nodes.push((planted_id(), planted));
}

// --- Positive -------------------------------------------------------------

/// Positive: the default state -- nothing pending, no telemetry, an empty
/// canvas, focus on the status bar -- passes.
#[test]
fn the_default_state_passes() -> Fallible {
    let state = ShellState::new(seed::empty()?);
    let report = check_tree(&export(&state))?;
    assert_eq!(
        report.nodes, 6,
        "window, status bar, readout, canvas, empty state, list"
    );
    Ok(())
}

/// Positive: the seeded canvas with every kind of node passes, and every node
/// reached carries a role and a non-empty name.
#[test]
fn the_seeded_canvas_passes() -> Fallible {
    let state = busy_shell()?;
    let update = export(&state);
    let report = check_tree(&update)?;
    assert_eq!(report.canvas_children, 1_000);
    assert_eq!(report.nodes, update.nodes.len());
    assert_eq!(
        report.nodes, 1_009,
        "five chrome nodes, 1,000 canvas nodes, one decision, three levels"
    );
    for (_, node) in &update.nodes {
        assert!(!matches!(
            node.role(),
            Role::Unknown | Role::GenericContainer
        ));
        assert!(node.label().is_some_and(|name| !name.trim().is_empty()));
    }
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a planted unlabelled node fails the check, and the failure names
/// it.
#[test]
fn a_planted_unlabelled_node_fails_and_is_named() -> Fallible {
    let state = busy_shell()?;
    let mut update = export(&state);
    plant(&mut update, Node::new(Role::Group));
    let refused = check_tree(&update);
    assert_eq!(
        refused,
        Err(TreeError::Unlabelled {
            id: planted_id(),
            role: Role::Group
        })
    );
    let message = refused
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    assert_eq!(
        message,
        "canvas node 4242 (#1099511632018) (Group) has no accessible name"
    );
    assert!(message.contains(&describe(&planted_id())));
    Ok(())
}

/// Negative: a blank name, a role an adapter drops and a hidden node each
/// fail, named.
#[test]
fn a_blank_name_a_dropped_role_and_a_hidden_node_fail() -> Fallible {
    let state = busy_shell()?;
    let mut blank = Node::new(Role::Group);
    blank.set_label("   ");
    let mut unknown = Node::new(Role::Unknown);
    unknown.set_label("Mystery");
    let mut container = Node::new(Role::GenericContainer);
    container.set_label("Wrapper");
    let mut hidden = Node::new(Role::Group);
    hidden.set_label("Hidden");
    hidden.set_hidden();
    let cases = [
        (
            blank,
            TreeError::Unlabelled {
                id: planted_id(),
                role: Role::Group,
            },
        ),
        (
            unknown,
            TreeError::NoRole {
                id: planted_id(),
                role: Role::Unknown,
            },
        ),
        (
            container,
            TreeError::NoRole {
                id: planted_id(),
                role: Role::GenericContainer,
            },
        ),
        (hidden, TreeError::Hidden(planted_id())),
    ];
    for (planted, expected) in cases {
        let mut update = export(&state);
        plant(&mut update, planted);
        assert_eq!(check_tree(&update), Err(expected));
    }
    Ok(())
}

/// Negative: a dangling child and an orphan fail, each named.
#[test]
fn a_dangling_child_and_an_orphan_fail() -> Fallible {
    let state = busy_shell()?;
    let mut dangling = export(&state);
    for (id, node) in &mut dangling.nodes {
        if *id == CANVAS_ID {
            node.push_child(planted_id());
        }
    }
    let expected = TreeError::Dangling {
        parent: CANVAS_ID,
        child: planted_id(),
    };
    assert_eq!(check_tree(&dangling), Err(expected));
    let mut orphan = export(&state);
    let mut stray = Node::new(Role::Group);
    stray.set_label("Stray");
    orphan.nodes.push((planted_id(), stray));
    assert_eq!(check_tree(&orphan), Err(TreeError::Orphan(planted_id())));
    Ok(())
}

/// Negative: a shared child, focus outside the tree and a missing root fail.
#[test]
fn a_shared_child_stray_focus_and_no_root_fail() -> Fallible {
    let state = busy_shell()?;
    let mut shared = export(&state);
    for (id, node) in &mut shared.nodes {
        if *id == ROOT_ID {
            node.push_child(node_id(NodeKey(0)));
        }
    }
    assert!(matches!(check_tree(&shared), Err(TreeError::Reached(_))));
    let mut unfocused = export(&state);
    unfocused.focus = planted_id();
    assert_eq!(
        check_tree(&unfocused),
        Err(TreeError::FocusAbsent(planted_id()))
    );
    let mut rootless = export(&state);
    rootless.tree = None;
    assert_eq!(check_tree(&rootless), Err(TreeError::NoTreeInfo));
    Ok(())
}

/// Negative: the model refuses an unlabelled node before it reaches the
/// tree, so the planted node above can only come from an exporter's defect.
#[test]
fn the_model_refuses_an_unlabelled_node() -> Fallible {
    let mut canvas = seed::empty()?;
    let rect = aegis_forum_shell::canvas::geometry::WorldRect::new(0.0, 0.0, 1.0, 1.0)?;
    let node = CanvasNode {
        key: NodeKey(1),
        label: " ".to_owned(),
        kind: NodeKind::Note,
        rect,
        actions: Vec::new(),
        levels: Vec::new(),
        state: ActionState::default(),
    };
    let refused = canvas.push(node);
    assert_eq!(refused, Err(RegistryError::Label { key: 1 }.into()));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the empty canvas exports its labelled empty state and passes;
/// without it, the check fails.
#[test]
fn the_empty_canvas_exports_a_labelled_empty_state() -> Fallible {
    let state = ShellState::new(seed::empty()?);
    let update = export(&state);
    let canvas = update
        .nodes
        .iter()
        .find(|(id, _)| *id == CANVAS_ID)
        .map(|(_, node)| node);
    assert_eq!(
        canvas.map(|node| node.children().to_vec()),
        Some(vec![EMPTY_STATE_ID])
    );
    let empty = update
        .nodes
        .iter()
        .find(|(id, _)| *id == EMPTY_STATE_ID)
        .map(|(_, node)| node);
    assert_eq!(
        empty
            .and_then(|node| node.label().map(str::to_owned))
            .as_deref(),
        Some(EMPTY_STATE_NAME)
    );
    assert_eq!(empty.map(Node::role), Some(Role::Label));
    assert_eq!(check_tree(&update)?.canvas_children, 1);
    let mut bare = update.clone();
    bare.nodes.retain(|(id, _)| *id != EMPTY_STATE_ID);
    for (id, node) in &mut bare.nodes {
        if *id == CANVAS_ID {
            node.clear_children();
        }
    }
    assert_eq!(check_tree(&bare), Err(TreeError::EmptyCanvasUnlabelled));
    Ok(())
}
