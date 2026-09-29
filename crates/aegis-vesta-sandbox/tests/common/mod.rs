// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fixtures shared by the M21 sandbox tests. Nothing here starts a process.

#![allow(dead_code)]

use aegis_vesta::{FIRST_GUEST_CID, VmId};
use aegis_vesta_sandbox::{EvaluationRequest, SandboxError, WireMetrics, request};

/// What every test in this suite returns.
pub type Fallible = Result<(), Box<dyn std::error::Error>>;

/// The first sandbox identifier the controller hands out.
pub const FIRST_VM: u32 = 100;

/// The context identifier the controller derives for [`FIRST_VM`].
pub const FIRST_CID: u32 = FIRST_GUEST_CID + FIRST_VM;

/// Builds a request for sandbox [`FIRST_VM`].
///
/// # Errors
///
/// Propagates [`request`].
pub fn first_request(tag: &str, metrics: WireMetrics) -> Result<EvaluationRequest, SandboxError> {
    request(VmId::new(FIRST_VM), tag, metrics)
}
