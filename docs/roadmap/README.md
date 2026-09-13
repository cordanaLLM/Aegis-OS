# Aegis OS activation roadmap

Status: draft requiring review

## Outcome and scope

Aegis OS is a proposed image-based Linux OS with sixteen subsystems (P01-P16): Rust host daemons, kernel-space eBPF programs, mkosi/systemd-repart/systemd-sysupdate image definitions, and Svelte UI packages. The concept is preserved unchanged. This roadmap orders repository preparation and component activation so that each step yields verifiable evidence at the lowest cost.

Outcome: a ranked, dependency-explicit path from the current state to a first real image with retained artifact, signature and boot evidence, then to hardware-backed slices and release. The current state is: governance gate green, licence scaffold committed, remote declared and resolving, and all sixteen components in `proposal` status. Every milestone states what unblocks it. No milestone claims build, boot, hardware or release evidence before the real inputs and checks exist.

Scope inclusions: component inventory reconciliation; one component promoted end-to-end; workstation-testable slices for the remaining components; the Aegis-side and producer-side halves of the Imago/Nucleus contract; eBPF verification on a local fixture and on the Nucleus kernel; a minimal image; hardware-backed slices; release and remote delivery.

Scope exclusions (from the sources and governance): architectural redesign of the sixteen subsystems; activation of imported CI, release or integration workflows (they suppress failures and print simulated success, see REQ-CI-01, REQ-CI-02 and REQ-BOOT-02); any claim derived from proposal narrative or benchmark figures; imported dependency versions treated as active pins; owners or dates beyond repository names and dates present in the sources.

Current verified state at revision time:

- LICENSE, LICENSES/, LICENSING.md and REUSE.toml are committed (a2c9626).
- The component inventory enrichment and a governance-only CI gate are committed (42a23c8).
- `origin` resolves to https://github.com/cordanaLLM/Aegis-OS.git with main at 42a23c8, read back with `git ls-remote`. Hosted ruleset readback is not verified.
- No Cargo.toml is tracked, and the crates/*/ directories contain no crate files.
- planning/components.json lists all sixteen components as `proposal`, with activation blockers and cheapest first slices.
- The private readiness matrix records the Imago/Nucleus edges as declared but unverified, and the configured producer origins as not resolving.

## Method

Dependency DAG built from the sixteen activation cards in planning/components.json: subsystem edges from the subsystem graph (export-062), external producers (Imago, Nucleus, Golusoris, upstream), toolchain admissions and hardware needs. Milestones are bounded, verifiable states. Each is costed (trivial=1, small=2, medium=4, large=8), scored by the number of milestones it transitively unblocks, and flagged for hardware or unverified cross-repository contracts. Ranking takes the available milestone (all blockers ranked) with the highest (1 + transitive unblocks) / cost, under hard rules: (1) done or purely local work ranks before anything needing hardware or an unverified cross-repository contract; (2) the first cross-repository milestone is stack.md step 2 (M09: pin Imago/Nucleus schemas and test one request/result pair locally), preceded by the Aegis-owned schema authoring (M18); (3) exactly one first component (P06, M02) is promoted end-to-end before any second component; (4) build, boot and release milestones are blocked with the evidence that unblocks them; (5) state=ready iff every blocked_by milestone is done. Ties break toward the milestone with more transitive unblocks, then toward the critical path to the first artifact (M11), then toward milestones without an unverified external contract in their own flags or ancestry, then toward milestones without hardware. blocked_by lists direct blockers only; unblocks is its exact inverse.

Cost scale:

- trivial: hours, no new toolchain.
- small: one crate, one schema set or one configuration set with tests, on an admitted toolchain.
- medium: several crates, a new toolchain admission, or a VM-based verification.
- large: a real artifact or real device evidence.

Unblocking value counts the milestones that become reachable once a milestone is done. The subsystem graph (export-062) is the graph of record; the two architecture documents (export-002, export-003) are annotated disputes.

Toolchain admission rule: every milestone that runs a new tool names that tool in its exit criteria and selects it through the template matrix with a pinned version before any gate runs. The admissions are:

- M02: Rust
- M03: systemd floor
- M18: mkosi, if D14 keeps it in Aegis
- M19: clang/BPF, bpftool and libbpf or aya
- M04: Node, pnpm and Playwright
- M10 and M11: QEMU, OVMF, swtpm and signing tools
- M20: TPM2 tools
- M21: Firecracker
- M12: GPU stack

Milestones that only reuse an admitted toolchain say so.

First-component justification (rule 3): P06 aegis-justitia is promoted first (D01). The comparison with P01/P02 is recorded honestly:

- P01/P02 admits no new analyzer toolchain and sits on the critical path to the first image.
- P01/P02's interface contract is the Imago/Nucleus boundary, which cannot be accepted while both builder identities are unresolved.
- P06's contract is wholly Aegis-owned, its drafted test module already covers the positive, negative and boundary rules, and every agent-facing path depends on it.
- P06 is not a committed crate: M02 must first create the workspace root, because the only workspace listing is the proposal export-006.
- M02 is trimmed to the decision engine. The consumer contracts and unit contract move to M14, which also closes D03.

## Ranked milestones

| Rank | ID | Milestone | State | Cost | HW | Ext. contract | Blocked by | Unblocks | Exit criteria (summary) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | M00 | Governance gate, split licence and remote declaration | done | trivial | no | no | - | M01 | `make verify-all` passes and reports preparation/governance scope only (reported by this session's preparation run and recorded in the private verification ... |
| 1 | M01 | Component inventory reconciliation and decision register | ready | small | no | no | M00 | M02 | Entry condition: `make verify-all` is re-run at the committed tip (42a23c8 or later) and passes |
| 2 | M02 | First component promoted end-to-end: P06 aegis-justitia decision engine | blocked | small | no | no | M01 | M14, M03, M17, M07, M04 | A workspace root Cargo.toml is created (the repository has none; export-006 is proposal data) with crates/aegis-justitia as its only activated member; the ... |
| 3 | M14 | P06 consumer interface contracts and hardened unit contract | blocked | small | no | no | M02 | M05, M06, M16, M20 | D03 (direction and transport of the P06/P09 action gate) is closed and D04 (P06 to P10 syscall intercept) is recorded |
| 4 | M03 | P01/P02 partition and transfer definitions validated offline with the host systemd | blocked | small | no | no | M02 | M18, M15 | build/ holds the repart definitions (00-esp, 10-root-a/b, 11-root-verity, 20-var) and the sysupdate transfer as reviewed files with source hashes |
| 5 | M18 | Aegis-side product input manifest and kernel requirement schemas (local) | blocked | small | no | no | M03 | M09 | The Aegis product input manifest schema (repart, sysupdate and mkosi configuration references, correlation id, exact revision, bounded retries) validates ... |
| 6 | M15 | P02 A/B candidate lifecycle state machine | blocked | small | no | no | M03 | M11 | The A/B lifecycle (candidate, signature check, delta acquisition, slot swap, watchdog, bless or rollback) is a Rust library with manifest, lock (location ... |
| 7 | M05 | Evolution loop logic: P13 Tellus SCI engine and P16 Athena lifecycle, Pareto gate and ledger | blocked | small | no | no | M14 | M19, M21 | crates/aegis-tellus and crates/aegis-athena have manifests, lock entries and positive/negative/boundary tests passing under cargo test and clippy; the Rust ... |
| 8 | M06 | Agent execution chain logic: P09 Minerva router/replay/solver and P10 Vesta bounded controllers | blocked | small | no | no | M14 | M08, M21 | crates/aegis-minerva and crates/aegis-vesta have manifests, lock entries and positive/negative/boundary tests; no GPU, D-Bus, Z3 FFI, KVM or Wasm engine is ... |
| 9 | M17 | Trivial leaf slices: P03 Vulcan and P15 Hestia validation crates | blocked | trivial | no | no | M02 | M12 | aegis-vulcan and aegis-hestia are added as workspace members (they are absent from the proposal workspace) with manifests, lock entries and ... |
| 10 | M07 | Real-time control-plane logic: P04 compositor registry/IPC, P07 Lictor classifier/broker, P08 Calliope plugin lifecycle | blocked | medium | no | no | M02 | M19 | crates/aegis-compositor, aegis-lictor and aegis-calliope have manifests, lock entries and positive/negative/boundary tests; no wlroots, GPU, PipeWire, ... |
| 11 | M08 | Leaf slices dependent on the agent chain: P11 Ludus and P14 Hephaestus | blocked | small | no | no | M06 | M12 | aegis-ludus and aegis-hephaestus have manifests, lock entries and positive/negative/boundary tests; the Rust toolchain from M02 is reused |
| 12 | M19 | eBPF objects compiled and loaded through the verifier on the host kernel (local fixture) | blocked | medium | no | no | M05, M07 | M10 | Toolchain admission: clang (BPF target), bpftool and libbpf (or aya, per the M01 register) are selected through the template matrix with pinned versions ... |
| 13 | M04 | UI accessibility harness: P12 Concordia tokens with Node/pnpm/Playwright admission | blocked | medium | no | no | M02 | M16 | Toolchain admission: Node, pnpm, Playwright browsers and Svelte are selected through the template matrix with pinned versions (D10) before any UI gate runs; ... |
| 14 | M16 | P05 Forum shell state and lifecycle with stubbed IPC | blocked | small | no | no | M04, M14 | - | ui/forum-shell has its package manifest and lockfile on the toolchain admitted in M04 |
| 15 | M09 | Cross-repository contract pin (stack.md step 2): one local Imago/Nucleus request/result pair | blocked | small | no | yes | M18 | M11, M10 | The M18 schemas are proposed to cordanaLLM/imago and cordanaLLM/nucleus; acceptance is recorded only when each producer consumes the payload, not a fixed ... |
| 16 | M11 | Minimal image build with artifact, signature and boot evidence (stack.md step 4) | blocked | large | yes | yes | M09, M15 | M13, M20, M12 | BLOCKED until: Imago accepts the M09 manifest and returns an image/UKI with digest and signature; Nucleus supplies the pinned kernel artifact; D07 is closed |
| 17 | M10 | eBPF objects re-verified against the Nucleus-pinned kernel in a VM | blocked | medium | yes | yes | M09, M19 | M12 | BLOCKED until: the M09 pair delivers a Nucleus kernel whose version/config digest is recorded |
| 18 | M21 | Workstation hardware slices: RAPL energy counters and KVM/Firecracker sandboxing | blocked | medium | yes | no | M05, M06 | - | Hardware: a host with readable RAPL counters and KVM; the host model and kernel are recorded |
| 19 | M13 | Release signing and remote delivery (stack.md step 5) | blocked | medium | no | yes | M11 | - | BLOCKED until: an M11 artifact exists, hosted ruleset/label readback for origin is retained (D11), publication settings and at least one consumer are ... |
| 20 | M20 | TPM2 attestation slice on swtpm: audit-record signing and /var unseal | blocked | medium | yes | yes | M11, M14 | - | BLOCKED until: an M11 image boots under QEMU with swtpm |
| 21 | M12 | GPU-backed slices: DMA-BUF, VFIO and P2PDMA paths | blocked | large | yes | yes | M08, M10, M11, M17 | - | BLOCKED until: a booted M11 image exists, the M10 kernel exposes VFIO/IOMMU, and a DMA-BUF-capable GPU with IOMMU groups is available on a recorded host |

Ready set (every blocked_by done): M01. Done: M00. All other milestones are blocked. M09, M10, M11, M12, M13, M20 and M21 also require evidence or hardware that does not exist yet in qualifying form, and they must not be claimed.

Critical path to the first artifact: M00 -> M01 -> M02 -> M03 -> M18 -> M09 -> M11 (with M15). Ranks express priority, not strict sequencing. M09 becomes ready as soon as M18 is done and can run in parallel with the local component milestones.

## Milestone exit criteria and epics

### M00 - Governance gate, split licence and remote declaration

Rank 0. State: done. Cost: trivial. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: no. Needs external contract: no. Blocked by: -. Unblocks: M01.

Evidence: commit a2c9626 (licence scaffold); commit 4d0d798 (canonical remote declared); commit 42a23c8 (component inventory enrichment and governance-only CI gate); git ls-remote --heads origin: refs/heads/main 42a23c8; private preparation verification log: PASS, preparation/governance only.

Exit criteria:

- `make verify-all` passes and reports preparation/governance scope only (reported by this session's preparation run and recorded in the private verification log; this revision did not re-run it because the gate may write local state)
- LICENSE, LICENSES/, LICENSING.md and REUSE.toml are tracked; REUSE.toml declares EUPL-1.2 for technical material and CC-BY-SA-4.0 for prose (commit a2c9626)
- Remote `origin` https://github.com/cordanaLLM/Aegis-OS.git is configured; `git ls-remote --heads origin` (read-only) shows main at the committed checkpoint 42a23c8; hosted ruleset/label readback, publication settings and hosted acceptance are not claimed and belong to M13
- The done state rests on committed tree 42a23c8 (which also commits the enriched planning/components.json); uncommitted working-tree edits to CHANGELOG.md and README.md and the untracked CONTEXT.md observed at revision time are outside these criteria

Cheapest exit: Read-only readback: `git ls-files LICENSE LICENSING.md REUSE.toml`, `git remote -v`, `git ls-remote --heads origin`.

Epics:

- **E00-1 Governance gate reported with its limits**. Requirements: REQ-BOOT-02. Acceptance: Positive: make verify-all passes. Negative: a drifted generated client file fails `compile-context --verify`. Boundary: the report states preparation/governance scope only; simulated boot output is never cited as evidence.
- **E00-2 Split licence scaffold committed**. Requirements: REQ-GOV-01. Acceptance: LICENSING.md and REUSE.toml record EUPL-1.2 for technical material and CC-BY-SA-4.0 for prose. The proposal Cargo workspace (export-006) also declares EUPL-1.2; no workspace manifest is committed in the repository yet.
- **E00-3 Remote declared; proposal identity recorded for reconciliation**. Requirements: REQ-GOV-02. Acceptance: origin resolves to cordanaLLM/Aegis-OS with main present remotely. The proposal identity cordanaLLM/aegis-os (export-006) differs in letter case and is carried into the M01 register; hosted readback is not claimed.

### M01 - Component inventory reconciliation and decision register

Rank 1. State: ready. Cost: small. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: no. Needs external contract: no. Blocked by: M00. Unblocks: M02.

Exit criteria:

- Entry condition: `make verify-all` is re-run at the committed tip (42a23c8 or later) and passes
- The inventory records, for each of the 12 Rust candidates and 3 UI candidates, its source export id and hash, proposed path, proposal-workspace membership (export-006) and missing manifests; it states that no Cargo.toml is committed and that crates/*/ directories hold no crate files
- The inventory decides where the P01/P02 definition parser and the P02 A/B lifecycle crate live, since crates/README.md reserves paths for P03-P16 only (D15)
- Every graph contradiction (P05/P12, P06/P09, P06/P10, P09/P14, P15/P02) and every source conflict (ledger hash, focus-ring width, CSS framework, kernel identity) is an open decision that names the sources on each side
- The toolchain and version-drift register lists every imported pin as proposal data (mkosi v24+, Node 20 vs 22, aya/memmap2/zenoh versions, the unverified release action) plus the unpinned upstream projects; imported CI/release workflows are marked inactive
- `make verify-all` still passes

Cheapest exit: Write the inventory and decision register as reviewed planning files. No code and no toolchain installation.

Epics:

- **E01-1 Crate and UI inventory with manifest status**. Requirements: REQ-WS-01, REQ-P14-07, REQ-P16-07, REQ-P08-08, REQ-P09-08, REQ-P06-07, REQ-P13-08, REQ-P12-08, REQ-P15-05. Acceptance: Positive: 12 Rust and 3 UI candidates are listed with source id and hash. Negative: a candidate without a source hash fails review. Boundary: Vulcan and Hestia appear with status 'not a proposal-workspace member', and the P01/P02 crate location is decided.
- **E01-2 Graph and source contradiction register**. Requirements: REQ-GRAPH-01, REQ-GRAPH-02, REQ-GRAPH-03, REQ-GRAPH-04, REQ-GRAPH-05, REQ-P09-03, REQ-P09-05, REQ-P10-07, REQ-P15-07, REQ-P14-05, REQ-P12-07, REQ-P12-09, REQ-UI-02. Acceptance: Each disputed edge or value names both sources and the affected contract. export-062 stays the graph of record until a decision changes it.
- **E01-3 Toolchain, identity and version-drift register**. Requirements: REQ-GOV-02, REQ-P01-08, REQ-P02-06, REQ-CI-03, REQ-UI-01, REQ-REL-02, REQ-P01-07, REQ-P03-08, REQ-P04-08, REQ-P01-01. Acceptance: Every pinned or floating toolchain is listed with its source and marked as proposal data. Nothing is installed. Each drift item becomes a decision.
- **E01-4 Imported workflow quarantine**. Requirements: REQ-CI-01, REQ-CI-02, REQ-BOOT-02, REQ-REL-01, REQ-P14-08. Acceptance: Imported CI, release and integration scripts stay inactive. Their failure-suppressing and simulated-output steps are listed as defects to fix before any activation.

### M02 - First component promoted end-to-end: P06 aegis-justitia decision engine

Rank 2. State: blocked. Cost: small. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: no. Needs external contract: no. Blocked by: M01. Unblocks: M14, M03, M17, M07, M04.

Exit criteria:

- A workspace root Cargo.toml is created (the repository has none; export-006 is proposal data) with crates/aegis-justitia as its only activated member; the crate manifest and a committed Cargo.lock exist
- Toolchain admission: the stable Rust toolchain (cargo, clippy, rustfmt) is selected through the template matrix with a pinned version before any cargo gate runs
- Risk-tier, maker-checker, Annex III, timeout and killswitch logic exists as library code (not only in #[cfg(test)]) with positive, negative and boundary tests passing under `cargo test -p aegis-justitia` and `cargo clippy -p aegis-justitia --all-targets -- -D warnings`
- Audit-ledger hashing sits behind a trait, and the algorithm decision (D02: SHA-256 or BLAKE3; MD5 excluded) is recorded
- Out of scope, deferred explicitly: the consumer contracts and the hardened unit contract (M14), eBPF compilation and verifier load (M19, M10), and TPM2 signing (M20)
- planning/components.json P06 status is advanced with the evidence path; `make verify-all` passes

Cheapest exit: Promote the drafted P06 test module into a library crate with a trait boundary for hashing and signing. No TPM2, D-Bus or eBPF.

Epics:

- **E02-1 Workspace root, manifest, lock and Rust toolchain admission**. Requirements: REQ-P06-07, REQ-P16-08, REQ-GOV-01. Acceptance: Positive: the crate builds under `cargo build -p aegis-justitia` and a workspace-wide invocation. Negative: an unlocked dependency change fails `cargo build --locked`. Boundary: clippy with -D warnings passes with zero warnings.
- **E02-2 Decision engine with positive, negative and boundary tests**. Requirements: REQ-P06-01, REQ-P06-03, REQ-P06-04, REQ-GOV-03. Acceptance: Positive: a Tier C action is allowed, and two distinct non-maker checkers are accepted. Negative: maker as checker is rejected, and a duplicate checker is rejected. Boundary: due timestamps equal to now and to now-1 both fail closed, and the killswitch blocks Tier C.
- **E02-3 Audit ledger algorithm decision and trait boundary**. Requirements: REQ-P06-09, REQ-P16-09, REQ-P16-01. Acceptance: Positive: one algorithm is recorded and a chain re-walk verifies. Negative: a single flipped byte is detected, and a missing signer returns an error. Boundary: an empty ledger verifies as the genesis state. MD5 is removed.

### M14 - P06 consumer interface contracts and hardened unit contract

Rank 3. State: blocked. Cost: small. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: no. Needs external contract: no. Blocked by: M02. Unblocks: M05, M06, M16, M20.

Exit criteria:

- D03 (direction and transport of the P06/P09 action gate) is closed and D04 (P06 to P10 syscall intercept) is recorded
- Versioned schemas exist for the DecisionRequest (P06 to P05), the action proposal (P09 and P06, direction per D03) and the signed audit record (P06 to P16); each schema has contract tests with positive, negative and boundary payloads
- The justitia-interceptor unit stays a declarative contract (IPAddressDeny=any) and is not installed or executed
- Uses the Rust toolchain admitted in M02; no new toolchain; `make verify-all` passes

Cheapest exit: Write the three schemas as typed Rust structs with serde round-trip tests. No transport is implemented.

Epics:

- **E14-1 Action-gate direction decision and action proposal contract**. Requirements: REQ-P09-03, REQ-P09-05, REQ-GRAPH-04, REQ-GRAPH-02. Acceptance: Positive: a signed, well-formed proposal round-trips. Negative: an unsigned or malformed proposal is rejected. Boundary: a proposal at the maximum field lengths is accepted, and one byte over is rejected.
- **E14-2 DecisionRequest and audit record contracts**. Requirements: REQ-GOV-03, REQ-P06-08, REQ-P06-10, REQ-P16-03. Acceptance: Positive: both schemas round-trip. Negative: an unknown schema version is rejected. Boundary: an audit record whose previous hash is the all-zero genesis value is accepted only as the first record. The PLD effective date is tracked as an external deadline.
- **E14-3 Hardened unit contract**. Requirements: REQ-P06-02. Acceptance: Positive: the unit file declares IPAddressDeny=any. Negative: a review check fails if the directive is removed. Boundary: the unit is not installed or started in this milestone.

### M03 - P01/P02 partition and transfer definitions validated offline with the host systemd

Rank 4. State: blocked. Cost: small. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: no. Needs external contract: no. Blocked by: M02. Unblocks: M18, M15.

Exit criteria:

- build/ holds the repart definitions (00-esp, 10-root-a/b, 11-root-verity, 20-var) and the sysupdate transfer as reviewed files with source hashes
- Toolchain admission: a systemd version floor is selected through the template matrix (the host observed at revision time runs systemd 261); the Rust toolchain from M02 is reused for the parser; mkosi is not used in this milestone
- Repart gate: `systemd-repart --empty=create --size=<N> --dry-run=yes --definitions=<dir> <scratch image>` exits 0 on valid definitions; the gate also fails on any 'Unknown key ... ignoring' diagnostic, because systemd-repart exits 0 on unknown keys (observed on systemd 261, where the imported ESP 'Subsystem=' and var 'BtrfsSubvolumes=' keys are ignored)
- Negative and boundary cases observed on systemd 261: a definition without Type= exits 1; SizeMinBytes greater than SizeMaxBytes exits 1; SizeMinBytes equal to SizeMaxBytes is accepted
- Transfer gate: `systemd-sysupdate --root=<scratch tree containing usr/lib/sysupdate.d> --offline list` parses the transfer definition unprivileged; on systemd 261 the imported definition is rejected with exit 1, so it must be rewritten to supported keys before the gate can pass
- The Rust definition parser crate (location per D15) has its manifest, lock and positive/negative/boundary tests; no `|| true` anywhere in the gate

Cheapest exit: Run the two systemd commands against scratch images and trees, plus the parser tests. No image build and no mkosi.

Epics:

- **E03-1 Repart analyzer gate**. Requirements: REQ-P01-02, REQ-P01-03, REQ-P02-03, REQ-P02-07, REQ-P01-10, REQ-CI-01. Acceptance: Positive: the dry run exits 0 with no ignored-key diagnostics. Negative: a missing Type= exits non-zero, and an unknown key fails the gate. Boundary: ESP min equal to max is accepted, and inverted min greater than max is rejected.
- **E03-2 Sysupdate transfer gate**. Requirements: REQ-P01-04, REQ-P02-04, REQ-P02-06. Acceptance: Positive: the rewritten transfer lists without error offline. Negative: the imported transfer is rejected (exit 1). Boundary: a transfer with both root slots and no writable target parses, and one with a single slot is flagged against the A/B requirement.
- **E03-3 Definition parser crate**. Requirements: REQ-P01-02, REQ-P01-10, REQ-WS-01. Acceptance: Positive: the definitions round-trip through the parser. Negative: a duplicate section or malformed size is rejected. Boundary: 512M and 1G parse exactly, and 511M below the minimum is flagged.
- **E03-4 PCR measurement acceptance target**. Requirements: REQ-P01-05. Acceptance: The PCR 0/4/7/11 strategy is documented as the acceptance target for M11 boot evidence. It is not claimed here.

### M18 - Aegis-side product input manifest and kernel requirement schemas (local)

Rank 5. State: blocked. Cost: small. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: no. Needs external contract: no. Blocked by: M03. Unblocks: M09.

Exit criteria:

- The Aegis product input manifest schema (repart, sysupdate and mkosi configuration references, correlation id, exact revision, bounded retries) validates the M03 files; a malformed manifest is rejected
- The kernel requirement schema (required BPF LSM, sched_ext, BTF, RAPL, VFIO/IOMMU and KVM features; ABI; accepted architectures; artifact digest and signature fields) is written as a payload, not a fixed symbol list
- D07 (boot kernel identity) and D18 (distribution release and package pinning) are recorded
- Toolchain admission: if D14 keeps mkosi validation in Aegis, mkosi is selected through the template matrix at or above the v24 floor before `mkosi summary` runs; otherwise mkosi validation is deferred to the Imago result
- No producer repository is contacted; the schemas are Aegis-owned files with positive, negative and boundary tests

Cheapest exit: Author the two schemas and validate them against the M03 files with the Rust toolchain from M02.

Epics:

- **E18-1 Product input manifest schema**. Requirements: REQ-P01-01, REQ-P01-06, REQ-P01-08, REQ-GOV-02. Acceptance: Positive: the manifest built from the M03 files validates. Negative: a manifest without a correlation id or exact revision is rejected. Boundary: a retry count at the bound is accepted, and one above is rejected.
- **E18-2 Kernel requirement schema and kernel identity decision**. Requirements: REQ-P01-09, REQ-P07-01, REQ-P06-05, REQ-P13-02, REQ-P07-06. Acceptance: Positive: the payload lists the kernel features required by P06, P07 and P13. Negative: an unknown architecture is rejected. Boundary: an empty requirement list is rejected explicitly, not accepted as 'no requirements'.
- **E18-3 mkosi admission decision**. Requirements: REQ-P01-08, REQ-P02-06. Acceptance: Positive: if admitted, `mkosi summary` parses the Output stanza. Negative: a version below the floor is refused. Boundary: the floor version itself is accepted.

### M15 - P02 A/B candidate lifecycle state machine

Rank 6. State: blocked. Cost: small. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: no. Needs external contract: no. Blocked by: M03. Unblocks: M11.

Exit criteria:

- The A/B lifecycle (candidate, signature check, delta acquisition, slot swap, watchdog, bless or rollback) is a Rust library with manifest, lock (location per D15) and positive, negative and boundary tests
- D13 (reversible consolidation versus permanent hardening) is recorded against the dm-verity requirement
- Uses the Rust toolchain admitted in M02; systemd and sysupdate calls are stubbed

Cheapest exit: Model the lifecycle as a pure state machine with a stubbed clock and stubbed sysupdate.

Epics:

- **E15-1 A/B lifecycle state machine**. Requirements: REQ-P02-08, REQ-P02-01, REQ-P02-02. Acceptance: Positive: the happy path reaches Bless. Negative: a signature failure reaches Discard. Boundary: watchdog expiry exactly at the timeout takes Rollback, and one tick before does not.
- **E15-2 Reversible consolidation decision**. Requirements: REQ-P02-05, REQ-P16-05. Acceptance: The decision is recorded, and a test shows that a reopened slot still requires a verity match before Bless.

### M05 - Evolution loop logic: P13 Tellus SCI engine and P16 Athena lifecycle, Pareto gate and ledger

Rank 7. State: blocked. Cost: small. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: no. Needs external contract: no. Blocked by: M14. Unblocks: M19, M21.

Exit criteria:

- crates/aegis-tellus and crates/aegis-athena have manifests, lock entries and positive/negative/boundary tests passing under cargo test and clippy; the Rust toolchain from M02 is reused and no new toolchain is admitted
- The SCI formula, defer threshold and slice bound are tested; RAPL and eBPF access are stubbed
- The Athena 7-stage state machine and Pareto predicate are tested at exact thresholds; the ledger is ported to Rust with the D02 algorithm
- Edge contracts are typed with negative tests for malformed payloads: P16 to P13 candidate SCI query, P16 to P02 promotion trigger, and consumption of the M14 audit record

Cheapest exit: Extract the arithmetic and state machines into lib targets and mock all I/O.

Epics:

- **E05-1 Tellus SCI arithmetic**. Requirements: REQ-P13-01, REQ-P13-06, REQ-P13-07, REQ-P13-08. Acceptance: Positive: a hand-computed SCI matches. Negative: functional_units <= 0 falls back without NaN. Boundary: 300.0 is not deferred, 300.001 is deferred, and slices never exceed 16.
- **E05-2 Athena lifecycle and Pareto gate**. Requirements: REQ-P16-01, REQ-P16-02, REQ-P16-05, REQ-P16-10, REQ-P15-04. Acceptance: Positive: a passing candidate reaches Publish. Negative: exceeding any single bound reaches Invalidate with a ledger entry, and an empty id is rejected. Boundary: latency 1.5 and retention 0.99 are exercised exactly.
- **E05-3 Hash-chained ledger in Rust**. Requirements: REQ-P16-03, REQ-P16-09, REQ-P06-09. Acceptance: Positive: a chain re-walk verifies. Negative: a tampered record is detected. Boundary: the genesis record is verified, and the algorithm matches D02.
- **E05-4 Evolution-loop edge contracts**. Requirements: REQ-P13-05, REQ-P13-03, REQ-P16-06, REQ-P01-04. Acceptance: Positive: typed request and response round-trip for each edge. Negative: malformed payloads are rejected. Boundary: the sysupdate call is stubbed, and an empty candidate list is handled explicitly.

### M06 - Agent execution chain logic: P09 Minerva router/replay/solver and P10 Vesta bounded controllers

Rank 8. State: blocked. Cost: small. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: no. Needs external contract: no. Blocked by: M14. Unblocks: M08, M21.

Exit criteria:

- crates/aegis-minerva and crates/aegis-vesta have manifests, lock entries and positive/negative/boundary tests; no GPU, D-Bus, Z3 FFI, KVM or Wasm engine is used; the Rust toolchain from M02 is reused
- The Wasm runtime language-boundary decision (D06) is recorded: a Rust-native runtime, or a protocol/FFI adapter with contract tests; no Go crate dependency
- The P06 action proposal contract from M14 is consumed; the P09 to P10 capsule request and the P09/P14 verification direction are typed with negative tests

Cheapest exit: Compile only the in-memory logic layers of the P09 and P10 candidates with cargo test.

Epics:

- **E06-1 Minerva router, replay and solver logic**. Requirements: REQ-P09-01, REQ-P09-02, REQ-P09-06, REQ-P09-07, REQ-P09-08. Acceptance: Positive: expert registration and routing succeed. Negative: route returns None when no expert matches. Boundary: the 32nd expert is accepted and the 33rd rejected; the 128th trajectory step is accepted and the 129th rejected; relabelling flips only negative rewards.
- **E06-2 Vesta bounded controllers**. Requirements: REQ-P10-01, REQ-P10-04, REQ-P10-05, REQ-P10-06, REQ-P10-07. Acceptance: Positive: 64 microVMs and 128 capsules are accepted. Negative: terminating an unknown id returns false. Boundary: the 65th microVM and 129th capsule fail. The boot-time literal is marked unmeasured.
- **E06-3 Wasm runtime boundary decision**. Requirements: REQ-P10-08, REQ-P10-02, REQ-P10-03. Acceptance: The decision is recorded. Venus and AF_VSOCK pricing stay deferred hardware-backed requirements (M21).
- **E06-4 Agent-chain edge contracts**. Requirements: REQ-P09-03, REQ-GRAPH-05, REQ-P14-05, REQ-P16-04. Acceptance: Positive: typed contracts round-trip. Negative: unsigned or malformed proposals are rejected. Boundary: a capsule request at the capsule bound is accepted, and one over is rejected.

### M17 - Trivial leaf slices: P03 Vulcan and P15 Hestia validation crates

Rank 9. State: blocked. Cost: trivial. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: no. Needs external contract: no. Blocked by: M02. Unblocks: M12.

Exit criteria:

- aegis-vulcan and aegis-hestia are added as workspace members (they are absent from the proposal workspace) with manifests, lock entries and positive/negative/boundary tests; the Rust toolchain from M02 is reused
- Scaffold assertions are refactored to Result per HISS-07 so that negative cases are ordinary tests
- The Hestia location decision (D09) is applied from the M01 inventory
- Interface contracts are typed for P03 to P09 (weight streaming descriptor), P03 to P15 (media ingest descriptor) and P15 to P04 (overlay registration), each with a negative test for a malformed payload; the transports stay stubbed until M12

Cheapest exit: Extract the validation arithmetic and bounds checks. No VFIO, GPU or PGlite runtime.

Epics:

- **E17-1 Vulcan validation crate**. Requirements: REQ-P03-04, REQ-P03-05, REQ-P03-08, REQ-P03-06, REQ-WS-01, REQ-P03-01, REQ-P03-02. Acceptance: Positive: an aligned BAR is accepted. Negative: a misaligned BAR is rejected. Boundary: block_count 0 and 8193 are rejected, 8192 is accepted, and the ring index wraps.
- **E17-2 Hestia vector-store state machine**. Requirements: REQ-P15-01, REQ-P15-05, REQ-P15-06, REQ-P15-08, REQ-GRAPH-03. Acceptance: Positive: an initialized store answers queries. Negative: a query before init fails. Boundary: limits 0 and 101 fail, and 1 and 100 pass.
- **E17-3 P03 and P15 interface contracts**. Requirements: REQ-P03-07, REQ-P09-04, REQ-P15-07. Acceptance: Positive: the descriptors round-trip. Negative: a malformed descriptor is rejected. Boundary: a descriptor at the maximum block count is accepted, and one over is rejected.

### M07 - Real-time control-plane logic: P04 compositor registry/IPC, P07 Lictor classifier/broker, P08 Calliope plugin lifecycle

Rank 10. State: blocked. Cost: medium. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: no. Needs external contract: no. Blocked by: M02. Unblocks: M19.

Exit criteria:

- crates/aegis-compositor, aegis-lictor and aegis-calliope have manifests, lock entries and positive/negative/boundary tests; no wlroots, GPU, PipeWire, cgroups or BPF attach; the Rust toolchain from M02 is reused
- The Zenoh crate version, if needed, is selected and locked at activation after checking current upstream; the export-006 pin is not inherited
- The surface registry (256) and IPC client (64) bounds, EWMA tier thresholds, plugin slots (32) and DMA-BUF buffers (64) are tested at their edges
- The wlroots-in-C mandate versus the Rust scaffold is recorded (D08), and the frame-pacing constant is pinned
- BPF C compilation is not part of this milestone (moved to M19)

Cheapest exit: Unit-test the pure state machines only.

Epics:

- **E07-1 Compositor registry and Tier-1 socket bounds**. Requirements: REQ-P04-04, REQ-P04-05, REQ-P04-07, REQ-P04-08. Acceptance: Positive: 256 surfaces and 64 clients are accepted. Negative: the 257th surface is rejected and the 65th client is not admitted. Boundary: a pacing-constant test pins the chosen value.
- **E07-2 wlroots and mesh design decision**. Requirements: REQ-P04-01, REQ-P04-02, REQ-P04-03, REQ-P04-06. Acceptance: The decision is recorded. The Zenoh version is selected at activation after checking current upstream, with a mocked transport test: a publish round-trips, a subscriber cannot write back, and an empty key expression is rejected.
- **E07-3 Lictor EWMA tiers and broker**. Requirements: REQ-P07-01, REQ-P07-02, REQ-P07-03, REQ-P07-04, REQ-P07-05, REQ-P01-07. Acceptance: Positive: bursts below each threshold classify per the source comparisons. Negative: an unregistered focus PID throttles nothing. Boundary: bursts exactly at each threshold classify per the strict comparison.
- **E07-4 Calliope plugin lifecycle and DMA-BUF descriptors**. Requirements: REQ-P08-01, REQ-P08-02, REQ-P08-03, REQ-P08-06, REQ-P08-07, REQ-P08-08. Acceptance: Positive: 32 slots are accepted. Negative: the 33rd slot and an illegal transition are rejected. Boundary: stride arithmetic is checked at 0 and at u32::MAX/4.
- **E07-5 Real-time edge contracts**. Requirements: REQ-P08-04, REQ-P08-05, REQ-P13-03. Acceptance: Positive: typed focus-switch, RTPRIO grant and DMA-BUF stream descriptors round-trip. Negative: malformed descriptors are rejected. Boundary: an RTPRIO value at the source value is accepted, and one above is rejected.

### M08 - Leaf slices dependent on the agent chain: P11 Ludus and P14 Hephaestus

Rank 11. State: blocked. Cost: small. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: no. Needs external contract: no. Blocked by: M06. Unblocks: M12.

Exit criteria:

- aegis-ludus and aegis-hephaestus have manifests, lock entries and positive/negative/boundary tests; the Rust toolchain from M02 is reused
- Assertions are refactored to Result per HISS-07
- The Steamworks proprietary-SDK licensing decision (D12) is recorded before any SDK is vendored
- Interface contracts are typed for P09 to P14 (VERIFY_CODE_CAD, direction per the M01 register), P14 to P15 (geometry viewport descriptor), P11 to P02 (transaction receipt, TPM2 signing stubbed) and P11 to P04 (rich presence), each with a negative test for a malformed payload

Cheapest exit: Extract the bounds and validation logic. No Steam, CAD kernel or solver runtime.

Epics:

- **E08-1 Ludus launch-argument validator**. Requirements: REQ-P11-02, REQ-P11-07, REQ-P11-01, REQ-P11-05. Acceptance: Positive: 64 args are accepted. Negative: 65 args are rejected. Boundary: an empty argument list is handled explicitly and its authentication outcome is recorded.
- **E08-2 Hephaestus bounded loops**. Requirements: REQ-P14-01, REQ-P14-02, REQ-P14-03, REQ-P14-04, REQ-P14-08. Acceptance: Positive: a mesh under the bound is accepted. Negative: a missing STEP path errors. Boundary: 500000 elements are accepted and 500001 rejected, and the iteration bound is honoured. CAD and solver versions are recorded as unpinned.
- **E08-3 P11 and P14 interface contracts**. Requirements: REQ-P14-05, REQ-P14-06, REQ-P11-06, REQ-P11-04. Acceptance: Positive: the descriptors round-trip. Negative: a receipt without a signature field is rejected. Boundary: a viewport descriptor at the mesh bound is accepted.

### M19 - eBPF objects compiled and loaded through the verifier on the host kernel (local fixture)

Rank 12. State: blocked. Cost: medium. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: no. Needs external contract: no. Blocked by: M05, M07. Unblocks: M10.

Exit criteria:

- Toolchain admission: clang (BPF target), bpftool and libbpf (or aya, per the M01 register) are selected through the template matrix with pinned versions before any compile
- action_gate, scx_cake and kepler_power compile warning-free with clang -target bpf
- The objects load through the BPF verifier on the host kernel with CAP_BPF; the host kernel version and config (BPF LSM, sched_ext, BTF) are recorded as observed facts, not assumed
- Negative: removing the ringbuf NULL check or an unroll bound makes the verifier reject the object. Boundary: a struct_ops load with all handlers stubbed succeeds where the host exposes sched_ext
- This is a non-qualifying local fixture: a host or stock kernel cannot close the Nucleus-kernel verification in M10

Cheapest exit: Compile the three objects and run a verifier load on the workstation kernel. No VM, image or Nucleus artifact.

Epics:

- **E19-1 action_gate LSM object**. Requirements: REQ-P06-05, REQ-P07-06. Acceptance: Positive: the program loads, and an exec event reaches a stub ringbuf consumer where BPF LSM is active. Negative: the unchecked-pointer variant is rejected. Boundary: if BPF LSM is not active on the host, attach is recorded as unavailable rather than passed.
- **E19-2 scx_cake struct_ops object**. Requirements: REQ-P07-04, REQ-P07-01. Acceptance: Positive: struct_ops loads where sched_ext is present. Negative: the unbounded-loop variant is rejected. Boundary: all-stub handlers load.
- **E19-3 kepler_power probe object**. Requirements: REQ-P13-02. Acceptance: Positive: the tracepoint program loads. Negative: an out-of-bounds map access variant is rejected. Boundary: the hardcoded TDP literal is recorded as a non-measurement.

### M04 - UI accessibility harness: P12 Concordia tokens with Node/pnpm/Playwright admission

Rank 13. State: blocked. Cost: medium. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: no. Needs external contract: no. Blocked by: M02. Unblocks: M16.

Exit criteria:

- Toolchain admission: Node, pnpm, Playwright browsers and Svelte are selected through the template matrix with pinned versions (D10) before any UI gate runs; package manifests and lockfiles are committed for ui/concordia-tokens
- The Playwright and axe-core suite runs headless in a container with zero violations on the default state; it fails when the focus outline is removed; the focus width and contrast boundary follows D16
- D17 (Bootstrap fork versus no monolithic CSS) is recorded; no `|| true` in the accessibility gate

Cheapest exit: Build the token file plus one Svelte component and run the axe suite in a container. No compositor and no daemons.

Epics:

- **E04-1 UI toolchain admission and lockfiles**. Requirements: REQ-UI-01, REQ-CI-03, REQ-P12-08, REQ-CI-02. Acceptance: Positive: the pinned Node and pnpm versions install from the lockfile. Negative: a lockfile mismatch fails `pnpm install --frozen-lockfile`. Boundary: the gate fails on the first violation and is not suppressed.
- **E04-2 Concordia tokens and axe-core harness**. Requirements: REQ-P12-01, REQ-P12-05, REQ-P12-06, REQ-P05-06, REQ-UI-02, REQ-P12-02, REQ-P12-03, REQ-P12-04, REQ-P12-07, REQ-P12-09. Acceptance: Positive: zero violations on the default state. Negative: a stripped outline fails. Boundary: the focus width chosen by D16 passes, one pixel less fails; 3:1 contrast passes and 2.99:1 fails; 200% text scaling causes no overflow. The truncated container digest is recorded as non-pinnable.

### M16 - P05 Forum shell state and lifecycle with stubbed IPC

Rank 14. State: blocked. Cost: small. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: no. Needs external contract: no. Blocked by: M04, M14. Unblocks: -.

Exit criteria:

- ui/forum-shell has its package manifest and lockfile on the toolchain admitted in M04
- The shell state and typed process lifecycle are unit-tested with compositor, Justitia and Tellus inputs stubbed
- The DecisionRequest consumer is typed against the M14 schema; the P05/P12 token-edge direction (D05) is recorded

Cheapest exit: Unit-test the Svelte store and lifecycle logic with mocked socket and D-Bus connections.

Epics:

- **E16-1 Forum shell state and lifecycle with stubs**. Requirements: REQ-P05-01, REQ-P05-02, REQ-P05-03, REQ-P05-04, REQ-P05-05, REQ-P05-07, REQ-P05-08, REQ-P04-07. Acceptance: Positive: each lifecycle transition in the source order succeeds. Negative: a transition from Deleted is rejected. Boundary: Rate-Limited to Quarantined at the limit is exercised exactly.
- **E16-2 Consumer contracts and token-edge decision**. Requirements: REQ-GRAPH-01, REQ-P13-04, REQ-P06-08. Acceptance: Positive: DecisionRequest and Tellus telemetry payloads parse. Negative: an unknown schema version is rejected. Boundary: a telemetry update with zero watts renders without error.

### M09 - Cross-repository contract pin (stack.md step 2): one local Imago/Nucleus request/result pair

Rank 15. State: blocked. Cost: small. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: no. Needs external contract: yes. Blocked by: M18. Unblocks: M11, M10.

Exit criteria:

- The M18 schemas are proposed to cordanaLLM/imago and cordanaLLM/nucleus; acceptance is recorded only when each producer consumes the payload, not a fixed symbol list
- One request/result pair is executed locally against pinned local imago and nucleus checkouts, with a positive result, a negative result (rejected payload with a correlated error) and a boundary result (empty requirement list rejected explicitly)
- Canonical producer identities are confirmed, or the unresolved GitHub identities (cordanaLLM/imago and cordanaLLM/nucleus did not resolve at revision time) and the non-canonical identities in builder workflows are recorded as the blocker
- Simulated output is not accepted as a result; no hosted dispatch is claimed

Cheapest exit: Run the pair against the pinned local checkouts without any hosted dispatch.

Epics:

- **E09-1 Imago consumption of the product input manifest**. Requirements: REQ-P01-01, REQ-P01-02, REQ-P01-03, REQ-P01-04, REQ-P01-06, REQ-P01-10. Acceptance: Positive: an accepted request returns image digest, signature reference and boot-evidence fields. Negative: a malformed manifest is rejected with a correlated error. Boundary: a retry at the bound is recorded, and one above is refused.
- **E09-2 Nucleus consumption of the kernel requirement payload**. Requirements: REQ-P01-09, REQ-P07-01, REQ-P06-05, REQ-P13-02. Acceptance: Positive: a result returns kernel version/config digest, artifact digest and provenance. Negative: an unsatisfiable feature is rejected with a correlated error. Boundary: an empty requirement list is rejected explicitly.
- **E09-3 Pair evidence and identity status**. Requirements: REQ-BOOT-02, REQ-GOV-02. Acceptance: The retained pair shows correlation id and exact revisions. Simulated output is refused. Identity status is recorded.

### M11 - Minimal image build with artifact, signature and boot evidence (stack.md step 4)

Rank 16. State: blocked. Cost: large. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: yes. Needs external contract: yes. Blocked by: M09, M15. Unblocks: M13, M20, M12.

Exit criteria:

- BLOCKED until: Imago accepts the M09 manifest and returns an image/UKI with digest and signature; Nucleus supplies the pinned kernel artifact; D07 is closed
- Toolchain admission: QEMU, OVMF, swtpm and the signing tools (cosign or sbsign) are selected through the template matrix with pinned versions before any boot or signing gate
- Unblocking evidence: retained image output digest, signature verification log, and a QEMU/OVMF/swtpm boot log on a KVM-capable host with real PCR 0/4/7/11 readback and dm-verity/LUKS2 unlock records
- Scope: the minimal image carries no UI and no kernel-attached eBPF programs (D20); the accessibility gate attaches when the UI enters an image, and eBPF objects enter through M10 and M12
- The A/B sysupdate transfer is exercised once between root-a and root-b, and observed transitions are compared with the M15 state machine

Cheapest exit: No cheaper exit exists: this is the first real artifact. Keep it to one image and one boot, and retain every log.

Epics:

- **E11-1 Imago result consumed and verified**. Requirements: REQ-P01-01, REQ-P01-06, REQ-P01-08. Acceptance: Positive: digest and signature verify locally. Negative: a tampered image digest or bad signature is rejected. Boundary: a producer version exactly at the floor is accepted, and one below is rejected.
- **E11-2 Real boot evidence replaces the simulated script**. Requirements: REQ-BOOT-01, REQ-BOOT-02, REQ-P01-05, REQ-P02-02, REQ-P02-01. Acceptance: Positive: PCR values are read back from swtpm, and the verity root hash matches. Negative: a modified root image fails verity and does not boot to the established state. Boundary: a PCR policy that omits PCR 11 fails to unseal /var. The imported script is not used.
- **E11-3 Analyzer gate without suppression in the image path**. Requirements: REQ-CI-01. Acceptance: Positive: the M03 gate passes on the image inputs. Negative: an ignored-key diagnostic fails the image acceptance. Boundary: no step carries failure suppression.
- **E11-4 A/B transfer exercised**. Requirements: REQ-P01-04, REQ-P02-04, REQ-P02-08. Acceptance: Positive: the second slot is written read-only and boots. Negative: a transfer with a bad signature is discarded. Boundary: watchdog expiry before bless rolls back once.

### M10 - eBPF objects re-verified against the Nucleus-pinned kernel in a VM

Rank 17. State: blocked. Cost: medium. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: yes. Needs external contract: yes. Blocked by: M09, M19. Unblocks: M12.

Exit criteria:

- BLOCKED until: the M09 pair delivers a Nucleus kernel whose version/config digest is recorded
- Hardware: a KVM-capable host is required to boot the Nucleus kernel in a VM (KVM was present on the workstation at revision time; that is not qualifying evidence by itself)
- Toolchain admission: QEMU is selected through the template matrix with a pinned version (shared with M11 if M11 lands first)
- action_gate, scx_cake and kepler_power from M19 load through the verifier on that kernel; sched_ext, BPF LSM and BTF availability are read from its config

Cheapest exit: Boot the Nucleus kernel directly in QEMU/KVM with a minimal initramfs and repeat the M19 loads. The M19 host-kernel fixture does not close this milestone.

Epics:

- **E10-1 action_gate on the Nucleus kernel**. Requirements: REQ-P06-05, REQ-P07-06. Acceptance: Positive: the program loads and attaches. Negative: the unchecked-pointer variant is rejected. Boundary: BPF LSM absent from the kernel config fails the milestone with a recorded reason.
- **E10-2 scx_cake on the Nucleus kernel**. Requirements: REQ-P07-04, REQ-P07-01. Acceptance: Positive: struct_ops attaches. Negative: the unbounded variant is rejected. Boundary: a kernel without sched_ext is recorded as a Nucleus requirement defect.
- **E10-3 kepler_power on the Nucleus kernel**. Requirements: REQ-P13-02. Acceptance: Positive: the tracepoint attaches. Negative: a missing tracepoint is reported, not ignored. Boundary: zero energy delta over an idle interval is recorded as a value, not an error.

### M21 - Workstation hardware slices: RAPL energy counters and KVM/Firecracker sandboxing

Rank 18. State: blocked. Cost: medium. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: yes. Needs external contract: no. Blocked by: M05, M06. Unblocks: -.

Exit criteria:

- Hardware: a host with readable RAPL counters and KVM; the host model and kernel are recorded
- Toolchain admission: Firecracker (and the jailer, if used) is selected through the template matrix with a pinned version before any microVM run
- Unblocking evidence: measured RAPL energy deltas replace the simulated wattage in the M05 engine; a measured microVM boot time and memory footprint replace the scaffold literals; one AF_VSOCK candidate evaluation round-trips
- Scope: these are host-fixture measurements; re-measurement inside an Aegis image is a later acceptance and is not claimed

Cheapest exit: Read powercap counters and boot one Firecracker microVM on the workstation. No Aegis image and no GPU.

Epics:

- **E21-1 RAPL-backed carbon telemetry**. Requirements: REQ-P13-02, REQ-P13-01. Acceptance: Positive: SCI is computed from measured energy. Negative: unreadable counters fail closed with an error, not a default value. Boundary: counter wraparound between two samples yields a correct positive delta.
- **E21-2 Firecracker and AF_VSOCK sandboxing**. Requirements: REQ-P10-01, REQ-P10-03, REQ-P16-04. Acceptance: Positive: a microVM boots and the candidate evaluation round-trips over AF_VSOCK. Negative: a microVM request above the memory limit is refused. Boundary: the 64th microVM on real KVM is accepted and the 65th refused.
- **E21-3 Venus GPU pricing deferred**. Requirements: REQ-P10-02. Acceptance: Recorded as deferred to M12. It is not claimed here.

### M13 - Release signing and remote delivery (stack.md step 5)

Rank 19. State: blocked. Cost: medium. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: no. Needs external contract: yes. Blocked by: M11. Unblocks: -.

Exit criteria:

- BLOCKED until: an M11 artifact exists, hosted ruleset/label readback for origin is retained (D11), publication settings and at least one consumer are recorded, and the hosted acceptance checks (sign-off and linear history on historical commits, recorded in the private readiness matrix) are resolved or explicitly waived
- Toolchain admission: the release signing tool and the release workflow's actions are selected and pinned; the unverified toolchain action reference is replaced
- Unblocking evidence: a verified release workflow run, a signature and provenance for the real M11 artifact, and remote ruleset/label readback

Cheapest exit: Record hosted ruleset/label readback for the existing origin first, and keep the release workflow inactive until an artifact exists.

Epics:

- **E13-1 Hosted ruleset/label and identity readback**. Requirements: REQ-GOV-02. Acceptance: Positive: rulesets and labels read back from the hosted side match .github/rulesets and .config/labels.yaml. Negative: a missing required check is reported as drift. Boundary: the proposal identity case difference is resolved or recorded.
- **E13-2 Signing and provenance for a real artifact**. Requirements: REQ-REL-01, REQ-REL-02. Acceptance: Positive: the signature verifies against the M11 artifact. Negative: verification fails against a modified artifact, and an unverified action reference fails workflow lint. Boundary: signing an absent artifact fails rather than producing an empty signature.
- **E13-3 Publication settings and consumers**. Requirements: REQ-P01-06. Acceptance: Positive: one recorded consumer enables delivery. Negative: delivery is refused with zero recorded consumers. Boundary: a consumer without a pinned version is not counted.

### M20 - TPM2 attestation slice on swtpm: audit-record signing and /var unseal

Rank 20. State: blocked. Cost: medium. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: yes. Needs external contract: yes. Blocked by: M11, M14. Unblocks: -.

Exit criteria:

- BLOCKED until: an M11 image boots under QEMU with swtpm
- Hardware: a KVM-capable host with swtpm; a physical TPM2 is optional and recorded separately if used
- Toolchain admission: tpm2-tools (or the chosen TSS library) is selected through the template matrix with a pinned version
- Unblocking evidence: a real PCR quote from swtpm signs one M14 audit record, and /var unseals only under the enrolled PCR policy

Cheapest exit: Use the M11 VM with swtpm. No physical TPM is required for this milestone.

Epics:

- **E20-1 PCR quote signs an audit record**. Requirements: REQ-P06-06, REQ-P06-01, REQ-GOV-03. Acceptance: Positive: an audit record signed with the quote verifies. Negative: a record signed under a different PCR state fails verification. Boundary: a quote over exactly PCR 0/4/7/11 is accepted, and a quote missing PCR 11 is rejected.
- **E20-2 PCR-sealed /var unseal**. Requirements: REQ-P02-02, REQ-P02-03. Acceptance: Positive: /var unseals under the enrolled policy. Negative: a wrong PCR policy fails to unseal. Boundary: changing only PCR 7 is enough to prevent unseal.

### M12 - GPU-backed slices: DMA-BUF, VFIO and P2PDMA paths

Rank 21. State: blocked. Cost: large. Owner repository: cordanaLLM/Aegis-OS. Needs hardware: yes. Needs external contract: yes. Blocked by: M08, M10, M11, M17. Unblocks: -.

Exit criteria:

- BLOCKED until: a booted M11 image exists, the M10 kernel exposes VFIO/IOMMU, and a DMA-BUF-capable GPU with IOMMU groups is available on a recorded host
- Toolchain admission: the GPU driver stack and, if selected, the template-native-gpu boundary are pinned through the template matrix with an ABI mapping and one backend fixture, or recorded as not needed
- Unblocking evidence: per-path logs with measured values (DMA transfer, preemption and VRAM residency) replacing every hardcoded literal in the scaffolds

Cheapest exit: Order the paths by hardware availability: DMA-BUF sharing first, then VFIO BAR mapping, then P2PDMA.

Epics:

- **E12-1 DMA-BUF sharing paths**. Requirements: REQ-P08-01, REQ-P11-03, REQ-P15-02, REQ-P15-03, REQ-P08-04. Acceptance: Positive: one measured zero-copy transfer per path. Negative: an invalid DMA-BUF fd is rejected without a CPU copy fallback being counted as success. Boundary: the 64th buffer is accepted and the 65th refused on real hardware.
- **E12-2 VFIO and P2PDMA paths**. Requirements: REQ-P03-03, REQ-P03-06, REQ-P03-07, REQ-P09-04. Acceptance: Positive: one measured NVMe-to-GPU transfer. Negative: a device outside its IOMMU group is refused. Boundary: a transfer of exactly 8192 blocks succeeds, and 8193 is refused before dispatch.
- **E12-3 GPU template boundary**. Requirements: REQ-P10-02, REQ-P03-02. Acceptance: Positive: one backend fixture result is retained, or a not-needed decision is recorded. Negative: a Go library as a direct Rust dependency is rejected. Boundary: an ABI mapping with zero functions is not accepted as a boundary.

## Risks and open decisions

Deviation from the docs/integration/stack.md activation order:

stack.md orders the steps as: (1) inventory; (2) pin the image/kernel producer schemas and test one request/result pair locally; (3) select one component; (4) build one minimal image; (5) enable remote delivery. This roadmap keeps a different order, and says so explicitly:

- The first component, P06 (M02, stack.md step 3), ranks before any schema work.
- The Aegis-owned half of step 2 (M18: product input manifest and kernel requirement schemas) ranks directly after M02 and the P01/P02 definitions (M03), at rank 5.
- The producer half of step 2 (M09: one request/result pair against pinned local imago/nucleus checkouts) is the first cross-repository milestone. It ranks after all purely local milestones.

Reasons for the different order:

- Both builder identities, cordanaLLM/imago and cordanaLLM/nucleus, did not resolve on GitHub at revision time.
- Builder workflows still reference non-canonical identities (private readiness matrix).
- Nucleus validates a fixed symbol list instead of consuming a requirement payload.
- Putting the producer pin first would stall every later milestone on repositories that Aegis cannot change.
- The roadmap method's rule 1 orders unverified cross-repository work after local work.

Mitigation: M09 is not delayed by its rank. It becomes ready once M18 is done. Open decision D19 recommends that stack.md's activation order defer to this roadmap through a reviewed change.

Risks, recorded from the cards and the private readiness matrix:

- Cross-repository contracts are declared but unverified; the configured producer origins do not resolve, and Aegis cannot close M09 alone.
- Imported workflows suppress failures (REQ-CI-01, REQ-CI-02), and the imported integration script prints boot/TPM2 success without executing anything (REQ-BOOT-02). They stay inactive, and any activated gate must be rewritten.
- The imported partition and transfer definitions do not pass the host systemd: systemd 261 ignores two repart keys and rejects the transfer definition. M03 must rewrite them, and its gate must treat ignored-key diagnostics as failures because systemd-repart exits 0 on them.
- Unprivileged sysupdate validation works only against a scratch root tree. Validation against a loop-mounted image needs loop-device privileges.
- Kernel identity is unresolved (REQ-P01-09 versus Nucleus ownership), and the "Linux 6.12+/7.3" phrasing in the sources is ambiguous. A host or stock kernel is only a non-qualifying fixture (M19).
- Three hash schemes are named for one ledger (REQ-P06-09, REQ-P16-09, REQ-P16-03), and the reference daemon uses MD5.
- The subsystem graph and the two architecture documents disagree on five edges (REQ-GRAPH-01 to REQ-GRAPH-05). Two source-internal value conflicts exist: focus width (D16) and CSS framework (D17).
- The Vesta design violates the language boundary (REQ-P10-08), and Ludus depends on a proprietary SDK (REQ-P11-01).
- Scaffold code is simulated: hardcoded fds, boot times, wattage and proof hashes are not runtime evidence.
- Imported dependency versions (aya, memmap2, zenoh, Node 20) are proposal data. Versions are selected at activation after checking current upstream.
- These upstream dependencies are unpinned: wlroots, Mesa, PGlite, Steamworks bindings, the CAD and solver stack, Firecracker, the Wasm engine, and the truncated validation-container digest (REQ-P12-04). The image inputs use Release=latest (D18).
- Proposal narrative and benchmark percentages are untrusted; they are cited only where a requirement text records them as claims.
- One source records an external deadline: PLD effective 9 December 2026 (REQ-P06-10).
- Hosted acceptance checks (sign-off and linear history) fail on historical commits, per the private readiness matrix. Rewriting history is out of scope.
- The M00 governance pass was reported by this session's preparation run and was not re-run during this revision. M01 re-runs it at the committed tip as its entry condition.

Open decisions:

- **D01** Which component is promoted first end-to-end? Options: P06 aegis-justitia decision engine; P01/P02 declarative definitions with a Rust parser; P13 aegis-tellus SCI arithmetic; P03 aegis-vulcan or P15 aegis-hestia (trivial slices). Recommended: P06 aegis-justitia (M02). Why: Both P06 and P01/P02 cost small and need no hardware or external contract for their first slice. Neither is a committed crate: the repository has no Cargo.toml, crates/aegis-justitia holds no files, and P06 is listed only in the proposed (export-006) workspace, so M02 must create the workspace root. P01/P02 needs no new toolchain for the analyzers (systemd is present) and sits on the critical path to the first image. However, its interface contract is the Imago/Nucleus boundary, which cannot be accepted while both builder identities are unresolved. P06's contract is wholly Aegis-owned, it has a drafted positive/negative/boundary test module (REQ-P06-03, REQ-P06-04), and it is the mandatory gate for every agent-facing path (REQ-GOV-03), so its downstream milestones (M14, M05-M08, M17, M19) are all workstation-reachable. P01/P02 follows immediately as M03. P13 unblocks only P16. P03/P15 are trivial but have low unblocking value.
- **D02** Which hash algorithm and signing scheme does the shared audit/checkpoint ledger use? Options: SHA-256 chain with TPM2 signature (schema in export-015, reference ledger export-040); BLAKE3 chain (export-002, export-025, export-062 naming); Keep the reference daemon's MD5 (export-031). Recommended: One algorithm behind a trait; SHA-256 unless a recorded decision selects BLAKE3; MD5 excluded. Why: Three schemes are named across sources (REQ-P06-09, REQ-P16-09, REQ-P16-01, REQ-P16-03). The only implementation-level agreement is SHA-256, and MD5 contradicts the subsystem's own schema. Record the choice before M02 closes so that M05 reuses it.
- **D03** Which direction and transport does the Justitia/Minerva action gate use? Options: export-062: P06 -> P09 over eBPF action_gate / D-Bus; export-002: P09 -> P06 (Minerva proposes, Justitia intercepts); export-003: Justitia intercepts Minerva in the master diagram. Recommended: Keep export-062 edge ids; adopt propose/intercept call semantics for the contract; record in M14. Why: REQ-P09-03, REQ-P09-05 and REQ-GRAPH-04 disagree on who calls whom. The M14 action proposal contract cannot be typed until this is written down. It no longer blocks M02.
- **D04** Does Justitia also gate Vesta sandbox execution (P06 -> P10 SYSCALL_INTERCEPT)? Options: Yes, add the edge to the graph of record; No, Vesta is gated transitively through P09. Recommended: Record as unresolved in M14; contract-test both paths in M06. Why: The edge exists only in export-002 (REQ-GRAPH-02). The subsystem graph omits it (REQ-P10-07).
- **D05** Which direction is the P05/P12 design-token edge? Options: P12 produces tokens consumed by P05 (export-002); P05 -> P12 as recorded in export-062 (CONSUMES_DESIGN_TOKENS). Recommended: P12 is the producer; keep export-062's edge id and annotate direction (M16). Why: REQ-GRAPH-01 conflicts with the export-062 edge. Both describe P05 consuming tokens; only the arrow differs.
- **D06** Which Wasm capsule runtime does Vesta use given the language-boundary rule? Options: Rust-native runtime; Out-of-process Wazero behind a protocol adapter with contract tests; Direct Wazero dependency (violates stack.md). Recommended: Rust-native runtime, or a protocol adapter with contract tests (M06). Why: REQ-P10-08 names a Go runtime, and docs/integration/stack.md forbids a Go library as a direct dependency of a Rust daemon.
- **D07** Which kernel boots the image: the mkosi linux-rt package or a Nucleus-produced artifact? Options: Nucleus artifact consumed through the M09 contract; Distribution linux-rt package pinned in mkosi.conf; Both, with Nucleus overriding when present. Recommended: Nucleus artifact through M09; the host or stock kernel only as a non-qualifying local fixture (M19); record in M18. Why: REQ-P01-09 pins linux-rt, while stack.md assigns kernel production to Nucleus.
- **D08** Is the compositor a C wlroots implementation or the pure-Rust scaffold? Options: C wlroots core with a Rust control daemon (export-013 mandate); Pure Rust scaffold (export-027 as written); Rust control daemon now, wlroots FFI decided at M12. Recommended: Decide in M07; keep the registry/IPC state machine backend-agnostic. Why: REQ-P04-01 mandates C wlroots. The only code artifact has no wlroots FFI, and no wlroots version is pinned anywhere.
- **D09** Where does P15 Hestia live: crates/aegis-hestia, ui/hestia-app, or both? Options: Rust crate only; Svelte UI package only; Both, with a typed boundary. Recommended: Both, with the boundary recorded in the M01 inventory and applied in M17. Why: export-030 places a Rust daemon under crates/, while export-007 lists only ui/hestia-app. The proposed workspace lists neither (REQ-WS-01).
- **D10** Which UI toolchain boundary is pinned? Options: Node 22 LTS + pnpm per the developer guide; Node 20 per the imported CI; sveltesentio as the shared UI framework candidate. Recommended: Node LTS + pnpm selected through the template matrix after checking current upstream support; sveltesentio evaluated as a candidate only (M04). Why: REQ-UI-01 and REQ-CI-03 conflict. The private readiness matrix lists sveltesentio as an optional candidate, not a dependency.
- **D11** When are hosted rulesets, labels and publication settings for the existing origin verified by readback? Options: As an M01 follow-up, without enabling release delivery; At M13 together with publication settings; Only after the hosted sign-off and linear-history checks are resolved. Recommended: Retain hosted readback early; keep release delivery (M13) blocked. Why: origin https://github.com/cordanaLLM/Aegis-OS.git resolves with main at 42a23c8. The private readiness matrix records hosted sign-off and linear-history failures on historical commits, and no publication settings or consumers exist yet.
- **D12** How does P11 reconcile a proprietary Steamworks SDK with the stated FOSS-compliance constraint? Options: Optional feature behind a build flag with recorded licence decision; Exclude Steamworks integration from the image; Proceed as drafted. Recommended: Optional feature with a recorded licensing decision before any SDK is vendored (M08). Why: REQ-P11-01 depends on a closed SDK, and the export-004 matrix labels P11's constraint as FOSS compliance.
- **D13** Reversible structural consolidation versus permanent hardening for P02? Options: Reversible consolidation (chosen in export-004); Permanent hardening. Recommended: Reversible consolidation, reconciled with the dm-verity requirement in M15. Why: REQ-P02-05 and REQ-P02-01 pull in different directions, and the interaction is unspecified.
- **D14** Does mkosi run in Aegis (for mkosi summary validation) or only inside Imago? Options: Admit mkosi through the Aegis template matrix at the v24+ floor; Validate mkosi configuration only through the Imago result. Recommended: Admit mkosi in Aegis only if M18 needs local validation; otherwise rely on the M09 Imago result. Why: mkosi was not installed on the workstation at revision time, and stack.md assigns image construction to Imago. The earlier draft required `mkosi summary` without an admission step.
- **D15** Where do the P01/P02 definition parser and the P02 A/B lifecycle crate live? Options: New crates under crates/ with an updated crates/README.md; A tools/ or build/ subpackage outside the daemon crates; Inside a future Imago-facing adapter package. Recommended: Decide in M01; M03 and M15 cannot create manifests until the path is recorded. Why: crates/README.md reserves paths for the P03-P16 daemons only. Neither P01 nor P02 has a proposed crate in the inventory.
- **D16** Which focus-indicator width is the accessibility boundary: 2px or 3px? Options: 2px stroke (export-023 test, export-020 report CSS); 3px ring width (export-042 tokens); 3px token with a 2px minimum test threshold. Recommended: 3px token with a 2px minimum threshold tested at the boundary, recorded before M04 closes. Why: REQ-P05-06 and REQ-UI-02 state 2px, and REQ-P12-05 sets 3px. The earlier draft pinned 2px without recording the conflict.
- **D17** Does Concordia use a Bootstrap fork or discard monolithic CSS libraries? Options: Bootstrap fork with better accessibility (export-004 prose); Design tokens without a monolithic CSS library (export-004 matrix). Recommended: Design tokens without a monolithic CSS library; record the prose statement as superseded (M04). Why: REQ-P12-07 and REQ-P12-09 come from the same source and conflict.
- **D18** Are the mkosi distribution release and package set pinned? Options: Pin a distribution snapshot and package versions in the product input manifest; Keep Release=latest and rely on Imago for reproducibility. Recommended: Pin in the M18 product input manifest. Why: REQ-P01-01 shows Release=latest, and REQ-P01-09 lists packages without versions, so the image inputs are not reproducible.
- **D19** Should docs/integration/stack.md's activation order defer to this roadmap? Options: Keep stack.md order: schema pin (step 2) before first component (step 3); Defer to the roadmap: Aegis-side schema authoring (M18) after the first component (M02) and P01/P02 definitions (M03); the cross-repository pair (M09) after all local milestones. Recommended: Defer to the roadmap, and update stack.md in a reviewed change that states the reason. Why: Both builder identities (cordanaLLM/imago, cordanaLLM/nucleus) did not resolve on GitHub at revision time. Pinning a producer schema first would leave every later milestone waiting on repositories that Aegis cannot change. The Aegis-owned half of step 2 (M18) is local and ranks directly after M03. The producer half (M09) becomes ready as soon as M18 is done and can run in parallel; it ranks after the local milestones only because rule 1 orders unverified cross-repository work last.
- **D20** Does the minimal image (M11) include the UI and kernel-attached eBPF programs? Options: No: one image, one boot, no UI, no eBPF objects; Yes: include the accessibility gate and eBPF objects. Recommended: No; the accessibility gate attaches when the UI enters an image, and eBPF objects enter through M10 and M12. Why: The earlier draft blocked M11 on the full UI milestone and on eBPF verification. That put the Node/Playwright admission on the first-artifact path, justified only by an imported CI gate that stays inactive.

## Evidence

Bundle sha256: 8186bf0336e16764216396a147c536d96b3933901f5b2b81a4d0d3b74ffa25c6. Sources cited by requirements (id, sha256):

- export-001 f32a74743af5ab85c0682a5384cf01b71b0e9e2878d8f8ce09a6d592211e4ea5
- export-002 7e0c95f4ea0570ea620952a4f69d45580a73956643eda3353b3f2ca273405a91
- export-003 13af15ffc31684e94023ae9aa84339b80b4dd6025332d9fefca743e83400500c
- export-004 15831276a058d0779bbc0df9d13d865ce7edffaee43ab6a0a0b39dbb0f5c3029
- export-006 6e694e01e13615bf977e9bb849b8a6a033d8aa6d3d0558d5522af4c440b9d097
- export-007 84f43472c5369d00a40ba9ded70915b4e4e18d28e4fa383f8ac73b71ed611ce8
- export-009 3068c85b768a2ab713883e549b7f1a82ebc4f7d2935b5c9efbe07642f0f813ed
- export-010 1748925bf81e5a610586d34bef30b62345de024a7271390be9b13d41efa079b5
- export-011 528da609dab7ad5931400eb1631cfb0b106da5f1310842f0025f73317069b433
- export-012 9b502c76509bcf22b1fa4ae2e93b7347589259d99254b217561782d7ac9e9384
- export-013 7f4c22813332ffd81f92f0b82a30553f5da3c1e7494088fec6861dc7aa0df3cf
- export-014 659691a2d7d04dc67783f2b3a01bbe6ae818f62b271718fcea6322e61d6b0e8a
- export-015 fcbe2caed363770ae2b9d36c583c1eeb1d0ffddc880a6c61b4b18f6200988cdc
- export-016 cfa58b5b23b8a0f75651be6fc0a68b410aefd66111b98b08d0f3c1fd3a85ada7
- export-017 afbc0af8056d5022c8c1e494cb549d9400cb78bae41d6140e4209f73bb3ddcdb
- export-018 5dbc6d071bbba9577c212c110a93298336ca0bab1b17a203e4048db5d32e59a9
- export-019 b0aa6e54ed57d638ba5dda2c3f8d58bf9517a6498ec4a5f411922c713cf8ceff
- export-020 1748f49bb7f8dd849fd0b3ce913b9d1e9a0c2e3bc2789c3ecec740f8a68f9f06
- export-021 6a3cda152e6c22d3333b40c369001617e4706341da244d8a19debc6607a490d3
- export-022 46cea660df6344df1d3cb033871e0f0e1a1a29c0e048bec5e11e85c388e3cee5
- export-023 a0d06b6b8e6c8a0ac76f32377d8c39293ad2f191771755694b70eac53abe31f4
- export-024 50a41ecd0b744f217c394e51e7e999ba596065701de92aea414e87bb3965c3e1
- export-025 6b23723ddb768ce4882c4f398cd1ee5409adf8851d5da8b4016434c5ba0ca3e8
- export-026 c2f1e433cd32b7a68245f99e648c661c31a407b03b22c6574b7d082980dc4b25
- export-027 f19640d7a7dad8d1aa587a164450ec17bae9e74f731761f0100f045a8922831f
- export-028 d74a93eac654d1e34b2e3cd7f35f2600f0f94dc38003aa7bd0cd81ea6e64e77d
- export-029 42799618652005cddcd75fd26067a16d8c4fef16cef5ef6041ce1ed9d0435d2f
- export-030 689d175667d602859de14157948337dddfc913bdfe645d51f501342e1e026bf4
- export-031 5ff68332814874c6c1e59a905c2041758ce24eafca50491beb288d561a7267db
- export-033 531cbdf98eb578897c9834848bc303cd7770a3febc9660cca0f50520c20b3c15
- export-034 213a95fc0d02401f89c198abac4ca5b80f34f9dbfce904dc9de1d5aae3cbec2a
- export-035 4c2146ffed32d1f2aafe53d3f457e2fb8d52cf268ef15625cf62eebe0e161ebe
- export-036 25813d24073335631e7c55c750de3d394787363f36c118f0c515e1556835b7dd
- export-037 ce490c88081f31b9efdb3dc6f4b3671269e877f4ef4b7e3ce590c7937d576d1c
- export-038 fe2cb01cfd13f13becd0aa0b996ae4838a052c60ff9d79b7c3e5e34529ece747
- export-040 84b1ca13cc33524ce5d508cdc99d8067e46c1ae7f7b110ecc69c348102011a20
- export-041 e05ddc9466fcfbf4615d6a466f03f0c64ae95745de77c613309cd628d8fb4513
- export-042 411fb8c2d731b6e6850d5bd911dad65ad46d12d1e021b35703ebaa6aa7790f70
- export-043 e3dbfa226e54fd6a6896e7a2e5371dd8b5d98a1546a99c0e823a8360905babd9
- export-046 9d566b66ab939bf0bbf745d3772d2bad08974010fe9df8f1c3abfbc6b247436d
- export-047 971948673346948f0e85dba99470667cb09399fe25dd804c84f5f8a83e46b26d
- export-048 293357bac7d3d2bbe238159a2b8a79a13693da8c91345c554fa3d8cd97ec7fa6
- export-049 4d9043f4af305e2e0866587587d37b3f2195cc382a66fde2fc0e3ef7782a2144
- export-052 78920f9334214134e704b47256b657c332f485ff096412caf2e336c50204ab95
- export-053 d85a1f6ceb0ed37ac9e25e6c576a58f034efbdce8bad3832336b0c6335e2deac
- export-054 41d6d121b142e68d3daec8d2708962631a644bd1c31f268db6c59f351a43c8fa
- export-055 c915f281532f2c48acdf1ff6d5e0343b6e5b3b95af6a9fae5a8b77edf2795f88
- export-056 39af243568ad90f2a9f26d25522143ab7159c6e00f1fad619b864b791ffdb2d2
- export-062 1ce919ed54bbeb6383a9cdf8170f1643d6282f398de762acc1d0fc4c7aff2853
- export-063 b284bb77c19f33a6674357a28f26f183984ee59707bd8c9dfd572da4dbf768a2

The verbatim quotes are held in the private generation result (`notebook-result.json`). This public document reproduces none of them.