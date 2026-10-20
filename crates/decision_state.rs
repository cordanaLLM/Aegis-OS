// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! D116: Whether a recorded decision is settled.
//!
//! One file, included by path into every crate whose register or decision module
//! records a decision state (`#[path = "../../decision_state.rs"]`), so the type
//! has one definition rather than one per crate. It is not a crate: a member
//! enters every dependency closure and carries a permanent name.

#![allow(dead_code)]

/// Whether a recorded decision is settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DecisionState {
    /// The decision is settled and the register names what was chosen.
    Closed,
    /// The decision is recorded but not settled.
    Unresolved,
}

impl DecisionState {
    /// Returns the stable name this state is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Unresolved => "unresolved",
        }
    }
}
