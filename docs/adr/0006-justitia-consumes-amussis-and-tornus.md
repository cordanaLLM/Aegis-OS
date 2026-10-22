<!-- markdownlint-disable MD013 -->
# ADR-0006: aegis-justitia consumes amussis-decision; tornus clones stay in Aegis

- **Status**: Accepted
- **Date**: 2026-10-22
- **Authors**: @lusoris (order recorded in the private controller ledger, 2026-10-22, paraphrased)

## Context

The Aegis OS decision interceptor (`crates/aegis-justitia`) and the standalone decision library `amussis-decision` (`plurano/amussis`) carry largely the same core modules. Upstream repository `plurano/amussis` imported `amussis-decision` with history from `cordanaLLM/Aegis-OS`, where it originated as crate `aegis-justitia`. Deity names remain the operating system convention (`aegis-justitia`, `aegis-hephaestus`), while generic names were adopted upstream (`amussis-decision`, `tornus-geometry`). Both repositories share the EUPL-1.2 license, Rust edition 2024, and pinned `rust-version = "1.99.1"`.

Comparing the module declarations in `crates/aegis-justitia/src/lib.rs` with `crates/amussis-decision/src/lib.rs` in `plurano/amussis` at `3b7b007` gives this split:

- Shared modules: `effects`, `engine`, `error`, `identity`, `intent`, `killswitch`, `ledger`, `outcome`, `oversight`, `registry`, `risk`, and `time`.
- Upstream only (`amussis-decision`): `encoding` (generic schema buffers and hex formatting), `proposal` (generic proposal schema), `ledger::anchor` (external chain anchors), and `killswitch::ReleaseError`.
- Operating system only (`aegis-justitia`): `contracts` (wire schemas `aegis.*`, subsystem graph edges `D03_ACTION_GATE` and `D04_SANDBOX_GATE`, and P-numbered subsystem payloads).
- Type renames upstream: `JustitiaEngine` became `DecisionEngine`, and `JustitiaError` became `DecisionError`.

Deduplication scans run with `praetorctl dedupe scan --across=<other> --json .` (praetorctl commit `8dacd54`, Aegis commit `c23db44`) found this cross-repository duplication, summed per Aegis directory:

- `crates/aegis-justitia/src`: 45 clone groups, 1907 LOC across core engine, identity, oversight, killswitch, and time.
- `crates/aegis-justitia/src/ledger`: 29 clone groups, 1055 LOC across the SHA-256 hash chain and signer bindings.
- `crates/aegis-justitia/tests`: 68 clone groups, 3149 LOC across decision engine and audit ledger integration suites.
- `crates/aegis-justitia/tests/common`: 10 clone groups, 214 LOC of test harness fixtures.
- `crates/aegis-justitia/src/contracts`: 5 clone groups, 173 LOC cloning `amussis-decision/src/encoding/fields.rs`.
- `crates/aegis-justitia/src/effects`: 1 clone group, 28 LOC cloning `system_clock.rs`.
- `crates/aegis-hephaestus/tests`: 5 clone groups, 191 LOC shared with the `tornus-geometry` test harnesses in `stubbed_effects.rs` and `allocation_bounds.rs` (`DX-d138647026` records 209 LOC).

The `aegis-justitia` slices listed below add up to about 6,600 duplicated lines with `amussis-decision`. The handover documents `docs/handover/aegis-os-schemas.md` and `docs/handover/aegis-os-service-unit.md` in `plurano/amussis` define what moved back to Aegis.

This deduplication follows established repository precedent recorded in `docs/roadmap/toolchain-admission.md` (lines 268-290, 345-356): `saeptum-capsule` (CUT-1632-03), `saeptum-microvm` (CUT-1632-04), and `tornus-geometry` (CUT-1632-01) were previously extracted from Aegis crates into standalone repositories under the `plurano` organisation.

## Decision

1. **Dependency direction**: Aegis-OS depends on plurano repositories (`plurano/amussis`, `plurano/tornus`), never the reverse. Upstream repository `plurano/amussis` carries zero Aegis names (enforced by `tools/origin_names.py` and recorded in its `README.md`). Deity names (`aegis-justitia`, `aegis-hephaestus`) and operating system abstractions remain exclusively inside Aegis-OS.

2. **Source of truth per module**:

   | Module | Source of truth | Architectural scope |
   | :--- | :--- | :--- |
   | `effects` | `amussis-decision` | System clock abstraction and monotonic timing primitives (`effects/system_clock.rs`). |
   | `engine` | `amussis-decision` | Core policy decision engine (`DecisionEngine`, `EngineConfig`). |
   | `error` | `amussis-decision` | Aggregate error type (`DecisionError`) wrapping all component errors. |
   | `identity` | `amussis-decision` | Actor, agent, checker, and resource identifier types (`AgentId`, `CheckerId`, `IntentId`). |
   | `intent` | `amussis-decision` | Action intent definitions, intent specifications, and intent error validation. |
   | `killswitch` | `amussis-decision` | Emergency halt state latch and release mechanics (`Killswitch`, `KillswitchState`). |
   | `ledger` | `amussis-decision` | Audit ledger, append-only JSONL sinks, hash chain verification, SHA-256 and Ed25519 bindings. |
   | `outcome` | `amussis-decision` | Decision interceptor outcomes, block reasons, and warning codes. |
   | `oversight` | `amussis-decision` | Multi-checker sets, voting, approvals, and adjudications. |
   | `registry` | `amussis-decision` | In-flight request registry and request state machine transitions. |
   | `risk` | `amussis-decision` | Risk tiers, action types, and oversight classification levels. |
   | `time` | `amussis-decision` | Monotonic deadlines, TTL approval windows, and Unix timestamps. |
   | `encoding` | `amussis-decision` | Upstream wire buffer encoding and hexadecimal field formatting (`encoding::fields`). |
   | `proposal` | `amussis-decision` | Generic action proposal schema (`amussis-decision` upstream only). |
   | `ledger::anchor` | `amussis-decision` | External blockchain / ledger anchor bindings (`amussis-decision` upstream only). |
   | `contracts` | `aegis-justitia` | Aegis wire schemas (`aegis.*`), subsystem graph edges (`D03_ACTION_GATE`, `D04_SANDBOX_GATE`), P-numbered payloads, hardened systemd service unit (`justitia-interceptor.service`), and unit contracts (`contracts/unit.rs`). |
   | Adapter layer | `aegis-justitia` | Re-exports `DecisionEngine` as `JustitiaEngine` and `DecisionError` as `JustitiaError` (`pub use amussis_decision::DecisionError as JustitiaError;`). Re-exporting is chosen over wrapper types because `crates/aegis-justitia/tests/public_surface_triples.rs` compares errors by value against variants such as `JustitiaError::Oversight(OversightError::SystemHalted)`, and those comparisons, the `#[from]` conversions behind `?` (HISS-07) and the generic parameters of the engine (`JustitiaEngine<H, S, K, C>` in `src/engine.rs`, `DecisionEngine<H, S, K, C>` upstream) all carry over unchanged through an alias. One difference is not cosmetic: upstream `DecisionError` has a `Release(ReleaseError)` variant that `JustitiaError` lacks, and neither enum is `#[non_exhaustive]`, so step 3 adds that variant to the Aegis public surface and covers it in the public-surface tests. |

3. **Versioning and pin strategy**:

   - `amussis-decision`: Published on crates.io at `0.2.0` (API `max_version 0.2.0`, git tag `v0.2.0`, commit `3b7b007`, license EUPL-1.2, edition 2024, `rust-version 1.99.1`). Aegis pins it exactly via `amussis-decision = "=0.2.0"` in root `Cargo.toml` under `[workspace.dependencies]`. A crates.io release is immutable and its checksum is recorded in `Cargo.lock`, and the git-source precedent in `docs/roadmap/toolchain-admission.md` already names a crates.io release as the trigger to leave a `rev =` pin. Renovate manages pin updates, and an admission row is added to `docs/roadmap/toolchain-admission.md` at migration time.
   - `tornus-geometry`: Repository `plurano/tornus` (`origin/main` commit `ef5418f`), package version 0.1.0 is not published on crates.io. Aegis maintains its existing git revision pin `rev = "df214e113701297d995cd3d14ffd877bd0cf8d27"` in `Cargo.toml` line 56, with review trigger set to a crates.io release or needed revision.

4. **Migration order**:

   Migration proceeds across six sequential pull requests, smallest first, each gated on `make verify-all`. A step that changes the public surface of `aegis-justitia` updates its positive, negative and boundary tests in the same pull request (HISS-15):

   - **Step 1**: Add `amussis-decision = "=0.2.0"` to `[workspace.dependencies]` in root `Cargo.toml` and record the dependency admission row in `docs/roadmap/toolchain-admission.md`.
   - **Step 2**: Refactor `crates/aegis-justitia/src/contracts/`: drop `src/contracts/encoding.rs`, consume `amussis_decision::encoding::fields`, and integrate halt-latch handling into `contracts/audit.rs` (per `docs/handover/aegis-os-schemas.md`).
   - **Step 3**: Replace the duplicated modules in `crates/aegis-justitia/src/` (including `src/effects/` and `src/ledger/`) with re-exports from `amussis_decision`, re-exporting `DecisionEngine as JustitiaEngine` and `DecisionError as JustitiaError`, and cover the added `Release` variant.
   - **Step 4**: De-duplicate test suite in `crates/aegis-justitia/tests/`. Under HISS-15, Aegis maintains positive, negative, and boundary tests for its own public interface and wire contracts:
     - *Retain*: `tests/public_surface_triples.rs` (public API contract verification), `tests/manifest_hygiene.rs` (dependency pinning), and wire schema contract tests (`tests/contract_action_proposal.rs`, `tests/contract_audit_record.rs`, `tests/contract_decision_request.rs`, `tests/contract_graph_decisions.rs`, and `tests/contract_unit.rs`).
     - *Delete*: tests that exercise only upstream behaviour and have an upstream counterpart (`tests/decision_engine.rs`, `tests/audit_ledger.rs`, `tests/refusal_audit.rs`, `tests/interface_triples.rs`, and the fixtures in `tests/common/mod.rs` that only they use). The pull request lists each deleted test next to its upstream counterpart.
     - *Classify in the same pull request*: the remaining files (`allocation_bounds.rs`, `chain.rs`, `contract_encoding.rs`, `jsonl_rendering.rs`, `public_surface.rs`, `registry_lifecycle.rs`, `totality.rs`) are kept or deleted by the same rule.
   - **Step 5**: Keep `crates/aegis-hephaestus/tests/` as it is. `stubbed_effects.rs` sweeps the sources of `aegis-hephaestus` itself and `allocation_bounds.rs` measures its own contract types, so neither is covered by the `tornus-geometry` tests it resembles; what the two repositories share is the test scaffolding, which step 6 handles. `aegis-hephaestus` already consumes `tornus-geometry` through the `rev =` pin, so no code moves in this step; the step records that result in the plan graph.
   - **Step 6**: Settle the shared test helpers in `crates/*.rs` and their copies in `plurano/amussis` and `plurano/tornus` once the first open question below is answered. Until then they stay accepted duplicates.

## Slice Targets

The following plan-graph slices define the migration targets across repositories:

| Slice ID | Groups / Lines | Aegis Paths | Other-Repo Paths | Target | Step |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `DX-59adcdd813-crates-1` | 8 groups / 107 lines | `crates/surface_scan.rs`, `crates/dependency_closure.rs`, `crates/child_deadline.rs` | `crates/dependency_closure.rs`, `crates/amussis-decision/tests/public_surface.rs` | open question | Step 6 |
| `DX-59adcdd813-crates-aegis-forum-shell-tests-1` | 2 groups / 15 lines | `crates/aegis-forum-shell/tests/manifest_hygiene.rs` | `crates/dependency_closure.rs` | stays in Aegis, is Aegis-only | none |
| `DX-59adcdd813-crates-aegis-janus-lifecycle-tests-1` | 2 groups / 21 lines | `crates/aegis-janus-lifecycle/tests/stubbed_effects.rs` | `crates/amussis-decision/tests/{public_surface.rs, manifest_hygiene.rs}` | stays in Aegis, is Aegis-only | none |
| `DX-59adcdd813-crates-aegis-justitia-src-1` | 45 groups / 1907 lines | `crates/aegis-justitia/src/{engine.rs, identity.rs, oversight.rs, lib.rs, killswitch.rs, time.rs}` | `crates/amussis-decision/src/{engine.rs, identity.rs, oversight.rs, lib.rs, killswitch.rs, time.rs}` | consume amussis-decision | Step 3 |
| `DX-59adcdd813-crates-aegis-justitia-src-contracts-1` | 5 groups / 173 lines | `crates/aegis-justitia/src/contracts/encoding.rs` | `crates/amussis-decision/src/encoding/fields.rs` | consume amussis-decision | Step 2 |
| `DX-59adcdd813-crates-aegis-justitia-src-effects-1` | 1 groups / 28 lines | `crates/aegis-justitia/src/effects/system_clock.rs` | `crates/amussis-decision/src/effects/system_clock.rs` | consume amussis-decision | Step 3 |
| `DX-59adcdd813-crates-aegis-justitia-src-ledger-1` | 29 groups / 1081 lines | `crates/aegis-justitia/src/ledger/{mod.rs, jsonl.rs, hash.rs, signer.rs, chain.rs, sha256.rs}` | `crates/amussis-decision/src/ledger/{mod.rs, jsonl.rs, hash.rs, signer.rs, sha256.rs}` | consume amussis-decision | Step 3 |
| `DX-59adcdd813-crates-aegis-justitia-tests-1` | 50 groups / 2311 lines | `crates/aegis-justitia/tests/{decision_engine.rs, audit_ledger.rs, refusal_audit.rs, interface_triples.rs}` | `crates/amussis-decision/tests/{decision_engine.rs, audit_ledger.rs, refusal_audit.rs, interface_triples.rs}` | consume amussis-decision | Step 4 |
| `DX-59adcdd813-crates-aegis-justitia-tests-2` | 18 groups / 838 lines | `crates/aegis-justitia/tests/{public_surface_triples.rs, manifest_hygiene.rs}` | `crates/amussis-decision/tests/{public_surface.rs, manifest_hygiene.rs}` | stays in Aegis, is Aegis-only | Step 4 |
| `DX-59adcdd813-crates-aegis-justitia-tests-common-1` | 10 groups / 246 lines | `crates/aegis-justitia/tests/common/mod.rs` | `crates/amussis-decision/tests/common/mod.rs` | consume amussis-decision | Step 4 |
| `DX-59adcdd813-crates-aegis-scaena-tests-1` | 5 groups / 58 lines | `crates/aegis-scaena/tests/manifest_hygiene.rs` | `crates/amussis-decision/tests/manifest_hygiene.rs` | stays in Aegis, is Aegis-only | none |
| `DX-59adcdd813-crates-amussis-decision-src-encoding-1` | 2 groups / 36 lines | `crates/contract_scaffold.rs`, `crates/contract_encoding.rs` | `crates/amussis-decision/src/encoding/{mod.rs, fields.rs}` | open question | Step 6 |
| `DX-59adcdd813-crates-amussis-decision-src-ledger-1` | 1 groups / 12 lines | `crates/millis_deadline.rs` | `crates/amussis-decision/src/ledger/mod.rs` | open question | Step 6 |
| `DX-59adcdd813-crates-amussis-decision-tests-1` | 13 groups / 149 lines | `crates/surface_scan.rs` | `crates/amussis-decision/tests/public_surface.rs` | open question | Step 6 |
| `DX-d138647026` | 5 groups / 209 lines | `crates/aegis-hephaestus/tests/{stubbed_effects.rs, allocation_bounds.rs, common/mod.rs}` | `crates/tornus-geometry/tests/{stubbed_effects.rs, allocation_bounds.rs, common/mod.rs}` | stays in Aegis, is Aegis-only; shared scaffolding follows the first open question | Step 5, Step 6 |
| Governance templates | amussis 33 groups / 9176 lines; tornus 8 groups / 2919 lines | `.config/lefthook/`, `.github/workflows/`, `tools/markdownlint/`, `tools/figures/` | `.config/lefthook/`, `.github/workflows/`, `tools/markdownlint/`, `tools/figures/` | shared repo tooling, not code: accepted duplicate via praetor templates | N/A (not a DX slice) |

## Consequences

- **Positive**: Removes about 6,600 duplicated lines of decision engine, ledger and test code between `aegis-justitia` and `amussis-decision`. Upstream algorithmic enhancements, ledger verification optimizations, and security audits flow directly to Aegis through automated dependency updates.
- **Positive**: Establishes a clean architectural boundary: Aegis-OS owns operating-system wire schemas, subsystem graph edges (`aegis.*`), and systemd unit bindings, while `amussis-decision` owns generic policy decision logic, audit ledgers, and hashing.
- **Positive**: `DX-d138647026` gets a recorded target without deleting tests that guard `aegis-hephaestus` itself.
- **Negative**: Introduces an external dependency boundary for `aegis-justitia`, requiring version updates and changelog reviews via Renovate.
- **Risks**:
  - *API drift*: Breaking changes in `amussis-decision` releases could require updates in the `aegis-justitia` adapter layer.
  - *Pin lag*: Critical bug or security fixes in `amussis-decision` require publishing a new crates.io release and updating the exact workspace pin in Aegis-OS.
  - *Wasm packaging asymmetry*: `plurano/amussis` provides `amussis-decision-wasm`, which Aegis does not consume because no Aegis crate targets WebAssembly today; a feature that lands only in the Wasm crate does not reach Aegis.

## Open Questions

- Whether the test-support harnesses in `crates/*.rs` (`surface_scan.rs`, `dependency_closure.rs`, `contract_scaffold.rs`, `contract_encoding.rs`, `millis_deadline.rs`, `child_deadline.rs`) should be extracted into a shared library crate across the fleet or maintained as accepted duplicated source files across workspace roots.
- Whether `amussis-decision` should eventually provide generic conversation schema traits under an optional feature flag, or whether schema contracts and subsystem graph edges remain permanently within Aegis-OS.
- When `plurano/tornus` will publish a `0.1.0` release to crates.io, enabling `tornus-geometry` to transition from a git revision pin to an exact crates.io version pin.

## References

- Upstream repositories and revisions:
  - `plurano/amussis`: `origin/main` commit `3b7b007` (2026-10-22), git tag `v0.2.0`, crates.io release `0.2.0`.
  - `plurano/tornus`: `origin/main` commit `ef5418f`, pinned git revision `df214e113701297d995cd3d14ffd877bd0cf8d27`.
  - `cordanaLLM/Aegis-OS`: deduplication scan baseline commit `c23db44`.
- Handover documentation:
  - `plurano/amussis`: `docs/handover/aegis-os-schemas.md` and `docs/handover/aegis-os-service-unit.md` at `3b7b007`.
- Deduplication scans (praetorctl commit `8dacd54`), reproducible from an Aegis checkout at `c23db44` beside the two upstream checkouts:
  - `praetorctl dedupe scan --across=<plurano/amussis checkout> --json .`
  - `praetorctl dedupe scan --across=<plurano/tornus checkout> --json .`
- Slice identifiers come from the private plan graph. Its clone hashes do not reproduce with praetorctl `8dacd54`, so each slice is matched to the scan by the directory its identifier names.
- Architectural precedents:
  - `docs/roadmap/toolchain-admission.md` (git-source table lines 345-356, narrative lines 268-290).
  - Root `Cargo.toml` line 56 (`tornus-geometry` pin).
