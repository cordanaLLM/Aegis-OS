// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Decision D06, applied: which `WebAssembly` runtime a Vesta capsule uses.
//!
//! The imported P10 source describes its capsules in terms of a Go-implemented
//! runtime and the graph of record repeats that runtime inside the P09 to P10
//! transport string, while `docs/integration/stack.md` states the rule that a
//! Go library is not a direct dependency of a Rust daemon without an adapter
//! and contract tests. The M01 register carries the collision as dispute
//! DSP-25 and poses it as D06 with three options.
//!
//! **The decision, recorded 2026-09-13: a Rust-native runtime**, selected
//! through the template matrix, with no Go runtime and no protocol adapter.
//! Milestone M06 is where it is applied, and this module is the record.
//!
//! # What applying it means here
//!
//! * The crate takes **no Go dependency and no runtime dependency at all**.
//!   `tests/manifest_hygiene.rs` reads the manifest and fails on either.
//! * [`AdmittedRuntime`] has exactly one variant, so a capsule request naming
//!   the Go runtime does not decode -- the rejected options stay representable
//!   in [`WasmRuntimeChoice`], which is the register, not the wire.
//! * Selecting a Rust-native runtime is not the same as pinning one. No
//!   version is pinned and no engine is linked; the M01 drift register keeps
//!   the plugin framework whose Rust SDK survives D06 as an unpinned row, and
//!   admission is `docs/roadmap/toolchain-admission.md`'s business.
//! * The graph string is annotated rather than edited, which is what the
//!   register says M01 decided. Nothing in this crate rewrites the recorded
//!   transport.
//!
//! # Scope
//!
//! Recording and applying D06 is not evidence that any runtime exists, runs, or
//! can execute a capsule. This crate loads no module.

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

/// The three runtimes D06 could have chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WasmRuntimeChoice {
    /// A Rust-native runtime, selected through the template matrix.
    RustNativeRuntime,
    /// The Go runtime out of process, behind a protocol adapter with contract
    /// tests.
    OutOfProcessAdapter,
    /// The Go runtime as a direct dependency of the Rust daemon.
    DirectGoDependency,
}

impl WasmRuntimeChoice {
    /// All three options, in the order the register lists them.
    ///
    /// The rejected options stay representable so the register states a choice
    /// between three readings rather than asserting the only one it can spell.
    pub const ALL: [Self; 3] = [
        Self::RustNativeRuntime,
        Self::OutOfProcessAdapter,
        Self::DirectGoDependency,
    ];

    /// Returns the stable name this option is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::RustNativeRuntime => "rust-native-runtime",
            Self::OutOfProcessAdapter => "out-of-process-adapter",
            Self::DirectGoDependency => "direct-go-dependency",
        }
    }

    /// Returns `true` when the option would put a Go library in the crate's
    /// own dependency set.
    #[must_use]
    pub const fn adds_go_dependency(self) -> bool {
        matches!(self, Self::DirectGoDependency)
    }

    /// Returns `true` when the option needs a protocol or foreign-function
    /// adapter with its own contract tests.
    #[must_use]
    pub const fn needs_adapter(self) -> bool {
        matches!(self, Self::OutOfProcessAdapter)
    }

    /// Returns `true` when `docs/integration/stack.md`'s language-boundary
    /// rule permits the option.
    ///
    /// The rule bars a Go library as a direct dependency of a Rust daemon
    /// *without* an adapter and contract tests, so the adapter option is
    /// permitted and the direct dependency is not.
    #[must_use]
    pub const fn permitted_by_language_boundary(self) -> bool {
        !self.adds_go_dependency()
    }
}

/// The runtime a capsule request may name.
///
/// One variant, on purpose, and the same device decision D03 used for the
/// action gate: encoding the outcome as a single-variant enum makes a payload
/// that names the rejected runtime fail to decode rather than merely be
/// discouraged. The wire tag says `rust-native-component-model` and not a
/// product name, because D06 chose a language boundary and not a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum AdmittedRuntime {
    /// A Rust-native component-model runtime, unpinned.
    #[serde(rename = "rust-native-component-model")]
    RustNativeComponentModel,
}

impl AdmittedRuntime {
    /// The runtime decision D06 settled, and the only one this build admits.
    pub const SETTLED: Self = Self::RustNativeComponentModel;

    /// Returns the stable tag the wire form carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::RustNativeComponentModel => "rust-native-component-model",
        }
    }

    /// Returns the register option this admitted runtime instantiates.
    #[must_use]
    pub const fn choice(self) -> WasmRuntimeChoice {
        match self {
            Self::RustNativeComponentModel => WasmRuntimeChoice::RustNativeRuntime,
        }
    }
}

/// A private source, cited by export identifier and digest prefix only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Citation {
    /// The export identifier, or the tracked path for a public source.
    pub export: &'static str,
    /// The first twelve hexadecimal characters of that source's sha256.
    pub sha256_prefix: &'static str,
}

/// One recorded decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecisionRecord {
    /// The decision identifier used in `docs/roadmap/README.md`.
    pub id: &'static str,
    /// The question the decision answers.
    pub question: &'static str,
    /// Whether the decision is settled.
    pub state: DecisionState,
    /// The option that was chosen.
    pub chosen: WasmRuntimeChoice,
    /// The options that were not chosen.
    pub rejected: [WasmRuntimeChoice; 2],
    /// What the choice rests on.
    pub rationale: &'static str,
    /// The milestone that applies the decision.
    pub applied_at: &'static str,
    /// The requirements the decision touches.
    pub touches: [&'static str; 3],
    /// The dispute this decision closes.
    pub dispute: &'static str,
    /// The sources that posed the question.
    pub sources: [Citation; 2],
}

/// D06: which `WebAssembly` capsule runtime P10 Vesta uses.
pub const D06_WASM_RUNTIME: DecisionRecord = DecisionRecord {
    id: "D06",
    question: "Which Wasm capsule runtime does Vesta use, given the language-boundary rule?",
    state: DecisionState::Closed,
    chosen: WasmRuntimeChoice::RustNativeRuntime,
    rejected: [
        WasmRuntimeChoice::OutOfProcessAdapter,
        WasmRuntimeChoice::DirectGoDependency,
    ],
    rationale: "the shared-stack rule bars a Go library as a direct dependency of a Rust \
                daemon, and an out-of-process adapter would buy a runtime this milestone \
                does not run at the price of a transport, a protocol and its contract \
                tests; a Rust-native runtime keeps the boundary inside one language",
    applied_at: "M06",
    touches: ["REQ-P10-08", "REQ-P10-06", "REQ-WS-01"],
    dispute: "DSP-25",
    sources: [
        Citation {
            export: "export-037",
            sha256_prefix: "ce490c88081f",
        },
        Citation {
            export: "public:docs/integration/stack.md",
            sha256_prefix: "bc062910505e",
        },
    ],
};
