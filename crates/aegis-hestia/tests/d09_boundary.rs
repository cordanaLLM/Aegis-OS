// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Exit criterion 3: decision D09 is applied, and the boundary it asks for
//! exists as a payload.
//!
//! D09 settled where P15 lives as **both** a Rust crate and a Svelte package,
//! joined by a typed boundary. This file holds the register against what it
//! records, and holds the boundary against what a boundary has to be: a
//! snapshot, carrying no method, handle or descriptor into the crate.
//!
//! # Scope
//!
//! Applying D09 is not evidence that a Svelte package exists or reads this
//! payload. No package is added by this milestone, and nothing here is proof
//! about one.

mod common;

use aegis_hestia::{
    Citation, D09_HESTIA_LOCATION, DecisionRecord, DecisionState, HestiaLocation, HestiaView,
    MAX_QUERY_LIMIT, MIN_QUERY_LIMIT, PGLITE_OPFS_BUFFER_MB, PipView, StoreView, ViewVersion,
    WaylandPipMediaController,
};

use common::{Fallible, initialised, registered, store};

// --- Positive -------------------------------------------------------------

/// Positive: the register records the decision that was taken.
#[test]
fn the_register_records_the_decision() {
    let record: DecisionRecord = D09_HESTIA_LOCATION;
    assert_eq!(record.id, "D09");
    assert_eq!(record.state, DecisionState::Closed);
    assert_eq!(record.state.name(), "closed");
    assert_eq!(record.chosen, HestiaLocation::BothWithTypedBoundary);
    assert_eq!(record.applied_at, "M17");
    assert_eq!(record.dispute, "DSP-24");
    assert_eq!(record.touches, ["REQ-P15-05", "REQ-P15-08", "REQ-WS-01"]);
    assert!(record.question.contains("Rust crate"));
    assert!(record.boundary.contains("snapshot"));
}

/// Positive: the chosen option puts the logic in Rust and the interface in
/// Svelte, and this crate is the Rust half.
#[test]
fn the_chosen_option_covers_both_halves() {
    let chosen = D09_HESTIA_LOCATION.chosen;
    assert!(chosen.includes_rust_crate());
    assert!(chosen.includes_ui_package());
    assert_eq!(chosen.name(), "both-with-typed-boundary");
    assert_eq!(HestiaLocation::ALL.len(), 3);
}

/// Positive: the boundary payload reports what the interface needs.
#[test]
fn the_boundary_payload_reports_the_store_and_the_surface() -> Fallible {
    let view: HestiaView = HestiaView::snapshot(&initialised()?, &registered()?);
    assert_eq!(view.schema, ViewVersion::V1);
    assert_eq!(view.schema.tag(), "aegis.p15.ui-view.v1");

    let store: StoreView = view.store;
    assert!(store.initialised);
    assert_eq!(store.buffer_mb, PGLITE_OPFS_BUFFER_MB);
    assert_eq!(store.min_query_limit, MIN_QUERY_LIMIT);
    assert_eq!(store.max_query_limit, MAX_QUERY_LIMIT);

    let pip: PipView = view.pip;
    assert!(pip.active);
    assert_eq!(pip.width, Some(640));
    assert_eq!(pip.height, Some(360));
    Ok(())
}

/// Positive: the payload round-trips as JSON, which is what crossing a
/// language boundary costs.
#[test]
fn the_boundary_payload_round_trips_as_json() -> Fallible {
    let view = HestiaView::snapshot(&initialised()?, &registered()?);
    let text = serde_json::to_string(&view)?;
    assert!(text.starts_with("{\"schema\":\"aegis.p15.ui-view.v1\""));
    assert_eq!(serde_json::from_str::<HestiaView>(&text)?, view);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: the rejected options stay representable and stay rejected.
///
/// A register that could only spell the option it chose would be stating a
/// fact, not recording a decision.
#[test]
fn the_rejected_options_are_recorded_and_distinct() {
    let record = D09_HESTIA_LOCATION;
    assert_eq!(
        record.rejected,
        [
            HestiaLocation::RustCrateOnly,
            HestiaLocation::SveltePackageOnly
        ]
    );
    assert!(!record.rejected.contains(&record.chosen));
    assert!(!HestiaLocation::RustCrateOnly.includes_ui_package());
    assert!(!HestiaLocation::SveltePackageOnly.includes_rust_crate());
    assert_eq!(DecisionState::Unresolved.name(), "unresolved");
}

/// Negative: a payload with an unknown field does not decode.
///
/// The boundary is versioned so a package built against another shape fails
/// rather than reading a field that moved.
#[test]
fn a_payload_with_an_unknown_field_does_not_decode() -> Fallible {
    let view = HestiaView::snapshot(&store()?, &WaylandPipMediaController::new());
    let text = serde_json::to_string(&view)?;
    let extra = text.replace("{\"schema\"", "{\"stowaway\":1,\"schema\"");
    assert!(serde_json::from_str::<HestiaView>(&extra).is_err());

    let renamed = text.replace("aegis.p15.ui-view.v1", "aegis.p15.ui-view.v2");
    assert!(serde_json::from_str::<HestiaView>(&renamed).is_err());
    Ok(())
}

/// Negative: the boundary carries no descriptor into the interface.
///
/// A `DMA-BUF` descriptor is meaningful only inside the process that holds it,
/// so handing its number across would be misleading. The surface's extents
/// cross; its descriptor does not.
#[test]
fn the_boundary_carries_no_descriptor() -> Fallible {
    let view = HestiaView::snapshot(&initialised()?, &registered()?);
    let text = serde_json::to_string(&view)?;
    assert!(!text.contains("dma"));
    assert!(!text.contains("\"fd\""));
    assert!(!text.contains(&common::SCAFFOLD_FD.to_string()));
    Ok(())
}

/// Negative: no crate source ships a user interface.
///
/// A regression gate over a named list of nine identifiers, not a proof that
/// no interface could ever be added. The list is deliberately made of markup,
/// bundler and embedding constructs rather than of the word "Svelte": the
/// decision register has to be able to name the option it rejected.
#[test]
fn no_source_ships_a_user_interface() -> Fallible {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let watched = [
        "<div",
        "<script",
        "html!",
        "include_str!",
        "include_bytes!",
        "wry",
        "webkit",
        "pnpm",
        "node_modules",
    ];
    assert_eq!(watched.len(), 9);
    let found = common::scan_sources(&root, &watched)?;
    assert!(
        found.is_empty(),
        "interface identifiers in library code: {found:?}"
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: an empty snapshot is a valid snapshot, and says so.
#[test]
fn an_empty_snapshot_is_still_a_snapshot() -> Fallible {
    let view = HestiaView::snapshot(&store()?, &WaylandPipMediaController::new());
    assert!(!view.store.initialised);
    assert!(!view.pip.active);
    assert_eq!(view.pip.width, None);
    assert_eq!(view.pip.height, None);
    let text = serde_json::to_string(&view)?;
    assert_eq!(serde_json::from_str::<HestiaView>(&text)?, view);
    Ok(())
}

/// Boundary: every source of the decision is cited by identifier and prefix
/// only, never by path.
#[test]
fn every_source_is_cited_by_identifier_and_prefix() {
    for source in D09_HESTIA_LOCATION.sources {
        let citation: Citation = source;
        assert!(citation.export.starts_with("export-"));
        assert_eq!(citation.sha256_prefix.len(), 12);
        assert!(
            citation
                .sha256_prefix
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        );
        assert!(!citation.export.contains('/'));
    }
}
