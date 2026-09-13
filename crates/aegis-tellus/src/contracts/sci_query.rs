// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The candidate SCI exchange on edge `EVALUATE_CANDIDATE_CARBON_SCI`.
//!
//! # Where the fields come from
//!
//! | Field | Source |
//! | :-- | :-- |
//! | `edge` | export-062 `1ce919ed54bb`, edge `EVALUATE_CANDIDATE_CARBON_SCI`, P16 to P13 |
//! | `candidates`, `energy-kwh`, `functional-units` | export-036 `25813d240733`, `calculate_sci_rate(energy_kwh, functional_units)` |
//! | `intensity`, `rates`, `deferred` | export-036 `25813d240733`, `SciCalculation` and `should_defer_spatiotemporal_tasks` |
//! | `correlation-id` | `docs/integration/stack.md`: every crossing carries a correlation identifier |
//!
//! # The empty list is a refusal, not an answer
//!
//! [`CandidateSciQuery::validate`] refuses a query carrying no candidate with
//! [`ContractError::EmptyCandidateList`]. The alternative -- answering it with
//! an empty rate list -- produces bytes a consumer that does not look closely
//! reads as "nothing exceeded its carbon bound", and P16 promotes on exactly
//! that reading. A question with no subject is refused at the boundary.
//!
//! # The response must answer the question it claims to answer
//!
//! [`CandidateSciResponse::answers`] checks the response against its query:
//! same correlation identifier, and one rate per candidate asked about, in the
//! order asked. A response with fewer rates is
//! [`ContractError::UnansweredQuery`] rather than a partial success.

use crate::contracts::graph::EdgeId;
use crate::contracts::{
    ContractError, Correlation, PayloadBuffer, SchemaId, decode_text, encode_into,
};
use crate::id::{CandidateId, CorrelationId};
use crate::sci::{EnergyKwh, GridIntensity, SciRate};

/// Scalar upper bound on the candidates one query may carry.
///
/// 16, the same figure as [`crate::power::MAX_CGROUP_SLICES`] and for the same
/// reason: a bound that can be held in a fixed array, chosen so the worst-case
/// payload fits [`super::MAX_CONTRACT_PAYLOAD_BYTES`] with headroom.
pub const MAX_QUERY_CANDIDATES: usize = 16;

/// A bounded, ordered list of the candidates a query asks about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CandidateList {
    items: [Option<CandidateId>; MAX_QUERY_CANDIDATES],
    len: usize,
}

impl CandidateList {
    /// Builds an empty list.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            items: [None; MAX_QUERY_CANDIDATES],
            len: 0,
        }
    }

    /// Appends a candidate.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::Malformed`] at [`MAX_QUERY_CANDIDATES`]. The
    /// correlation is the schema alone because a list is built before the
    /// query that carries it exists.
    pub fn push(&mut self, candidate: CandidateId) -> Result<(), ContractError> {
        let slot = self
            .items
            .get_mut(self.len)
            .ok_or(ContractError::Malformed {
                correlation: Correlation::new(SchemaId::CandidateSciQuery, None),
            })?;
        *slot = Some(candidate);
        self.len = self.len.saturating_add(1);
        Ok(())
    }

    /// Returns how many candidates the list carries.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when the list carries no candidate.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the candidate at `index`, when there is one.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<CandidateId> {
        self.items.get(index).copied().flatten()
    }
}

/// One candidate's evaluated rate.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RateEntry {
    /// The candidate the rate is for.
    pub candidate: CandidateId,
    /// The SCI rate, in gCO2eq per functional unit.
    pub rate: SciRate,
    /// Whether the offered functional-unit count was substituted.
    pub fell_back: bool,
}

impl RateEntry {
    /// Builds an entry.
    #[must_use]
    pub const fn new(candidate: CandidateId, rate: SciRate, fell_back: bool) -> Self {
        Self {
            candidate,
            rate,
            fell_back,
        }
    }
}

/// A bounded, ordered list of evaluated rates.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RateList {
    items: [Option<RateEntry>; MAX_QUERY_CANDIDATES],
    len: usize,
}

impl RateList {
    /// Builds an empty list.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            items: [None; MAX_QUERY_CANDIDATES],
            len: 0,
        }
    }

    /// Appends an entry.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::Malformed`] at [`MAX_QUERY_CANDIDATES`].
    pub fn push(&mut self, entry: RateEntry) -> Result<(), ContractError> {
        let slot = self
            .items
            .get_mut(self.len)
            .ok_or(ContractError::Malformed {
                correlation: Correlation::new(SchemaId::CandidateSciResponse, None),
            })?;
        *slot = Some(entry);
        self.len = self.len.saturating_add(1);
        Ok(())
    }

    /// Returns how many entries the list carries.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when the list carries no entry.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the entry at `index`, when there is one.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<RateEntry> {
        self.items.get(index).copied().flatten()
    }
}

/// The contract versions of the SCI query this build admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum SciQueryVersion {
    /// Version 1, tagged `aegis.p16-p13.sci-query.v1`.
    #[serde(rename = "aegis.p16-p13.sci-query.v1")]
    V1,
}

/// One request from P16 Athena for the SCI rate of a set of candidates.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CandidateSciQuery {
    /// The contract version this payload claims.
    pub schema: SciQueryVersion,
    /// The subsystem-graph edge the payload travels on.
    pub edge: EdgeId,
    /// The identifier threading this query to its response.
    pub correlation_id: CorrelationId,
    /// The candidates the query asks about.
    pub candidates: CandidateList,
    /// The energy attributed to each candidate's run, `E`.
    pub energy_kwh: EnergyKwh,
    /// The functional units the rate is expressed per, `R`, as offered.
    ///
    /// A bare `f64` on the wire, because the recorded acceptance asks a
    /// non-positive count to fall back rather than to be refused, and a
    /// payload that cannot carry one could never exercise that.
    pub functional_units: f64,
}

impl CandidateSciQuery {
    /// The contract this type instantiates.
    pub const SCHEMA: SchemaId = SchemaId::CandidateSciQuery;

    /// The edge the query travels on, kept from the graph of record.
    pub const EDGE: EdgeId = EdgeId::EvaluateCandidateCarbonSci;

    /// Returns what a refusal of this payload is about.
    #[must_use]
    pub const fn correlation(&self) -> Correlation {
        Correlation::new(Self::SCHEMA, Some(self.correlation_id))
    }

    /// Checks the invariants the field types cannot express.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::WrongEdge`] for a payload naming another edge
    /// and [`ContractError::EmptyCandidateList`] for a query with no subject.
    pub fn validate(&self) -> Result<(), ContractError> {
        let correlation = self.correlation();
        if self.edge != Self::EDGE {
            return Err(ContractError::WrongEdge {
                correlation,
                found: self.edge.name(),
            });
        }
        if self.candidates.is_empty() {
            return Err(ContractError::EmptyCandidateList { correlation });
        }
        Ok(())
    }

    /// Encodes a validated query into `buffer`.
    ///
    /// # Errors
    ///
    /// Propagates [`Self::validate`], and returns
    /// [`ContractError::PayloadTooLong`] when the payload does not fit.
    pub fn encode_into<'b>(&self, buffer: &'b mut PayloadBuffer) -> Result<&'b str, ContractError> {
        self.validate()?;
        encode_into(self, self.correlation(), buffer)
    }

    /// Decodes and validates one query payload.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::UnknownVersion`] for a payload naming another
    /// contract version, [`ContractError::PayloadTooLong`] past the byte
    /// bound, [`ContractError::Malformed`] for anything the schema refuses,
    /// and otherwise whatever [`Self::validate`] refuses.
    pub fn decode(text: &str) -> Result<Self, ContractError> {
        let decoded: Self = decode_text(Self::SCHEMA, text)?;
        decoded.validate()?;
        Ok(decoded)
    }
}

/// The contract versions of the SCI response this build admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum SciResponseVersion {
    /// Version 1, tagged `aegis.p13-p16.sci-response.v1`.
    #[serde(rename = "aegis.p13-p16.sci-response.v1")]
    V1,
}

/// P13 Tellus's answer to one [`CandidateSciQuery`].
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CandidateSciResponse {
    /// The contract version this payload claims.
    pub schema: SciResponseVersion,
    /// The subsystem-graph edge the payload travels on.
    pub edge: EdgeId,
    /// The identifier of the query this answers.
    pub correlation_id: CorrelationId,
    /// The grid carbon intensity the rates were computed at, `I`.
    pub intensity: GridIntensity,
    /// One rate per candidate asked about, in the order asked.
    pub rates: RateList,
    /// Whether background work should be shifted at this intensity.
    pub deferred: bool,
}

impl CandidateSciResponse {
    /// The contract this type instantiates.
    pub const SCHEMA: SchemaId = SchemaId::CandidateSciResponse;

    /// The edge the response travels on, kept from the graph of record.
    pub const EDGE: EdgeId = EdgeId::EvaluateCandidateCarbonSci;

    /// Returns what a refusal of this payload is about.
    #[must_use]
    pub const fn correlation(&self) -> Correlation {
        Correlation::new(Self::SCHEMA, Some(self.correlation_id))
    }

    /// Checks the invariants the field types cannot express.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::WrongEdge`] for a payload naming another edge
    /// and [`ContractError::EmptyCandidateList`] for a response with no rate.
    pub fn validate(&self) -> Result<(), ContractError> {
        let correlation = self.correlation();
        if self.edge != Self::EDGE {
            return Err(ContractError::WrongEdge {
                correlation,
                found: self.edge.name(),
            });
        }
        if self.rates.is_empty() {
            return Err(ContractError::EmptyCandidateList { correlation });
        }
        Ok(())
    }

    /// Checks this response against the query it claims to answer.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::UnansweredQuery`] when the correlation
    /// identifiers differ, when the counts differ, or when a rate names a
    /// candidate the query did not ask about.
    pub fn answers(&self, query: &CandidateSciQuery) -> Result<(), ContractError> {
        let correlation = self.correlation();
        let asked = query.candidates.len();
        if self.correlation_id != query.correlation_id || self.rates.len() != asked {
            return Err(ContractError::UnansweredQuery {
                correlation,
                asked,
                answered: self.rates.len(),
            });
        }
        for index in 0..asked {
            let matched = self
                .rates
                .get(index)
                .zip(query.candidates.get(index))
                .is_some_and(|(entry, candidate)| entry.candidate == candidate);
            if !matched {
                return Err(ContractError::UnansweredQuery {
                    correlation,
                    asked,
                    answered: index,
                });
            }
        }
        Ok(())
    }

    /// Encodes a validated response into `buffer`.
    ///
    /// # Errors
    ///
    /// Propagates [`Self::validate`], and returns
    /// [`ContractError::PayloadTooLong`] when the payload does not fit.
    pub fn encode_into<'b>(&self, buffer: &'b mut PayloadBuffer) -> Result<&'b str, ContractError> {
        self.validate()?;
        encode_into(self, self.correlation(), buffer)
    }

    /// Decodes and validates one response payload.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::UnknownVersion`] for a payload naming another
    /// contract version, [`ContractError::PayloadTooLong`] past the byte
    /// bound, [`ContractError::Malformed`] for anything the schema refuses,
    /// and otherwise whatever [`Self::validate`] refuses.
    pub fn decode(text: &str) -> Result<Self, ContractError> {
        let decoded: Self = decode_text(Self::SCHEMA, text)?;
        decoded.validate()?;
        Ok(decoded)
    }
}
