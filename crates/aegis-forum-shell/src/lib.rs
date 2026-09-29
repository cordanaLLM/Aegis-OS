// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P05 Forum shell: the model the native shell runs on (milestone
//! M16, ADR-0004).
//!
//! ADR-0004 makes the whole P05 shell one native Rust program on gpui
//! (decisions D82, D101, D102). This crate is the half of it that needs no
//! toolkit, no display and no bus, and it is tested on any machine:
//!
//! | Module | What it holds | Decision or requirement |
//! | :-- | :-- | :-- |
//! | [`lifecycle`] | the five-state process lifecycle and its quarantine limit | D96, REQ-P05-03 |
//! | [`processes`] | the managed process table, driven by stubbed P04, P06 and P13 inputs | REQ-P05-01, REQ-P05-07, REQ-P04-07 |
//! | [`jsonrpc`] | line-delimited JSON-RPC 2.0 framing under a byte bound and deadlines | D77, HISS-02 |
//! | [`consumers`] | the decision request and carbon telemetry, decoded by their producers' crates | E16-2 |
//! | [`session`] | one inbound stream: lines in, state updated, Responses out | D77 |
//! | [`endpoint`] | `SYNC_DESKTOP_SHELL`'s endpoint and per-hop target, recorded and not asserted | D32 |
//! | [`canvas`] | the node registry, the camera and the `QuadTree` cull index | D74 |
//! | [`focus`], [`walk`] | the keyboard model and the keyboard-only walk | REQ-P05-09, REQ-P05-10 |
//! | [`a11y`] | the `accesskit::TreeUpdate` export and the tree check | REQ-P05-11, REQ-P12-06 |
//! | [`platform`] | the compositor link and the D-Bus interfaces, as traits | REQ-P05-05 |
//! | [`tokens`] | the P05/P12 design-token edge with its direction annotated | D05, REQ-GRAPH-01 |
//! | [`state`] | the shell state everything above reads and writes | -- |
//!
//! # What this crate does not do
//!
//! It paints nothing, creates no window or surface, opens no real socket and
//! contacts no D-Bus daemon. Its only accessibility dependency is accesskit
//! 0.25.1, the data model, with no platform adapter; gpui, `accesskit_unix`,
//! zbus and atspi are M28's admission, and the hygiene test refuses them here.
//! A pass is model-level evidence: it closes no accessibility gate for the
//! running shell, which M28 checks over AT-SPI and M29 in a live session.
//!
//! # Design invariants (see `AGENTS.md`)
//!
//! No recursion (HISS-01): the cull index, the tree check and the walks use
//! explicit stacks or bounded loops. Every loop has a scalar bound and every
//! read and write a deadline (HISS-02). The cull query allocates nothing past
//! the caller's buffer (HISS-03). No `unwrap`, `expect` or panic (HISS-07),
//! and `unsafe` is forbidden by the workspace lints.

pub mod a11y;
pub mod canvas;
pub mod consumers;
pub mod endpoint;
pub mod focus;
pub mod jsonrpc;
pub mod lifecycle;
pub mod platform;
pub mod processes;
pub mod session;
pub mod state;
pub mod tokens;
pub mod walk;
