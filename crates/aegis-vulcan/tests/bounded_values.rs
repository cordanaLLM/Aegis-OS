// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded field types, at their bounds.
//!
//! Every field this crate puts on a wire validates while it is decoded rather
//! than after, which is only worth anything if the validation is exact. This
//! file holds each bound at the value that passes and the value that does not.

mod common;

use aegis_vulcan::{
    Correlation, CorrelationId, IdError, IommuGroup, Lba, MAX_CORRELATION_LEN, RENDER_NODE_PREFIX,
    SchemaId, VfioDeviceConfig, VramAddress,
};

use common::{CORRELATION, Fallible, bar, device};

// --- Positive -------------------------------------------------------------

/// Positive: a well-formed correlation identifier parses and renders back.
#[test]
fn a_correlation_identifier_round_trips() -> Fallible {
    let id = CorrelationId::parse(CORRELATION)?;
    assert_eq!(id.to_string(), CORRELATION);
    assert_eq!(id.len(), CORRELATION.len());
    assert!(!id.is_empty());
    assert_eq!(id.as_bytes(), CORRELATION.as_bytes());
    Ok(())
}

/// Positive: the device configuration carries the scaffold's own values.
#[test]
fn the_device_configuration_carries_its_fields() -> Fallible {
    let config: VfioDeviceConfig = device()?;
    assert_eq!(config.group, IommuGroup::new(12));
    assert_eq!(config.vendor_id, 0x144d);
    assert_eq!(config.device_id, 0xa808);
    assert_eq!(config.bar0, bar()?);
    assert_eq!(config.validate(), Ok(()));
    Ok(())
}

/// Positive: the addressing newtypes keep their values and stay distinct.
#[test]
fn the_addressing_newtypes_keep_their_values() {
    let lba = Lba::new(0x1000);
    let destination = VramAddress::new(0xe000_0000);
    assert_eq!(lba.get(), 0x1000);
    assert_eq!(destination.get(), 0xe000_0000);
    assert_eq!(IommuGroup::new(12).get(), 12);
}

/// Positive: a correlation renders its schema, with and without an identifier.
#[test]
fn a_correlation_renders_its_schema() -> Fallible {
    let id = CorrelationId::parse(CORRELATION)?;
    let with = Correlation::new(SchemaId::MediaIngest, Some(id));
    assert_eq!(with.schema(), SchemaId::MediaIngest);
    assert_eq!(with.id(), Some(id));
    assert!(with.to_string().contains(CORRELATION));

    let without = Correlation::new(SchemaId::WeightStream, None);
    assert_eq!(without.id(), None);
    assert!(without.to_string().ends_with("[uncorrelated]"));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: an empty identifier is refused rather than defaulted.
#[test]
fn an_empty_correlation_identifier_is_refused() {
    assert_eq!(CorrelationId::parse(""), Err(IdError::Empty));
}

/// Negative: an identifier outside the charset is refused, never sanitised.
#[test]
fn an_out_of_charset_identifier_is_refused() {
    for raw in [
        "has space",
        "curly{brace}",
        "quote\"mark",
        "new\nline",
        "caf\u{e9}",
    ] {
        assert_eq!(
            CorrelationId::parse(raw),
            Err(IdError::Charset),
            "{raw:?} was not refused"
        );
    }
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the identifier is admitted at its bound and refused one past it.
#[test]
fn the_identifier_bound_is_exact() {
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

    let far_past = "a".repeat(512);
    assert_eq!(
        CorrelationId::parse(&far_past),
        Err(IdError::TooLong {
            max: MAX_CORRELATION_LEN,
            actual: 512,
        })
    );
}

/// Boundary: a single character is an admissible identifier.
#[test]
fn a_single_character_identifier_is_admissible() -> Fallible {
    let id = CorrelationId::parse("a")?;
    assert_eq!(id.len(), 1);
    assert_eq!(id.to_string(), "a");
    Ok(())
}

/// Boundary: the render-node prefix is exactly what the device name carries.
#[test]
fn the_render_node_prefix_is_the_device_name_prefix() {
    assert_eq!(RENDER_NODE_PREFIX, "renderD");
    assert!(format!("{RENDER_NODE_PREFIX}129").starts_with(RENDER_NODE_PREFIX));
}
