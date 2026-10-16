// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! What the P10 controllers refuse, and why.
//!
//! The controllers moved to `saeptum-capsule` (cut unit CUT-1632-03), and
//! their refusal type moved with them as `SandboxError`. Aegis keeps the name
//! its callers and the P09 and P16 contracts already use.

pub use saeptum_capsule::error::SandboxError as VestaError;
