// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The subsystem-graph edge P09's own contract carries, and the direction
//! dispute attached to it.
//!
//! The graph of record (export-062 `1ce919ed54bb`) draws `VERIFY_CODE_CAD` from
//! P09 Minerva to P14 Hephaestus; the dataflow document (export-002
//! `7e0c95f4ea05`) draws the same solver-backed verification the other way, and
//! REQ-GRAPH-05 records that disagreement. The M01 register carries it as
//! dispute DSP-05 against **open** decision D27.
//!
//! # Both readings stay representable, and one is carried
//!
//! [`CadVerificationDirection`] has two variants, because D27 is not settled
//! and encoding it as a single-variant enum would settle it by implementation
//! -- which is the mistake the M14 module documentation warns about for D04.
//! What this schema does is narrower and is stated in the type: a
//! [`CadVerificationRequest`](super::cad_verification::CadVerificationRequest)
//! is a **submission**, so it carries
//! [`CadVerificationDirection::MinervaSubmitsHephaestusVerifies`] and refuses
//! the other reading with
//! [`ContractError::WrongDirection`](super::ContractError::WrongDirection).
//! If D27 later closes the other way, the payload for it is a request type in
//! P14's crate and this one is retired; nothing here has to be un-decided
//! first.

/// The subsystem-graph edge this crate's own contract touches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum EdgeId {
    /// P09 to P14: parametric constraint and dimension checks.
    #[serde(rename = "VERIFY_CODE_CAD")]
    VerifyCodeCad,
}

impl EdgeId {
    /// Every edge this enum names.
    pub const ALL: [Self; 1] = [Self::VerifyCodeCad];

    /// Returns the edge identifier as it is written in the graph of record.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::VerifyCodeCad => "VERIFY_CODE_CAD",
        }
    }

    /// Returns the transport the graph of record records for this edge.
    ///
    /// Recorded verbatim. The solver it names is not admitted by this
    /// milestone and nothing in this crate acts on the string.
    #[must_use]
    pub const fn recorded_transport(self) -> &'static str {
        match self {
            Self::VerifyCodeCad => "Z3 Symbolic Solver / PAL Engine",
        }
    }
}

/// Which way the code-CAD verification edge runs (decision D27, open).
///
/// Both variants are admissible as readings, because the dispute is recorded
/// and not settled. Neither is authoritative on its own:
/// [`Self::MinervaSubmitsHephaestusVerifies`] is the graph of record, and
/// [`Self::HephaestusSubmitsMinervaVerifies`] is the dataflow document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum CadVerificationDirection {
    /// P09 submits a parametric script; P14 returns a constraint verdict.
    MinervaSubmitsHephaestusVerifies,
    /// P14 submits; P09 verifies, which is what the dataflow document draws.
    HephaestusSubmitsMinervaVerifies,
}

impl CadVerificationDirection {
    /// Both readings D27 leaves open, in a fixed order.
    pub const BOTH: [Self; 2] = [
        Self::MinervaSubmitsHephaestusVerifies,
        Self::HephaestusSubmitsMinervaVerifies,
    ];

    /// The reading the graph of record draws, and the one this crate's request
    /// carries.
    pub const GRAPH_OF_RECORD: Self = Self::MinervaSubmitsHephaestusVerifies;

    /// Returns the stable tag the wire form carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::MinervaSubmitsHephaestusVerifies => "minerva-submits-hephaestus-verifies",
            Self::HephaestusSubmitsMinervaVerifies => "hephaestus-submits-minerva-verifies",
        }
    }

    /// Returns the subsystem that submits under this reading.
    #[must_use]
    pub const fn submitter(self) -> &'static str {
        match self {
            Self::MinervaSubmitsHephaestusVerifies => "P09_Minerva",
            Self::HephaestusSubmitsMinervaVerifies => "P14_Hephaestus",
        }
    }

    /// Returns the subsystem that verifies under this reading.
    #[must_use]
    pub const fn verifier(self) -> &'static str {
        match self {
            Self::MinervaSubmitsHephaestusVerifies => "P14_Hephaestus",
            Self::HephaestusSubmitsMinervaVerifies => "P09_Minerva",
        }
    }

    /// Returns `true` when the graph of record draws this reading.
    #[must_use]
    pub const fn is_graph_of_record(self) -> bool {
        matches!(self, Self::MinervaSubmitsHephaestusVerifies)
    }

    /// Returns the source that draws this reading, cited by export identifier.
    #[must_use]
    pub const fn evidence(self) -> &'static str {
        match self {
            Self::MinervaSubmitsHephaestusVerifies => "export-062",
            Self::HephaestusSubmitsMinervaVerifies => "export-002",
        }
    }
}
