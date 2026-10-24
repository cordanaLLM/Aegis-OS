// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! HISS-03: allocation bounds on decoded contract payloads.
//!
//! The M14 consumer contracts hold to this standard: a decoded contract payload
//! owns no heap, and an escape-free payload is decoded without a copy; a payload
//! that spells the same value with a JSON escape is not, because `serde_json`
//! unescapes into a heap scratch buffer. That last case is the falsifier for
//! "decoding allocates nothing", and it is asserted rather than avoided.

mod common;

use aegis_justitia::{
    ActionProposal, DecisionRequest, MAX_CONTRACT_PAYLOAD_BYTES, MAX_TARGET_LEN, OversightClass,
    RiskTier, SignedAuditRecord, TargetResource,
};

use common::{
    CORRELATION, Fallible, assert_action_proposal_over_bound_refused, audit_record,
    decision_request, encoded, proposal,
};

// --- Milestone M14: what decoding one contract payload costs --------------

/// Whether `serde_json` handed a string field a slice borrowed straight out of
/// the payload, or a copy it had to build in a heap scratch buffer first.
///
/// This is the allocation signal a test in this crate can actually observe.
/// The workspace sets `unsafe_code = "forbid"` for every target of the package,
/// so a counting `#[global_allocator]` cannot be written here: implementing
/// `GlobalAlloc` needs `unsafe impl`, which is a hard error rather than a
/// warning. The distinction below is not a proxy for the allocation, it is the
/// allocation. `serde_json` can hand a visitor a borrowed slice only while the
/// field is literally present in the input; the moment the field carries an
/// escape it must unescape into a `Vec<u8>` scratch buffer and pass a reference
/// into that buffer instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Provenance {
    /// The field was a slice of the input: no scratch, and no heap.
    Borrowed,
    /// The field carried a JSON escape, so the decoder unescaped it into a heap
    /// scratch buffer before the visitor ever saw it.
    Copied,
}

/// The visitor that records which of the two `serde_json` used.
struct ProvenanceVisitor;

impl<'de> serde::de::Visitor<'de> for ProvenanceVisitor {
    type Value = Provenance;

    fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("any JSON string")
    }

    fn visit_borrowed_str<E: serde::de::Error>(self, _value: &'de str) -> Result<Provenance, E> {
        Ok(Provenance::Borrowed)
    }

    fn visit_str<E: serde::de::Error>(self, _value: &str) -> Result<Provenance, E> {
        Ok(Provenance::Copied)
    }
}

impl<'de> serde::Deserialize<'de> for Provenance {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(ProvenanceVisitor)
    }
}

/// Reads one field of any contract payload for its provenance and nothing else.
///
/// Every other field is unknown to this view and is skipped, so the witness
/// works against all three schemas without naming any of them.
#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
struct CorrelationWitness {
    /// How the correlation identifier reached the visitor.
    correlation_id: Provenance,
}

/// Returns how `serde_json` delivered the correlation identifier of `text`.
///
/// # Errors
///
/// Propagates a parse failure.
fn provenance(text: &str) -> Fallible<Provenance> {
    Ok(serde_json::from_str::<CorrelationWitness>(text)?.correlation_id)
}

/// Returns `text` with the correlation identifier spelled through a `\u0069`
/// escape: the same string, one JSON escape sequence, nothing else changed.
fn escaped(text: &str) -> String {
    let plain = format!("\"{CORRELATION}\"");
    let escape = format!("\"\\u0069{}\"", CORRELATION.trim_start_matches('i'));
    text.replace(&plain, &escape)
}

/// Accepts only a `Copy` value, and returns its width.
///
/// The bound is the assertion: a `String`, a `Vec`, a `Box` or any other
/// heap-owning field anywhere inside the type makes it non-`Copy`, and this
/// stops compiling. It is a stronger statement than any run-time count, and it
/// is the half of the allocation claim that survived falsification.
fn inline_only<T: Copy>(_value: &T) -> usize {
    core::mem::size_of::<T>()
}

// --- Positive -------------------------------------------------------------

/// Positive: a decoded contract payload owns no heap, and a whole decoded
/// payload fits inside the same scalar bound as its wire form.
#[test]
fn a_decoded_contract_payload_owns_no_heap() -> Fallible<()> {
    let widths = [
        inline_only(&ActionProposal::decode(&encoded(&proposal()?)?)?),
        inline_only(&DecisionRequest::decode(&encoded(&decision_request(
            RiskTier::TierAConsequential,
            OversightClass::Standard,
            300,
        )?)?)?),
        inline_only(&SignedAuditRecord::decode(&encoded(&audit_record(1)?)?)?),
    ];
    for width in widths {
        assert!(width > 0);
        assert!(
            width <= MAX_CONTRACT_PAYLOAD_BYTES,
            "a decoded payload of {width} bytes is wider than the \
             {MAX_CONTRACT_PAYLOAD_BYTES}-byte bound on its wire form"
        );
    }
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: one JSON escape makes the decoder copy a field into a heap scratch
/// buffer, so "decoding allocates nothing" is false as stated.
///
/// This is the falsifier. Both spellings are accepted and both decode to the
/// same value, so the escape is not a hostile payload that could simply be
/// refused: it is an ordinary, admissible spelling that costs heap. What is
/// true, and what the contract documentation now claims, is the narrower
/// property the other two tests here assert.
#[test]
fn one_json_escape_makes_the_decoder_copy_a_field_into_scratch() -> Fallible<()> {
    let payloads = [
        encoded(&proposal()?)?,
        encoded(&decision_request(
            RiskTier::TierAConsequential,
            OversightClass::Standard,
            300,
        )?)?,
        encoded(&audit_record(1)?)?,
    ];
    for plain in payloads {
        let with_escape = escaped(&plain);
        assert_ne!(plain, with_escape, "the fixture carries the identifier");
        assert_eq!(provenance(&plain)?, Provenance::Borrowed);
        assert_eq!(
            provenance(&with_escape)?,
            Provenance::Copied,
            "an escaped field cannot be borrowed, so the decoder allocated"
        );
    }

    let plain = encoded(&proposal()?)?;
    assert_eq!(
        ActionProposal::decode(&escaped(&plain))?,
        ActionProposal::decode(&plain)?,
        "the escaped spelling is admissible and decodes to the same value"
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: whatever scratch a decode takes is bounded by
/// [`MAX_CONTRACT_PAYLOAD_BYTES`], because a longer payload never reaches the
/// parser at all.
///
/// The widest escape a contract admits is a target resource of
/// [`MAX_TARGET_LEN`] bytes with every byte escaped, which doubles the field on
/// the wire. It still fits the payload bound, and the buffer it is unescaped
/// into is narrower than the payload that carried it. One byte past the bound
/// is refused before parsing, so no scratch is reserved for it.
#[test]
fn no_decoder_scratch_can_exceed_the_contract_payload_bound() -> Fallible<()> {
    let mut widest = proposal()?;
    widest.target = TargetResource::parse(&"/".repeat(MAX_TARGET_LEN))?;
    let plain = encoded(&widest)?;
    let with_escapes = plain.replace('/', "\\/");
    assert!(
        with_escapes.len() > plain.len(),
        "the escape widened the field"
    );
    assert!(
        with_escapes.len() <= MAX_CONTRACT_PAYLOAD_BYTES,
        "the widest escaped payload is {} bytes, past the bound",
        with_escapes.len()
    );
    let decoded = ActionProposal::decode(&with_escapes)?;
    assert_eq!(decoded, widest);
    assert!(
        decoded.target.len() <= with_escapes.len(),
        "the buffer the field is unescaped into is narrower than the payload \
         that carried it, so the payload bound bounds it too"
    );
    assert!(decoded.target.len() <= MAX_CONTRACT_PAYLOAD_BYTES);

    assert_action_proposal_over_bound_refused();
    Ok(())
}
