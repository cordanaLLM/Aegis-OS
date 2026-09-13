<!-- markdownlint-disable MD013 -->
# Project Governance

## Overview

Aegis OS is an open project maintained under Praetor governance. This document
describes how decisions are made, how components are activated, and how
maintainership is earned. It describes governance only. This repository does
not build a product image, does not boot on any machine, and has never
published a release; those gates, along with hardware and accessibility, remain
separately blocked. `make readiness` prints the current component and milestone
state.

## Decision Making & Architecture Decision Records

- Routine changes are decided through pull request review and approval by a
  maintainer.
- Changes to the OS concept, to an invariant, or to shared ownership boundaries
  require an Architecture Decision Record in `docs/adr/`. Once an ADR is
  accepted its text is immutable; later changes need a superseding ADR.
- Imported proposal data under the private working directory never becomes
  policy by import; only reviewed, tracked files are authoritative.

## Component Activation

A component listed in `planning/components.json` moves from `proposal` to
`activated` — the only advanced status the gate admits — only when it records an
`activation_evidence` block that `tools/verify_preparation.py` accepts: a
component directory named after the component, a git-tracked manifest inside
that directory whose `[package] name` is the component's name, a git-tracked
dependency lock, a test command that names the component, and the milestone the
activation closes. The component's remaining `activation_blockers` stay recorded
and non-empty: activation certifies tracked code and tests, never a build, boot,
hardware or release claim. A pinned interface contract and positive, negative
and boundary tests are required by `AGENTS.md` and checked in review.
`tools/verify_preparation.py` is the authority on the machine-checked half; read
`planning/components.json` for the recorded precedent.

Shared ownership with Imago, Nucleus, and Golusoris is defined in
`docs/integration/stack.md`. The ordered path is kept in `docs/roadmap/` and
`planning/roadmap.json`.

## Maintainer Roles & Responsibilities

- **Maintainers** review pull requests, keep `make verify-all` green, triage
  issues, and own the roadmap's blocking states.
- **Contributors** submit changes, documentation, bug reports, or answer
  questions. Every contribution follows `CONTRIBUTING.md` and `LICENSING.md`.

## Becoming a Maintainer

Contributors who sustain reviewed contributions across several roadmap
milestones, follow the HISS invariants, and review constructively may be
nominated by an existing maintainer.
