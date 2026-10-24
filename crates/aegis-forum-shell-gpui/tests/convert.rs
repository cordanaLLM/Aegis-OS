// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Tests for [`aegis_forum_shell_gpui::convert::to_gpui`] (M28 criterion 1).
//!
//! Positive: converts `a11y::export()` of a real `ShellState`; verifies node count,
//! tree root, focus, labels, roles, bounds, and actions match.
//! Negative: non-ROOT tree ID, missing tree info, or unsupported properties fail
//! with exact [`ConvertError`] variants.
//! Boundary: empty canvas, single root node, and 1,000-node largest seeded fixture.

use accesskit::{Action, Node, Rect, Role, TreeId, TreeInfo, TreeUpdate, Uuid};
use aegis_forum_shell::a11y::{self, EMPTY_STATE_NAME, ROOT_ID, level_id};
use aegis_forum_shell::canvas::registry::{ActionKind, MAX_VALUE, NodeKey};
use aegis_forum_shell::canvas::{Target, seed};
use aegis_forum_shell::focus::Key;
use aegis_forum_shell::state::ShellState;
use aegis_forum_shell_gpui::convert::{ConvertError, to_gpui};

fn verify_node_meta(
    conv_id: gpui::accesskit::NodeId,
    orig_id: accesskit::NodeId,
    conv_node: &gpui::accesskit::Node,
    orig_node: &accesskit::Node,
) {
    assert_eq!(conv_id.0, orig_id.0);
    assert_eq!(
        format!("{:?}", conv_node.role()),
        format!("{:?}", orig_node.role())
    );
    assert_eq!(conv_node.label(), orig_node.label());
    assert_eq!(conv_node.is_hidden(), orig_node.is_hidden());
    assert_eq!(conv_node.role_description(), orig_node.role_description());
}

fn verify_node_bounds(
    conv_node: &gpui::accesskit::Node,
    orig_node: &accesskit::Node,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(bounds) = orig_node.bounds() {
        let conv_bounds = conv_node.bounds().ok_or("missing converted bounds")?;
        assert_eq!(conv_bounds.x0, bounds.x0);
        assert_eq!(conv_bounds.y0, bounds.y0);
        assert_eq!(conv_bounds.x1, bounds.x1);
        assert_eq!(conv_bounds.y1, bounds.y1);
    }
    if orig_node.supports_action(Action::Focus) {
        assert!(conv_node.supports_action(gpui::accesskit::Action::Focus));
    }
    Ok(())
}

fn verify_node_numeric(conv_node: &gpui::accesskit::Node, orig_node: &accesskit::Node) {
    assert_eq!(conv_node.numeric_value(), orig_node.numeric_value());
    assert_eq!(conv_node.min_numeric_value(), orig_node.min_numeric_value());
    assert_eq!(conv_node.max_numeric_value(), orig_node.max_numeric_value());
    assert_eq!(
        conv_node.numeric_value_step(),
        orig_node.numeric_value_step()
    );
}

fn verify_node_pair(
    orig_id: accesskit::NodeId,
    orig_node: &accesskit::Node,
    conv_id: gpui::accesskit::NodeId,
    conv_node: &gpui::accesskit::Node,
) -> Result<(), Box<dyn std::error::Error>> {
    verify_node_meta(conv_id, orig_id, conv_node, orig_node);
    verify_node_bounds(conv_node, orig_node)?;
    verify_node_numeric(conv_node, orig_node);
    Ok(())
}

// --- Positive -------------------------------------------------------------

fn open_seeded_slider() -> Result<(ShellState, accesskit::NodeId), Box<dyn std::error::Error>> {
    let mut state = ShellState::new(seed::seeded()?);
    state.focus.focus_node(&mut state.canvas, NodeKey(99))?;
    state.focus.press(&mut state.canvas, Key::Enter)?;
    let id = level_id(NodeKey(99), 0);
    Ok((state, id))
}

/// Positive: export of seeded shell with open instrument slider converts cleanly,
/// preserving slider role, value 0, min -8, max 8, and step 1.
#[test]
fn export_of_open_slider_converts_with_slider_semantics() -> Result<(), Box<dyn std::error::Error>>
{
    let (state, slider_id) = open_seeded_slider()?;
    let original = a11y::export(&state);

    let converted = to_gpui(&original)?;

    assert_eq!(converted.nodes.len(), original.nodes.len());
    let (_, slider_node) = converted
        .nodes
        .iter()
        .find(|(id, _)| id.0 == slider_id.0)
        .ok_or("missing slider node in converted tree")?;

    assert_eq!(
        format!("{:?}", slider_node.role()),
        format!("{:?}", Role::Slider)
    );
    assert_eq!(slider_node.numeric_value(), Some(0.0));
    assert_eq!(slider_node.min_numeric_value(), Some(f64::from(-MAX_VALUE)));
    assert_eq!(slider_node.max_numeric_value(), Some(f64::from(MAX_VALUE)));
    assert_eq!(slider_node.numeric_value_step(), Some(1.0));

    for ((orig_id, orig_node), (conv_id, conv_node)) in
        original.nodes.iter().zip(converted.nodes.iter())
    {
        verify_node_pair(*orig_id, orig_node, *conv_id, conv_node)?;
    }
    Ok(())
}

/// Positive: converting an export of a populated shell state preserves node count,
/// root, focus, labels, roles, and actions.
#[test]
fn export_of_populated_shell_converts_losslessly() -> Result<(), Box<dyn std::error::Error>> {
    let canvas = seed::seeded()?;
    let state = ShellState::new(canvas);
    let original = a11y::export(&state);

    let converted = to_gpui(&original)?;

    assert_eq!(converted.nodes.len(), original.nodes.len());
    let orig_tree = original.tree.as_ref().ok_or("missing orig tree")?;
    let conv_tree = converted.tree.as_ref().ok_or("missing conv tree")?;
    assert_eq!(conv_tree.root.0, orig_tree.root.0);
    assert_eq!(converted.tree_id.0, TreeId::ROOT.0);
    assert_eq!(converted.focus.0, original.focus.0);

    for ((orig_id, orig_node), (conv_id, conv_node)) in
        original.nodes.iter().zip(converted.nodes.iter())
    {
        verify_node_pair(*orig_id, orig_node, *conv_id, conv_node)?;
    }
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: non-ROOT `tree_id` returns [`ConvertError::NonRootTreeId`].
#[test]
fn non_root_tree_id_fails_with_exact_variant() {
    let non_root = TreeId(Uuid::from_u128(0x1234_5678_9abc_def0));
    let update = TreeUpdate {
        nodes: vec![(ROOT_ID, Node::new(Role::Window))],
        tree: Some(TreeInfo::new(ROOT_ID)),
        tree_id: non_root,
        focus: ROOT_ID,
    };

    assert_eq!(
        to_gpui(&update).err(),
        Some(ConvertError::NonRootTreeId(non_root))
    );
}

/// Negative: missing tree info returns [`ConvertError::MissingTreeInfo`].
#[test]
fn missing_tree_info_fails_with_exact_variant() {
    let update = TreeUpdate {
        nodes: vec![(ROOT_ID, Node::new(Role::Window))],
        tree: None,
        tree_id: TreeId::ROOT,
        focus: ROOT_ID,
    };

    assert_eq!(to_gpui(&update).err(), Some(ConvertError::MissingTreeInfo));
}

fn single_node_update(node: Node) -> TreeUpdate {
    TreeUpdate {
        nodes: vec![(ROOT_ID, node)],
        tree: Some(TreeInfo::new(ROOT_ID)),
        tree_id: TreeId::ROOT,
        focus: ROOT_ID,
    }
}

/// Negative: node with `html_id` (unsupported in 0.24.1) returns [`ConvertError::UnsupportedProperty`].
#[test]
fn unsupported_property_fails_with_exact_variant() {
    let mut node = Node::new(Role::Window);
    node.set_html_id("main-window");
    let update = single_node_update(node);

    assert_eq!(
        to_gpui(&update).err(),
        Some(ConvertError::UnsupportedProperty {
            node_id: ROOT_ID,
            property: "html_id",
        })
    );
}

/// Negative: node with non-ROOT `tree_id` graft returns [`ConvertError::NonRootTreeId`].
#[test]
fn graft_node_non_root_tree_id_fails() {
    let non_root = TreeId(Uuid::from_u128(0xbeef_cafe_0000_1111));
    let mut node = Node::new(Role::Group);
    node.set_tree_id(non_root);
    let update = single_node_update(node);

    assert_eq!(
        to_gpui(&update).err(),
        Some(ConvertError::NonRootTreeId(non_root))
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: empty canvas shell exports empty state node, converts cleanly.
#[test]
fn empty_canvas_shell_converts_cleanly() -> Result<(), Box<dyn std::error::Error>> {
    let state = ShellState::new(seed::empty()?);
    let original = a11y::export(&state);

    let converted = to_gpui(&original)?;

    assert_eq!(converted.nodes.len(), original.nodes.len());
    let empty_label = converted
        .nodes
        .iter()
        .find(|(id, _)| id.0 == a11y::EMPTY_STATE_ID.0)
        .ok_or("missing empty state node")?;
    assert_eq!(empty_label.1.label(), Some(EMPTY_STATE_NAME));
    assert_eq!(
        format!("{:?}", empty_label.1.role()),
        format!("{:?}", Role::Label)
    );
    Ok(())
}

/// Boundary: single minimal root node converts cleanly.
#[test]
fn single_root_node_converts_cleanly() -> Result<(), Box<dyn std::error::Error>> {
    let mut root = Node::new(Role::Window);
    root.set_label("Minimal");
    root.set_bounds(Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 800.0,
        y1: 600.0,
    });
    let update = single_node_update(root);

    let converted = to_gpui(&update)?;

    assert_eq!(converted.nodes.len(), 1);
    let (first_id, first_node) = converted.nodes.first().ok_or("missing root node")?;
    assert_eq!(first_id.0, ROOT_ID.0);
    assert_eq!(
        format!("{:?}", first_node.role()),
        format!("{:?}", Role::Window)
    );
    assert_eq!(first_node.label(), Some("Minimal"));
    Ok(())
}

/// Boundary: largest fixture used by M16 tests (1,000 seeded nodes) converts cleanly.
#[test]
fn largest_one_thousand_node_fixture_converts_cleanly() -> Result<(), Box<dyn std::error::Error>> {
    let canvas = seed::seeded()?;
    assert_eq!(canvas.registry().len(), 1_000);
    let state = ShellState::new(canvas);
    let original = a11y::export(&state);
    assert!(original.nodes.len() > 1_000);

    let converted = to_gpui(&original)?;

    assert_eq!(converted.nodes.len(), original.nodes.len());
    assert_eq!(converted.focus.0, original.focus.0);
    Ok(())
}

/// Boundary: slider numeric values at bounds -8.0 and 8.0 survive conversion exactly.
#[test]
fn slider_boundary_numeric_values_survive_conversion() -> Result<(), Box<dyn std::error::Error>> {
    for boundary_val in [f64::from(-MAX_VALUE), f64::from(MAX_VALUE)] {
        let mut node = Node::new(Role::Slider);
        node.set_numeric_value(boundary_val);
        node.set_min_numeric_value(f64::from(-MAX_VALUE));
        node.set_max_numeric_value(f64::from(MAX_VALUE));
        node.set_numeric_value_step(1.0);
        let update = single_node_update(node);

        let converted = to_gpui(&update)?;
        let (_, conv_node) = converted.nodes.first().ok_or("missing converted node")?;
        assert_eq!(conv_node.numeric_value(), Some(boundary_val));
        assert_eq!(conv_node.min_numeric_value(), Some(f64::from(-MAX_VALUE)));
        assert_eq!(conv_node.max_numeric_value(), Some(f64::from(MAX_VALUE)));
        assert_eq!(conv_node.numeric_value_step(), Some(1.0));
    }
    Ok(())
}

/// Boundary: open seeded slider driven to -8 and 8 boundary values survives conversion exactly.
#[test]
fn seeded_slider_at_boundary_values_survives_conversion() -> Result<(), Box<dyn std::error::Error>>
{
    let (mut state, slider_id) = open_seeded_slider()?;

    // Drive value to max (8)
    for _ in 0..10 {
        state
            .canvas
            .apply(Target::Level(NodeKey(99), 0), ActionKind::Increment)?;
    }
    let update_max = a11y::export(&state);
    let conv_max = to_gpui(&update_max)?;
    let (_, node_max) = conv_max
        .nodes
        .iter()
        .find(|(id, _)| id.0 == slider_id.0)
        .ok_or("missing slider node at max boundary")?;
    assert_eq!(node_max.numeric_value(), Some(f64::from(MAX_VALUE)));

    // Drive value to min (-8)
    for _ in 0..20 {
        state
            .canvas
            .apply(Target::Level(NodeKey(99), 0), ActionKind::Decrement)?;
    }
    let update_min = a11y::export(&state);
    let conv_min = to_gpui(&update_min)?;
    let (_, node_min) = conv_min
        .nodes
        .iter()
        .find(|(id, _)| id.0 == slider_id.0)
        .ok_or("missing slider node at min boundary")?;
    assert_eq!(node_min.numeric_value(), Some(f64::from(-MAX_VALUE)));

    Ok(())
}

// --- Allow-list roundtrip and Role Pair Invariants ------------------------

fn assert_unmapped_property_fails(node: Node) {
    let update = single_node_update(node);
    assert_eq!(
        to_gpui(&update).err(),
        Some(ConvertError::UnsupportedProperty {
            node_id: ROOT_ID,
            property: "unmapped property",
        })
    );
}

/// Negative: node with an unmapped property (`value`) fails allow-list round-trip check.
#[test]
fn unmapped_property_value_fails_roundtrip_check() {
    let mut node = Node::new(Role::Window);
    node.set_value("unexpected-value");
    assert_unmapped_property_fails(node);
}

/// Negative: node with an unmapped property (`description`) fails allow-list round-trip check.
#[test]
fn unmapped_property_description_fails_roundtrip_check() {
    let mut node = Node::new(Role::Window);
    node.set_description("unexpected-description");
    assert_unmapped_property_fails(node);
}

/// Negative: node with an unmapped property (`placeholder`) fails allow-list round-trip check.
#[test]
fn unmapped_property_placeholder_fails_roundtrip_check() {
    let mut node = Node::new(Role::Window);
    node.set_placeholder("filter...");
    assert_unmapped_property_fails(node);
}

/// Positive: every pair in `ROLE_PAIRS` maps 0.25.1 variant to identical 0.24.1 Debug name.
#[test]
fn all_role_pairs_have_identical_debug_names() {
    use aegis_forum_shell_gpui::convert::ROLE_PAIRS;
    assert_eq!(ROLE_PAIRS.len(), 182);
    for (src_role, tgt_role) in ROLE_PAIRS {
        assert_eq!(format!("{src_role:?}"), format!("{tgt_role:?}"));
    }
}
