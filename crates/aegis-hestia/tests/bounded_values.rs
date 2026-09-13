// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded field types, at their bounds.
//!
//! Every field this crate puts on a wire or into a snapshot validates while it
//! is built rather than afterwards, which is only worth anything if the
//! validation is exact. This file holds each bound at the value that passes
//! and the value that does not.

mod common;

use aegis_hestia::{
    Correlation, CorrelationId, EMBEDDING_DIMENSIONS, Embedding, HestiaError, IdError,
    MAX_CORRELATION_LEN, MAX_STORAGE_PATH_LEN, MAX_SUBSCRIBERS, MAX_SURFACE_ID_LEN,
    PGLITE_SUBVOLUME, SCAFFOLD_STORAGE_PATH, SchemaId, StoragePath, SurfaceId,
};

use common::{CORRELATION, Fallible, STORAGE, SURFACE, correlation, embedding, surface_id};

// --- Positive -------------------------------------------------------------

/// Positive: the scaffold's own storage path parses and renders back.
#[test]
fn the_scaffold_storage_path_round_trips() -> Fallible {
    assert_eq!(SCAFFOLD_STORAGE_PATH, STORAGE);
    let path: StoragePath = StoragePath::parse(SCAFFOLD_STORAGE_PATH)?;
    assert_eq!(path.to_string(), SCAFFOLD_STORAGE_PATH);
    assert_eq!(path.len(), SCAFFOLD_STORAGE_PATH.len());
    assert!(!path.is_empty());
    assert_eq!(path.as_bytes(), SCAFFOLD_STORAGE_PATH.as_bytes());
    Ok(())
}

/// Positive: the recorded subvolume is a legal path component.
#[test]
fn the_recorded_subvolume_is_a_legal_component() -> Fallible {
    assert_eq!(PGLITE_SUBVOLUME, "@pglite");
    let path = StoragePath::parse(&format!("/var/lib/{PGLITE_SUBVOLUME}"))?;
    assert!(path.to_string().ends_with(PGLITE_SUBVOLUME));
    Ok(())
}

/// Positive: the identifiers parse and render back.
#[test]
fn the_identifiers_round_trip() -> Fallible {
    assert_eq!(correlation()?.to_string(), CORRELATION);
    let id: SurfaceId = surface_id()?;
    assert_eq!(id.to_string(), SURFACE);
    assert_eq!(id.len(), SURFACE.len());
    assert!(!id.is_empty());
    assert_eq!(id.as_bytes(), SURFACE.as_bytes());
    Ok(())
}

/// Positive: an embedding keeps its components, and its width is recorded.
#[test]
fn an_embedding_keeps_its_components() {
    let vector: Embedding = embedding();
    assert_eq!(vector.components().len(), EMBEDDING_DIMENSIONS);
    assert_eq!(EMBEDDING_DIMENSIONS, 4);
    assert_eq!(vector.components().first(), Some(&0.12));
    assert_eq!(MAX_SUBSCRIBERS, 16);
}

/// Positive: a correlation renders its schema, with and without an identifier.
#[test]
fn a_correlation_renders_its_schema() -> Fallible {
    let with = Correlation::new(SchemaId::OverlayRegistration, Some(correlation()?));
    assert_eq!(with.schema(), SchemaId::OverlayRegistration);
    assert_eq!(with.id(), Some(correlation()?));
    assert!(with.to_string().contains(CORRELATION));

    let without = Correlation::new(SchemaId::OverlayRegistration, None);
    assert_eq!(without.id(), None);
    assert!(without.to_string().ends_with("[uncorrelated]"));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Returns the reason a path was refused for, or a message when it parsed.
fn refusal(raw: &str) -> Result<&'static str, Box<dyn std::error::Error>> {
    match StoragePath::parse(raw) {
        Ok(path) => Err(format!("{raw:?} parsed as {path}").into()),
        Err(HestiaError::StoragePath { reason }) => Ok(reason),
        Err(other) => Err(format!("{raw:?} was refused as {other:?}").into()),
    }
}

/// Negative: a relative or traversing path is refused, never normalised.
#[test]
fn a_relative_or_traversing_path_is_refused() -> Fallible {
    for raw in ["var/pglite", "pglite", "./pglite"] {
        assert!(
            refusal(raw)?.contains("absolute"),
            "{raw:?} was not refused"
        );
    }
    assert!(refusal("/var/../etc/pglite")?.contains("parent-directory"));
    Ok(())
}

/// Negative: an empty path and an out-of-charset path are refused.
#[test]
fn an_empty_or_out_of_charset_path_is_refused() -> Fallible {
    assert!(refusal("")?.contains("empty"));
    for raw in ["/var/pg lite", "/var/pg\"lite", "/var/pg\nlite"] {
        assert!(
            refusal(raw)?.contains("character set"),
            "{raw:?} was not refused"
        );
    }
    Ok(())
}

/// Negative: an empty or out-of-charset identifier is refused.
#[test]
fn an_empty_or_out_of_charset_identifier_is_refused() {
    assert_eq!(CorrelationId::parse(""), Err(IdError::Empty));
    assert_eq!(SurfaceId::parse(""), Err(IdError::Empty));
    for raw in ["has space", "curly{brace}", "quote\"mark"] {
        assert_eq!(CorrelationId::parse(raw), Err(IdError::Charset));
        assert_eq!(SurfaceId::parse(raw), Err(IdError::Charset));
    }
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the path is admitted at its bound and refused one past it.
#[test]
fn the_storage_path_bound_is_exact() -> Fallible {
    let tail = "a".repeat(MAX_STORAGE_PATH_LEN.saturating_sub(1));
    let at_bound = format!("/{tail}");
    assert_eq!(
        StoragePath::parse(&at_bound).map(|path| path.len()),
        Ok(MAX_STORAGE_PATH_LEN)
    );
    assert!(refusal(&format!("/{tail}a"))?.contains("bound"));
    Ok(())
}

/// Boundary: the identifiers are admitted at their bounds and refused past.
#[test]
fn the_identifier_bounds_are_exact() {
    let at_bound = "a".repeat(MAX_CORRELATION_LEN);
    assert_eq!(
        CorrelationId::parse(&at_bound).map(|id| id.len()),
        Ok(MAX_CORRELATION_LEN)
    );
    let past = "a".repeat(MAX_CORRELATION_LEN.saturating_add(1));
    assert_eq!(
        CorrelationId::parse(&past),
        Err(IdError::TooLong {
            max: MAX_CORRELATION_LEN,
            actual: MAX_CORRELATION_LEN.saturating_add(1),
        })
    );

    let surface_at_bound = "s".repeat(MAX_SURFACE_ID_LEN);
    assert_eq!(
        SurfaceId::parse(&surface_at_bound).map(|id| id.len()),
        Ok(MAX_SURFACE_ID_LEN)
    );
    assert_eq!(
        SurfaceId::parse(&"s".repeat(MAX_SURFACE_ID_LEN.saturating_add(1))),
        Err(IdError::TooLong {
            max: MAX_SURFACE_ID_LEN,
            actual: MAX_SURFACE_ID_LEN.saturating_add(1),
        })
    );
    assert_eq!(
        SurfaceId::parse(&"s".repeat(512)),
        Err(IdError::TooLong {
            max: MAX_SURFACE_ID_LEN,
            actual: 512,
        })
    );
}

/// Boundary: the shortest admissible path is the root itself.
#[test]
fn the_shortest_admissible_path_is_the_root() -> Fallible {
    let root = StoragePath::parse("/")?;
    assert_eq!(root.len(), 1);
    assert_eq!(root.to_string(), "/");
    Ok(())
}
