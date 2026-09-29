// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The P05/P12 design-token edge, with its direction annotated (D05,
//! REQ-GRAPH-01, dispute DSP-01).
//!
//! The graph of record (export-062 `1ce919ed54bb`) draws the edge from
//! P05 Forum to P12 Concordia and names it `CONSUMES_DESIGN_TOKENS`; the
//! dataflow document (export-002 `7e0c95f4ea05`) draws the arrow the other
//! way, P12 supplying tokens to P05. D05 reconciles them: P12 produces the
//! tokens and P05 consumes them, and the graph's edge identifier is kept with
//! the direction annotated -- which [`TOKEN_EDGE`] is.
//!
//! Under D101 (ADR-0004) the native shell consumes the tokens as Rust theme
//! constants that M28 generates from the M04 token file,
//! `ui/concordia-tokens/concordia-tokens.css`, the single token source. This
//! crate paints nothing, so it holds none of them yet.

/// One subsystem-graph edge and the direction data flows along it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnnotatedEdge {
    /// The edge identifier the graph of record gives.
    pub edge: &'static str,
    /// The component the graph draws the edge from.
    pub graph_from: &'static str,
    /// The component the graph draws the edge to.
    pub graph_to: &'static str,
    /// The component whose data travels along the edge.
    pub producer: &'static str,
    /// The component that receives it.
    pub consumer: &'static str,
    /// The form the consumer takes it in.
    pub form: &'static str,
}

/// The design-token edge as D05 annotates it.
pub const TOKEN_EDGE: AnnotatedEdge = AnnotatedEdge {
    edge: "CONSUMES_DESIGN_TOKENS",
    graph_from: "P05",
    graph_to: "P12",
    producer: "P12",
    consumer: "P05",
    form: "Rust theme constants generated from ui/concordia-tokens/concordia-tokens.css (M28, D101)",
};
