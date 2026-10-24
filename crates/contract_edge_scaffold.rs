// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The contract surface of a crate whose only own refusal is `WrongEdge`.
//!
//! One file, included by path into the contracts modules of `aegis-calliope`
//! and `aegis-vesta` (`#[path = "../../../contract_edge_scaffold.rs"] #[macro_use]
//! mod contract_edge_scaffold;`), after `contract_scaffold.rs` and
//! `contract_codec_scaffold.rs`, whose `define_contract_error!` and
//! `scaffold_contract!` it calls. It lives apart
//! from `contract_scaffold.rs` so the crates that declare their own refusals
//! never see its `WrongEdge` variant.

/// Scaffolds contract boilerplate and refusal enum for twin contracts whose
/// only extra refusal variant is `WrongEdge`.
macro_rules! scaffold_edge_contract {
    ($schema:ident, $id:ident, $max_bytes:ident) => {
        /// What a refusal is about: the contract, and the payload when it could be read.
        pub type Correlation = contract_scaffold::Correlation<$schema, $id>;

        /// A fixed buffer holding one encoded contract payload.
        pub type PayloadBuffer = contract_scaffold::PayloadBuffer<$max_bytes>;

        define_contract_error! {
            $schema, $id;
            /// Reasons a contract payload is refused.
            ///
            /// Every variant carries its [`Correlation`], so no refusal is anonymous.
            pub enum ContractError {
                /// The payload travels on an edge this schema does not carry.
                #[error("{correlation}: this schema does not travel on the edge the payload names")]
                WrongEdge {
                    /// What the refusal is about.
                    correlation: Correlation,
                },
            }
        }

        scaffold_contract!($schema, $id, $max_bytes, ContractError);
    };
}
