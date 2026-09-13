// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Decision D08, applied: how the P04 compositor is implemented, and which
//! dependencies that admits.
//!
//! The P04 blueprint rejects libweston and Smithay for the fast runtime loop
//! and mandates a C wlroots implementation that owns the event loop
//! (REQ-P04-01, export-013 `7f4c22813332`), while the only code candidate is a
//! pure Rust scaffold with no wlroots binding and no pinned version anywhere
//! (export-027 `f19640d7a7da`). The M01 register carries the collision as
//! dispute DSP-23 and poses it as D08.
//!
//! **The decision, recorded 2026-09-13 in ADR-0001: a pure Rust compositor.**
//! The C wlroots mandate is superseded and no C compositor toolchain is
//! admitted. Milestone M07 is where it is applied, and this module is the
//! record.
//!
//! # What applying it means here
//!
//! * The crate takes **no compositor dependency of any kind**, C or Rust, and
//!   `tests/manifest_hygiene.rs` reads the manifest and fails on either.
//! * [`AdmittedBackend`] has exactly one variant, so nothing in this crate can
//!   spell a wlroots backend; the rejected options stay representable in
//!   [`CompositorImplementation`], which is the register, not the wire.
//! * Selecting pure Rust is not the same as pinning a library. The ADR says
//!   the Rust compositor library is selected at M07 "after checking current
//!   upstream support", and [`COMPOSITOR_LIBRARY_CHECK`] is that check, run on
//!   the date it records. Its outcome is that **no library is admitted at
//!   M07**: this milestone builds a backend-agnostic registry and state
//!   machine, and admitting a library it never calls would add a dependency
//!   this gate cannot exercise. The admission moves to the milestone that
//!   first needs a display path.
//!
//! # The mesh dependency, checked the same way
//!
//! [`ZENOH_CHECK`] records the same decision for Tier 2. The imported
//! workspace manifest proposes `zenoh = "1.0"` (export-006 `6e694e01e136`);
//! that requirement is **not inherited**, because an imported version is
//! proposal data. Current upstream was read at activation and is recorded
//! below. The outcome is again that the crate is not admitted here: the
//! acceptance this milestone has to meet is a **mocked** transport test, and
//! [`MockedMesh`](crate::MockedMesh) is that mock. A dependency whose 42
//! direct requirements include an async runtime, added for a test that never
//! calls it, would be a larger change than the thing it was added for.
//!
//! Nothing in either record says the library is unsuitable. Both say the
//! version is selected when the component that needs it is activated, against
//! upstream at that moment, and not before.

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

/// The three ways D08 could have had the compositor implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CompositorImplementation {
    /// Pure Rust, with no foreign-function compositor boundary.
    PureRust,
    /// A C wlroots implementation owning the event loop, as the blueprint
    /// mandates.
    CWlrootsBinding,
    /// A higher-level framework -- libweston or Smithay -- which the blueprint
    /// rejects for the fast runtime loop.
    HigherLevelFramework,
}

impl CompositorImplementation {
    /// All three options, in the order the register lists them.
    ///
    /// The rejected options stay representable so the register states a choice
    /// between three readings rather than asserting the only one it can spell.
    pub const ALL: [Self; 3] = [
        Self::PureRust,
        Self::CWlrootsBinding,
        Self::HigherLevelFramework,
    ];

    /// Returns the stable name this option is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::PureRust => "pure-rust-compositor",
            Self::CWlrootsBinding => "c-wlroots-binding",
            Self::HigherLevelFramework => "higher-level-framework",
        }
    }

    /// Returns `true` when the option would admit a C toolchain and a
    /// foreign-function boundary into the workspace.
    #[must_use]
    pub const fn adds_c_toolchain(self) -> bool {
        matches!(self, Self::CWlrootsBinding)
    }

    /// Returns `true` when the P04 blueprint rejects the option outright.
    #[must_use]
    pub const fn rejected_by_blueprint(self) -> bool {
        matches!(self, Self::HigherLevelFramework)
    }
}

/// The compositor backend a build may name.
///
/// One variant, on purpose, and the same device decision D06 used for the
/// capsule runtime: encoding the outcome as a single-variant enum means
/// nothing in this crate can spell the superseded mandate, rather than merely
/// being discouraged from doing so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AdmittedBackend {
    /// A pure Rust backend, unpinned and not yet selected as a package.
    PureRustBackendAgnostic,
}

impl AdmittedBackend {
    /// The backend decision D08 settled, and the only one this build admits.
    pub const SETTLED: Self = Self::PureRustBackendAgnostic;

    /// Returns the stable tag this backend is recorded under.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::PureRustBackendAgnostic => "pure-rust-backend-agnostic",
        }
    }

    /// Returns the register option this admitted backend instantiates.
    #[must_use]
    pub const fn implementation(self) -> CompositorImplementation {
        match self {
            Self::PureRustBackendAgnostic => CompositorImplementation::PureRust,
        }
    }
}

/// A source, cited by export identifier and digest prefix only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Citation {
    /// The export identifier.
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
    pub chosen: CompositorImplementation,
    /// The options that were not chosen.
    pub rejected: [CompositorImplementation; 2],
    /// What the choice rests on.
    pub rationale: &'static str,
    /// The milestone that applies the decision.
    pub applied_at: &'static str,
    /// The architecture decision record that carries it.
    pub adr: &'static str,
    /// The requirements the decision touches.
    pub touches: [&'static str; 2],
    /// The dispute this decision closes.
    pub dispute: &'static str,
    /// The sources that posed the question.
    pub sources: [Citation; 2],
}

/// D08: how the P04 compositor is implemented.
pub const D08_COMPOSITOR_IMPLEMENTATION: DecisionRecord = DecisionRecord {
    id: "D08",
    question: "Is P04 implemented against a C wlroots core, as the blueprint mandates, or in \
               pure Rust, as the only code candidate is?",
    state: DecisionState::Closed,
    chosen: CompositorImplementation::PureRust,
    rejected: [
        CompositorImplementation::CWlrootsBinding,
        CompositorImplementation::HigherLevelFramework,
    ],
    rationale: "the C mandate would add a C toolchain, a foreign-function boundary and a \
                second memory-safety regime to the component at the centre of the mesh, and \
                no wlroots version is pinned in any source; the blueprint's own latency \
                argument for it is unbacked, and a budget is a claim this milestone cannot \
                measure either way",
    applied_at: "M07",
    adr: "ADR-0001",
    touches: ["REQ-P04-01", "REQ-P04-04"],
    dispute: "DSP-23",
    sources: [
        Citation {
            export: "export-013",
            sha256_prefix: "7f4c22813332",
        },
        Citation {
            export: "export-027",
            sha256_prefix: "f19640d7a7da",
        },
    ],
};

/// One dependency whose version was read from current upstream at activation.
///
/// The record exists so that "checked upstream" is a dated statement with
/// commands behind it rather than a claim, so the commands are recorded in the
/// form that reproduces the figures. Two details make that form load-bearing
/// rather than decorative: crates.io answers a request carrying no
/// `User-Agent` with HTTP 403 and a zero-byte body, and the
/// `/api/v1/crates/<name>` endpoint carries no dependency data at all. A
/// record with one header-less command behind it would therefore reproduce
/// neither figure, which is why [`Self::version_command`] and
/// [`Self::dependency_command`] are separate fields and why both carry the
/// header.
///
/// What the record does **not** hold is a judgement about the library:
/// `admitted_here` false means this milestone does not call it, not that it is
/// unsuitable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpstreamCheck {
    /// The crate that was checked.
    pub crate_name: &'static str,
    /// The version requirement the imported manifest proposes, if any.
    pub proposed_requirement: &'static str,
    /// Whether this repository inherits that proposed requirement.
    pub inherits_proposal: bool,
    /// The highest stable version upstream reported.
    pub upstream_max_stable: &'static str,
    /// The date upstream published that version.
    pub upstream_released_on: &'static str,
    /// The date the check was run.
    pub checked_on: &'static str,
    /// The command [`Self::upstream_max_stable`] and
    /// [`Self::upstream_released_on`] were read with.
    ///
    /// It carries the `User-Agent` header crates.io requires: the same request
    /// without one is answered 403 with a zero-byte body, so a command
    /// recorded without the header would reproduce nothing.
    pub version_command: &'static str,
    /// The command the direct-dependency counts in [`Self::reason`] were read
    /// with.
    ///
    /// A second command rather than a second reading of the first: the
    /// `/api/v1/crates/<name>` endpoint reports versions and no dependency at
    /// all, so the counts come from the per-version `dependencies` endpoint,
    /// which is why this command names a version and the other does not.
    pub dependency_command: &'static str,
    /// Whether this milestone admits the crate as a dependency.
    pub admitted_here: bool,
    /// Where the admission would be made instead.
    pub admitted_at: &'static str,
    /// Why the outcome is what it is.
    pub reason: &'static str,
}

/// The Tier-2 mesh transport check, run at activation.
pub const ZENOH_CHECK: UpstreamCheck = UpstreamCheck {
    crate_name: "zenoh",
    proposed_requirement: "1.0",
    inherits_proposal: false,
    upstream_max_stable: "1.10.1",
    upstream_released_on: "2026-09-07",
    checked_on: "2026-09-13",
    version_command: "curl -sS -H 'User-Agent: aegis-os-upstream-check \
                      (https://github.com/cordanaLLM/Aegis-OS)' \
                      https://crates.io/api/v1/crates/zenoh | jq -r '.crate.max_stable_version, \
                      (.versions[] | select(.num == \"1.10.1\") | .created_at)'",
    dependency_command: "curl -sS -H 'User-Agent: aegis-os-upstream-check \
                         (https://github.com/cordanaLLM/Aegis-OS)' \
                         https://crates.io/api/v1/crates/zenoh/1.10.1/dependencies | jq -c \
                         '[.dependencies[] | select(.kind == \"normal\")] | \
                         group_by(.optional) | map({optional: .[0].optional, count: length})'",
    admitted_here: false,
    admitted_at: "the milestone that first carries a payload between two processes; M23 for \
                  the P07 and P08 timing path, M12 for the display path",
    reason: "the acceptance this milestone has to meet is a mocked transport test, and \
             MockedMesh is that mock. The dependency command reported 44 normal-kind direct \
             dependencies on the 1.10.1 release, of which 42 are required and 2 optional; \
             tokio is among the required ones, and none of them would this gate execute, so \
             admitting it would add a resolution graph larger than the slice it was added \
             for. The proposed 1.0 requirement is two minor series behind current upstream, \
             which is exactly why an imported pin is proposal data",
};

/// The Rust compositor library check ADR-0001 asks for, run at activation.
pub const COMPOSITOR_LIBRARY_CHECK: UpstreamCheck = UpstreamCheck {
    crate_name: "smithay",
    proposed_requirement: "none; the imported manifest declares no compositor dependency",
    inherits_proposal: false,
    upstream_max_stable: "0.7.0",
    upstream_released_on: "2025-06-24",
    checked_on: "2026-09-13",
    version_command: "curl -sS -H 'User-Agent: aegis-os-upstream-check \
                      (https://github.com/cordanaLLM/Aegis-OS)' \
                      https://crates.io/api/v1/crates/smithay | jq -r \
                      '.crate.max_stable_version, (.versions[] | select(.num == \"0.7.0\") | \
                      .created_at)'",
    dependency_command: "curl -sS -H 'User-Agent: aegis-os-upstream-check \
                         (https://github.com/cordanaLLM/Aegis-OS)' \
                         https://crates.io/api/v1/crates/smithay/0.7.0/dependencies | jq -c \
                         '[.dependencies[] | select(.kind == \"normal\")] | \
                         group_by(.optional) | map({optional: .[0].optional, count: length})'",
    admitted_here: false,
    admitted_at: "the milestone that first drives a display, which is M12",
    reason: "M07's scope is the backend-agnostic registry and IPC state machine, and this \
             crate opens no display. The dependency command reported 44 normal-kind direct \
             dependencies on the 0.7.0 release -- the figure is the normal-kind total, not a \
             required count like zenoh's 42, because only 19 are required and 25 are \
             optional. drm, gbm, input and libseat are four of the optional ones, and all \
             four sit inside the default feature closure, by way of backend_drm, \
             backend_gbm, backend_libinput and backend_session_libseat; a default build \
             therefore pulls the system-library boundary into a milestone whose exit \
             criteria forbid GPU and wlroots work. Choosing pure Rust is a language \
             boundary, which D08 settles; choosing a package is an admission, which needs a \
             gate that calls it",
};

/// Both upstream checks this milestone ran.
pub const M07_UPSTREAM_CHECKS: [UpstreamCheck; 2] = [ZENOH_CHECK, COMPOSITOR_LIBRARY_CHECK];
