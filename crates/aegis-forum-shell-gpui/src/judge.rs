// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Pure AT-SPI tree evaluation without a live D-Bus session (M28 E28-3).
//!
//! Provides the data structures representing accessible nodes read from AT-SPI,
//! and five deterministic evaluation rules checking button exposure, focus
//! reporting, list node reachability, element labelling, and hidden subtree filtering.

use std::collections::HashSet;

use crate::fixture::{
    BUTTON_LABEL, BUTTON_LABEL_AFTER, HIDDEN_CHILD_LABELS, HIDDEN_LABEL, SEEDED_NODES, SHOWN_LABEL,
    row_label,
};

/// A normalized snapshot of an accessible node read over AT-SPI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadNode {
    /// Numerical hierarchy path from the application root to this node.
    pub path: Vec<usize>,
    /// AT-SPI role string (e.g. `"button"`, `"panel"`, `"list item"`).
    pub role: String,
    /// Accessible name (e.g. from `aria_label` or text content).
    pub name: String,
    /// Active AT-SPI state names (e.g. `"showing"`, `"focused"`, `"visible"`).
    pub states: Vec<String>,
    /// Available action names (e.g. `["click"]`).
    pub actions: Vec<String>,
    /// Window-relative bounding box `(x, y, width, height)` if supported.
    pub extents: Option<(i32, i32, i32, i32)>,
}

/// The result of evaluating a single AT-SPI acceptance case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseResult {
    /// The unique case identifier (e.g. `"atspi/button-exposed"`).
    pub name: &'static str,
    /// Whether the tree satisfied the case requirement.
    pub passed: bool,
    /// Explanation if the case failed, or empty string on pass.
    pub why: String,
}

/// Formats a [`CaseResult`] as a standard verification line.
#[must_use]
pub fn line(result: &CaseResult) -> String {
    if result.passed {
        format!("CASE {} pass", result.name)
    } else {
        format!("CASE {} fail: {}", result.name, result.why)
    }
}

/// Evaluates whether the fixture button is correctly exposed with role, click action, and extents.
#[must_use]
pub fn atspi_button_exposed(nodes: &[ReadNode]) -> CaseResult {
    let matches: Vec<&ReadNode> = nodes
        .iter()
        .filter(|n| n.name == BUTTON_LABEL || n.name == BUTTON_LABEL_AFTER)
        .collect();

    if matches.is_empty() {
        return CaseResult {
            name: "atspi/button-exposed",
            passed: false,
            why: format!("no node named '{BUTTON_LABEL}' or '{BUTTON_LABEL_AFTER}'"),
        };
    }
    if matches.len() > 1 {
        return CaseResult {
            name: "atspi/button-exposed",
            passed: false,
            why: format!("found {} button nodes, want exactly 1", matches.len()),
        };
    }
    let Some(btn) = matches.first().copied() else {
        return CaseResult {
            name: "atspi/button-exposed",
            passed: false,
            why: "missing button element".to_owned(),
        };
    };

    if btn.role != "button" {
        return CaseResult {
            name: "atspi/button-exposed",
            passed: false,
            why: format!("role {}, want button", btn.role),
        };
    }
    if !btn.actions.iter().any(|a| a == "click") {
        return CaseResult {
            name: "atspi/button-exposed",
            passed: false,
            why: "actions do not contain click".to_owned(),
        };
    }
    match btn.extents {
        Some((_x, _y, w, h)) if w > 0 && h > 0 => CaseResult {
            name: "atspi/button-exposed",
            passed: true,
            why: String::new(),
        },
        Some((_x, _y, w, h)) => CaseResult {
            name: "atspi/button-exposed",
            passed: false,
            why: format!("extents non-positive: {w}x{h}"),
        },
        None => CaseResult {
            name: "atspi/button-exposed",
            passed: false,
            why: "extents missing".to_owned(),
        },
    }
}

/// Evaluates whether the button node reports focused state.
#[must_use]
pub fn atspi_focus_reported(nodes: &[ReadNode]) -> CaseResult {
    let btn = nodes
        .iter()
        .find(|n| n.name == BUTTON_LABEL || n.name == BUTTON_LABEL_AFTER);

    let Some(btn) = btn else {
        return CaseResult {
            name: "atspi/focus-reported",
            passed: false,
            why: "button node not found".to_owned(),
        };
    };

    if btn.states.iter().any(|s| s == "focused") {
        CaseResult {
            name: "atspi/focus-reported",
            passed: true,
            why: String::new(),
        }
    } else {
        CaseResult {
            name: "atspi/focus-reported",
            passed: false,
            why: "button node does not have focused state".to_owned(),
        }
    }
}

/// Evaluates whether all 1,000 seeded list nodes are reachable without omission or duplicate.
#[must_use]
pub fn atspi_seeded_reachable(nodes: &[ReadNode]) -> CaseResult {
    let mut seen = HashSet::new();
    let mut duplicates = Vec::new();

    for node in nodes {
        if node.role == "list item" && !seen.insert(&node.name) {
            duplicates.push(node.name.clone());
        }
    }

    if let Some(dup) = duplicates.first() {
        return CaseResult {
            name: "atspi/seeded-reachable",
            passed: false,
            why: format!("duplicate list item '{dup}'"),
        };
    }

    let mut reachable_count = 0usize;
    let mut first_missing = None;
    for i in 0..SEEDED_NODES {
        let label = row_label(i);
        if seen.contains(&label) {
            reachable_count = reachable_count.saturating_add(1);
        } else if first_missing.is_none() {
            first_missing = Some(label);
        }
    }

    if reachable_count == SEEDED_NODES {
        CaseResult {
            name: "atspi/seeded-reachable",
            passed: true,
            why: String::new(),
        }
    } else {
        let missing = first_missing.unwrap_or_else(|| "unknown".to_owned());
        CaseResult {
            name: "atspi/seeded-reachable",
            passed: false,
            why: format!(
                "{reachable_count} of {SEEDED_NODES} seeded nodes reachable; first missing {missing}"
            ),
        }
    }
}

/// Evaluates whether every non-root node has a non-empty name and a valid role.
#[must_use]
pub fn atspi_every_node_named(nodes: &[ReadNode]) -> CaseResult {
    for node in nodes {
        if node.path.is_empty() {
            continue;
        }
        let path_str = node
            .path
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join("/");
        if node.name.trim().is_empty() {
            return CaseResult {
                name: "atspi/every-node-named",
                passed: false,
                why: format!("{} at path {} has no name", node.role, path_str),
            };
        }
        if node.role == "invalid" || node.role == "unknown" {
            return CaseResult {
                name: "atspi/every-node-named",
                passed: false,
                why: format!("node at path {path_str} has invalid or unknown role"),
            };
        }
    }
    CaseResult {
        name: "atspi/every-node-named",
        passed: true,
        why: String::new(),
    }
}

/// Evaluates whether hidden subtrees are excluded from showing while shown siblings remain visible.
#[must_use]
pub fn atspi_hidden_subtree_hidden(nodes: &[ReadNode]) -> CaseResult {
    let shown_ok = nodes
        .iter()
        .any(|n| n.name == SHOWN_LABEL && n.states.iter().any(|s| s == "showing"));

    if !shown_ok {
        return CaseResult {
            name: "atspi/hidden-subtree-hidden",
            passed: false,
            why: format!("shown panel '{SHOWN_LABEL}' not present or not showing"),
        };
    }

    for node in nodes {
        let is_hidden_target =
            node.name == HIDDEN_LABEL || HIDDEN_CHILD_LABELS.iter().any(|&lbl| lbl == node.name);
        if is_hidden_target && node.states.iter().any(|s| s == "showing") {
            return CaseResult {
                name: "atspi/hidden-subtree-hidden",
                passed: false,
                why: format!("hidden element '{}' carries showing state", node.name),
            };
        }
    }

    CaseResult {
        name: "atspi/hidden-subtree-hidden",
        passed: true,
        why: String::new(),
    }
}

/// Evaluates all five acceptance cases across the accessible tree in canonical order.
#[must_use]
pub fn judge_tree(nodes: &[ReadNode]) -> Vec<CaseResult> {
    vec![
        atspi_button_exposed(nodes),
        atspi_focus_reported(nodes),
        atspi_seeded_reachable(nodes),
        atspi_every_node_named(nodes),
        atspi_hidden_subtree_hidden(nodes),
    ]
}
