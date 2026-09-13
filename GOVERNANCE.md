<!-- markdownlint-disable MD013 -->
# Project Governance

## Overview

Aegis OS is a planning-stage open project maintained under Praetor governance.
This document describes how decisions are made, how components are activated,
and how maintainership is earned. It does not claim any runtime, build, boot, or
release capability.

## Decision Making & Architecture Decision Records

- Routine changes are decided through pull request review and approval by a
  maintainer.
- Changes to the OS concept, to an invariant, or to shared ownership boundaries
  require an Architecture Decision Record in `docs/adr/`. Once an ADR is
  accepted its text is immutable; later changes need a superseding ADR.
- Imported proposal data under the private working directory never becomes
  policy by import; only reviewed, tracked files are authoritative.

## Component Activation

A component listed in `planning/components.json` moves from proposal to active
only when all of the following exist: its component manifest, a dependency lock,
its interface contract, and positive, negative, and boundary tests. Shared
ownership with Imago, Nucleus, and Golusoris is defined in
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
