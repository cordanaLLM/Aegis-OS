# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Split licensing: EUPL-1.2 for technical material and CC BY-SA 4.0 for original prose, with REUSE metadata, pinned canonical licence texts, and a fail-closed digest check in `make verify-all`.
- Community and governance scaffold: code of conduct, governance, maintainers, support, code owners, issue templates, and Renovate configuration.
- Source-cited roadmap under `docs/roadmap/` with machine-readable milestone blocking states in `planning/roadmap.json`, validated by `make verify-all` and listed by `make readiness`.
- Enriched component inventory: graph edges, external producers, hardware requirements, candidate sources, and the cheapest hardware-free first slice per subsystem.
- Governance-only GitHub Actions workflow (Preparation gate) that builds the pinned praetorctl, runs `make verify-all`, REUSE lint, and a DCO sign-off check.
- `CONTEXT.md` domain glossary and `make verify-reuse`.

### Changed
- Recorded maintainer decisions D01-D20 in the roadmap and component inventory; ADR-0001 (pure Rust compositor) and ADR-0002 (no Steamworks in the image) supersede the corresponding blueprint requirements.
- Governance documents now state the Rust-neutral I/O timeout rule, the DCO sign-off requirement, the planning-stage security scope, and the ADR numbering convention consistently.
- The pre-push gate runs Praetor's pipeline in dry-run mode while the repository has no Go module; Go-only hook jobs were removed.
- Reserved implementation directories name the components they will host and their activation preconditions.

### Fixed
- Duplicate `.gitignore` rule, stale `standardsctl` reference in the pull request template, and the gatekeeper persona's invalid `--target` flag.
