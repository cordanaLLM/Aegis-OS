<!-- markdownlint-disable MD013 -->
# ADR-0007: Test support stays duplicated across workspaces; delete public_surface_triples

- **Status**: Accepted
- **Date**: 2026-10-24
- **Authors**: Controller decisions aegis-os-adr0006-oq1-test-support-home, aegis-os-adr0006-oq1-record, aegis-os-adr0006-surface-triples
- **Supersedes**: ADR-0006 step 4, the retain entry for tests/public_surface_triples.rs, in part
- **Answers**: ADR-0006 open question 1

## Context

ADR-0006 open question 1 asked whether the test-support harnesses in `crates/*.rs` (`crates/surface_scan.rs`, `crates/dependency_closure.rs`, `crates/contract_scaffold.rs`, `crates/contract_encoding.rs`, `crates/millis_deadline.rs`, and `crates/child_deadline.rs`) move into a shared library crate across the fleet or remain as duplicated source files across workspace roots.

No shared test-support crate exists in the fleet. Creating a dedicated shared crate would introduce a permanent crate name, cross-repository release dependencies, versioning overhead, and coordination friction across independent repositories for non-production test scaffolding.

In addition, ADR-0006 step 4 retained `crates/aegis-justitia/tests/public_surface_triples.rs` under the assumption that it was required for public API contract verification of `aegis-justitia`. Cross clone groups between `crates/aegis-justitia/tests` and `amussis-decision` v0.2.0 also required an authoritative duplicate policy.

## Decision

1. **Test-support harnesses stay duplicated**: The test-support harnesses in `crates/*.rs` (`crates/surface_scan.rs`, `crates/dependency_closure.rs`, `crates/contract_scaffold.rs`, `crates/contract_encoding.rs`, `crates/millis_deadline.rs`, and `crates/child_deadline.rs`) remain accepted duplicated source files in each workspace root (`cordanaLLM/Aegis-OS`, `plurano/amussis`, and `plurano/tornus`). Copies are reviewed and maintained independently per repository. Drift between copies is acceptable because each copy exists to test its own workspace.

2. **Cross clone groups accepted as duplicates**: The cross clone groups between `crates/aegis-justitia/tests` and `amussis-decision` v0.2.0 are accepted duplicates as test-support helpers and Aegis-owned tests (contract wire fixtures, allocation witnesses, manifest hygiene of the Aegis workspace, and jsonl feature wiring). A helper group longer than 35 lines is not accepted as a duplicate: the `Provenance`/`ProvenanceVisitor` helper (45 lines) in `crates/aegis-justitia/tests/allocation_bounds.rs` stays open for a follow-up refactoring.

3. **Delete public_surface_triples.rs**: `crates/aegis-justitia/tests/public_surface_triples.rs` is deleted. ADR-0006 step 4 retained it for public API contract verification, but `crates/surface_scan.rs` (`parse_declared_name` and the documentation for `declared_name`) skips `pub use` re-exports, scanning only keyword-declared items. The public surface sweep in `crates/aegis-justitia/tests/public_surface.rs` never needed `public_surface_triples.rs`. All 20 tests in `public_surface_triples.rs` exercise re-exported `amussis-decision` items and have same-named counterparts upstream at the pinned version `0.2.0` (`crates/amussis-decision/tests/public_surface_triples.rs`). Measured verification confirms that `public_surface.rs` passes with the file removed (`cargo test -p aegis-justitia --all-features --test public_surface`). Unused fixtures in `crates/aegis-justitia/tests/common/mod.rs` (`record_draft` and related imports) are removed.

## Consequences

- **Positive**: Deletes 654 lines and 20 integration tests from `crates/aegis-justitia/tests/public_surface_triples.rs` that duplicated the upstream suite.
- **Positive**: Eliminates dead fixture code (`record_draft`) from `crates/aegis-justitia/tests/common/mod.rs`.
- **Positive**: Avoids creating a shared test library crate and its cross-repository release dependency overhead.
- **Positive**: Resolves ADR-0006 open question 1 with an explicit duplicate-retention policy for workspace root test scaffolding.
- **Negative**: Aegis no longer re-runs those 20 checks locally during `cargo test -p aegis-justitia`. A pin bump of `amussis-decision` relies on the upstream suite at the new version, while Renovate update pull requests run Aegis's remaining tests.

## References

- ADR-0006: `docs/adr/0006-justitia-consumes-amussis-and-tornus.md`
- Controller decision `aegis-os-adr0006-oq1-test-support-home`
- Controller decision `aegis-os-adr0006-oq1-record`
- Controller decision `aegis-os-adr0006-surface-triples`
