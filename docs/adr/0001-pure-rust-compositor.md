<!-- markdownlint-disable MD013 -->
# ADR-0001: Implement the P04 compositor in pure Rust

- **Status**: Accepted
- **Date**: 2026-09-13
- **Authors**: @lusoris (decision), roadmap decision D08

## Context

The P04 Mercurius blueprint rejects libweston and Smithay for the fast runtime
loop and mandates a C wlroots implementation that owns the event loop
(requirement REQ-P04-01). The only code candidate for P04 is a Rust scaffold
with no wlroots binding, and no wlroots version is pinned anywhere in the
sources. Keeping the mandate would add a C toolchain, an FFI boundary, and a
second memory-safety regime to the component that sits at the centre of the IPC
mesh (P05, P07, P08, P10, P11 and P15 all connect to it).

The roadmap had left the choice open until M07 with a backend-agnostic registry
and IPC state machine. The maintainer decided the question directly.

## Decision

P04 aegis-compositor is implemented as a pure Rust compositor. The C wlroots
mandate in the P04 blueprint is superseded. The Rust compositor library is
selected at M07 through the template matrix after checking current upstream
support; no C compositor toolchain is admitted.

## Consequences

- **Positive**: one language and one memory-safety model across the host
  daemons; the HISS-07 and HISS-09 rules apply to the compositor without an FFI
  exception; no C toolchain admission on the M07 path.
- **Negative**: the blueprint's latency argument for wlroots (control of the
  event loop for the fast runtime loop) is no longer backed by its own
  rationale. The frame-pacing and IPC latency budgets must be demonstrated with
  Rust implementation evidence in M07 and on real GPU hardware in M12 before
  they are claimed.
- REQ-P04-01 stays in the requirement set as source evidence and is marked
  superseded by this ADR.

## References

- Roadmap decision D08, `docs/roadmap/README.md` (Risks and decisions).
- Requirement REQ-P04-01, `docs/roadmap/requirements.md`.
- Maintainer answer, 2026-09-13, to "The P04 blueprint mandates a C wlroots
  compositor core, but the only code candidate is pure Rust with no wlroots
  binding. Which way?": "Pure Rust compositor".
