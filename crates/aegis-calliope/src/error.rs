// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! What the P08 tables and constructors refuse, and why.
//!
//! The imported scaffold (export-026 `c2f1e433cd32`) returns
//! `Box<dyn Error>` built from a string literal, so the plugin-capacity
//! refusal and an unrelated failure are indistinguishable to a program, and
//! the bound that was hit is only in the sentence. Each refusal is a variant
//! here instead, and each carries the bound or the value it refused, so a test
//! asserts the bound rather than a sentence.

use crate::dmabuf::FourCc;
use crate::id::IdError;
use crate::plugin::PluginStage;

/// Reasons a P08 table or constructor refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CalliopeError {
    /// The sandboxed-plugin table is full.
    #[error("the plugin host holds its maximum of {max} sandbox slots")]
    PluginTableFull {
        /// The scalar bound that was hit.
        max: usize,
    },
    /// The host holds no slot with that identifier.
    #[error("the plugin host holds no slot {slot}")]
    UnknownSlot {
        /// The slot that was named.
        slot: u32,
    },
    /// The typed tolerance lifecycle does not admit that step.
    #[error("a plugin may not step from {from:?} to {to:?}")]
    IllegalStageTransition {
        /// The stage the plugin is in.
        from: PluginStage,
        /// The stage that was asked for.
        to: PluginStage,
    },
    /// The DMA-BUF descriptor table is full.
    #[error("the DMA-BUF table holds its maximum of {max} buffers")]
    BufferTableFull {
        /// The scalar bound that was hit.
        max: usize,
    },
    /// A row stride would not fit in the 32-bit field that carries it.
    #[error("a stride of {width} x {bytes_per_pixel} bytes does not fit in u32")]
    StrideOverflow {
        /// The row width in pixels.
        width: u32,
        /// The bytes each pixel costs in the declared format.
        bytes_per_pixel: u32,
    },
    /// The four-character code names no format this build admits.
    #[error("the four-character code {fourcc} names no admitted pixel format")]
    UnknownPixelFormat {
        /// The code that was offered.
        fourcc: FourCc,
    },
    /// A real-time priority was outside the range the grant admits.
    #[error("a real-time priority of {value} is outside {min}..={max}")]
    RtPrioOutOfRange {
        /// The priority that was offered.
        value: u8,
        /// The lowest admissible priority.
        min: u8,
        /// The highest admissible priority.
        max: u8,
    },
    /// A graph quantum was outside the admissible range.
    #[error("a quantum of {samples} samples is outside {min}..={max}")]
    QuantumOutOfRange {
        /// The quantum that was offered.
        samples: u32,
        /// The smallest admissible quantum.
        min: u32,
        /// The largest admissible quantum.
        max: u32,
    },
    /// A sample rate names no rate this build admits.
    #[error("a sample rate of {hz} Hz is not an admitted rate")]
    SampleRateUnsupported {
        /// The rate that was offered.
        hz: u32,
    },
    /// The drift window is full.
    #[error("the drift window holds its maximum of {max} samples")]
    DriftWindowFull {
        /// The scalar bound that was hit.
        max: usize,
    },
    /// An identifier was refused by its validating constructor.
    #[error("an identifier was refused: {0}")]
    Identifier(#[from] IdError),
}
