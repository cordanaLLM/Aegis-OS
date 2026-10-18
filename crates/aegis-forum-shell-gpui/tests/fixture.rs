// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fixture unit tests (M28 E28-3): variants, rendered row ranges, row labels.

use std::str::FromStr;

use aegis_forum_shell_gpui::fixture::{
    FIXTURE_HEIGHT, IN_VIEW, ROW_HEIGHT, SEEDED_NODES, UNLABELLED_ROW, Variant, rendered_rows,
    row_is_labelled, row_label,
};

// --- Positive -------------------------------------------------------------

/// Positive: [`Variant`] round-trips every variant name via `FromStr` and `as_str`.
#[test]
fn variant_round_trips_all_names() -> Result<(), Box<dyn std::error::Error>> {
    let variants = [
        Variant::Sound,
        Variant::ButtonAsContainer,
        Variant::Unlabelled,
        Variant::Culled,
        Variant::HiddenShown,
    ];
    for variant in variants {
        let name = variant.as_str();
        let parsed = Variant::from_str(name)?;
        assert_eq!(parsed, variant);
    }
    Ok(())
}

/// Positive: Sound variant renders 0..1000 with every row labelled.
#[test]
fn sound_variant_renders_all_labelled() {
    let range = rendered_rows(Variant::Sound);
    assert_eq!(range, 0..SEEDED_NODES);
    for i in 0..SEEDED_NODES {
        assert!(row_is_labelled(Variant::Sound, i));
    }
}

/// Positive: `row_label(0)` equals "Node 0".
#[test]
fn row_label_zero_equals_node_zero() {
    assert_eq!(row_label(0), "Node 0");
}

// --- Negative -------------------------------------------------------------

/// Negative: unknown variant is refused and the error message names the allowed names.
#[test]
fn unknown_variant_refused_with_allowed_list() {
    let res = Variant::from_str("nonexistent-variant");
    assert!(res.is_err());
    if let Err(err_msg) = res {
        assert!(err_msg.contains("nonexistent-variant"));
        assert!(err_msg.contains("sound"));
        assert!(err_msg.contains("button-as-container"));
        assert!(err_msg.contains("unlabelled"));
        assert!(err_msg.contains("culled"));
        assert!(err_msg.contains("hidden-shown"));
    }
}

/// Negative: Unlabelled variant leaves exactly row 517 unlabelled.
#[test]
fn unlabelled_variant_leaves_exactly_row_517_unlabelled() {
    assert!(!row_is_labelled(Variant::Unlabelled, UNLABELLED_ROW));
    for i in 0..SEEDED_NODES {
        if i != UNLABELLED_ROW {
            assert!(row_is_labelled(Variant::Unlabelled, i));
        }
    }
}

/// Negative: Culled variant renders exactly `IN_VIEW` rows.
#[test]
fn culled_variant_renders_exactly_in_view_rows() {
    let range = rendered_rows(Variant::Culled);
    assert_eq!(range, 0..IN_VIEW);
    assert_eq!(range.len(), IN_VIEW);
}

// --- Boundary -------------------------------------------------------------

/// Boundary: row 999 is rendered in Sound and 1000 is not.
#[test]
fn sound_boundary_row_999_in_1000_not() {
    let range = rendered_rows(Variant::Sound);
    assert!(range.contains(&999));
    assert!(!range.contains(&1000));
}

/// Boundary: `IN_VIEW` * `ROW_HEIGHT` is within `FIXTURE_HEIGHT`.
#[test]
fn in_view_height_within_fixture_height() {
    #[expect(clippy::cast_precision_loss, reason = "IN_VIEW is small scalar 10")]
    let total_view_height = (IN_VIEW as f32) * ROW_HEIGHT;
    assert!(total_view_height <= FIXTURE_HEIGHT);
}
