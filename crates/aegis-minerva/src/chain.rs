// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The two edges out of P09 whose payload types other crates own.
//!
//! P09 sits in the middle of the agent execution chain, and two of its three
//! edges end in a crate that already types the payload:
//!
//! | Edge | Type | Owner |
//! | :-- | :-- | :-- |
//! | P09 to P06, `ACTION_GATE_INTERCEPT` | [`ActionProposal`] | `aegis-justitia`, milestone M14 |
//! | P09 to P10, `EXECUTE_WASMED_CAPSULE` | [`CapsuleRequest`] | `aegis-vesta`, this milestone |
//!
//! **Neither type is redefined here.** This module builds them, which is the
//! point: a producer that fills in a consumer's type cannot drift from it, and
//! a field the consumer adds is a compile error here rather than a payload that
//! silently stops validating. The constants the consumers declare --
//! [`ActionProposal::EDGE`], [`ActionProposal::DIRECTION`],
//! [`CapsuleRequest::EDGE`], [`CapsuleRequest::RUNTIME`] -- are read rather
//! than restated, so P09 cannot name an edge P06 does not gate, the direction
//! decision D03 rejected, or the `WebAssembly` runtime decision D06 rejected.
//!
//! # The correlation identifier is parsed twice, on purpose
//!
//! Each consumer validates the identifier with its own constructor and its own
//! bound: P06 admits up to
//! [`MAX_IDENTITY_LEN`](aegis_justitia::MAX_IDENTITY_LEN) bytes and P10 up to
//! [`MAX_CORRELATION_LEN`](aegis_vesta::MAX_CORRELATION_LEN). Threading one
//! pre-parsed value through both would mean one crate trusting the other's
//! bound, which is the opposite of a typed boundary.
//!
//! # What this module does not do
//!
//! It sends nothing. There is no bus connection, no socket, no signing and no
//! sandbox. A draft becomes a payload; who carries it is later work (M19 and
//! M10 for the eBPF half of the action gate, M22 for a real sandbox).

use aegis_justitia::{
    ActionProposal, ActionProposalVersion, ActionType, AgentId, ContractError as GateError,
    Digest32, Identity, MakerId, OversightClass, OversightSignature, RiskTier,
    SandboxAdmissionPath, TargetResource, UnixSeconds,
};
use aegis_vesta::{
    CapabilitySet, CapsuleMemoryLimit, CapsuleRequest, CapsuleRequestVersion, CapsuleSlot,
    CorrelationId as CapsuleCorrelationId, Label as CapsuleLabel, VmmIdentity,
};

/// Everything P09 knows about an action before P06 sees it.
///
/// A draft is not a proposal: it carries no schema tag, no edge and no
/// direction, because those are P06's to declare and [`Self::into_proposal`]
/// reads them from P06's own constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProposalDraft {
    /// The identifier threading the proposal, its decision and its audit record.
    pub correlation_id: Identity,
    /// The agent process on whose behalf the action is proposed.
    pub agent_id: AgentId,
    /// The principal accountable for the proposal.
    pub maker: MakerId,
    /// What the action does.
    pub action_type: ActionType,
    /// The resource the action touches.
    pub target: TargetResource,
    /// The tier P09 declares. P06 may only raise it.
    pub declared_tier: RiskTier,
    /// The regulatory oversight class P09 declares.
    pub oversight: OversightClass,
    /// The digest of the action payload the proposal stands for.
    pub payload_hash: Digest32,
    /// When the proposal was made.
    pub proposed_at: UnixSeconds,
    /// The proposer's seal. A draft without one produces no proposal.
    pub seal: Option<OversightSignature>,
}

impl ProposalDraft {
    /// Builds the M14 proposal this draft stands for.
    ///
    /// # Errors
    ///
    /// Propagates [`ActionProposal::validate`]: an unsealed or empty-sealed
    /// draft is [`GateError::Unsigned`], and a proposal naming another edge
    /// is [`GateError::Malformed`] -- which this method cannot produce,
    /// because it reads the edge from [`ActionProposal::EDGE`] rather than
    /// taking one.
    pub fn into_proposal(self) -> Result<ActionProposal, GateError> {
        let proposal = ActionProposal {
            schema: ActionProposalVersion::V1,
            edge: ActionProposal::EDGE,
            direction: ActionProposal::DIRECTION,
            correlation_id: self.correlation_id,
            agent_id: self.agent_id,
            maker: self.maker,
            action_type: self.action_type,
            target: self.target,
            declared_tier: self.declared_tier,
            oversight: self.oversight,
            payload_hash: self.payload_hash,
            proposed_at: self.proposed_at,
            signature: self.seal,
        };
        proposal.validate()?;
        Ok(proposal)
    }
}

/// Everything P09 knows about a capsule before P10 runs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapsuleDispatch {
    /// The identifier threading the request to P10's evaluation.
    pub correlation_id: CapsuleCorrelationId,
    /// The capsule to execute.
    pub capsule: CapsuleLabel,
    /// The slot P10 is asked to place it in.
    pub slot: CapsuleSlot,
    /// The memory limit the capsule is admitted with.
    pub memory_limit: CapsuleMemoryLimit,
    /// The capabilities the capsule is granted.
    pub capabilities: CapabilitySet,
    /// The monitor the sandbox should run under (decision D58).
    pub vmm: VmmIdentity,
    /// The gating path the request travels (decision D04, unresolved).
    pub admission: SandboxAdmissionPath,
}

impl CapsuleDispatch {
    /// Builds the P10 capsule request this dispatch stands for.
    ///
    /// # Errors
    ///
    /// Propagates [`CapsuleRequest::validate`], which this method cannot make
    /// fail: the edge comes from [`CapsuleRequest::EDGE`] and the runtime from
    /// [`CapsuleRequest::RUNTIME`], so P09 cannot name P10's other edge or the
    /// runtime D06 rejected. The `Result` is kept so that a later invariant
    /// P10 adds is a compile-time obligation here rather than a silent gap.
    pub fn into_request(self) -> Result<CapsuleRequest, aegis_vesta::ContractError> {
        let request = CapsuleRequest {
            schema: CapsuleRequestVersion::V1,
            edge: CapsuleRequest::EDGE,
            admission: self.admission,
            runtime: CapsuleRequest::RUNTIME,
            vmm: self.vmm,
            correlation_id: self.correlation_id,
            capsule: self.capsule,
            slot: self.slot,
            memory_limit_bytes: self.memory_limit,
            capabilities: self.capabilities,
        };
        request.validate()?;
        Ok(request)
    }
}
