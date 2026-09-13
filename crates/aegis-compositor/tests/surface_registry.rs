// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E07-1, the registry half: 256 surfaces and the 257th.
//!
//! Positive: 256 surfaces are accepted, and every field of a row reads back.
//! Negative: the 257th is refused, a duplicate identifier is refused, and
//! focusing a surface the registry does not hold is refused **without clearing
//! the focus it already had** -- which is what the scaffold does instead.
//! Boundary: the 256th is accepted and the 257th is not.

mod common;

use aegis_compositor::{
    CompositorError, MAX_SURFACES, SurfaceGeometry, SurfaceId, SurfaceLayer, SurfaceRegistry,
    UNIT_MEMORY_MAX_MIB, WaylandSurface,
};

use common::{Fallible, SHELL_PID, filled_registry, shell_request};

// --- Positive -------------------------------------------------------------

/// Positive: the registry accepts its full complement of surfaces.
#[test]
fn the_registry_accepts_two_hundred_and_fifty_six_surfaces() -> Fallible {
    let registry = filled_registry(MAX_SURFACES)?;
    assert_eq!(registry.count(), MAX_SURFACES);
    assert_eq!(MAX_SURFACES, 256);
    assert!(!registry.is_empty());
    assert_eq!(registry.focused(), None);
    Ok(())
}

/// Positive: a registered row reads back with every field it was given.
#[test]
fn a_registered_row_reads_back() -> Fallible {
    let registry = common::scaffold_registry()?;
    let row: WaylandSurface = registry
        .get(SurfaceId::new(1))
        .ok_or("the registry must hold the surface it registered")?;
    let fields = (
        row.id,
        row.title.to_string(),
        row.geometry,
        row.layer,
        row.pid,
        row.focused,
    );
    assert_eq!(
        fields,
        (
            SurfaceId::new(1),
            common::SHELL.to_owned(),
            SurfaceGeometry::new(1920, 1080),
            SurfaceLayer::Top,
            SHELL_PID,
            false,
        )
    );
    Ok(())
}

/// Positive: focusing a surface returns the row that now holds focus, and
/// clears the flag on every other row.
#[test]
fn focusing_a_surface_returns_the_row_that_holds_it() -> Fallible {
    let mut registry = common::scaffold_registry()?;
    let shell = SurfaceId::new(1);
    let focused = registry.focus(shell)?;
    assert_eq!(focused.id, shell);
    assert_eq!(focused.pid, SHELL_PID);
    assert!(focused.focused);
    assert_eq!(registry.focused(), Some(shell));
    let other = registry
        .get(SurfaceId::new(2))
        .ok_or("the overlay must still be held")?;
    assert!(!other.focused);
    Ok(())
}

/// Positive: the layers are counted per layer, and each carries a distinct
/// stacking index and name.
#[test]
fn the_layers_are_counted_and_ordered() -> Fallible {
    let registry = common::scaffold_registry()?;
    assert_eq!(registry.layer_count(SurfaceLayer::Top), 1);
    assert_eq!(registry.layer_count(SurfaceLayer::Overlay), 1);
    assert_eq!(registry.layer_count(SurfaceLayer::Background), 0);
    assert_eq!(registry.layer_count(SurfaceLayer::Bottom), 0);
    let indices: Vec<u32> = SurfaceLayer::ALL.iter().map(|l| l.index()).collect();
    assert_eq!(indices, vec![0, 1, 2, 3]);
    assert!(SurfaceLayer::Overlay.is_admission_gate());
    assert!(!SurfaceLayer::Top.is_admission_gate());
    assert_eq!(SurfaceLayer::Background.name(), "background");
    Ok(())
}

/// Positive: the unit's recorded memory ceiling is carried as a number, not
/// applied as one.
#[test]
fn the_unit_memory_ceiling_is_recorded() {
    assert_eq!(UNIT_MEMORY_MAX_MIB, 256);
}

// --- Negative -------------------------------------------------------------

/// Negative: the 257th surface is refused with the bound it hit.
#[test]
fn the_two_hundred_and_fifty_seventh_surface_is_refused() -> Fallible {
    let mut registry = filled_registry(MAX_SURFACES)?;
    let raw = u32::try_from(MAX_SURFACES)
        .unwrap_or(u32::MAX)
        .saturating_add(1);
    let refusal = registry.register(shell_request(raw)?);
    assert_eq!(
        refusal,
        Err(CompositorError::SurfaceRegistryFull { max: MAX_SURFACES })
    );
    assert_eq!(registry.count(), MAX_SURFACES);
    Ok(())
}

/// Negative: a duplicate identifier is refused rather than stored twice.
///
/// The scaffold pushes a second row with the same identifier, after which
/// focusing it focuses whichever copy the sweep reaches first.
#[test]
fn a_duplicate_identifier_is_refused() -> Fallible {
    let mut registry = common::scaffold_registry()?;
    let refusal = registry.register(shell_request(1)?);
    assert_eq!(refusal, Err(CompositorError::DuplicateSurface { id: 1 }));
    assert_eq!(registry.count(), 2);
    Ok(())
}

/// Negative: focusing a surface the registry does not hold is refused, and
/// leaves the focus it already had.
///
/// The scaffold sweeps the whole table clearing every focus flag and setting
/// none, so an unknown identifier leaves the session with nothing focused.
#[test]
fn focusing_an_unknown_surface_leaves_the_existing_focus() -> Fallible {
    let mut registry = common::scaffold_registry()?;
    let shell = SurfaceId::new(1);
    registry.focus(shell)?;
    let refusal = registry.focus(SurfaceId::new(99));
    assert_eq!(refusal, Err(CompositorError::UnknownSurface { id: 99 }));
    assert_eq!(registry.focused(), Some(shell));
    let row = registry.get(shell).ok_or("the shell must still be held")?;
    assert!(row.focused);
    Ok(())
}

/// Negative: an empty registry holds nothing and focuses nothing.
#[test]
fn an_empty_registry_holds_nothing() {
    let mut registry = SurfaceRegistry::default();
    assert!(registry.is_empty());
    assert_eq!(registry.count(), 0);
    assert_eq!(registry.focused(), None);
    assert_eq!(registry.get(SurfaceId::new(1)), None);
    assert_eq!(
        registry.focus(SurfaceId::new(1)),
        Err(CompositorError::UnknownSurface { id: 1 })
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the 256th surface is accepted and the 257th is not.
#[test]
fn the_bound_is_exact_at_two_hundred_and_fifty_six() -> Fallible {
    let mut registry = filled_registry(MAX_SURFACES.saturating_sub(1))?;
    assert_eq!(registry.count(), 255);
    let last = u32::try_from(MAX_SURFACES).unwrap_or(u32::MAX);
    let id = registry.register(shell_request(last)?)?;
    assert_eq!(id, SurfaceId::new(256));
    assert_eq!(registry.count(), MAX_SURFACES);
    let over = last.saturating_add(1);
    assert!(registry.register(shell_request(over)?).is_err());
    Ok(())
}

/// Boundary: a refused registration stores nothing, so the bound is a wall
/// rather than a warning.
#[test]
fn a_refused_registration_stores_nothing() -> Fallible {
    let mut registry = filled_registry(MAX_SURFACES)?;
    let raw = u32::try_from(MAX_SURFACES)
        .unwrap_or(u32::MAX)
        .saturating_add(1);
    assert!(registry.register(shell_request(raw)?).is_err());
    assert_eq!(registry.get(SurfaceId::new(raw)), None);
    assert_eq!(registry.count(), MAX_SURFACES);
    Ok(())
}
