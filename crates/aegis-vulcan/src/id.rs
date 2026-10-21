// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded, allocation-free correlation identifier.
//!
//! `docs/integration/stack.md` asks every crossing to carry a correlation
//! identifier. It is validated on construction and stored inline, so no
//! decision path allocates for one and no identifier can be silently truncated
//! or sanitised: a malformed input is refused.

#[path = "../../correlation_id.rs"]
mod correlation_id;

pub use correlation_id::{CorrelationId, IdError, MAX_CORRELATION_LEN};
