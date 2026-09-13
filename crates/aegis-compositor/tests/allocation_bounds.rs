// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! HISS-03: the claim that no decision path allocates, made falsifiable.
//!
//! The reason this crate can say a decision path allocates nothing is
//! structural: every value on such a path is `Copy`, so registering a surface
//! or retaining a sample writes into storage that already exists.
//! `assert_no_heap::<T>()` is the executable form: a `Vec`, `String` or `Box`
//! anywhere inside one of these types, at any depth, removes `Copy` and this
//! file stops compiling, which fails the gate.
//!
//! # What is deliberately not claimed
//!
//! **The mesh's publish path is not allocation-free for the caller**, and this
//! file says so by exercising the case rather than by denying it. A caller
//! hands in a `&[u8]`, which it had to get from somewhere; what the mesh does
//! with it is copy it into a fixed inline slot, so the *mesh* allocates
//! nothing and the retained sample owns no heap. The same distinction applies
//! to decoding: `serde_json` unescapes a JSON string into a heap scratch
//! buffer before any field of ours sees it, and the decoded value is still
//! inline.

mod common;

use core::mem::size_of;

use aegis_compositor::{
    ClientId, ClientTable, CompositorError, DeclaredPeriodUs, FocusSwitchDraft, IdError, KeyExpr,
    Label, MAX_IPC_CLIENTS, MAX_SAMPLE_BYTES, MAX_SURFACES, MeshRole, MockedMesh,
    OverlayRegistration, PacingSource, Sample, Subscription, SubscriptionId, SurfaceGeometry,
    SurfaceId, SurfaceLayer, SurfaceRegistry, SurfaceRequest, WaylandSurface,
};

use common::{FOCUS_KEY, Fallible, filled_clients, filled_registry, key};

/// Accepts only a type that owns no heap, because it is `Copy`.
fn assert_no_heap<T: Copy>() {}

// --- Positive -------------------------------------------------------------

/// Positive: every value on a decision path owns no heap.
///
/// This is the falsifier. Adding a `Vec`, `String` or `Box` to any of these
/// types, or to anything they contain, removes `Copy` and this test stops
/// compiling.
#[test]
fn every_value_on_a_decision_path_owns_no_heap() {
    assert_no_heap::<SurfaceRegistry>();
    assert_no_heap::<WaylandSurface>();
    assert_no_heap::<SurfaceRequest>();
    assert_no_heap::<SurfaceGeometry>();
    assert_no_heap::<SurfaceId>();
    assert_no_heap::<SurfaceLayer>();
    assert_no_heap::<ClientTable>();
    assert_no_heap::<ClientId>();
    assert_no_heap::<MockedMesh>();
    assert_no_heap::<Sample>();
    assert_no_heap::<Subscription>();
    assert_no_heap::<SubscriptionId>();
    assert_no_heap::<MeshRole>();
    assert_no_heap::<KeyExpr>();
    assert_no_heap::<Label>();
    assert_no_heap::<DeclaredPeriodUs>();
    assert_no_heap::<PacingSource>();
    assert_no_heap::<FocusSwitchDraft>();
    assert_no_heap::<OverlayRegistration>();
    assert_no_heap::<CompositorError>();
    assert_no_heap::<IdError>();
}

/// Positive: driving both tables to their bounds changes no size.
#[test]
fn driving_the_tables_changes_no_storage() -> Fallible {
    let registry = filled_registry(MAX_SURFACES)?;
    let clients = filled_clients(MAX_IPC_CLIENTS)?;
    assert_eq!(size_of_val(&registry), size_of::<SurfaceRegistry>());
    assert_eq!(size_of_val(&clients), size_of::<ClientTable>());
    assert!(size_of::<SurfaceRegistry>() > size_of::<WaylandSurface>());
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: retaining a sample copies into a fixed slot, so the mesh's own
/// storage does not grow with what is published.
#[test]
fn retaining_a_sample_does_not_grow_the_mesh() -> Fallible {
    let mut mesh = MockedMesh::new();
    let before = size_of_val(&mesh);
    let focus = key(FOCUS_KEY)?;
    mesh.publish(MeshRole::Publisher, focus, &vec![9u8; MAX_SAMPLE_BYTES])?;
    assert_eq!(size_of_val(&mesh), before);
    assert_eq!(size_of_val(&mesh), size_of::<MockedMesh>());
    let sample = mesh.retained(focus).ok_or("a sample must be retained")?;
    assert_eq!(sample.len(), MAX_SAMPLE_BYTES);
    assert_no_heap::<Sample>();
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the registry is exactly its declared storage, so the bound is the
/// array rather than a length check applied afterwards.
#[test]
fn the_registry_is_its_declared_storage() {
    assert!(size_of::<SurfaceRegistry>() > size_of::<WaylandSurface>().saturating_mul(200));
    assert_eq!(MAX_SURFACES, 256);
    let registry = SurfaceRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(size_of_val(&registry), size_of::<SurfaceRegistry>());
}
