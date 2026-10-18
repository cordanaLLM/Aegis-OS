// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Tree judge unit tests (M28 E28-3): pure AT-SPI node evaluation without a bus.

use aegis_forum_shell_gpui::fixture::{
    BUTTON_LABEL, BUTTON_LABEL_AFTER, FIXTURE_TITLE, HIDDEN_CHILD_LABELS, HIDDEN_LABEL, LIST_LABEL,
    SEEDED_NODES, SHOWN_LABEL, row_label,
};
use aegis_forum_shell_gpui::judge::{
    ReadNode, atspi_button_exposed, atspi_every_node_named, atspi_focus_reported,
    atspi_hidden_subtree_hidden, atspi_seeded_reachable, judge_tree, line,
};

fn build_sound_tree(seeded_count: usize) -> Vec<ReadNode> {
    let mut nodes = Vec::with_capacity(seeded_count.saturating_add(4));
    nodes.push(ReadNode {
        path: Vec::new(),
        role: "application".to_owned(),
        name: "aegis-forum-shell".to_owned(),
        states: vec!["showing".to_owned()],
        actions: Vec::new(),
        extents: None,
    });
    nodes.push(ReadNode {
        path: vec![0],
        role: "frame".to_owned(),
        name: FIXTURE_TITLE.to_owned(),
        states: vec!["showing".to_owned()],
        actions: Vec::new(),
        extents: None,
    });
    nodes.push(ReadNode {
        path: vec![0, 0],
        role: "button".to_owned(),
        name: BUTTON_LABEL.to_owned(),
        states: vec!["showing".to_owned(), "focused".to_owned()],
        actions: vec!["click".to_owned()],
        extents: Some((0, 0, 100, 40)),
    });
    nodes.push(ReadNode {
        path: vec![0, 1],
        role: "list".to_owned(),
        name: LIST_LABEL.to_owned(),
        states: vec!["showing".to_owned()],
        actions: Vec::new(),
        extents: None,
    });
    for i in 0..seeded_count {
        nodes.push(ReadNode {
            path: vec![0, 1, i],
            role: "list item".to_owned(),
            name: row_label(i),
            states: vec!["showing".to_owned()],
            actions: Vec::new(),
            extents: None,
        });
    }
    nodes.push(ReadNode {
        path: vec![0, 2],
        role: "panel".to_owned(),
        name: SHOWN_LABEL.to_owned(),
        states: vec!["showing".to_owned()],
        actions: Vec::new(),
        extents: None,
    });
    nodes
}

// --- Positive -------------------------------------------------------------

/// Positive: all five cases in `judge_tree` pass on a sound tree.
#[test]
fn judge_tree_sound_passes_all() {
    let tree = build_sound_tree(SEEDED_NODES);
    let results = judge_tree(&tree);
    assert_eq!(results.len(), 5);
    for res in &results {
        assert!(res.passed, "case {} failed: {}", res.name, res.why);
        let l = line(res);
        assert!(l.starts_with(&format!("CASE {} pass", res.name)));
    }
}

// --- Negative -------------------------------------------------------------

/// Negative: button role panel fails `atspi/button-exposed` naming "panel".
#[test]
fn button_as_panel_fails() {
    let mut tree = build_sound_tree(SEEDED_NODES);
    if let Some(btn) = tree.iter_mut().find(|n| n.name == BUTTON_LABEL) {
        btn.role = "panel".to_owned();
    }
    let res = atspi_button_exposed(&tree);
    assert!(!res.passed);
    assert!(res.why.contains("panel"), "why was: {}", res.why);
}

/// Negative: item 517 nameless fails `atspi/every-node-named` naming 517.
#[test]
fn nameless_item_517_fails_naming_path() {
    let mut tree = build_sound_tree(SEEDED_NODES);
    let target = row_label(517);
    if let Some(item) = tree.iter_mut().find(|n| n.name == target) {
        item.name.clear();
    }
    let res = atspi_every_node_named(&tree);
    assert!(!res.passed);
    assert!(res.why.contains("517"), "why was: {}", res.why);
}

/// Negative: 10 items only fails `atspi/seeded-reachable` with "10 of 1000".
#[test]
fn ten_items_fails_seeded_reachable() {
    let tree = build_sound_tree(10);
    let res = atspi_seeded_reachable(&tree);
    assert!(!res.passed);
    assert!(res.why.contains("10 of 1000"), "why was: {}", res.why);
}

/// Negative: hidden child carrying showing fails `atspi/hidden-subtree-hidden`.
#[test]
fn hidden_child_showing_fails() {
    let mut tree = build_sound_tree(SEEDED_NODES);
    let child_name = HIDDEN_CHILD_LABELS
        .first()
        .copied()
        .unwrap_or("Hidden child 1");
    tree.push(ReadNode {
        path: vec![0, 3, 0],
        role: "panel".to_owned(),
        name: child_name.to_owned(),
        states: vec!["showing".to_owned()],
        actions: Vec::new(),
        extents: None,
    });
    let res = atspi_hidden_subtree_hidden(&tree);
    assert!(!res.passed);
}

/// Negative: hidden parent carrying showing fails `atspi/hidden-subtree-hidden`.
#[test]
fn hidden_parent_showing_fails() {
    let mut tree = build_sound_tree(SEEDED_NODES);
    tree.push(ReadNode {
        path: vec![0, 3],
        role: "panel".to_owned(),
        name: HIDDEN_LABEL.to_owned(),
        states: vec!["showing".to_owned()],
        actions: Vec::new(),
        extents: None,
    });
    let res = atspi_hidden_subtree_hidden(&tree);
    assert!(!res.passed);
}

/// Negative: button without "click" action fails `atspi/button-exposed`.
#[test]
fn button_without_click_fails() {
    let mut tree = build_sound_tree(SEEDED_NODES);
    if let Some(btn) = tree.iter_mut().find(|n| n.name == BUTTON_LABEL) {
        btn.actions.clear();
    }
    let res = atspi_button_exposed(&tree);
    assert!(!res.passed);
    assert!(res.why.contains("click"), "why was: {}", res.why);
}

/// Negative: a button that is not focused fails `atspi/focus-reported`.
#[test]
fn button_not_focused_fails() {
    let mut tree = build_sound_tree(SEEDED_NODES);
    if let Some(btn) = tree.iter_mut().find(|n| n.name == BUTTON_LABEL) {
        btn.states.retain(|s| s != "focused");
    }
    let res = atspi_focus_reported(&tree);
    assert!(!res.passed);
    assert!(res.why.contains("focused"), "why was: {}", res.why);
}

/// Boundary: `focusable` alone is not `focused`, and a tree with no button
/// fails `atspi/focus-reported` naming the missing node.
#[test]
fn focusable_is_not_focused_and_no_button_fails() {
    let mut tree = build_sound_tree(SEEDED_NODES);
    if let Some(btn) = tree.iter_mut().find(|n| n.name == BUTTON_LABEL) {
        btn.states = vec!["showing".to_owned(), "focusable".to_owned()];
    }
    assert!(!atspi_focus_reported(&tree).passed);
    tree.retain(|n| n.name != BUTTON_LABEL);
    let res = atspi_focus_reported(&tree);
    assert!(!res.passed);
    assert!(res.why.contains("not found"), "why was: {}", res.why);
}

/// Negative: duplicate "Node 5" fails `atspi/seeded-reachable`.
#[test]
fn duplicate_node_5_fails_seeded_reachable() {
    let mut tree = build_sound_tree(SEEDED_NODES);
    tree.push(ReadNode {
        path: vec![0, 1, 1001],
        role: "list item".to_owned(),
        name: row_label(5),
        states: vec!["showing".to_owned()],
        actions: Vec::new(),
        extents: None,
    });
    let res = atspi_seeded_reachable(&tree);
    assert!(!res.passed);
    assert!(res.why.contains("duplicate") || res.why.contains("Node 5"));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: exactly 1000 items passes and 999 fails naming "Node 999".
#[test]
fn exactly_1000_passes_and_999_fails_naming_missing() {
    let tree_1000 = build_sound_tree(1000);
    assert!(atspi_seeded_reachable(&tree_1000).passed);

    let tree_999 = build_sound_tree(999);
    let res = atspi_seeded_reachable(&tree_999);
    assert!(!res.passed);
    assert!(res.why.contains("Node 999"), "why was: {}", res.why);
}

/// Boundary: zero-width extents fail, 1x1 passes.
#[test]
fn button_extents_zero_width_fails_one_by_one_passes() {
    let mut tree = build_sound_tree(SEEDED_NODES);
    if let Some(btn) = tree.iter_mut().find(|n| n.name == BUTTON_LABEL) {
        btn.extents = Some((0, 0, 0, 40));
    }
    assert!(!atspi_button_exposed(&tree).passed);

    if let Some(btn) = tree.iter_mut().find(|n| n.name == BUTTON_LABEL) {
        btn.extents = Some((0, 0, 1, 1));
    }
    assert!(atspi_button_exposed(&tree).passed);
}

/// Boundary: `BUTTON_LABEL_AFTER` accepted as the button's name.
#[test]
fn button_label_after_accepted() {
    let mut tree = build_sound_tree(SEEDED_NODES);
    if let Some(btn) = tree.iter_mut().find(|n| n.name == BUTTON_LABEL) {
        btn.name = BUTTON_LABEL_AFTER.to_owned();
    }
    assert!(atspi_button_exposed(&tree).passed);
    assert!(atspi_focus_reported(&tree).passed);
}

/// Boundary: the application object (path []) unnamed is ignored by `every-node-named`.
#[test]
fn application_object_unnamed_ignored() {
    let mut tree = build_sound_tree(SEEDED_NODES);
    if let Some(app) = tree.iter_mut().find(|n| n.path.is_empty()) {
        app.name.clear();
    }
    assert!(atspi_every_node_named(&tree).passed);
}
