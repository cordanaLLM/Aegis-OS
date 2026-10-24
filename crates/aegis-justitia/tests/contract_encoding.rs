// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis consumer schema encoding and buffer tests.
//!
//! Asserts correlation naming and payload buffer boundaries owned by Aegis OS.

mod common;

#[path = "../../test_assertions.rs"]
mod test_assertions;

use aegis_justitia::{Correlation, EdgeId, Identity, PayloadBuffer, SchemaId};

use common::{CORRELATION, Fallible};

// --- Positive -------------------------------------------------------------

/// Positive: a refusal names the contract it belongs to and the payload it
/// came from, and says so when it cannot.
#[test]
fn a_correlation_names_its_contract_and_payload() -> Fallible<()> {
    let identified = Correlation::new(
        SchemaId::ActionProposal,
        Some(Identity::parse(CORRELATION)?),
    );
    assert_eq!(identified.schema(), SchemaId::ActionProposal);
    assert_eq!(identified.id(), Some(Identity::parse(CORRELATION)?));
    assert_eq!(
        identified.to_string(),
        format!("{} [{CORRELATION}]", SchemaId::ActionProposal.tag())
    );

    let anonymous = Correlation::new(SchemaId::SignedAuditRecord, None);
    assert_eq!(anonymous.id(), None);
    assert!(anonymous.to_string().ends_with("[uncorrelated]"));
    assert_eq!(
        anonymous.schema().edge(),
        EdgeId::AuditReconstructiveCandidate
    );
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: an empty buffer yields an empty payload rather than stale bytes.
#[test]
fn an_untouched_payload_buffer_holds_nothing() {
    let buffer = PayloadBuffer::default();
    test_assertions::assert_empty_buffer_view(
        buffer.is_empty(),
        buffer.len(),
        buffer.as_bytes(),
        buffer.as_str(),
    );
    assert_eq!(PayloadBuffer::new().len(), buffer.len());
}
