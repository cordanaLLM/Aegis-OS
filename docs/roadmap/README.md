# Aegis OS activation roadmap

Status: reviewed draft; decisions D01-D20 recorded by the maintainer on
2026-09-13

## Outcome and scope

Aegis OS is an image-based Linux OS with sixteen subsystems (P01-P16):
Rust host daemons, kernel-space eBPF programs,
mkosi/systemd-repart/systemd-sysupdate image definitions, and Svelte UI
packages. The concept is preserved unchanged. This roadmap orders repository
preparation and component activation so that each step yields verifiable
evidence at the lowest cost.

Outcome: a ranked, dependency-explicit path from the current state to a first
real image with retained artifact, signature and boot evidence, then to
hardware-backed slices and release. The current state is: governance gate green,
licence scaffold committed, and remote declared and resolving. Every milestone
states what unblocks it. No milestone claims build, boot, hardware or release
evidence before the real inputs and checks exist.

Scope inclusions: component inventory reconciliation; one component promoted
end-to-end; workstation-testable slices for the remaining components; the
Aegis-side and producer-side halves of the Imago/Nucleus contract; eBPF
verification on a local fixture and on the Nucleus kernel; a minimal image;
hardware-backed slices; release and remote delivery.

Scope exclusions (from the sources and governance): architectural redesign of
the sixteen subsystems; activation of imported CI, release or integration
workflows (they suppress failures and print simulated success, see REQ-CI-01,
REQ-CI-02 and REQ-BOOT-02); any claim derived from proposal narrative or
benchmark figures; imported dependency versions treated as active pins; owners
or dates beyond repository names and dates present in the sources.

Current verified state at revision time:

- LICENSE, LICENSES/, LICENSING.md and REUSE.toml are committed (a2c9626).
- The component inventory enrichment and a governance-only CI gate are committed
  (42a23c8).
- `origin` resolves to <https://github.com/cordanaLLM/Aegis-OS.git>, read back
  with `git ls-remote`. The hosted ruleset `praetor-main-protection` is active
  on `main` and reads back as deletion and non-fast-forward protection, linear
  history, pull requests, and the required `Preparation gate` status check.
- Every milestone and epic is mirrored as a GitHub milestone and a `roadmap`
  issue; `planning/roadmap.json` stays the source of truth.
- The private readiness matrix records the Imago/Nucleus edges as declared but
  unverified, and the configured producer origins as not resolving.

## Method

Dependency DAG built from the sixteen activation cards in
planning/components.json: subsystem edges from the subsystem graph (export-062),
external producers (Imago, Nucleus, Golusoris, upstream), toolchain admissions
and hardware needs. Milestones are bounded, verifiable states. Each is costed
(trivial=1, small=2, medium=4, large=8), scored by the number of milestones it
transitively unblocks, and flagged for hardware or unverified cross-repository
contracts. Ranking takes the available milestone (all blockers ranked) with the
highest (1 + transitive unblocks) / cost, under hard rules: (1) done or purely
local work ranks first, and from 2026-09-13 (D68) hardware
work whose capability is present and measured on the recorded reference profile
in planning/hardware-profile.json ranks with it, while work needing an absent
capability or an unverified cross-repository contract still ranks last; (2) the
first cross-repository milestone is stack.md
step 2 (M09: pin Imago/Nucleus schemas and test one request/result pair
locally), preceded by the Aegis-owned schema authoring (M18); (3) exactly one
first component (P06, M02) is promoted end-to-end before any second component;
(4) build, boot and release milestones are blocked with the evidence that
unblocks them; (5) state=ready iff every blocked_by milestone is done. Ties
break toward the milestone with more transitive unblocks, then toward the
critical path to the first artifact (M11), then toward milestones without an
unverified external contract in their own flags or ancestry, then toward
milestones without hardware. blocked_by lists direct blockers only; unblocks is
its exact inverse.

Cost scale:

- trivial: hours, no new toolchain.
- small: one crate, one schema set or one configuration set with tests, on an
  admitted toolchain.
- medium: several crates, a new toolchain admission, or a VM-based verification.
- large: a real artifact or real device evidence.

Unblocking value counts the milestones that become reachable once a milestone is
done. The subsystem graph (export-062) is the graph of record; the two
architecture documents (export-002, export-003) are annotated disputes.

Toolchain admission rule: every milestone that runs a new tool names that tool
in its exit criteria and selects it through the template matrix with a pinned
version before any gate runs. The admissions are:

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

First-component justification (rule 3): P06 aegis-justitia is promoted first
(D01). The comparison with P01/P02 is recorded honestly:

- P01/P02 admits no new analyzer toolchain and sits on the critical path to the
  first image.
- P01/P02's interface contract is the Imago/Nucleus boundary, which cannot be
  accepted while both builder identities are unresolved.
- P06's contract is wholly Aegis-owned, its drafted test module already covers
  the positive, negative and boundary rules, and every agent-facing path depends
  on it.
- P06 is not a committed crate: M02 must first create the workspace root,
  because the only workspace listing is the proposal export-006.
- M02 is trimmed to the decision engine. The consumer contracts and unit
  contract move to M14, which also closes D03's direction half. D03's
  transport half stays open as DSP-03 against decision D26.

## Ranked milestones

The order is computed, not hand-assigned. tools/rank_roadmap.py ranks every
milestone by tier and then by unblocking value per unit cost, and make
verify-all fails when the committed order drifts from it. Tiers, cheapest
resistance first: local work, which since D68 includes hardware whose capability
is measured on the reference profile; hardware that is not measured there; work
needing a capability the profile lacks; work needing an unverified
cross-repository contract; release. A milestone may sit away from its computed
position only by recording rank_override with a reason, which the gate then
accepts and a reader can audit. Run tools/rank_roadmap.py --explain to see each
score.

| Rank | ID | Milestone | State | Cost | HW | Ext. contract | Reference profile | Blocked by | Unblocks |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | M00 | Governance gate, split licence and remote declaration | done | trivial | no | no | not-hardware | - | M01 |
| 1 | M01 | Component inventory reconciliation and decision register | done | small | no | no | not-hardware | M00 | M02 |
| 2 | M02 | First component promoted end-to-end: P06 aegis-justitia decision engine | done | small | no | no | not-hardware | M01 | M14, M03, M17, M07, M04 |
| 3 | M14 | P06 consumer interface contracts and hardened unit contract | done | small | no | no | not-hardware | M02 | M05, M06, M16, M20 |
| 4 | M03 | P01/P02 definitions validated offline with host systemd | done | small | no | no | not-hardware | M02 | M18, M15 |
| 5 | M18 | Aegis-side product input manifest and kernel requirement schemas (local) | done | small | no | no | not-hardware | M03 | M09, M26 |
| 6 | M15 | P02 A/B candidate lifecycle state machine | done | small | no | no | not-hardware | M03 | M24 |
| 7 | M26 | Aegis-built kernel with the pinned configuration | done | medium | no | no | full | M18 | M23 |
| 8 | M17 | Trivial leaf slices: P03 Vulcan and P15 Hestia validation crates | done | trivial | no | no | not-hardware | M02 | M25 |
| 9 | M05 | Evolution loop logic: P13 Tellus SCI and P16 Athena lifecycle | ready | small | no | no | not-hardware | M14 | M19, M21 |
| 10 | M06 | Agent execution chain logic: P09 Minerva and P10 Vesta | ready | small | no | no | not-hardware | M14 | M08, M22 |
| 11 | M08 | Leaf slices dependent on the agent chain: P11 Ludus and P14 Hephaestus | blocked | small | no | no | not-hardware | M06 | M25 |
| 12 | M07 | Real-time control plane: P04, P07 and P08 logic | ready | medium | no | no | not-hardware | M02 | M19, M23 |
| 13 | M19 | eBPF objects loaded through the verifier on the host kernel | blocked | medium | no | no | full | M05, M07 | M10 |
| 14 | M23 | P07 and P08 latency fixtures on a realtime kernel guest | blocked | medium | yes | no | full | M07, M26 | M10 |
| 15 | M24 | Local boot harness over an externally supplied artifact | ready | large | yes | no | full | M15 | M11 |
| 16 | M04 | UI accessibility harness: P12 Concordia tokens | ready | medium | no | no | not-hardware | M02 | M16 |
| 17 | M16 | P05 Forum shell state and lifecycle with stubbed IPC | blocked | small | no | no | not-hardware | M04, M14 | - |
| 18 | M21 | Workstation hardware slices: RAPL counters and KVM sandboxing | blocked | small | yes | no | full (privileged read) | M05 | - |
| 19 | M25 | GPU DMA-BUF sharing and VFIO passthrough slices | blocked | medium | yes | no | full | M08, M17 | M12 |
| 20 | M22 | P10 microVM sandbox measurements on KVM | blocked | medium | yes | no | full | M06 | - |
| 21 | M09 | Cross-repository contract pin: one local request/result pair | ready | small | no | yes | partial | M18 | M11, M10 |
| 22 | M11 | Minimal image build with artifact, signature and boot evidence | blocked | medium | yes | yes | partial | M09, M24 | M13, M20, M12 |
| 23 | M10 | eBPF objects re-verified against the Nucleus-pinned kernel in a VM | blocked | medium | yes | yes | partial | M09, M19, M23 | M12 |
| 24 | M20 | TPM2 attestation slice on swtpm: audit-record signing and /var unseal | blocked | medium | yes | yes | full | M11, M14 | - |
| 25 | M12 | GPU-backed slices: DMA-BUF, VFIO and P2PDMA paths | blocked | large | yes | yes | partial | M25, M10, M11 | - |
| 26 | M13 | Release signing and remote delivery (stack.md step 5) | blocked | medium | no | yes | partial | M11 | - |

The reference profile column records what the machine in
`planning/hardware-profile.json` can evidence for that milestone. A pass there
is development evidence only; see `docs/roadmap/hardware.md`.

## Milestone exit criteria and epics

### M00 - Governance gate, split licence and remote declaration

Rank 0. State: done. Cost: trivial. Owner repository: cordanaLLM/Aegis-OS. Needs
hardware: no. Needs external contract: no. Reference profile: not-hardware.
Blocked by: nothing. Unblocks: M01.

Exit criteria:

- `make verify-all` passes and reports preparation/governance scope only
  (reported by this session's preparation run and recorded in the private
  verification log; this revision did not re-run it because the gate may write
  local state)
- LICENSE, LICENSES/, LICENSING.md and REUSE.toml are tracked; REUSE.toml
  declares EUPL-1.2 for technical material and CC-BY-SA-4.0 for prose (commit
  a2c9626)
- Remote `origin` <https://github.com/cordanaLLM/Aegis-OS.git> is configured;
  `git ls-remote --heads origin` (read-only) shows main at the committed
  checkpoint 42a23c8; hosted ruleset/label readback, publication settings and
  hosted acceptance are not claimed and belong to M13
- The done state rests on committed tree 42a23c8 (which also commits the
  enriched planning/components.json); uncommitted working-tree edits to
  CHANGELOG.md and README.md and the untracked CONTEXT.md observed at revision
  time are outside these criteria

Cheapest exit: Read-only readback: `git ls-files LICENSE LICENSING.md
REUSE.toml`, `git remote -v`, `git ls-remote --heads origin`.

Epics:

- **E00-1 Governance gate reported with its limits**. Requirements: REQ-BOOT-02.
  Acceptance: Positive: make verify-all passes. Negative: a drifted generated
  client file fails `compile-context --verify`. Boundary: the report states
  preparation/governance scope only; simulated boot output is never cited as
  evidence.
- **E00-2 Split licence scaffold committed**. Requirements: REQ-GOV-01.
  Acceptance: LICENSING.md and REUSE.toml record EUPL-1.2 for technical material
  and CC-BY-SA-4.0 for prose. The proposal Cargo workspace (export-006) also
  declares EUPL-1.2; no workspace manifest is committed in the repository yet.
- **E00-3 Remote declared; proposal identity recorded for reconciliation**.
  Requirements: REQ-GOV-02. Acceptance: origin resolves to cordanaLLM/Aegis-OS
  with main present remotely. The proposal identity cordanaLLM/aegis-os
  (export-006) differs in letter case and is carried into the M01 register;
  hosted readback is not claimed.

Evidence:

- commit a2c9626 (licence scaffold)
- commit 4d0d798 (canonical remote declared)
- commit 42a23c8 (component inventory enrichment and governance-only CI gate)
- git ls-remote --heads origin: refs/heads/main 42a23c8
- private preparation verification log: PASS, preparation/governance only
- 34cdaee (roadmap)
- hosted ruleset praetor-main-protection active with required check Preparation
  gate
- Preparation gate CI green on main

### M01 - Component inventory reconciliation and decision register

Rank 1. State: done. Cost: small. Owner repository: cordanaLLM/Aegis-OS. Needs
hardware: no. Needs external contract: no. Reference profile: not-hardware.
Blocked by: M00. Unblocks: M02.

Exit criteria:

- Entry condition: `make verify-all` is re-run at the committed tip (42a23c8 or
  later) and passes
- The inventory records, for each of the 12 Rust candidates and 3 UI candidates,
  its source export id and hash, proposed path, proposal-workspace membership
  (export-006) and missing manifests; it states that no Cargo.toml is committed
  and that crates/*/ directories hold no crate files
- The inventory decides where the P01/P02 definition parser and the P02 A/B
  lifecycle crate live, since crates/README.md reserves paths for P03-P16 only
  (D15)
- Every graph contradiction (P05/P12, P06/P09, P06/P10, P09/P14, P15/P02) and
  every source conflict (ledger hash, focus-ring width, CSS framework, kernel
  identity) is an open decision that names the sources on each side
- The toolchain and version-drift register lists every imported pin as proposal
  data (mkosi v24+, Node 20 vs 22, aya/memmap2/zenoh versions, the unverified
  release action) plus the unpinned upstream projects; imported CI/release
  workflows are marked inactive
- `make verify-all` still passes

Cheapest exit: Write the inventory and decision register as reviewed planning
files. No code and no toolchain installation.

Epics:

- **E01-1 Crate and UI inventory with manifest status**. Requirements:
  REQ-WS-01, REQ-P14-07, REQ-P16-07, REQ-P08-08, REQ-P09-08, REQ-P06-07,
  REQ-P13-08, REQ-P12-08, REQ-P15-05. Acceptance: Positive: 12 Rust and 3 UI
  candidates are listed with source id and hash. Negative: a candidate without a
  source hash fails review. Boundary: Vulcan and Hestia appear with status 'not
  a proposal-workspace member', and the P01/P02 crate location is decided.
- **E01-2 Graph and source contradiction register**. Requirements: REQ-GRAPH-01,
  REQ-GRAPH-02, REQ-GRAPH-03, REQ-GRAPH-04, REQ-GRAPH-05, REQ-P09-03,
  REQ-P09-05, REQ-P10-07, REQ-P15-07, REQ-P14-05, REQ-P12-07, REQ-P12-09,
  REQ-UI-02. Acceptance: Each disputed edge or value names both sources and the
  affected contract. export-062 stays the graph of record until a decision
  changes it.
- **E01-3 Toolchain, identity and version-drift register**. Requirements:
  REQ-GOV-02, REQ-P01-08, REQ-P02-06, REQ-CI-03, REQ-UI-01, REQ-REL-02,
  REQ-P01-07, REQ-P03-08, REQ-P04-08, REQ-P01-01. Acceptance: Every pinned or
  floating toolchain is listed with its source and marked as proposal data.
  Nothing is installed. Each drift item becomes a decision.
- **E01-4 Imported workflow quarantine**. Requirements: REQ-CI-01, REQ-CI-02,
  REQ-BOOT-02, REQ-REL-01, REQ-P14-08. Acceptance: Imported CI, release and
  integration scripts stay inactive. Their failure-suppressing and
  simulated-output steps are listed as defects to fix before any activation.

Evidence:

- planning/candidates.json: 17 candidate rows covering all sixteen components,
  27 contradictions (15 open), 56 toolchain drift rows, 8 quarantined imported
  artefacts
- docs/roadmap/inventory.md: the public register, recording that no Cargo.toml,
  Cargo.lock or package manifest is committed and that crates/ and ui/ hold only
  README.md
- tools/verify_preparation.py verify_candidates(): validates schema, the pinned
  source bundle, component coverage, digest form, row bounds, contradiction
  sides and status, and quarantine invariants; it recomputes the digest of every
  public repository citation and fails closed on a manifest claim while the
  component is still a proposal
- tools/test_preparation.py: ten register tests covering positive, negative and
  boundary cases, including the public-citation mismatch and the summary length
  boundary
- make verify-all at this commit: 35 tests OK, praetorctl audit and
  compile-context --verify pass, licence digests and roadmap blocking states
  verified
- Scope limit: planning evidence only. No manifest, lock, build, image, boot,
  hardware, accessibility or release gate is closed or claimed by this milestone

### M02 - First component promoted end-to-end: P06 aegis-justitia decision engine

Rank 2. State: done. Cost: small. Owner repository: cordanaLLM/Aegis-OS. Needs
hardware: no. Needs external contract: no. Reference profile: not-hardware.
Blocked by: M01. Unblocks: M14, M03, M17, M07, M04.

Exit criteria:

- A workspace root Cargo.toml is created (the repository has none; export-006 is
  proposal data) with crates/aegis-justitia as its only activated member; the
  crate manifest and a committed Cargo.lock exist
- Toolchain admission: the stable Rust toolchain (cargo, clippy, rustfmt) is
  selected through the template matrix with a pinned version before any cargo
  gate runs
- Risk-tier, maker-checker, Annex III, timeout and killswitch logic exists as
  library code (not only in #[cfg(test)]) with positive, negative and boundary
  tests passing under `cargo test -p aegis-justitia` and `cargo clippy -p
  aegis-justitia --all-targets -- -D warnings`
- Audit-ledger hashing sits behind a trait, and the algorithm decision (D02:
  SHA-256 or BLAKE3; MD5 excluded) is recorded
- Out of scope, deferred explicitly: the consumer contracts and the hardened
  unit contract (M14), eBPF compilation and verifier load (M19, M10), and TPM2
  signing (M20)
- planning/components.json P06 status is advanced with the evidence path; `make
  verify-all` passes
- The pinned Rust toolchain is installed through rustup (extra/rustup 1.29.1)
  and recorded in a committed rust-toolchain.toml; `rustup show
  active-toolchain` inside the workspace reports the pinned toolchain and
  `rustup which rustc` resolves to the rustup path rather than /usr/bin/rustc;
  the pinned release and the distribution release currently carry the same
  version string because the distribution ships current stable, and the template
  matrix row cites both values so the substitution is auditable.

The template matrix row itself is
[the toolchain admission matrix](toolchain-admission.md). It records the pinned
Rust 1.98.1, the rustup 1.29.1 that provides it, the distribution `rust
1:1.98.1-1.1` it replaced, clippy 0.1.98 and rustfmt 1.9.0 as pinned components,
the crates the workspace pins, and the gate tools that are still pinned by
nothing.

Cheapest exit: Promote the drafted P06 test module into a library crate with a
trait boundary for hashing and signing. No TPM2, D-Bus or eBPF.

Epics:

- **E02-1 Workspace root, manifest, lock and Rust toolchain admission**.
  Requirements: REQ-P06-07, REQ-P16-08, REQ-GOV-01. Acceptance: Positive: the
  crate builds under `cargo build -p aegis-justitia` and a workspace-wide
  invocation. Negative: an unlocked dependency change fails `cargo build
  --locked`. Boundary: clippy with -D warnings passes with zero warnings.
- **E02-2 Decision engine with positive, negative and boundary tests**.
  Requirements: REQ-P06-01, REQ-P06-03, REQ-P06-04, REQ-GOV-03. Acceptance:
  Positive: a Tier C action is allowed, and two distinct non-maker checkers are
  accepted. Negative: maker as checker is rejected, and a duplicate checker is
  rejected. Boundary: due timestamps equal to now and to now-1 both fail closed,
  and the killswitch blocks Tier C.
- **E02-3 Audit ledger algorithm decision and trait boundary**. Requirements:
  REQ-P06-09, REQ-P16-09, REQ-P16-01. Acceptance: Positive: one algorithm is
  recorded and a chain re-walk verifies. Negative: a single flipped byte is
  detected, and a missing signer returns an error. Boundary: an empty ledger
  verifies as the genesis state. MD5 is removed.

### M14 - P06 consumer interface contracts and hardened unit contract

Rank 3. State: done. Cost: small. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: no. Needs external contract: no. Reference profile:
not-hardware. Blocked by: M02. Unblocks: M05, M06, M16, M20.

Delivered: `crates/aegis-justitia/src/contracts/` holds the three versioned
schemas as typed Rust structs -- the action proposal from P09 Minerva, the
decision request to P05 Forum and the signed audit record to P16 Athena -- each
with an explicit contract-version tag, a stable field encoding and a decoder
that lands every field in a fixed inline buffer and names the payload every
refusal refers to, including refusals whose version tag is itself unreadable.
D03's direction half is closed in the type system and D04 is recorded as
unresolved with both admission paths kept representable. The hardened unit stays
a contract file at `crates/aegis-justitia/contracts/justitia-
interceptor.service`, reviewed by library code and installed nowhere.

Not delivered, and not claimed: any transport. There is no D-Bus connection, no
socket, no eBPF program and no TPM2 signing; a signature field is a field
encoding, and nothing in the crate produces or verifies one. Nor is a transport
decided, which is the weaker and separate statement: D03's transport half stays
open, DSP-03 still records three transports for the P06/P09 edge, and decision
D26 carries it. Nor is either Vesta admission path behaviourally contract-tested
-- the D04 tests sweep a decision register, and M06 owns the admission contract.
Decoding is not unconditionally allocation-free either: an escape-free payload
costs no heap, an admissible JSON escape costs a bounded scratch copy inside
`serde_json`, and `tests/allocation_bounds.rs` asserts the difference. M16, M19,
M10 and M20 remain blocked.

Exit criteria:

- D03's direction half (the P06/P09 action gate is propose/intercept) is closed
  and D04 (P06 to P10 syscall intercept) is recorded; D03's transport half stays
  open and is carried as dispute DSP-03 against open decision D26, which this
  milestone does not touch
- Versioned schemas exist for the DecisionRequest (P06 to P05), the action
  proposal (P09 and P06, direction per D03) and the signed audit record (P06 to
  P16); each schema has contract tests with positive, negative and boundary
  payloads
- The justitia-interceptor unit stays a declarative contract (IPAddressDeny=any)
  and is not installed or executed
- Uses the Rust toolchain admitted in M02; no new toolchain; `make verify-all`
  passes

Cheapest exit: Write the three schemas as typed Rust structs with serde
round-trip tests. No transport is implemented.

Epics:

- **E14-1 Action-gate direction decision and action proposal contract**.
  Requirements: REQ-P09-03, REQ-P09-05, REQ-GRAPH-04, REQ-GRAPH-02. Acceptance:
  Positive: a signed, well-formed proposal round-trips. Negative: an unsigned or
  malformed proposal is rejected. Boundary: a proposal at the maximum field
  lengths is accepted, and one byte over is rejected.
- **E14-2 DecisionRequest and audit record contracts**. Requirements:
  REQ-GOV-03, REQ-P06-08, REQ-P06-10, REQ-P16-03. Acceptance: Positive: both
  schemas round-trip. Negative: an unknown schema version is rejected. Boundary:
  an audit record whose previous hash is the all-zero genesis value is accepted
  only as the first record. The PLD effective date is tracked as an external
  deadline.
- **E14-3 Hardened unit contract**. Requirements: REQ-P06-02. Acceptance:
  Positive: the unit file declares IPAddressDeny=any. Negative: a review check
  fails if the directive is removed. Boundary: the unit is not installed or
  started in this milestone.

### M03 - P01/P02 definitions validated offline with host systemd

Rank 4. State: done. Cost: small. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: no. Needs external contract: no. Reference profile:
not-hardware. Blocked by: M02. Unblocks: M18, M15.

Exit criteria:

- build/ holds the repart definitions (00-esp, 10-root-a/b, 11-root-verity,
  20-var) and the sysupdate transfer as reviewed files with source hashes
- Toolchain admission: a systemd version floor is selected through the template
  matrix (the host observed at revision time runs systemd 261); the Rust
  toolchain from M02 is reused for the parser; mkosi is not used in this
  milestone
- Repart gate: `systemd-repart --empty=create --size=<N> --dry-run=yes
  --definitions=<dir> <scratch image>` exits 0 on valid definitions; the gate
  also fails on any 'Unknown key ... ignoring' diagnostic, because
  systemd-repart exits 0 on unknown keys (observed on systemd 261, where the
  imported ESP 'Subsystem=' and var 'BtrfsSubvolumes=' keys are ignored)
- Negative and boundary cases observed on systemd 261: a definition without
  Type= exits 1; SizeMinBytes greater than SizeMaxBytes exits 1; SizeMinBytes
  equal to SizeMaxBytes is accepted
- Transfer gate: `systemd-sysupdate --root=<scratch tree containing
  usr/lib/sysupdate.d> --offline list` parses the transfer definition
  unprivileged; on systemd 261 the imported definition is rejected with exit 1,
  so it must be rewritten to supported keys before the gate can pass
- The Rust definition parser crate (location per D15) has its manifest, lock and
  positive/negative/boundary tests; no `|| true` anywhere in the gate
- The systemd version floor admitted through the template matrix records the
  exact reference-profile value `systemd 261 (261.3-1-arch)` from `systemctl
  --version`, so a future floor change is a visible diff rather than an
  inherited assumption.
- The repart and sysupdate gates are re-run and their exit codes and diagnostics
  retained; a pass here is development evidence on the reference profile only
  and does not close the image or boot gate.

Cheapest exit: Run the two systemd commands against scratch images and trees,
plus the parser tests. No image build and no mkosi.

Epics:

- **E03-1 Repart analyzer gate**. Requirements: REQ-P01-02, REQ-P01-03,
  REQ-P02-03, REQ-P02-07, REQ-P01-10, REQ-CI-01. Acceptance: Positive: the dry
  run exits 0 with no ignored-key diagnostics. Negative: a missing Type= exits
  non-zero, and an unknown key fails the gate. Boundary: ESP min equal to max is
  accepted, and inverted min greater than max is rejected.
- **E03-2 Sysupdate transfer gate**. Requirements: REQ-P01-04, REQ-P02-04,
  REQ-P02-06. Acceptance: Positive: the rewritten transfer lists without error
  offline. Negative: the imported transfer is rejected (exit 1). Boundary: a
  transfer with both root slots and no writable target parses, and one with a
  single slot is flagged against the A/B requirement.
- **E03-3 Definition parser crate**. Requirements: REQ-P01-02, REQ-P01-10,
  REQ-WS-01. Acceptance: Positive: the definitions round-trip through the
  parser. Negative: a duplicate section or malformed size is rejected. Boundary:
  512M and 1G parse exactly, and 511M below the minimum is flagged.
- **E03-4 PCR measurement acceptance target**. Requirements: REQ-P01-05.
  Acceptance: The PCR 0/4/7/11 strategy is documented as the acceptance target
  for M11 boot evidence. It is not claimed here.

### M18 - Aegis-side product input manifest and kernel requirement schemas (local)

Rank 5. State: done. Cost: small. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: no. Needs external contract: no. Reference profile:
not-hardware. Blocked by: M03. Unblocks: M09, M26.

Exit criteria:

- The Aegis product input manifest schema (repart, sysupdate and mkosi
  configuration references, correlation id, exact revision, bounded retries)
  validates the M03 files; a malformed manifest is rejected
- The kernel requirement schema (required BPF LSM, sched_ext, BTF, RAPL,
  VFIO/IOMMU and KVM features; ABI; accepted architectures; artifact digest and
  signature fields) is written as a payload, not a fixed symbol list
- D07 (boot kernel identity) and D18 (distribution release and package pinning)
  are recorded
- Toolchain admission: D56 keeps mkosi validation in Aegis, so mkosi is
  selected through the template matrix before `mkosi summary` runs. The
  admission is the exact pin `mkosi 27` (distribution package `extra/mkosi
  27-1`), read back from the reference profile with `mkosi --version` and
  `pacman -Qi mkosi`, and it supersedes the register's inherited `mkosi v24+`
  floor. `build/mkosi.conf` carries `MinimumVersion=27`, so the floor is
  enforced by mkosi itself rather than by a version string in a document
- No producer repository is contacted; the schemas are Aegis-owned files with
  positive, negative and boundary tests
- The kernel requirement schema is validated positively against
  planning/hardware-profile.json as a real reference payload, with each
  asserted feature traceable to its probe command (zgrep /proc/config.gz,
  /sys/kernel/security/lsm, /sys/kernel/btf/vmlinux, /sys/class/powercap,
  /sys/kernel/iommu_groups).
- Negative case bound to a measured absence: a requirement payload demanding
  CONFIG_PREEMPT_RT is rejected against the reference profile, whose running
  kernel reports '# CONFIG_PREEMPT_RT is not set' with
  CONFIG_PREEMPT_DYNAMIC=y.
- Boundary case: a requirement payload demanding only CONFIG_HZ_1000 is
  accepted, since the reference profile sets CONFIG_HZ_1000=y while still
  failing the PREEMPT_RT requirement — proving the schema discriminates per
  feature and not per kernel flavour.
- D56 is recorded, and the branch taken is the Aegis-side one: mkosi is
  installed on the reference profile and pinned at `extra/mkosi 27-1`, which
  supersedes the register's inherited 'mkosi v24+' row with an exact pin. The
  Imago-result branch is not taken, because Imago is a scaffold and cannot
  return a result to defer to.
- mkosi 27 is installed on the reference profile and validates the image
  definitions locally while Imago remains a scaffold (D56); the validation is
  Aegis-side and constructs no product image, and it moves to consuming an
  Imago result once Imago returns real artifacts
- The kernel requirement schema expresses the realtime and scheduler options
  P07 and P08 need (preemption model, timer frequency, sched_ext, BPF LSM,
  BTF), and renders them as a Kconfig fragment: one `CONFIG_X=y`, `CONFIG_X=m`
  or `# CONFIG_X is not set` line per required feature, with the recorded
  requirement identifier above it and nothing that is neither a comment nor an
  assignment. That is the form milestone M26 applies to a base configuration,
  because D70 as amended builds the kernel in this repository while Nucleus is
  a scaffold and a distribution package is only an interim fixture source

Cheapest exit: Author the two schemas and validate them against the M03 files
with the Rust toolchain from M02.

Evidence:

- Disclosure, because a milestone that edits its own bar must say so where the
  bar is judged: three of this milestone's eleven exit criteria were rewritten
  in planning/roadmap.json during this delivery and before the state moved to
  done. None is a relaxation. Criterion 4 read 'Toolchain admission: if D14
  keeps mkosi validation in Aegis, mkosi is selected through the template matrix
  at or above the v24 floor before `mkosi summary` runs; otherwise mkosi
  validation is deferred to the Imago result'; an either/or cannot be judged
  done, so it now records the branch that was actually taken and the exact pin
  `mkosi 27` that supersedes the inherited 'mkosi v24+' floor. Criterion 9 read
  'D56 is recorded: mkosi is not installed and M18 takes D14's "validate through
  the Imago result" branch, or mkosi is pinned at extra/mkosi 27-1 which
  supersedes the register's inherited "mkosi v24+" row with an exact pin'; its
  first half is false on the reference profile -- mkosi 27 is installed -- and
  the Imago-result branch has no result to defer to while Imago is a scaffold,
  so the criterion now names the Aegis-side branch and why the other is
  unavailable. Criterion 11 read 'The kernel requirement schema expresses the
  realtime and scheduler options P07 and P08 need (preemption model, timer
  frequency, sched_ext, BPF LSM, BTF), because Nucleus builds the kernel and a
  distribution package is only an interim fixture source (D70)'; it is strictly
  strengthened by adding the Kconfig-fragment requirement that D70 as amended
  (PR #93) makes M26's input. No other criterion changed: 1-3, 5-8 and 10 are
  judged exactly as they were written.
- crates/aegis-fabrica-defs carries both M18 schemas: src/manifest.rs
  (ProductInputManifest: correlation id, exact 40-character revision, pinned
  distribution snapshot, repart/sysupdate/mkosi/kernel-requirement references,
  package set, boot kernel identity, bounded retries) and src/kernel.rs
  (KernelRequirement: feature rows, ABI bounds, accepted architectures, artifact
  digest and signature), both built on the ten bounded field types in
  src/field.rs that validate during decoding through #[serde(try_from =
  "String")]; `cargo build --locked` exits 0
- The reviewed manifest build/product-input.json validates the M03 files:
  crates/aegis-fabrica-defs/tests/product_input_manifest.rs reads the five
  drop-ins in build/repart.d and the transfer in build/sysupdate.d and runs them
  through the same RepartDefinition and TransferDefinition parsers the M03 gate
  uses. A set that does not parse is DefinitionRefused, a set that parses but
  drops the alternate root slot is RequirementNotMet with
  MissingAlternateRootSlot{slots:1}, and a manifest without a correlation id,
  without a 40-character revision, or with snapshot "latest" does not decode at
  all. Verified by `cargo test --locked --all-features`
- The kernel requirement is a payload rather than a fixed symbol list:
  build/kernel-requirement.json carries thirteen {symbol, state, probe,
  required-by} rows and the crate names no Kconfig symbol in code. `grep -rn
  'CONFIG_[A-Z]' crates/aegis-fabrica-defs/src` returns four hits, all inside
  documentation comments (three `CONFIG_X` placeholders and one `CONFIG_BPF_LSM`
  example)
- D07 (boot kernel identity) and D18 (distribution release and package pinning)
  are recorded in docs/roadmap/README.md and are carried by the schema rather
  than only by prose: the manifest's kernel block can spell all three D07
  sources (distribution-package, built-here, producer-artifact) and records the
  pinned default package linux-rt alongside the source in force, and D18's pin
  is the SnapshotId field type, which refuses "latest" and "rolling" during
  decoding
- mkosi is admitted with an exact pin read back from the reference profile:
  `mkosi --version` prints `mkosi 27` and `pacman -Qi mkosi` reports Version
  27-1 installed from repository extra. docs/roadmap/toolchain-admission.md
  replaces M03's 'installed and not admitted' paragraph with the row,
  superseding the M01 register's inherited 'mkosi v24+' floor with the exact pin
- `mkosi summary` parses the Output stanza of the reviewed build/mkosi.conf:
  `python3 tools/verify_mkosi_definitions.py` exits 0 with mkosi/positive exit 0
  (Output Format disk, Output aegis-os.raw, Image ID aegis-os, Snapshot
  2026/09/13, and Repart Directories resolving to the reviewed build/repart.d),
  mkosi/negative-below-floor exit 1 with `mkosi 28 or newer is required by this
  configuration (found 27)`, and mkosi/boundary-at-floor exit 0 at
  MinimumVersion=27. The gate is wired into `make verify-all` as `make
  verify-mkosi` and skips with a reason on a host without mkosi or below the
  floor. The gate also asserts the absence of an unreviewed partition-definition
  set rather than only the presence of the reviewed one: exactly one resolved
  `RepartDirectories=` row must be the reviewed directory, compared as a
  resolved path rather than by the tail of the path, and any other row holding a
  `.conf` fails the case by name. Reproduced before and after: a
  `build/mkosi.conf.d/99-stowaway.conf` drop-in adding a second
  `RepartDirectories=` row that points at another directory ending
  `build/repart.d` was accepted with exit 0 by the suffix test, and is refused
  with exit 1 by the resolved-path test, while the reviewed definition with no
  drop-in still exits 0. This matters because git cannot track an empty
  directory, so a `mkosi.repart/` or `mkosi.conf.d/` tree can exist in a
  checkout, be read by mkosi as a default path, and never appear in `git status`
  or any diff
- Each asserted feature is traceable to its probe command, and the probe's path
  is asserted against the same capability's evidence command in
  planning/hardware-profile.json. Read back on the reference profile: `zgrep
  CONFIG_PREEMPT_RT /proc/config.gz` prints `# CONFIG_PREEMPT_RT is not set`;
  `zgrep CONFIG_PREEMPT_DYNAMIC /proc/config.gz` prints
  `CONFIG_PREEMPT_DYNAMIC=y`; `zgrep CONFIG_HZ_1000 /proc/config.gz` prints
  `CONFIG_HZ_1000=y`; `cat /sys/kernel/security/lsm` prints
  `capability,landlock,lockdown,yama,bpf`; `ls /sys/kernel/btf/vmlinux` lists
  the blob; `ls /sys/class/powercap` lists intel-rapl, intel-rapl:0 and
  intel-rapl:0:0; `ls /sys/kernel/iommu_groups | wc -l` prints 38
- Positive against a real reference payload:
  build/kernel-requirement.reference.json is satisfied by
  planning/hardware-profile.json with no unmet row -- architecture x86-64,
  kernel release 7.2.4-1-cachyos, the module ABI, thirteen Kconfig symbol states
  and the four runtime capabilities the probes read
- Negative bound to a measured absence: build/kernel-requirement.json, the
  product requirement, is refused against the same profile with exactly one
  unmet row, StateMismatch for CONFIG_PREEMPT_RT (required built-in, observed
  not set). The reference kernel is PREEMPT_DYNAMIC, so this workstation is not
  a kernel the product requirement admits
- Boundary: a payload demanding only CONFIG_HZ_1000 is accepted against the same
  profile that refuses the product requirement, and relaxing only the
  CONFIG_PREEMPT_RT row makes the product requirement pass with no other change.
  The schema therefore discriminates per feature and not per kernel flavour
- The payload is expressible as the kernel configuration fragment M26 consumes:
  KernelRequirement::config_fragment renders CONFIG_PREEMPT_RT=y,
  CONFIG_HZ_1000=y, CONFIG_SCHED_CLASS_EXT=y, CONFIG_BPF_LSM=y,
  CONFIG_DEBUG_INFO_BTF=y and CONFIG_VFIO=m -- one assignment per feature row
  with its requirement identifier on the line above, and no line that is neither
  a comment nor an assignment. The renderer validates before it writes a line
  and returns the refusal otherwise, because every field of the payload is
  public: a caller that assembled a requirement without decoding it would
  otherwise receive a fragment silently short of the rows past the feature
  bound, and M26 applies this fragment to a base configuration. A payload at
  MAX_FEATURES renders 64 assignments; one row past it is refused with
  TooMany{what: "features"} instead of rendering 64
- No producer repository is contacted and no gate here needs a network: `unshare
  -rn cargo test --locked --all-features --offline` exits 0 over 29 test
  binaries and two doc-test groups, 275 cases with no failure, and `unshare -rn
  mkosi --no-pager --directory build summary` exits 0
- Gates re-run on the reference profile, each exit 0: `cargo fmt --check`,
  `cargo build --locked`, `cargo test --locked --all-features` (275 cases over
  29 test binaries and two doc-test groups), `cargo clippy --locked
  --all-targets --all-features -- -D warnings`, `RUSTDOCFLAGS='-D warnings'
  cargo doc --locked --no-deps`, `python3 tools/verify_preparation.py`, `python3
  tools/rank_roadmap.py`, `python3 -B -m unittest discover -s tools -p
  'test_*.py'` (113 tests), `make verify-all`, `make verify-mkosi`, `npx
  markdownlint-cli2`, `yamllint .`, `flake8 .` and `black --check --line-length
  100`
- Scope: development evidence on the reference profile recorded in
  planning/hardware-profile.json. What was produced is two schemas, three
  reviewed payload files, one reviewed image definition and a configuration
  parse. No image, UKI, kernel, artefact, signature or boot evidence is produced
  or claimed, no producer repository is contacted, and the image, kernel, boot,
  hardware and release gates remain blocked

Epics:

- **E18-1 Product input manifest schema**. Requirements: REQ-P01-01, REQ-P01-06,
  REQ-P01-08, REQ-GOV-02. Acceptance: Positive: the manifest built from the M03
  files validates. Negative: a manifest without a correlation id or exact
  revision is rejected. Boundary: a retry count at the bound is accepted, and
  one above is rejected.
- **E18-2 Kernel requirement schema and kernel identity decision**.
  Requirements: REQ-P01-09, REQ-P07-01, REQ-P06-05, REQ-P13-02, REQ-P07-06.
  Acceptance: Positive: the payload lists the kernel features required by P06,
  P07 and P13. Negative: an unknown architecture is rejected. Boundary: an empty
  requirement list is rejected explicitly, not accepted as 'no requirements'.
- **E18-3 mkosi admission decision**. Requirements: REQ-P01-08, REQ-P02-06.
  Acceptance: Positive: if admitted, `mkosi summary` parses the Output stanza.
  Negative: a version below the floor is refused. Boundary: the floor version
  itself is accepted.

### M15 - P02 A/B candidate lifecycle state machine

Rank 6. State: done. Cost: small. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: no. Needs external contract: no. Reference profile:
not-hardware. Blocked by: M03. Unblocks: M24.

Exit criteria:

- The A/B lifecycle (candidate, signature check, delta acquisition, slot swap,
  watchdog, bless or rollback) is a Rust library with manifest, lock (location
  per D15) and positive, negative and boundary tests
- D13 (reversible consolidation versus permanent hardening) is recorded against
  the dm-verity requirement
- Uses the Rust toolchain admitted in M02; systemd and sysupdate calls are
  stubbed
- The state machine exposes a machine-readable transition trace so that M24 can
  diff a real QEMU A/B sysupdate transfer against it byte-for-byte rather than
  by narrative comparison.

Cheapest exit: Model the lifecycle as a pure state machine with a stubbed clock
and stubbed sysupdate.

Epics:

- **E15-1 A/B lifecycle state machine**. Requirements: REQ-P02-08, REQ-P02-01,
  REQ-P02-02. Acceptance: Positive: the happy path reaches Bless. Negative: a
  signature failure reaches Discard. Boundary: watchdog expiry exactly at the
  timeout takes Rollback, and one tick before does not.
- **E15-2 Reversible consolidation decision**. Requirements: REQ-P02-05,
  REQ-P16-05. Acceptance: The decision is recorded, and a test shows that a
  reopened slot still requires a verity match before Bless.

### M17 - Trivial leaf slices: P03 Vulcan and P15 Hestia validation crates

Rank 8. State: done. Cost: trivial. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: no. Needs external contract: no. Reference profile:
not-hardware. Blocked by: M02. Unblocks: M25.

Exit criteria:

- aegis-vulcan and aegis-hestia are added as workspace members (they are absent
  from the proposal workspace) with manifests, lock entries and
  positive/negative/boundary tests; the Rust toolchain from M02 is reused
- Scaffold assertions are refactored to Result per HISS-07 so that negative
  cases are ordinary tests
- The Hestia location decision (D09) is applied from the M01 inventory
- Interface contracts are typed for P03 to P09 (weight streaming descriptor),
  P03 to P15 (media ingest descriptor) and P15 to P04 (overlay registration),
  each with a negative test for a malformed payload; the transports stay stubbed
  until M12
- The P03-to-P09 weight-streaming descriptor and the P03-to-P15 media-ingest
  descriptor are typed so that the reference profile's actual DMA-BUF export
  path (i915 renderD129, amdgpu renderD130, nvidia_drm modeset=Y) can be bound
  at M25 without reshaping the descriptor.

Cheapest exit: Extract the validation arithmetic and bounds checks. No VFIO, GPU
or PGlite runtime.

Epics:

- **E17-1 Vulcan validation crate**. Requirements: REQ-P03-04, REQ-P03-05,
  REQ-P03-08, REQ-P03-06, REQ-WS-01, REQ-P03-01, REQ-P03-02. Acceptance:
  Positive: an aligned BAR is accepted. Negative: a misaligned BAR is rejected.
  Boundary: block_count 0 and 8193 are rejected, 8192 is accepted, and the ring
  index wraps.
- **E17-2 Hestia vector-store state machine**. Requirements: REQ-P15-01,
  REQ-P15-05, REQ-P15-06, REQ-P15-08, REQ-GRAPH-03. Acceptance: Positive: an
  initialized store answers queries. Negative: a query before init fails.
  Boundary: limits 0 and 101 fail, and 1 and 100 pass.
- **E17-3 P03 and P15 interface contracts**. Requirements: REQ-P03-07,
  REQ-P09-04, REQ-P15-07. Acceptance: Positive: the descriptors round-trip.
  Negative: a malformed descriptor is rejected. Boundary: a descriptor at the
  maximum block count is accepted, and one over is rejected.

### M05 - Evolution loop logic: P13 Tellus SCI and P16 Athena lifecycle

Rank 9. State: ready. Cost: small. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: no. Needs external contract: no. Reference profile:
not-hardware. Blocked by: M14. Unblocks: M19, M21.

Exit criteria:

- crates/aegis-tellus and crates/aegis-athena have manifests, lock entries and
  positive/negative/boundary tests passing under cargo test and clippy; the Rust
  toolchain from M02 is reused and no new toolchain is admitted
- The SCI formula, defer threshold and slice bound are tested; RAPL and eBPF
  access are stubbed
- The Athena 7-stage state machine and Pareto predicate are tested at exact
  thresholds; the ledger is ported to Rust with the D02 algorithm
- Edge contracts are typed with negative tests for malformed payloads: P16 to
  P13 candidate SCI query, P16 to P02 promotion trigger, and consumption of the
  M14 audit record
- The SCI engine's wattage input is behind a seam with a recorded simulated
  constant, so that M21 can substitute a measured RAPL delta without touching
  the arithmetic under test.
- The seam accepts a zone list, because the reference profile exposes only
  package-0 and core and has no dram or psys zone — the engine must not assume a
  DRAM domain exists (D60).

Cheapest exit: Extract the arithmetic and state machines into lib targets and
mock all I/O.

Epics:

- **E05-1 Tellus SCI arithmetic**. Requirements: REQ-P13-01, REQ-P13-06,
  REQ-P13-07, REQ-P13-08. Acceptance: Positive: a hand-computed SCI matches.
  Negative: functional_units <= 0 falls back without NaN. Boundary: 300.0 is not
  deferred, 300.001 is deferred, and slices never exceed 16.
- **E05-2 Athena lifecycle and Pareto gate**. Requirements: REQ-P16-01,
  REQ-P16-02, REQ-P16-05, REQ-P16-10, REQ-P15-04. Acceptance: Positive: a
  passing candidate reaches Publish. Negative: exceeding any single bound
  reaches Invalidate with a ledger entry, and an empty id is rejected. Boundary:
  latency 1.5 and retention 0.99 are exercised exactly.
- **E05-3 Hash-chained ledger in Rust**. Requirements: REQ-P16-03, REQ-P16-09,
  REQ-P06-09. Acceptance: Positive: a chain re-walk verifies. Negative: a
  tampered record is detected. Boundary: the genesis record is verified, and the
  algorithm matches D02.
- **E05-4 Evolution-loop edge contracts**. Requirements: REQ-P13-05, REQ-P13-03,
  REQ-P16-06, REQ-P01-04. Acceptance: Positive: typed request and response
  round-trip for each edge. Negative: malformed payloads are rejected. Boundary:
  the sysupdate call is stubbed, and an empty candidate list is handled
  explicitly.

### M06 - Agent execution chain logic: P09 Minerva and P10 Vesta

Rank 10. State: ready. Cost: small. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: no. Needs external contract: no. Reference profile:
not-hardware. Blocked by: M14. Unblocks: M08, M22.

Exit criteria:

- crates/aegis-minerva and crates/aegis-vesta have manifests, lock entries and
  positive/negative/boundary tests; no GPU, D-Bus, Z3 FFI, KVM or Wasm engine is
  used; the Rust toolchain from M02 is reused
- The Wasm runtime language-boundary decision (D06) is recorded: a Rust-native
  runtime, or a protocol/FFI adapter with contract tests; no Go crate dependency
- The P06 action proposal contract from M14 is consumed; the P09 to P10 capsule
  request and the P09/P14 verification direction are typed with negative tests
- The P10 capsule request type carries a VMM identity field, because the
  reference profile can supply either Firecracker 1.17.0 (packaged, absent) or
  QEMU 11.1.1 microvm (installed), and D58 requires every measurement to record
  which VMM produced it.

Cheapest exit: Compile only the in-memory logic layers of the P09 and P10
candidates with cargo test.

Epics:

- **E06-1 Minerva router, replay and solver logic**. Requirements: REQ-P09-01,
  REQ-P09-02, REQ-P09-06, REQ-P09-07, REQ-P09-08. Acceptance: Positive: expert
  registration and routing succeed. Negative: route returns None when no expert
  matches. Boundary: the 32nd expert is accepted and the 33rd rejected; the
  128th trajectory step is accepted and the 129th rejected; relabelling flips
  only negative rewards.
- **E06-2 Vesta bounded controllers**. Requirements: REQ-P10-01, REQ-P10-04,
  REQ-P10-05, REQ-P10-06, REQ-P10-07. Acceptance: Positive: 64 microVMs and 128
  capsules are accepted. Negative: terminating an unknown id returns false.
  Boundary: the 65th microVM and 129th capsule fail. The boot-time literal is
  marked unmeasured.
- **E06-3 Wasm runtime boundary decision**. Requirements: REQ-P10-08,
  REQ-P10-02, REQ-P10-03. Acceptance: The decision is recorded. Venus and
  AF_VSOCK pricing stay deferred hardware-backed requirements (M21).
- **E06-4 Agent-chain edge contracts**. Requirements: REQ-P09-03, REQ-GRAPH-05,
  REQ-P14-05, REQ-P16-04. Acceptance: Positive: typed contracts round-trip.
  Negative: unsigned or malformed proposals are rejected. Boundary: a capsule
  request at the capsule bound is accepted, and one over is rejected.

### M08 - Leaf slices dependent on the agent chain: P11 Ludus and P14 Hephaestus

Rank 11. State: blocked. Cost: small. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: no. Needs external contract: no. Reference profile:
not-hardware. Blocked by: M06. Unblocks: M25.

Exit criteria:

- aegis-ludus and aegis-hephaestus have manifests, lock entries and
  positive/negative/boundary tests; the Rust toolchain from M02 is reused
- Assertions are refactored to Result per HISS-07
- P11 Ludus integrates no Steamworks SDK (D12, ADR-0002); a negative test proves
  the image and the crate build without it
- Interface contracts are typed for P09 to P14 (VERIFY_CODE_CAD, direction per
  the M01 register), P14 to P15 (geometry viewport descriptor), P11 to P02
  (transaction receipt, TPM2 signing stubbed) and P11 to P04 (rich presence),
  each with a negative test for a malformed payload
- The P11-to-P02 transaction-receipt contract records that the reference profile
  has a TPM2 (tpm0 version 2) but NO FIDO2 authenticator (`lsusb | grep -iE
  'yubi|fido|solo|token|nitro'` empty), so the FIDO2 half of P11 is an explicit
  procurement dependency and not a stub that could be mistaken for coverage.

Cheapest exit: Extract the bounds and validation logic. No Steam, CAD kernel or
solver runtime.

Epics:

- **E08-1 Ludus launch-argument validator**. Requirements: REQ-P11-02,
  REQ-P11-07, REQ-P11-01, REQ-P11-05. Acceptance: Positive: 64 args are
  accepted. Negative: 65 args are rejected. Boundary: an empty argument list is
  handled explicitly and its authentication outcome is recorded.
- **E08-2 Hephaestus bounded loops**. Requirements: REQ-P14-01, REQ-P14-02,
  REQ-P14-03, REQ-P14-04, REQ-P14-08. Acceptance: Positive: a mesh under the
  bound is accepted. Negative: a missing STEP path errors. Boundary: 500000
  elements are accepted and 500001 rejected, and the iteration bound is
  honoured. CAD and solver versions are recorded as unpinned.
- **E08-3 P11 and P14 interface contracts**. Requirements: REQ-P14-05,
  REQ-P14-06, REQ-P11-06, REQ-P11-04. Acceptance: Positive: the descriptors
  round-trip. Negative: a receipt without a signature field is rejected.
  Boundary: a viewport descriptor at the mesh bound is accepted.

### M07 - Real-time control plane: P04, P07 and P08 logic

Rank 12. State: ready. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: no. Needs external contract: no. Reference profile:
not-hardware. Blocked by: M02. Unblocks: M19, M23.

Exit criteria:

- crates/aegis-compositor, aegis-lictor and aegis-calliope have manifests, lock
  entries and positive/negative/boundary tests; no wlroots, GPU, PipeWire,
  cgroups or BPF attach; the Rust toolchain from M02 is reused
- The Zenoh crate version, if needed, is selected and locked at activation after
  checking current upstream; the export-006 pin is not inherited
- The surface registry (256) and IPC client (64) bounds, EWMA tier thresholds,
  plugin slots (32) and DMA-BUF buffers (64) are tested at their edges
- P04 is implemented as a pure Rust compositor (D08, ADR-0001); no C wlroots
  toolchain is admitted, and the frame-pacing constant is pinned
- BPF C compilation is not part of this milestone (moved to M19)
- One criterion states that no latency or determinism figure is produced by this
  milestone, because the reference profile runs PREEMPT_DYNAMIC and not
  PREEMPT_RT; every timing claim for P07/P08 is deferred to M23.
- The RLIMIT_MEMLOCK and SCHED_RR assumptions are recorded as measured on the
  reference profile (`ulimit -r` -> 99, above the required 95; `ulimit -l` ->
  8192 KB, below the 'infinity' P08 expects), so the memlock gap is visible as a
  privileged-configuration action rather than silently assumed.

Cheapest exit: Unit-test the pure state machines only.

Epics:

- **E07-1 Compositor registry and Tier-1 socket bounds**. Requirements:
  REQ-P04-04, REQ-P04-05, REQ-P04-07, REQ-P04-08. Acceptance: Positive: 256
  surfaces and 64 clients are accepted. Negative: the 257th surface is rejected
  and the 65th client is not admitted. Boundary: a pacing-constant test pins the
  chosen value.
- **E07-2 wlroots and mesh design decision**. Requirements: REQ-P04-01,
  REQ-P04-02, REQ-P04-03, REQ-P04-06. Acceptance: The decision is recorded. The
  Zenoh version is selected at activation after checking current upstream, with
  a mocked transport test: a publish round-trips, a subscriber cannot write
  back, and an empty key expression is rejected.
- **E07-3 Lictor EWMA tiers and broker**. Requirements: REQ-P07-01, REQ-P07-02,
  REQ-P07-03, REQ-P07-04, REQ-P07-05, REQ-P01-07. Acceptance: Positive: bursts
  below each threshold classify per the source comparisons. Negative: an
  unregistered focus PID throttles nothing. Boundary: bursts exactly at each
  threshold classify per the strict comparison.
- **E07-4 Calliope plugin lifecycle and DMA-BUF descriptors**. Requirements:
  REQ-P08-01, REQ-P08-02, REQ-P08-03, REQ-P08-06, REQ-P08-07, REQ-P08-08.
  Acceptance: Positive: 32 slots are accepted. Negative: the 33rd slot and an
  illegal transition are rejected. Boundary: stride arithmetic is checked at 0
  and at u32::MAX/4.
- **E07-5 Real-time edge contracts**. Requirements: REQ-P08-04, REQ-P08-05,
  REQ-P13-03. Acceptance: Positive: typed focus-switch, RTPRIO grant and DMA-BUF
  stream descriptors round-trip. Negative: malformed descriptors are rejected.
  Boundary: an RTPRIO value at the source value is accepted, and one above is
  rejected.

### M19 - eBPF objects loaded through the verifier on the host kernel

Rank 13. State: blocked. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: no. Needs external contract: no. Reference profile: full.
Blocked by: M05, M07. Unblocks: M10.

Exit criteria:

- Toolchain admission: clang (BPF target), bpftool and libbpf (or aya, per the
  M01 register) are selected through the template matrix with pinned versions
  before any compile
- action_gate, scx_cake and kepler_power compile warning-free with clang -target
  bpf
- The objects load through the BPF verifier on the host kernel with CAP_BPF; the
  host kernel version and config (BPF LSM, sched_ext, BTF) are recorded as
  observed facts, not assumed
- Negative: removing the ringbuf NULL check or an unroll bound makes the
  verifier reject the object. Boundary: a struct_ops load with all handlers
  stubbed succeeds where the host exposes sched_ext
- This is a non-qualifying local fixture: a host or stock kernel cannot close
  the Nucleus-kernel verification in M10
- The three pinned tool versions are recorded from the reference profile before
  the first compile: clang 22.1.8 (`clang -print-targets` listing bpf, bpfeb,
  bpfel), bpftool v7.8.0 and libbpf v1.8.
- The verifier log is retained for every load, positive and negative, and the
  negative case is proven by the log rejecting the object — not by a non-zero
  exit code alone: removing the ringbuf NULL check or an unroll bound must
  produce a named verifier rejection in the log.
- The scx_cake struct_ops boundary load explicitly attaches AND detaches on the
  host kernel, and the pre-existing scheduler is recorded and restored:
  /sys/kernel/sched_ext/root/ops reads `ghostbrew` before the test, the stubbed
  scheduler during it, and `ghostbrew` again after, with nr_rejected and
  switch_all captured at each point. Only one scx scheduler can hold root/ops,
  so this is a deliberate disruptive step, not a background one (D67).
- The host kernel identity and config are recorded as observed facts:
  7.2.4-1-cachyos PREEMPT_DYNAMIC, CONFIG_SCHED_CLASS_EXT=y, CONFIG_BPF_LSM=y,
  CONFIG_DEBUG_INFO_BTF=y, with the probe command beside each.
- The provenance of bpf/scx_cake.bpf.c is recorded against extra/scx-scheds
  1.1.3-2, which already installs /usr/bin/scx_cake on the reference profile:
  either an upstream fork with its commit pinned, or an Aegis original (D66).
- One criterion restates that this is a non-qualifying local fixture: a pass on
  the reference profile is development evidence only, it does not close M10's
  Nucleus-kernel verification, and it does not close any hardware or release
  gate.

Cheapest exit: Compile the three objects and run a verifier load on the
workstation kernel. No VM, image or Nucleus artifact.

Epics:

- **E19-1 action_gate LSM object**. Requirements: REQ-P06-05, REQ-P07-06.
  Acceptance: Positive: the program loads, and an exec event reaches a stub
  ringbuf consumer where BPF LSM is active. Negative: the unchecked-pointer
  variant is rejected. Boundary: if BPF LSM is not active on the host, attach is
  recorded as unavailable rather than passed.
- **E19-2 scx_cake struct_ops object**. Requirements: REQ-P07-04, REQ-P07-01.
  Acceptance: Positive: struct_ops loads where sched_ext is present. Negative:
  the unbounded-loop variant is rejected. Boundary: all-stub handlers load.
- **E19-3 kepler_power probe object**. Requirements: REQ-P13-02. Acceptance:
  Positive: the tracepoint program loads. Negative: an out-of-bounds map access
  variant is rejected. Boundary: the hardcoded TDP literal is recorded as a
  non-measurement.

### M21 - Workstation hardware slices: RAPL counters and KVM sandboxing

Rank 18. State: blocked. Cost: small. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: yes. Needs external contract: no. Reference profile: full
(privileged read). Blocked by: M05. Unblocks: nothing.

Exit criteria:

- Hardware: a host with readable RAPL counters and KVM; the host model and
  kernel are recorded
- Toolchain admission: Firecracker (and the jailer, if used) is selected through
  the template matrix with a pinned version before any microVM run
- Unblocking evidence: measured RAPL energy deltas replace the simulated wattage
  in the M05 engine; a measured microVM boot time and memory footprint replace
  the scaffold literals; one AF_VSOCK candidate evaluation round-trips
- Scope: these are host-fixture measurements; re-measurement inside an Aegis
  image is a later acceptance and is not claimed
- Measured RAPL energy deltas from the sysfs energy counter replace the
  simulated wattage constant in the M05 engine: at least two reads of
  /sys/class/powercap/intel-rapl:0/energy_uj separated by a recorded interval,
  with the host model (AMD Ryzen 9 9950X3D) and kernel (7.2.4-1-cachyos)
  recorded beside them.
- The reader handles counter rollover explicitly, with a boundary test against
  the measured max_energy_range_uj of 65532610987 uJ; a wrap must produce a
  correct positive delta and not a negative or absurd one.
- The zone enumeration is recorded as measured, not assumed: `ls -d
  /sys/class/powercap/*` on the reference profile yields only intel-rapl,
  intel-rapl:0 (package-0) and intel-rapl:0:0 (core). A negative test asserts
  that a request for a dram or psys zone fails explicitly rather than silently
  returning zero (D60).
- The privileged-read path is recorded: energy_uj is mode 0400 (CVE-2020-8694
  mitigation), so the criterion names whether the reader runs as root, holds
  CAP_DAC_OVERRIDE, or uses a privileged daemon — and an unprivileged read is a
  negative test that must fail.
- Accuracy is stated honestly in the retained evidence: AMD RAPL is a
  model-based estimate derived from activity counters, not a measured power
  rail, so only same-zone deltas are treated as trustworthy and absolute watts
  carry vendor-defined error.
- One criterion states that a pass on the reference profile is development
  evidence only: it does not qualify hardware, does not close the hardware gate,
  and is not release evidence; re-measurement inside an Aegis image remains a
  later acceptance.
- The energy counter read is privileged:
  /sys/class/powercap/intel-rapl:0/energy_uj is mode 0400 on the reference
  profile, so the measurement runs as root and the unprivileged gate records
  only that the counter exists

Cheapest exit: Read powercap counters and boot one Firecracker microVM on the
workstation. No Aegis image and no GPU.

Epics:

- **E21-1 RAPL-backed carbon telemetry**. Requirements: REQ-P13-02, REQ-P13-01.
  Acceptance: Positive: SCI is computed from measured energy. Negative:
  unreadable counters fail closed with an error, not a default value. Boundary:
  counter wraparound between two samples yields a correct positive delta.
- **E21-2 Firecracker and AF_VSOCK sandboxing**. Requirements: REQ-P10-01,
  REQ-P10-03, REQ-P16-04. Acceptance: Positive: a microVM boots and the
  candidate evaluation round-trips over AF_VSOCK. Negative: a microVM request
  above the memory limit is refused. Boundary: the 64th microVM on real KVM is
  accepted and the 65th refused.
- **E21-3 Venus GPU pricing deferred**. Requirements: REQ-P10-02. Acceptance:
  Recorded as deferred to M12. It is not claimed here.

### M24 - Local boot harness over an externally supplied artifact

Rank 15. State: ready. Cost: large. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: yes. Needs external contract: no. Reference profile: full.
Blocked by: M15. Unblocks: M11.

Split from M11 after the reference profile was recorded, so a locally verifiable
slice no longer waits behind one that is not.

Exit criteria:

- Toolchain admission before any boot: QEMU 11.1.1, the edk2 OVMF images present
  on the reference profile (OVMF_CODE.4m.fd, OVMF_VARS.4m.fd,
  OVMF_CODE.secboot.4m.fd) and swtpm 0.10.2 are selected through the template
  matrix with pinned versions
- The harness is parameterised over an externally supplied bootable artifact: a
  pinned upstream distribution image or an Imago return. Aegis constructs no
  image here; image construction stays with cordanaLLM/imago per
  docs/integration/stack.md
- A swtpm instance is attached to the guest and PCR 0, 4, 7 and 11 are read back
  from inside the booted guest and retained; the reading records that host
  Secure Boot is disabled, so PCR 7 documents the firmware state rather than
  attesting a trusted chain
- Positive: the supplied artifact boots headless under QEMU with KVM and the
  harness captures the console log, the PCR values and the exit status
- Negative: an artifact whose digest does not match the pinned value is refused
  before boot, and a guest that fails to reach the login prompt within the
  recorded timeout fails the harness
- Boundary: the harness is exercised at its timeout, one second under and one
  second over, and reports the two outcomes differently
- A pass here is development evidence on the reference profile. It closes no
  image, boot, hardware or release gate, and it is not evidence for M11

Cheapest exit: Boot a pinned upstream image headless under QEMU with OVMF and
swtpm, and retain the console log and PCR readback. No image is constructed.

Epics:

- **E24-1 Imago result consumed and verified**. Requirements: REQ-P01-01,
  REQ-P01-06, REQ-P01-08. Acceptance: Positive: digest and signature verify
  locally. Negative: a tampered image digest or bad signature is rejected.
  Boundary: a producer version exactly at the floor is accepted, and one below
  is rejected.
- **E24-2 Real boot evidence replaces the simulated script**. Requirements:
  REQ-BOOT-01, REQ-BOOT-02, REQ-P01-05, REQ-P02-02, REQ-P02-01. Acceptance:
  Positive: PCR values are read back from swtpm, and the verity root hash
  matches. Negative: a modified root image fails verity and does not boot to the
  established state. Boundary: a PCR policy that omits PCR 11 fails to unseal
  /var. The imported script is not used.

### M04 - UI accessibility harness: P12 Concordia tokens

Rank 16. State: ready. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: no. Needs external contract: no. Reference profile:
not-hardware. Blocked by: M02. Unblocks: M16.

Exit criteria:

- Toolchain admission: Node, pnpm, Playwright browsers and Svelte are selected
  through the template matrix with pinned versions (D10: latest stable versions
  at activation, fast adoption through Renovate, sveltesentio adopted once
  mature) before any UI gate runs; package manifests and lockfiles are committed
  for ui/concordia-tokens
- The Playwright and axe-core suite runs headless in a container with zero
  violations on the default state; it fails when the focus outline is removed;
  the focus width and contrast boundary follows D16
- D17 (Bootstrap fork versus no monolithic CSS) is recorded; no `|| true` in the
  accessibility gate
- The admitted Node, pnpm, Playwright and Svelte versions are committed as
  lockfiles and the admission explicitly resolves the M01 drift-register row:
  the reference profile runs Node v26.8.2 and pnpm 10.29.3, neither of which is
  the register's Node 20 or Node 22, so the row is closed with an exact pin and
  not inherited (D65).
- The Playwright browser revision is pinned to the version actually exercised;
  the reference profile has chromium-1228 cached, and the gate must fail rather
  than silently download a different revision.
- The Node major is the latest release line (26.x on the reference profile) and
  is tracked forward by Renovate rather than pinned to an older line (D65)

Cheapest exit: Build the token file plus one Svelte component and run the axe
suite in a container. No compositor and no daemons.

Epics:

- **E04-1 UI toolchain admission and lockfiles**. Requirements: REQ-UI-01,
  REQ-CI-03, REQ-P12-08, REQ-CI-02. Acceptance: Positive: the pinned Node and
  pnpm versions install from the lockfile. Negative: a lockfile mismatch fails
  `pnpm install --frozen-lockfile`. Boundary: the gate fails on the first
  violation and is not suppressed.
- **E04-2 Concordia tokens and axe-core harness**. Requirements: REQ-P12-01,
  REQ-P12-05, REQ-P12-06, REQ-P05-06, REQ-UI-02, REQ-P12-02, REQ-P12-03,
  REQ-P12-04, REQ-P12-07, REQ-P12-09. Acceptance: Positive: zero violations on
  the default state. Negative: a stripped outline fails. Boundary: the focus
  width chosen by D16 passes, one pixel less fails; 3:1 contrast passes and
  2.99:1 fails; 200% text scaling causes no overflow. The truncated container
  digest is recorded as non-pinnable.

### M16 - P05 Forum shell state and lifecycle with stubbed IPC

Rank 17. State: blocked. Cost: small. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: no. Needs external contract: no. Reference profile:
not-hardware. Blocked by: M04, M14. Unblocks: nothing.

Exit criteria:

- ui/forum-shell has its package manifest and lockfile on the toolchain admitted
  in M04
- The shell state and typed process lifecycle are unit-tested with compositor,
  Justitia and Tellus inputs stubbed
- The DecisionRequest consumer is typed against the M14 schema; the P05/P12
  token-edge direction (D05) is recorded

Cheapest exit: Unit-test the Svelte store and lifecycle logic with mocked socket
and D-Bus connections.

Epics:

- **E16-1 Forum shell state and lifecycle with stubs**. Requirements:
  REQ-P05-01, REQ-P05-02, REQ-P05-03, REQ-P05-04, REQ-P05-05, REQ-P05-07,
  REQ-P05-08, REQ-P04-07. Acceptance: Positive: each lifecycle transition in the
  source order succeeds. Negative: a transition from Deleted is rejected.
  Boundary: Rate-Limited to Quarantined at the limit is exercised exactly.
- **E16-2 Consumer contracts and token-edge decision**. Requirements:
  REQ-GRAPH-01, REQ-P13-04, REQ-P06-08. Acceptance: Positive: DecisionRequest
  and Tellus telemetry payloads parse. Negative: an unknown schema version is
  rejected. Boundary: a telemetry update with zero watts renders without error.

### M25 - GPU DMA-BUF sharing and VFIO passthrough slices

Rank 19. State: blocked. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: yes. Needs external contract: no. Reference profile: full.
Blocked by: M08, M17. Unblocks: M12.

Split from M12 after the reference profile was recorded, so a locally verifiable
slice no longer waits behind one that is not.

Exit criteria:

- Toolchain admission: QEMU 11.1.1 and the vfio-pci and vfio_iommu_type1 modules
  are selected through the template matrix with pinned versions before any
  passthrough run
- DMA-BUF export and import is demonstrated end-to-end with measured transfer
  values replacing the scaffold literals, across at least two of the three
  vendor drivers, using the unprivileged path: /dev/udmabuf and PRIME export and
  import through the render nodes
- The privileged DMA-BUF heap path (/dev/dma_heap/system, root-only on the
  reference profile) is recorded separately and never gates an unprivileged run
- drm_sched is recorded as measured, not assumed: amdgpu is the drm_sched
  provider on the reference profile, i915 is not, and the Intel card serves
  drm_sched only if it is rebound to the xe driver, which is recorded as a
  decision before it is relied upon
- VFIO BAR mapping is demonstrated by binding a GPU that is the sole occupant of
  its IOMMU group and that has no connected display output at bind time, both
  verified and recorded
- Resizable BAR state is recorded as measured (BAR1 current size on the discrete
  card) rather than assumed
- A pass here is development evidence on the reference profile and closes no
  hardware gate

Cheapest exit: Demonstrate DMA-BUF export and import across two of the three
vendor drivers and bind one GPU through VFIO, replacing scaffold literals with
measured values.

Epics:

- **E25-1 VFIO passthrough of a single-occupant IOMMU group**. Requirements:
  REQ-P03-03, REQ-P03-06. Acceptance: Positive: the BARs of the group-isolated
  device map into a guest. Negative: a device sharing an IOMMU group with
  another function is refused. Boundary: a device with a connected display
  output is refused at bind time.
- **E25-2 DMA-BUF sharing across vendor drivers**. Requirements: REQ-P08-02,
  REQ-P04-06. Acceptance: Positive: a buffer exported by one driver imports into
  another and the measured transfer replaces the scaffold literal. Negative: an
  import with a mismatched modifier is rejected. Boundary: the smallest and the
  largest supported allocation both round-trip.

### M22 - P10 microVM sandbox measurements on KVM

Rank 20. State: blocked. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: yes. Needs external contract: no. Reference profile: full.
Blocked by: M06. Unblocks: nothing.

Split from M21 after the reference profile was recorded, so a locally verifiable
slice no longer waits behind one that is not.

Exit criteria:

- D58 is decided: Firecracker is the sandbox VMM, pinned through the template
  matrix before any microVM run (1.17.0 installed on the reference profile,
  currently absent) or QEMU 11.1.1 with the microvm machine type and
  MICROVM.4m.fd (already present).
- Every measured boot time and memory footprint that replaces a scaffold literal
  records which VMM produced it; Firecracker and QEMU microvm numbers are never
  presented as interchangeable.
- One AF_VSOCK candidate evaluation round-trips over the measured transport,
  with /dev/vhost-vsock and the CONFIG_VHOST_VSOCK=m module recorded as the
  mechanism.
- Verify against the pinned release before relying on it: the sandbox VMM's
  device passthrough limits are read from the pinned release documentation
  rather than assumed
- One criterion states that a pass on the reference profile is development
  evidence only: it does not qualify hardware, does not close the hardware gate,
  and is not release evidence.
- A pass here is development evidence on the reference profile and closes no
  hardware gate
- Firecracker 1.17.0 and its jailer are installed on the reference profile; the
  pinned version is recorded at activation
- Firecracker has no PCI passthrough, so passthrough work stays with the
  QEMU-based harness (D58)

Cheapest exit: Run the candidate evaluation sandbox on the admitted VMM and
record measured boot time, footprint and one AF_VSOCK round trip.

Epics:

- **E22-1 RAPL-backed carbon telemetry**. Requirements: REQ-P13-02, REQ-P13-01.
  Acceptance: Positive: SCI is computed from measured energy. Negative:
  unreadable counters fail closed with an error, not a default value. Boundary:
  counter wraparound between two samples yields a correct positive delta.
- **E22-2 Firecracker and AF_VSOCK sandboxing**. Requirements: REQ-P10-01,
  REQ-P10-03, REQ-P16-04. Acceptance: Positive: a microVM boots and the
  candidate evaluation round-trips over AF_VSOCK. Negative: a microVM request
  above the memory limit is refused. Boundary: the 64th microVM on real KVM is
  accepted and the 65th refused.

### M26 - Aegis-built kernel with the pinned configuration

Rank 7. State: done. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: no. Needs external contract: no. Reference profile: full.
Blocked by: M18. Unblocks: M23.

Exit criteria:

- D70 applies: Aegis builds the kernel here while Nucleus is a scaffold, exactly
  as D56 keeps image-definition validation here while Imago is a scaffold. The
  build moves to Nucleus when Nucleus returns real artifacts, and this
  milestone's output is never a release artifact
- Toolchain admission: the compiler, make, bc, flex, bison, pahole and the
  archive tools are selected through the template matrix with the
  reference-profile versions recorded
- The kernel source is pinned by version and digest, and the configuration is
  expressed as a tracked fragment applied to a named base configuration, not as
  a full copied .config
- The fragment sets exactly what the M18 schema requires and the build reads it
  back: CONFIG_PREEMPT_RT, the timer frequency, CONFIG_SCHED_CLASS_EXT,
  CONFIG_BPF_LSM and CONFIG_DEBUG_INFO_BTF are confirmed in the produced .config
  rather than assumed
- Positive: the build produces a bootable image and the guest reports the
  required options from inside itself. Negative: a fragment that contradicts a
  required option fails the build gate rather than producing a silently
  non-conforming kernel. Boundary: a required option set as a module where the
  schema demands built-in is rejected
- A pass here is development evidence on the reference profile. It closes no
  boot, hardware or release gate, and it is not a substitute for the Nucleus
  contract in M09

Cheapest exit: Apply a configuration fragment to a pinned upstream source,
build, and read the produced configuration back from inside a guest. No
packaging, no signing, no release.

Evidence:

- D70 applied, and the limit stated where the claim is made: this milestone
  builds a kernel in Aegis because Nucleus is a scaffold, exactly as D56 keeps
  image-definition validation here while Imago is a scaffold. What was produced
  is a bzImage and 19 modules in a build tree outside the repository, plus a
  guest that read the configuration back. Nothing was packaged, signed,
  installed, written to a device or published; no bootloader entry exists; no
  module was loaded on the host. Construction returns to Nucleus, which owns it
  under docs/integration/stack.md, once Nucleus returns real artifacts against
  the M09 contract
- The source is pinned by version and digest and verified two ways on every
  run, not once by hand: build/kernel/source.pin.json pins linux-7.2.5 with
  sha256 55ddf0df8325d9dad96fcff7bd93977d22e3f50af06527572af59b77c7632b78,
  which is the value the published sha256sums.asc for v7.x lists and the value
  sha256sum prints for the downloaded tarball; sha256sums.asc itself verifies
  as 'Good signature' from key B8868C80BA62A1FFFAF5FDA9632D3A06589DA6B1
  (Kernel.org checksum autosigner); and `xz -dc linux-7.2.5.tar.xz | gpg
  --verify linux-7.2.5.tar.sign -` verifies as 'Good signature from "Greg
  Kroah-Hartman <gregkh@linuxfoundation.org>"', primary key
  647F28654894E3BD457199BE38DBBDC86092693E. The gate refuses a source whose
  signature is good but made by a key other than the pinned fingerprint.
  Neither key is certified by a local trust path, so gpg also prints its usual
  untrusted-key warning: the pin asserts the fingerprint, not a web of trust
- The configuration is a fragment on a named base, never a copied .config. The
  base is x86_64_defconfig of the pinned source;
  build/kernel/10-base-support.config and
  build/kernel/50-aegis-requirement.config are applied in that order by the
  kernel's own scripts/kconfig/merge_config.sh -m, then resolved with `make
  olddefconfig`. The produced .config is 5,520 lines and is not tracked
- The requirement fragment is generated by the M18 producer rather than
  written: build/kernel/50-aegis-requirement.config is byte-identical to what
  KernelRequirement::config_fragment renders from
  build/kernel-requirement.json, and
  crates/aegis-fabrica-defs/tests/kernel_fragment.rs fails on a one-byte
  difference. Two further bindings keep that honest: the fragment assigns
  exactly the thirteen payload symbols and no others, and
  10-base-support.config assigns none of them, so a hand-written file cannot
  satisfy a schema row while the generated fragment says nothing. Every line of
  the support fragment names the Kconfig dependency it satisfies, read out of
  the pinned source: PREEMPT_RT 'depends on EXPERT && ARCH_SUPPORTS_RT &&
  !COMPILE_TEST' in kernel/Kconfig.preempt with ARCH_SUPPORTS_RT selected by
  arch/x86/Kconfig, so no out-of-tree realtime patch is involved
- Build, on the reference profile's 32 threads: `make O=<build>/out/positive
  -j32 bzImage modules` exits 0 in 69 s from a freshly configured tree and
  prints 'Kernel: arch/x86/boot/bzImage is ready  (#1)'. The artifacts are a
  16,438,272-byte bzImage and 19 modules including vfio.ko, vfio-pci.ko,
  vfio_iommu_type1.ko, intel_rapl_common.ko and intel_rapl_msr.ko, so the
  payload's module-state rows are compiled and not only configured. One row is
  honest about a limit: CONFIG_KVM=m is carried by the produced configuration,
  which is what the row asks about, but no kvm.ko is built because
  arch/x86/kvm/Kconfig makes the built module KVM_X86, 'def_tristate KVM if
  (KVM_INTEL != n || KVM_AMD != n)', and x86_64_defconfig selects neither
  vendor module
- Positive read-back, from inside the guest and not from the build tree:
  `qemu-system-x86_64 -enable-kvm -cpu max -kernel <bzImage> -initrd <cpio>`
  boots, and the guest's /init mounts its own /proc and prints
  'AEGIS-M26-UNAME-R 7.2.5-aegis-m26', 'AEGIS-M26-UNAME-V #1 SMP PREEMPT_RT Sun
  Sep 13 18:32:20 CEST 2026' (the build timestamp moves with every rebuild; the
  release string does not) and the whole of its own /proc/config.gz, which
  exists because CONFIG_IKCONFIG_PROC is set. Read out of that dump:
  CONFIG_PREEMPT_RT=y, CONFIG_HZ_1000=y with CONFIG_HZ=1000,
  CONFIG_SCHED_CLASS_EXT=y, CONFIG_BPF_LSM=y, CONFIG_BPF_SYSCALL=y,
  CONFIG_DEBUG_INFO_BTF=y, CONFIG_CGROUP_BPF=y, CONFIG_POWERCAP=y,
  CONFIG_IOMMU_API=y, CONFIG_INTEL_RAPL=m, CONFIG_VFIO=m, CONFIG_VFIO_PCI=m and
  CONFIG_KVM=m -- all thirteen rows. The guest reports on a second serial line
  so a kernel printk cannot splice itself into the dump, and then powers itself
  off through magic SysRq ('reboot: Power down'), so the gate's 180 s deadline
  is a failure signal rather than the normal exit path
- The read-back demonstrably runs on the built artifact. Three claims are
  checked together: the release the guest reports equals the one this build
  produced (include/config/kernel.release, 7.2.5-aegis-m26, from
  CONFIG_LOCALVERSION="-aegis-m26" with LOCALVERSION_AUTO off) and is not the
  host's 7.2.4-1-cachyos; every requirement row holds in the text the guest
  printed; and that text is the produced .config byte for byte, all 5,520
  lines. Negative: the same check over the reference host's own /proc/config.gz
  is refused with 'CONFIG_PREEMPT_RT: REQ-P07-01 requires built-in, observed
  'n''. Boundary: the host's release through the identity check is refused
  twice, 'not the built release' and 'the guest reports the host's own
  release', so a read-back cannot be satisfied by the machine the gate is
  running on. The two host-side cases are independent units: the boundary one
  reads only os.uname(), so it runs and is counted even on a kernel that
  publishes no /proc/config.gz, where the negative one alone is skipped by
  name, and a run that failed both counts two failures rather than one
- Negative and boundary on the configuration itself, each a real kconfig round
  trip in its own object tree. Appending '# CONFIG_PREEMPT_RT is not set' is
  refused with 'CONFIG_PREEMPT_RT: REQ-P07-01 requires built-in, observed 'n''.
  Appending 'CONFIG_VFIO=y' where the schema demands a module is refused with
  'CONFIG_VFIO: REQ-P03-06 requires module, observed 'y''. Appending
  'CONFIG_BPF_SYSCALL=m' where the schema demands built-in produced the finding
  this gate exists for: every symbol the payload demands built-in is a bool in
  the pinned source, so =m is unreachable, and kconfig neither honours the line
  nor complains -- it resolves the symbol to n. The fragment therefore yields a
  kernel without the feature rather than one carrying it as a module, and
  CONFIG_BPF_LSM and CONFIG_SCHED_CLASS_EXT fall with it. All three are refused
  by name; nothing in the build itself says so
- Toolchain admission, every version read back from the tool rather than from
  memory and every floor taken from the pinned source's own
  scripts/min-tool-version.sh and Documentation/process/changes.rst: gcc 16.2.1
  (floor 8.1.0), ld 2.47 (2.30), make 4.4.1 (4.0), bc 1.08.2 (1.06.95), flex
  2.6.4 (2.5.35), bison 3.8.2 (2.0), pahole 1.31 (1.26), tar 1.35 (1.28), bash
  5.3.15 (4.2), mount 2.42.3 (2.10), and perl 5.42.2, cpio 2.15, xz 5.8.3, gzip
  1.14, gpg 2.4.9 and qemu-system-x86_64 11.1.1 with no floor declared by the
  source. docs/roadmap/toolchain-admission.md carries the rows and
  tools/test_kernel_build.py reads that table back: the tool names are compared
  as sets, and each row's reference value and its Floor cell are compared
  against the gate's own TOOLCHAIN entry. A row present on one side only, or a
  floor edited on the page alone, fails make verify-all; both holes were
  probed, with a phantom ccache row for a tool the gate never runs and with the
  gcc floor rewritten to 99.0.0 while TOOLCHAIN still said 8.1.0, and each is
  now a failure. clang 22.1.8 is installed and deliberately not admitted: the
  gate builds with gcc
- Gate placement, stated rather than assumed: the gate is `make verify-kernel`
  (tools/verify_kernel_build.py, seven cases, no '|| true' anywhere) and make
  verify-all does NOT invoke it. Reasons recorded on the target itself: a full
  run downloads 160 MB, extracts 1.4 GB and compiles a kernel (91 s wall on
  this profile with the source already present), and the CI runner has no
  kernel toolchain, no /dev/kvm and no emulator, so wiring it in would add a
  step that can only skip. CI therefore never compiles a kernel and never boots
  one. What CI does re-run is the binding that matters everywhere:
  crates/aegis-fabrica-defs/tests/kernel_fragment.rs (7 cases) fails if the
  tracked fragment stops being what the M18 schema renders, and
  tools/test_kernel_build.py (30 cases) fails if the pin, the fragments or the
  recorded toolchain drift apart. A host that cannot run the gate has two
  outcomes and they are not the same claim. A tool that is missing or below the
  floor the pinned source declares prints a 'SKIP: ...; the kernel build gate
  did not run.' line and exits 0 -- observed with pahole off PATH, where make
  verify-kernel also exits 0 -- so an exit 0 from this target is evidence only
  when the case lines are above it; that SKIP-exits-0 idiom is the repository's
  existing convention, shared with verify-systemd and verify-mkosi. A host that
  has the toolchain but cannot obtain the pinned source, with no network and no
  cached tarball, instead prints 'FAIL: the gate could not run: ... could not
  be downloaded' and exits 1 -- observed with a curl that cannot resolve
  cdn.kernel.org -- because an unverifiable source is a refusal rather than a
  skip
- HISS-02 on the gate's own external commands, counted rather than asserted: an
  AST sweep over tools/verify_kernel_build.py finds 15 external call sites and
  15 of them carry a deadline. Fourteen are run(argv, deadline) or
  subprocess.run(timeout=...); the fifteenth is the `xz --decompress --stdout`
  that feeds gpg during signature verification, a subprocess.Popen, which has
  no timeout parameter at all, so its bound is structural -- a try/finally
  kills the child on every path out of the body. Without that finally a gpg
  that exceeds EXTRACT_TIMEOUT raises past the kill and the Popen context
  manager's own exit waits on the still-running decompressor with no bound.
  Reproduced with a PATH shim (xz ignoring SIGPIPE and holding for 120 s, gpg
  replaced by sleep, EXTRACT_TIMEOUT patched to 3 s): the gate's own deadline
  fired at 3 s, the GateError never surfaced, and the process was still blocked
  in that context-manager exit, inside an unbounded wait, when an outer timeout
  killed it 42 s later at exit 124. With the finally in place the same shim
  fails closed in 3.0 s with 'gpg exceeded its 3s deadline' and leaves no child
  behind. tools/test_kernel_build.py covers the good-signature,
  refused-signature and expired-deadline paths
- Gates re-run on the reference profile, each exit 0: `cargo fmt --check`,
  `cargo build --locked`, `cargo test --locked --all-features`, `cargo clippy
  --locked --all-targets --all-features -- -D warnings`, `RUSTDOCFLAGS='-D
  warnings' cargo doc --locked --no-deps`, `make verify-all`, `make
  verify-kernel`, `python3 tools/rank_roadmap.py`, `python3 -B -m unittest
  discover -s tools -p 'test_*.py'`, `npx markdownlint-cli2@0.23.2`, `yamllint
  .`, `flake8 .`, `black --check --line-length 100 tools .config/agent/hooks`,
  and `git status --porcelain Cargo.lock` empty
- Scope: development evidence on the reference profile recorded in
  planning/hardware-profile.json. It is not a substitute for the Nucleus
  contract in M09, which pins one request/result pair against a real producer.
  No boot gate is closed: the guest boot is a configuration read-back, not a
  measured boot, with no UKI, no Secure Boot, no TPM measurement and no bootctl
  evidence. No latency, jitter or determinism figure is claimed --
  CONFIG_PREEMPT_RT is confirmed as a configuration state and uname -v reports
  PREEMPT_RT, and measuring what that buys is M23, which stays blocked because
  its other blocker M07 is ready rather than done. The guest's userspace is the
  host's own bash, mount, uname, gzip and sleep with their library closure;
  only the kernel under test is built here. The image, kernel-artifact, boot,
  hardware and release gates remain blocked

Epics:

- **E26-1 Pinned source and configuration fragment**. Requirements: REQ-P07-01,
  REQ-P01-09. Acceptance: Positive: the fragment applies to the pinned base and
  the produced configuration carries every required option. Negative: a
  contradictory fragment fails the gate. Boundary: an option set as a module
  where the schema demands built-in is rejected.
- **E26-2 Guest boot and configuration read-back**. Requirements: REQ-P07-02,
  REQ-P08-01. Acceptance: Positive: the guest boots and reports the required
  options from inside itself. Negative: a kernel missing a required option is
  refused by the read-back check. Boundary: the read-back runs on the exact
  built artifact, not on the host kernel.

### M23 - P07 and P08 latency fixtures on a realtime kernel guest

Rank 14. State: blocked. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: yes. Needs external contract: no. Reference profile: full.
Blocked by: M07, M26. Unblocks: M10.

Split from M07 after the reference profile was recorded, so a locally verifiable
slice no longer waits behind one that is not.

Exit criteria:

- Toolchain admission: QEMU 11.1.1 and the realtime kernel package are selected
  through the template matrix with pinned versions
- D70 is applied: the guest kernel is an interim source for the latency fixture
  only. the guest kernel is the one this repository builds in M26 while Nucleus
  is a scaffold, with its configuration read back from inside the guest; the
  kernel the product ships is built by Nucleus against the M18 schema once
  Nucleus is real
- D57 is recorded with the realtime kernel obtained without modifying the
  reference host: the distribution package is downloaded only (no installation,
  no bootloader entry) and its kernel image and modules are passed to the guest
- The guest kernel's configuration is read back from inside the virtual machine
  and confirms CONFIG_PREEMPT_RT, while the same probe on the reference host
  confirms it is not set there
- Positive: the latency fixture produces measured figures for the tier
  thresholds on the guest. Negative: the same fixture on the non-realtime host
  is recorded as not satisfying the determinism claim. Boundary: a run at the
  threshold and one step beyond it are reported differently
- A pass here is development evidence on the reference profile and closes no
  hardware gate

Cheapest exit: Boot the distribution realtime kernel in a guest and run the
latency fixture there, recording that the reference host itself is not realtime.

Epics:

- **E23-1 Compositor registry and Tier-1 socket bounds**. Requirements:
  REQ-P04-04, REQ-P04-05, REQ-P04-07, REQ-P04-08. Acceptance: Positive: 256
  surfaces and 64 clients are accepted. Negative: the 257th surface is rejected
  and the 65th client is not admitted. Boundary: a pacing-constant test pins the
  chosen value.
- **E23-2 wlroots and mesh design decision**. Requirements: REQ-P04-01,
  REQ-P04-02, REQ-P04-03, REQ-P04-06. Acceptance: The decision is recorded. The
  Zenoh version is selected at activation after checking current upstream, with
  a mocked transport test: a publish round-trips, a subscriber cannot write
  back, and an empty key expression is rejected.

### M09 - Cross-repository contract pin: one local request/result pair

Rank 21. State: ready. Cost: small. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: no. Needs external contract: yes. Reference profile: partial.
Blocked by: M18. Unblocks: M11, M10.

Exit criteria:

- The M18 schemas are proposed to cordanaLLM/imago and cordanaLLM/nucleus;
  acceptance is recorded only when each producer consumes the payload, not a
  fixed symbol list
- One request/result pair is executed locally against pinned local imago and
  nucleus checkouts, with a positive result, a negative result (rejected payload
  with a correlated error) and a boundary result (empty requirement list
  rejected explicitly)
- Canonical producer identities are confirmed, or the unresolved GitHub
  identities (cordanaLLM/imago and cordanaLLM/nucleus did not resolve at
  revision time) and the non-canonical identities in builder workflows are
  recorded as the blocker
- Simulated output is not accepted as a result; no hosted dispatch is claimed
- Until the producer repositories exist, the producer-side schemas, fixtures and
  results stay tracked in Aegis as dogfooding input for creating them; nothing
  is published to cordanaLLM/imago or cordanaLLM/nucleus from here
- The unresolved producer identities are recorded as the blocker with the
  verifying command and its output retained verbatim: `git ls-remote --heads
  https://github.com/cordanaLLM/imago.git` and the same for nucleus, both
  returning 'Repository not found' at the reference-profile probe date.
- The local checkout commits actually exercised are pinned in the evidence
  (imago 4f116fc, nucleus 78ca8f2 as observed), so the dogfooding run is
  reproducible and is never mistaken for hosted acceptance.

Cheapest exit: Run the pair against the pinned local checkouts without any
hosted dispatch.

Epics:

- **E09-1 Imago consumption of the product input manifest**. Requirements:
  REQ-P01-01, REQ-P01-02, REQ-P01-03, REQ-P01-04, REQ-P01-06, REQ-P01-10.
  Acceptance: Positive: an accepted request returns image digest, signature
  reference and boot-evidence fields. Negative: a malformed manifest is rejected
  with a correlated error. Boundary: a retry at the bound is recorded, and one
  above is refused.
- **E09-2 Nucleus consumption of the kernel requirement payload**. Requirements:
  REQ-P01-09, REQ-P07-01, REQ-P06-05, REQ-P13-02. Acceptance: Positive: a result
  returns kernel version/config digest, artifact digest and provenance.
  Negative: an unsatisfiable feature is rejected with a correlated error.
  Boundary: an empty requirement list is rejected explicitly.
- **E09-3 Pair evidence and identity status**. Requirements: REQ-BOOT-02,
  REQ-GOV-02. Acceptance: The retained pair shows correlation id and exact
  revisions. Simulated output is refused. Identity status is recorded.

### M11 - Minimal image build with artifact, signature and boot evidence

Rank 22. State: blocked. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: yes. Needs external contract: yes. Reference profile: partial.
Blocked by: M09, M24. Unblocks: M13, M20, M12.

Exit criteria:

- BLOCKED until: Imago accepts the M09 manifest and returns an image/UKI with
  digest and signature. The kernel is the pinned distribution linux-rt package
  from the M18 manifest unless a Nucleus artifact is already available (D07)
- Toolchain admission: QEMU, OVMF, swtpm and the signing tools (cosign or
  sbsign) are selected through the template matrix with pinned versions before
  any boot or signing gate
- Unblocking evidence: retained image output digest, signature verification log,
  and a QEMU/OVMF/swtpm boot log on a KVM-capable host with real PCR 0/4/7/11
  readback and dm-verity/LUKS2 unlock records
- Scope: the minimal image carries no UI and no kernel-attached eBPF programs
  (D20); the accessibility gate attaches when the UI enters an image, and eBPF
  objects enter through M10 and M12
- The A/B sysupdate transfer is exercised once between root-a and root-b, and
  observed transitions are compared with the M15 state machine
- The M24 harness is reused unchanged and the only new input is the Imago
  artifact; a criterion states that no second boot apparatus is built here.
- The artifact's digest and signature are verified with the pinned cosign (2.6.3
  on the reference profile) before the boot runs, and a tampered-digest negative
  case is exercised.
- The Secure Boot position is restated explicitly so it cannot be lost between
  milestones: a QEMU/OVMF boot proves the image boots, not that it boots signed,
  unless the guest VARS store was enrolled per D62; the reference profile's host
  firmware cannot verify it (SecureBoot 0, SetupMode 0).
- One criterion states that one boot on one developer workstation is development
  evidence only: it does not qualify hardware, does not close the hardware or
  release gate, and the retained logs must say so on their face.

Cheapest exit: No cheaper exit exists: this is the first real artifact. Keep it
to one image and one boot, and retain every log.

Epics:

- **E11-1 Imago result consumed and verified**. Requirements: REQ-P01-01,
  REQ-P01-06, REQ-P01-08. Acceptance: Positive: digest and signature verify
  locally. Negative: a tampered image digest or bad signature is rejected.
  Boundary: a producer version exactly at the floor is accepted, and one below
  is rejected.
- **E11-2 Real boot evidence replaces the simulated script**. Requirements:
  REQ-BOOT-01, REQ-BOOT-02, REQ-P01-05, REQ-P02-02, REQ-P02-01. Acceptance:
  Positive: PCR values are read back from swtpm, and the verity root hash
  matches. Negative: a modified root image fails verity and does not boot to the
  established state. Boundary: a PCR policy that omits PCR 11 fails to unseal
  /var. The imported script is not used.
- **E11-3 Analyzer gate without suppression in the image path**. Requirements:
  REQ-CI-01. Acceptance: Positive: the M03 gate passes on the image inputs.
  Negative: an ignored-key diagnostic fails the image acceptance. Boundary: no
  step carries failure suppression.
- **E11-4 A/B transfer exercised**. Requirements: REQ-P01-04, REQ-P02-04,
  REQ-P02-08. Acceptance: Positive: the second slot is written read-only and
  boots. Negative: a transfer with a bad signature is discarded. Boundary:
  watchdog expiry before bless rolls back once.

### M10 - eBPF objects re-verified against the Nucleus-pinned kernel in a VM

Rank 23. State: blocked. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: yes. Needs external contract: yes. Reference profile: partial.
Blocked by: M09, M19, M23. Unblocks: M12.

Exit criteria:

- BLOCKED until: the M09 pair delivers a Nucleus kernel whose version/config
  digest is recorded
- Hardware: a KVM-capable host is required to boot the Nucleus kernel in a VM
  (KVM was present on the workstation at revision time; that is not qualifying
  evidence by itself)
- Toolchain admission: QEMU is selected through the template matrix with a
  pinned version (shared with M11 if M11 lands first)
- action_gate, scx_cake and kepler_power from M19 load through the verifier on
  that kernel; sched_ext, BPF LSM and BTF availability are read from its config
- The M23 direct-kernel-boot harness is reused with only the kernel image and
  initramfs substituted; no second harness is built.
- sched_ext, BPF LSM and BTF availability are read from the Nucleus kernel's own
  config inside the VM, and each value is diffed against the reference profile's
  host value (CONFIG_SCHED_CLASS_EXT=y, bpf in the active LSM list,
  /sys/kernel/btf/vmlinux present) so that a capability the host happened to
  supply cannot be silently assumed of the kernel under test.
- One criterion restates that the M19 host-kernel fixture does not close this
  milestone and that a pass here on the reference profile's VM is development
  evidence only, closing neither the hardware nor the release gate.

Cheapest exit: Boot the Nucleus kernel directly in QEMU/KVM with a minimal
initramfs and repeat the M19 loads. The M19 host-kernel fixture does not close
this milestone.

Epics:

- **E10-1 action_gate on the Nucleus kernel**. Requirements: REQ-P06-05,
  REQ-P07-06. Acceptance: Positive: the program loads and attaches. Negative:
  the unchecked-pointer variant is rejected. Boundary: BPF LSM absent from the
  kernel config fails the milestone with a recorded reason.
- **E10-2 scx_cake on the Nucleus kernel**. Requirements: REQ-P07-04,
  REQ-P07-01. Acceptance: Positive: struct_ops attaches. Negative: the unbounded
  variant is rejected. Boundary: a kernel without sched_ext is recorded as a
  Nucleus requirement defect.
- **E10-3 kepler_power on the Nucleus kernel**. Requirements: REQ-P13-02.
  Acceptance: Positive: the tracepoint attaches. Negative: a missing tracepoint
  is reported, not ignored. Boundary: zero energy delta over an idle interval is
  recorded as a value, not an error.

### M20 - TPM2 attestation slice on swtpm: audit-record signing and /var unseal

Rank 24. State: blocked. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: yes. Needs external contract: yes. Reference profile: full.
Blocked by: M11, M14. Unblocks: nothing.

Exit criteria:

- BLOCKED until: an M11 image boots under QEMU with swtpm
- Hardware: a KVM-capable host with swtpm; a physical TPM2 is optional and
  recorded separately if used
- Toolchain admission: tpm2-tools (or the chosen TSS library) is selected
  through the template matrix with a pinned version
- Unblocking evidence: a real PCR quote from swtpm signs one M14 audit record,
  and /var unseals only under the enrolled PCR policy
- tpm2-tools 5.8 (or the chosen TSS library, e.g. a Rust tss-esapi binding) is
  pinned through the template matrix before any quote is taken; the reference
  profile has neither installed today.
- The swtpm quote path is recorded as pre-proven on the M24 harness, so this
  milestone exercises it against the real M11 image rather than debugging the
  mechanism for the first time.
- The PCR binding is chosen honestly and justified in the evidence: PCR 11 (the
  UKI) and PCR 0/4 are the defensible local subset, because on the reference
  profile Secure Boot is disabled and no guest keys are enrolled, so a PCR 7
  policy would attest to an unsigned configuration rather than a trusted boot
  policy.
- The emulated and the physical TPM2 are recorded as two distinct measurements
  and never conflated: a swtpm quote proves the code path, not a hardware root
  of trust.
- One criterion states that a pass on the reference profile is development
  evidence only: it does not qualify hardware, does not close the hardware or
  release gate, and attestation rooted in a firmware-verified boot chain is not
  claimed.
- The quote covers PCR 0, 4, 7 and 11, while the unseal policy binds PCR 0, 4
  and 11 only: on the reference profile Secure Boot is disabled, so a PCR 7
  policy would attest to that state rather than to a trusted chain (D63)

Cheapest exit: Use the M11 VM with swtpm. No physical TPM is required for this
milestone.

Epics:

- **E20-1 PCR quote signs an audit record**. Requirements: REQ-P06-06,
  REQ-P06-01, REQ-GOV-03. Acceptance: Positive: an audit record signed with the
  quote verifies. Negative: a record signed under a different PCR state fails
  verification. Boundary: a quote over exactly PCR 0/4/7/11 is accepted, and a
  quote missing PCR 11 is rejected.
- **E20-2 PCR-sealed /var unseal**. Requirements: REQ-P02-02, REQ-P02-03.
  Acceptance: Positive: /var unseals under the enrolled policy. Negative: a
  wrong PCR policy fails to unseal. Boundary: changing only PCR 7 (recorded in
  the quote, excluded from the unseal policy) is enough to prevent unseal.

### M12 - GPU-backed slices: DMA-BUF, VFIO and P2PDMA paths

Rank 25. State: blocked. Cost: large. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: yes. Needs external contract: yes. Reference profile: partial.
Blocked by: M25, M10, M11. Unblocks: nothing.

Exit criteria:

- BLOCKED until: a booted M11 image exists, the M10 kernel exposes VFIO/IOMMU,
  and a DMA-BUF-capable GPU with IOMMU groups is available on a recorded host
- Toolchain admission: the GPU driver stack and, if selected, the
  template-native-gpu boundary are pinned through the template matrix with an
  ABI mapping and one backend fixture, or recorded as not needed
- Unblocking evidence: per-path logs with measured values (DMA transfer,
  preemption and VRAM residency) replacing every hardcoded literal in the
  scaffolds
- The P2PDMA precondition is stated as a measured absence with its probe: `ls -d
  /sys/bus/pci/devices/*/p2pdma` returns no matches on the reference profile, so
  no p2pmem provider exists and this path cannot be evidenced here at any cost
  in effort.
- The hardware dependency is named concretely rather than as 'a suitable GPU': a
  CMB-capable enterprise NVMe controller or a datacentre-class GPU, recorded as
  a procurement item with the reason (GPUDirect Storage and RDMA are not offered
  on GeForce AD102).
- Per D59, every P03/P09 exit criterion phrased as 'CUDA GPUDirect' is either
  reframed to DMA-BUF plus host-mediated BAR transfer — which M25 evidences on
  this profile — or explicitly deferred to real datacentre hardware; no
  criterion is allowed to read as satisfied by the RTX 4090.
- The DMA-BUF and VFIO results from M25 are consumed as inputs and explicitly
  not re-derived; what this milestone adds is re-measurement inside a booted
  Aegis image plus the P2P path.
- One criterion states that any pass obtained on the reference profile is
  development evidence only and that the P2P scaffold literals stay literals
  until the named hardware exists.

Cheapest exit: Order the paths by hardware availability: DMA-BUF sharing first,
then VFIO BAR mapping, then P2PDMA.

Epics:

- **E12-1 DMA-BUF sharing paths**. Requirements: REQ-P08-01, REQ-P11-03,
  REQ-P15-02, REQ-P15-03, REQ-P08-04. Acceptance: Positive: one measured
  zero-copy transfer per path. Negative: an invalid DMA-BUF fd is rejected
  without a CPU copy fallback being counted as success. Boundary: the 64th
  buffer is accepted and the 65th refused on real hardware.
- **E12-2 VFIO and P2PDMA paths**. Requirements: REQ-P03-03, REQ-P03-06,
  REQ-P03-07, REQ-P09-04. Acceptance: Positive: one measured NVMe-to-GPU
  transfer. Negative: a device outside its IOMMU group is refused. Boundary: a
  transfer of exactly 8192 blocks succeeds, and 8193 is refused before dispatch.
- **E12-3 GPU template boundary**. Requirements: REQ-P10-02, REQ-P03-02.
  Acceptance: Positive: one backend fixture result is retained, or a not-needed
  decision is recorded. Negative: a Go library as a direct Rust dependency is
  rejected. Boundary: an ABI mapping with zero functions is not accepted as a
  boundary.

### M13 - Release signing and remote delivery (stack.md step 5)

Rank 26. State: blocked. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: no. Needs external contract: yes. Reference profile: partial.
Blocked by: M11. Unblocks: nothing.

Exit criteria:

- BLOCKED until: an M11 artifact exists, hosted ruleset/label readback for
  origin is retained (D11), publication settings and at least one consumer are
  recorded, and the hosted acceptance checks (sign-off and linear history on
  historical commits, recorded in the private readiness matrix) are resolved or
  explicitly waived
- Toolchain admission: the release signing tool and the release workflow's
  actions are selected and pinned; the unverified toolchain action reference is
  replaced
- Unblocking evidence: a verified release workflow run, a signature and
  provenance for the real M11 artifact, and remote ruleset/label readback
- One criterion states that no measurement taken on the reference development
  machine is admissible as release evidence, and that the retained release
  evidence must come from the hosted workflow run and the remote readback only.

Cheapest exit: Record hosted ruleset/label readback for the existing origin
first, and keep the release workflow inactive until an artifact exists.

Epics:

- **E13-1 Hosted ruleset/label and identity readback**. Requirements:
  REQ-GOV-02. Acceptance: Positive: rulesets and labels read back from the
  hosted side match .github/rulesets and .config/labels.yaml. Negative: a
  missing required check is reported as drift. Boundary: the proposal identity
  case difference is resolved or recorded.
- **E13-2 Signing and provenance for a real artifact**. Requirements:
  REQ-REL-01, REQ-REL-02. Acceptance: Positive: the signature verifies against
  the M11 artifact. Negative: verification fails against a modified artifact,
  and an unverified action reference fails workflow lint. Boundary: signing an
  absent artifact fails rather than producing an empty signature.
- **E13-3 Publication settings and consumers**. Requirements: REQ-P01-06.
  Acceptance: Positive: one recorded consumer enables delivery. Negative:
  delivery is refused with zero recorded consumers. Boundary: a consumer without
  a pinned version is not counted.

## Risks and decisions

Deviation from the docs/integration/stack.md activation order:

stack.md orders the steps as: (1) inventory; (2) pin the image/kernel producer
schemas and test one request/result pair locally; (3) select one component; (4)
build one minimal image; (5) enable remote delivery. This roadmap keeps a
different order, and says so explicitly:

- The first component, P06 (M02, stack.md step 3), ranks before any schema work.
- The Aegis-owned half of step 2 (M18: product input manifest and kernel
  requirement schemas) ranks directly after M02 and the P01/P02 definitions
  (M03), at rank 5.
- The producer half of step 2 (M09: one request/result pair against pinned local
  imago/nucleus checkouts) is the first cross-repository milestone. It ranks
  after all purely local milestones.

Reasons for the different order:

- Both builder identities, cordanaLLM/imago and cordanaLLM/nucleus, did not
  resolve on GitHub at revision time.
- Builder workflows still reference non-canonical identities (private readiness
  matrix).
- Nucleus validates a fixed symbol list instead of consuming a requirement
  payload.
- Putting the producer pin first would stall every later milestone on
  repositories that Aegis cannot change.
- The roadmap method's rule 1 orders unverified cross-repository work after
  local work.

Mitigation: M09 is not delayed by its rank. It becomes ready once M18 is done.
Open decision D19 recommends that stack.md's activation order defer to this
roadmap through a reviewed change.

Risks, recorded from the cards and the private readiness matrix:

- Cross-repository contracts are declared but unverified; the configured
  producer origins do not resolve, and Aegis cannot close M09 alone.
- Imported workflows suppress failures (REQ-CI-01, REQ-CI-02), and the imported
  integration script prints boot/TPM2 success without executing anything
  (REQ-BOOT-02). They stay inactive, and any activated gate must be rewritten.
- The imported partition and transfer definitions do not pass the host systemd:
  systemd 261 ignores two repart keys and rejects the transfer definition. M03
  must rewrite them, and its gate must treat ignored-key diagnostics as failures
  because systemd-repart exits 0 on them.
- Unprivileged sysupdate validation works only against a scratch root tree.
  Validation against a loop-mounted image needs loop-device privileges.
- Kernel identity is unresolved (REQ-P01-09 versus Nucleus ownership), and the
  "Linux 6.12+/7.3" phrasing in the sources is ambiguous. A host or stock kernel
  is only a non-qualifying fixture (M19).
- Three hash schemes are named for one ledger (REQ-P06-09, REQ-P16-09,
  REQ-P16-03), and the reference daemon uses MD5.
- The subsystem graph and the two architecture documents disagree on five edges
  (REQ-GRAPH-01 to REQ-GRAPH-05). Two source-internal value conflicts exist:
  focus width (D16) and CSS framework (D17).
- The Vesta design violates the language boundary (REQ-P10-08), and Ludus
  depends on a proprietary SDK (REQ-P11-01).
- Scaffold code is simulated: hardcoded fds, boot times, wattage and proof
  hashes are not runtime evidence.
- Imported dependency versions (aya, memmap2, zenoh, Node 20) are proposal data.
  Versions are selected at activation after checking current upstream.
- These upstream dependencies are unpinned: wlroots, Mesa, PGlite, Steamworks
  bindings, the CAD and solver stack, Firecracker, the Wasm engine, and the
  truncated validation-container digest (REQ-P12-04). The image inputs use
  Release=latest (D18).
- Proposal narrative and benchmark percentages are untrusted; they are cited
  only where a requirement text records them as claims.
- One source records an external deadline: PLD effective 9 December 2026
  (REQ-P06-10).
- Hosted acceptance checks (sign-off and linear history) fail on historical
  commits, per the private readiness matrix. Rewriting history is out of scope.
- The M00 governance pass was reported by this session's preparation run and was
  not re-run during this revision. M01 re-runs it at the committed tip as its
  entry condition.

Open decisions:

- **D01** Which component is promoted first end-to-end? Options: P06
  aegis-justitia decision engine; P01/P02 declarative definitions with a Rust
  parser; P13 aegis-tellus SCI arithmetic; P03 aegis-vulcan or P15 aegis-hestia
  (trivial slices). Recommended: P06 aegis-justitia (M02). Why: Both P06 and
  P01/P02 cost small and need no hardware or external contract for their first
  slice. Neither is a committed crate: the repository has no Cargo.toml,
  crates/aegis-justitia holds no files, and P06 is listed only in the proposed
  (export-006) workspace, so M02 must create the workspace root. P01/P02 needs
  no new toolchain for the analyzers (systemd is present) and sits on the
  critical path to the first image. However, its interface contract is the
  Imago/Nucleus boundary, which cannot be accepted while both builder identities
  are unresolved. P06's contract is wholly Aegis-owned, it has a drafted
  positive/negative/boundary test module (REQ-P06-03, REQ-P06-04), and it is the
  mandatory gate for every agent-facing path (REQ-GOV-03), so its downstream
  milestones (M14, M05-M08, M17, M19) are all workstation-reachable. P01/P02
  follows immediately as M03. P13 unblocks only P16. P03/P15 are trivial but
  have low unblocking value. **Decision (2026-09-13):** P06 aegis-justitia is
  promoted first (M02).
- **D02** Which hash algorithm and signing scheme does the shared
  audit/checkpoint ledger use? Options: SHA-256 chain with TPM2 signature
  (schema in export-015, reference ledger export-040); BLAKE3 chain (export-002,
  export-025, export-062 naming); Keep the reference daemon's MD5 (export-031).
  Recommended: One algorithm behind a trait; SHA-256 unless a recorded decision
  selects BLAKE3; MD5 excluded. Why: Three schemes are named across sources
  (REQ-P06-09, REQ-P16-09, REQ-P16-01, REQ-P16-03). The only
  implementation-level agreement is SHA-256, and MD5 contradicts the subsystem's
  own schema. Record the choice before M02 closes so that M05 reuses it.
  **Decision (2026-09-13):** SHA-256 hash chain behind a hash-algorithm trait;
  MD5 excluded; BLAKE3 only through a later recorded decision.
- **D03** Which direction and transport does the Justitia/Minerva action gate
  use? Options: export-062: P06 -> P09 over eBPF action_gate / D-Bus;
  export-002: P09 -> P06 (Minerva proposes, Justitia intercepts); export-003:
  Justitia intercepts Minerva in the master diagram. Recommended: Keep
  export-062 edge ids; adopt propose/intercept call semantics for the contract;
  record in M14. Why: REQ-P09-03, REQ-P09-05 and REQ-GRAPH-04 disagree on who
  calls whom. The M14 action proposal contract cannot be typed until this is
  written down. It no longer blocks M02. **Decision (2026-09-13):**
  Propose/intercept semantics: P09 Minerva proposes an action and P06 Justitia
  intercepts and decides; the subsystem graph's edge identifiers are kept (typed
  in M14). **Typed (M14):** `aegis_justitia::contracts::graph::GateDirection`
  admits a single variant, so the settled direction is the only one an action
  proposal can carry and the opposite reading does not decode; the edge is still
  named `ACTION_GATE_INTERCEPT`. **Still open:** only the direction was decided.
  The transport half of this question is untouched -- DSP-03 records three
  transports for the one edge and is carried as decision D26 -- and the code
  record is split to say so: `D03_ACTION_GATE_DIRECTION` is closed,
  `D03_ACTION_GATE_TRANSPORT` is not, and `D03_ACTION_GATE` is the pair.
- **D04** Does Justitia also gate Vesta sandbox execution (P06 -> P10
  SYSCALL_INTERCEPT)? Options: Yes, add the edge to the graph of record; No,
  Vesta is gated transitively through P09. Recommended: Record as unresolved in
  M14; contract-test both paths in M06. Why: The edge exists only in export-002
  (REQ-GRAPH-02). The subsystem graph omits it (REQ-P10-07). **Decision
  (2026-09-13):** Recorded as unresolved in M14; both the direct P06 to P10 path
  and the transitive path through P09 are contract-tested in M06 before the
  graph is changed. **Recorded (M14):**
  `aegis_justitia::contracts::graph::D04_SANDBOX_GATE` carries the unresolved
  state and `SandboxAdmissionPath` keeps both readings representable, each with
  its hops and its graph membership under test; neither is authoritative.
- **D05** Which direction is the P05/P12 design-token edge? Options: P12
  produces tokens consumed by P05 (export-002); P05 -> P12 as recorded in
  export-062 (CONSUMES_DESIGN_TOKENS). Recommended: P12 is the producer; keep
  export-062's edge id and annotate direction (M16). Why: REQ-GRAPH-01 conflicts
  with the export-062 edge. Both describe P05 consuming tokens; only the arrow
  differs. **Decision (2026-09-13):** P12 Concordia produces the design tokens
  and P05 Forum consumes them; the graph's edge identifier is kept with the
  direction annotated (M16).
- **D06** Which Wasm capsule runtime does Vesta use given the language-boundary
  rule? Options: Rust-native runtime; Out-of-process Wazero behind a protocol
  adapter with contract tests; Direct Wazero dependency (violates stack.md).
  Recommended: Rust-native runtime, or a protocol adapter with contract tests
  (M06). Why: REQ-P10-08 names a Go runtime, and docs/integration/stack.md
  forbids a Go library as a direct dependency of a Rust daemon. **Decision
  (2026-09-13):** A Rust-native WebAssembly runtime, selected at M06 through the
  template matrix; no Go runtime and no protocol adapter.
- **D07** Which kernel boots the image: the mkosi linux-rt package or a
  Nucleus-produced artifact? Options: Nucleus artifact consumed through the M09
  contract; Distribution linux-rt package pinned in mkosi.conf; Both, with
  Nucleus overriding when present. Recommended: Nucleus artifact through M09;
  the host or stock kernel only as a non-qualifying local fixture (M19); record
  in M18. Why: REQ-P01-09 pins linux-rt, while stack.md assigns kernel
  production to Nucleus. **Decision (2026-09-13):** Both: the image boots a
  pinned distribution linux-rt kernel by default, and a Nucleus kernel artifact
  overrides it once the M09 contract delivers one. The Nucleus half of M09 no
  longer gates the first image. **Recorded (M18):** the identity is a field of
  the product input manifest rather than prose. `build/product-input.json`
  carries `kernel.source` with the three sources D07 admits --
  `distribution-package`, `producer-artifact`, and `built-here` for the kernel
  D70 as amended builds in this repository (M26) -- alongside
  `kernel.default-package`, which stays `linux-rt` whichever source is in
  force. The version that name resolves to is fixed by the dated snapshot, not
  by a second pin that could disagree with it.
- **D08** Is the compositor a C wlroots implementation or the pure-Rust
  scaffold? Options: C wlroots core with a Rust control daemon (export-013
  mandate); Pure Rust scaffold (export-027 as written); Rust control daemon now,
  wlroots FFI decided at M12. Recommended: Decide in M07; keep the registry/IPC
  state machine backend-agnostic. Why: REQ-P04-01 mandates C wlroots. The only
  code artifact has no wlroots FFI, and no wlroots version is pinned anywhere.
  **Decision (2026-09-13):** Pure Rust compositor; the C wlroots mandate is
  superseded (ADR-0001).
- **D09** Where does P15 Hestia live: crates/aegis-hestia, ui/hestia-app, or
  both? Options: Rust crate only; Svelte UI package only; Both, with a typed
  boundary. Recommended: Both, with the boundary recorded in the M01 inventory
  and applied in M17. Why: export-030 places a Rust daemon under crates/, while
  export-007 lists only ui/hestia-app. The proposed workspace lists neither
  (REQ-WS-01). **Decision (2026-09-13):** Both: a Rust crate for the storage and
  vector logic and a Svelte package for the UI, with a typed boundary (M17).
- **D10** Which UI toolchain boundary is pinned? Options: Node 22 LTS + pnpm per
  the developer guide; Node 20 per the imported CI; sveltesentio as the shared
  UI framework candidate. Recommended: Node LTS + pnpm selected through the
  template matrix after checking current upstream support; sveltesentio
  evaluated as a candidate only (M04). Why: REQ-UI-01 and REQ-CI-03 conflict.
  The private readiness matrix lists sveltesentio as an optional candidate, not
  a dependency. **Decision (2026-09-13):** The latest stable toolchain versions
  at activation with fast adoption through Renovate; the shared sveltesentio
  framework replaces the direct stack once it is mature enough.
- **D11** When are hosted rulesets, labels and publication settings for the
  existing origin verified by readback? Options: As an M01 follow-up, without
  enabling release delivery; At M13 together with publication settings; Only
  after the hosted sign-off and linear-history checks are resolved. Recommended:
  Retain hosted readback early; keep release delivery (M13) blocked. Why: origin
  <https://github.com/cordanaLLM/Aegis-OS.git> resolves with main at 42a23c8.
  The private readiness matrix records hosted sign-off and linear-history
  failures on historical commits, and no publication settings or consumers exist
  yet. **Decision (2026-09-13):** Hosted ruleset, label and identity readback
  done early (M00 evidence); release delivery stays blocked in M13.
- **D12** How does P11 reconcile a proprietary Steamworks SDK with the stated
  FOSS-compliance constraint? Options: Optional feature behind a build flag with
  recorded licence decision; Exclude Steamworks integration from the image;
  Proceed as drafted. Recommended: Optional feature with a recorded licensing
  decision before any SDK is vendored (M08). Why: REQ-P11-01 depends on a closed
  SDK, and the export-004 matrix labels P11's constraint as FOSS compliance.
  **Decision (2026-09-13):** Steamworks is excluded from the Aegis image; P11
  Ludus covers only free and open-source integration paths (ADR-0002).
- **D13** Reversible structural consolidation versus permanent hardening for
  P02? Options: Reversible consolidation (chosen in export-004); Permanent
  hardening. Recommended: Reversible consolidation, reconciled with the
  dm-verity requirement in M15. Why: REQ-P02-05 and REQ-P02-01 pull in different
  directions, and the interaction is unspecified. **Decision (2026-09-13):**
  Reversible structural consolidation, reconciled with dm-verity root integrity
  in M15. Applied in M15: reopening a blessed slot discards its dm-verity
  measurement, so re-blessing requires a fresh measurement equal to the signed
  release root hash. The register is
  `crates/aegis-janus-lifecycle/src/decision.rs`; the rule is held by the state
  machine next to it.
- **D14** Does mkosi run in Aegis (for mkosi summary validation) or only inside
  Imago? Options: Admit mkosi through the Aegis template matrix at the v24+
  floor; Validate mkosi configuration only through the Imago result.
  Recommended: Admit mkosi in Aegis only if M18 needs local validation;
  otherwise rely on the M09 Imago result. Why: mkosi was not installed on the
  workstation at revision time, and stack.md assigns image construction to
  Imago. The earlier draft required `mkosi summary` without an admission step.
  **Decision (2026-09-13):** mkosi is admitted in Aegis only if M18 needs local
  validation; otherwise the M09 Imago result validates the configuration.
- **D15** Where do the P01/P02 definition parser and the P02 A/B lifecycle crate
  live? Options: New crates under crates/ with an updated crates/README.md; A
  tools/ or build/ subpackage outside the daemon crates; Inside a future
  Imago-facing adapter package. Recommended: Decide in M01; M03 and M15 cannot
  create manifests until the path is recorded. Why: crates/README.md reserves
  paths for the P03-P16 daemons only. Neither P01 nor P02 has a proposed crate
  in the inventory. **Decision (2026-09-13):** New crates under crates/ in the
  shared Rust workspace for the P01/P02 definition parser and the P02 A/B
  lifecycle; crates/README.md is updated when they are created. Both exist:
  `crates/aegis-fabrica-defs` (M03) and `crates/aegis-janus-lifecycle` (M15).
- **D16** Which focus-indicator width is the accessibility boundary: 2px or 3px?
  Options: 2px stroke (export-023 test, export-020 report CSS); 3px ring width
  (export-042 tokens); 3px token with a 2px minimum test threshold. Recommended:
  3px token with a 2px minimum threshold tested at the boundary, recorded before
  M04 closes. Why: REQ-P05-06 and REQ-UI-02 state 2px, and REQ-P12-05 sets 3px.
  The earlier draft pinned 2px without recording the conflict. **Decision
  (2026-09-13):** A 3px focus-ring token with a 2px minimum enforced by the
  boundary test.
- **D17** Does Concordia use a Bootstrap fork or discard monolithic CSS
  libraries? Options: Bootstrap fork with better accessibility (export-004
  prose); Design tokens without a monolithic CSS library (export-004 matrix).
  Recommended: Design tokens without a monolithic CSS library; record the prose
  statement as superseded (M04). Why: REQ-P12-07 and REQ-P12-09 come from the
  same source and conflict. **Decision (2026-09-13):** Design tokens without a
  monolithic CSS library; the Bootstrap-fork statement is superseded.
- **D18** Are the mkosi distribution release and package set pinned? Options:
  Pin a distribution snapshot and package versions in the product input
  manifest; Keep Release=latest and rely on Imago for reproducibility.
  Recommended: Pin in the M18 product input manifest. Why: REQ-P01-01 shows
  Release=latest, and REQ-P01-09 lists packages without versions, so the image
  inputs are not reproducible. **Decision (2026-09-13):** The distribution
  snapshot and package versions are pinned in the M18 product input manifest.
  **Recorded (M18):** the pin is `Snapshot=2026/09/13` in `build/mkosi.conf`
  and the matching `distribution.snapshot` field of
  `build/product-input.json`. mkosi 27 resolves an Arch snapshot against
  `https://archive.archlinux.org/repos/<snapshot>/$repo/os/$arch`, and its
  snapshot identifier is formatted `%Y/%m/%d`; both were read from the
  installed version's own source rather than from memory, and no mirror was
  contacted. A dated snapshot fixes every package version at once, so the
  manifest's package set carries names and no versions: pacman has no
  version-pinned install syntax, and a second per-package pin could only
  disagree with the snapshot. The manifest's `SnapshotId` field type refuses
  `latest` and `rolling` while decoding, so the unpinned spelling REQ-P01-01
  records is not representable.
- **D19** Should docs/integration/stack.md's activation order defer to this
  roadmap? Options: Keep stack.md order: schema pin (step 2) before first
  component (step 3); Defer to the roadmap: Aegis-side schema authoring (M18)
  after the first component (M02) and P01/P02 definitions (M03); the
  cross-repository pair (M09) after all local milestones. Recommended: Defer to
  the roadmap, and update stack.md in a reviewed change that states the reason.
  Why: Both builder identities (cordanaLLM/imago, cordanaLLM/nucleus) did not
  resolve on GitHub at revision time. Pinning a producer schema first would
  leave every later milestone waiting on repositories that Aegis cannot change.
  The Aegis-owned half of step 2 (M18) is local and ranks directly after M03.
  The producer half (M09) becomes ready as soon as M18 is done and can run in
  parallel; it ranks after the local milestones only because rule 1 orders
  unverified cross-repository work last. **Decision (2026-09-13):**
  docs/integration/stack.md defers to the roadmap's order (applied in commit
  34cdaee).
- **D20** Does the minimal image (M11) include the UI and kernel-attached eBPF
  programs? Options: No: one image, one boot, no UI, no eBPF objects; Yes:
  include the accessibility gate and eBPF objects. Recommended: No; the
  accessibility gate attaches when the UI enters an image, and eBPF objects
  enter through M10 and M12. Why: The earlier draft blocked M11 on the full UI
  milestone and on eBPF verification. That put the Node/Playwright admission on
  the first-artifact path, justified only by an imported CI gate that stays
  inactive. **Decision (2026-09-13):** The first image boots only: no UI and no
  kernel-attached eBPF programs.

### Decisions from the reference profile (2026-09-13)

- **D68** Does the ranking rule count measured local hardware as local work?
  Decision: yes. A milestone whose hardware capability is present and measured
  on the recorded reference profile ranks with local work; a pass there stays
  development evidence and closes no gate.
- **D56** Is mkosi installed locally for Aegis-side validation, or does M18 rely
  on the Imago result? (D14 relates.) Recommended: Do not install. Take D14's
  recorded 'validate through the Imago result' branch. If M18 later needs local
  validation, pin extra/mkosi 27-1 and use that exact pin to close the M01 drift
  register's inherited 'mkosi v24+' row. Why: `command -v mkosi` exits 1 on the
  reference profile while `pacman -Ss '^mkosi$'` shows extra/mkosi 27-1
  available, so this is a free choice rather than a blocker. D14 already decided
  mkosi is admitted only if M18 needs local validation, M03's own criteria say
  mkosi is not used there, and stack.md assigns image construction to Imago. The
  absence of mkosi is therefore an argument for the Imago-result branch, and
  installing it would edge Aegis toward work it does not own. Version 27-1
  clears the v24+ floor, so if the branch is taken later the register row is
  resolved by an exact pin instead of an inherited range.
  **Decision (2026-09-13):** mkosi runs inside Aegis for local image-definition
  validation for as long as Imago is a scaffold: this repository cannot defer
  validation to a producer that cannot yet produce. The validation is Aegis-side
  only and constructs no product image; when Imago returns real artifacts, the
  check moves to consuming that result. **Recorded (M18):** mkosi is admitted
  at the exact pin `mkosi 27` (distribution package `extra/mkosi 27-1`), read
  back with `mkosi --version` and `pacman -Qi mkosi`, superseding the M01
  register's inherited `mkosi v24+` row. `make verify-mkosi` parses
  `build/mkosi.conf` with the host's own mkosi and requires the Output stanza
  of the main image; the floor is enforced by mkosi itself through
  `MinimumVersion=27`, which makes an older mkosi refuse the configuration
  rather than silently accept it.
- **D57** Where does the PREEMPT_RT kernel for P07/P08 latency work come from: a
  distribution linux-rt package booted as a QEMU guest kernel, a distribution
  linux-rt package installed as a host boot entry, or a request to Nucleus?
  Recommended: Distribution linux-rt 7.2.5.rt3.arch1-1 booted as a QEMU/KVM
  guest kernel at the new M23. Do not add a host boot entry and do not block on
  Nucleus. Why: The reference host runs '# CONFIG_PREEMPT_RT is not set' with
  CONFIG_PREEMPT_DYNAMIC=y, so the capability is genuinely absent — but `pacman
  -Ss` confirms extra/linux-rt 7.2.5.rt3.arch1-1 and extra/linux-rt-lts
  6.18.51.rt6.arch1-1 are packaged, and D07 has already decided the image boots
  a pinned distribution linux-rt kernel by default. A guest kernel needs no
  reboot, no firmware change and no change to the host's running scx scheduler,
  and the resulting direct-kernel-boot harness is reusable verbatim by M10 for
  the Nucleus kernel. This converts P07's determinism work from 'unprovable
  until Nucleus exists' into category-3 work available now, and it feeds D55
  (kernel version floor) with a measured value instead of an estimate.
- **D58** Is Firecracker or QEMU microvm the sandbox VMM for P10 and the new
  M22? Recommended: Pin Firecracker 1.17.0 (extra/firecracker 1.17.0-1) for the
  headline boot-time and footprint numbers, with QEMU 11.1.1 microvm as a
  recorded fallback. Every measurement must record which VMM produced it, and
  the two sets are never presented as interchangeable. Why: `command -v
  firecracker` exits 1 but the package exists; QEMU 11.1.1 is already installed
  and /usr/share/edk2/x64/MICROVM.4m.fd is already on disk, so a comparable
  measurement is available today at zero install cost. The numbers are not
  equivalent, and M21's original criterion says measured boot time and footprint
  replace the scaffold literals — so which VMM produced a literal is
  load-bearing. A second fact forces the decision rather than deferring it:
  Firecracker has no PCI passthrough, so a MicroVmInstance with is_gpu_enabled
  set needs a different VMM regardless of the three GPUs and clean IOMMU groups
  present on this profile.
  **Decision (2026-09-13):** Firecracker is the sandbox VMM for P10 and M22,
  matching the concept's sub-125 ms boot target and its jailer isolation model.
  It has no PCI passthrough, so passthrough work stays with the QEMU-based
  harness.
- **D59** Do consumer GPU limits change P03's (and P09's) acceptance, given that
  the reference profile cannot demonstrate PCIe P2PDMA or CUDA GPUDirect at all?
  Recommended: Yes. Reframe P03/P09 acceptance to DMA-BUF plus host-mediated BAR
  transfer, which the profile evidences fully at the new M25, and move every
  'CUDA GPUDirect' criterion behind an explicit named procurement dependency in
  M12 (a CMB-capable enterprise NVMe controller or a datacentre-class GPU). Why:
  CONFIG_PCI_P2PDMA=y expresses only the kernel's willingness: `ls -d
  /sys/bus/pci/devices/*/p2pdma` returns no matches, so not one device on this
  machine publishes a p2pmem pool, none of the three Samsung consumer NVMe
  drives exposes a Controller Memory Buffer, and GPUDirect Storage/RDMA are
  gated to datacentre SKUs so the RTX 4090 cannot serve them (nvidia_fs not
  found, no /dev/nvidia-fs; installing a gds package would not change this).
  Leaving the criteria phrased as GPUDirect invites a future reader to assume a
  4090 covers them. Saying so plainly also lets the two paths that DO work —
  DMA-BUF across three vendors, and VFIO BAR mapping with the Arc A380 alone in
  IOMMU group 17 — proceed at rank 17 instead of waiting behind an impossible
  sibling.
- **D60** What energy scope does P13 accept, given that the reference profile
  exposes only package-0 and core RAPL zones with no dram and no psys zone,
  root-only access, and a model-based AMD estimate? Recommended: P13 measures
  the package and core zones only. DRAM energy is modelled and labelled as
  modelled, never reported as measured. The reader requires root or
  CAP_DAC_OVERRIDE, handles rollover at the measured max_energy_range_uj of
  65532610987 uJ, and treats only same-zone deltas as trustworthy. Why: P13's
  stated requirement is 'pkg/DRAM energy accumulation' and the DRAM half does
  not exist on this platform: `ls -d /sys/class/powercap/*` returns only
  intel-rapl, intel-rapl:0 (package-0) and intel-rapl:0:0 (core). There is no
  ACPI power_meter (ACPI000D) fallback either; the only alternatives are
  device-scoped (amdgpu power1_input, i915 energy1_input). energy_uj is mode
  0400 under the CVE-2020-8694 mitigation, so an unprivileged Tellus daemon
  cannot read it and the perf power/energy-pkg route is blocked by
  perf_event_paranoid too. Deciding this now keeps M21 from either silently
  returning zero for a missing zone or quietly reporting a modelled number as a
  measurement. It also follows that the P05/P09 20 W per-agent cap is
  apportioned from a socket-level counter, never measured per process.
- **D61** Is rustup installed to satisfy the template matrix's pinned-toolchain
  admission, or is the distribution rustc accepted? Recommended: Install
  extra/rustup 1.29.1 and commit a rust-toolchain.toml pinning the exact stable
  version, before M02's first cargo gate runs. Why: This is the most
  widely-needed gap on the profile: every Rust milestone from M02 onward carries
  the clause that the stable toolchain is selected through the template matrix
  with a pinned version before any cargo gate. `command -v rustup` exits 1 and
  only the distribution rustc/cargo 1.98.1 is installed, which moves with system
  updates and therefore cannot satisfy a pinned admission at all. M02 is the
  single ready milestone, so this is the first thing that blocks real work — and
  it is the reason M02 is category 2 rather than category 1.
  **Decision (2026-09-13):** rustup is installed and provides the pinned
  toolchain, replacing the distribution rust package. rust-toolchain.toml pins
  the channel, CI honours it automatically, and the template matrix row records
  both the pinned version and the distribution version it replaced.
- **D62** How is Secure Boot evidenced, given that the reference host's firmware
  has Secure Boot disabled AND is not in Setup Mode? Recommended: Guest-side
  only, scoped to the new M24: generate a custom-key OVMF VARS store with
  virt-firmware 26.9-1 or sbctl 0.18-2, sign the UKI with sbsigntools 0.9.5, and
  boot against OVMF_CODE.secboot.4m.fd. Host firmware enrolment stays an
  operator action recorded in planning/hardware-profile.json and is never a
  roadmap milestone. Why: `od` on both `SecureBoot-*` and `SetupMode-*` returns
  6 0 0 0 0: Secure Boot is off and the vendor PK/KEK/db/dbx are enrolled, so
  custom host enrolment would require a physical firmware-setup visit and a
  reboot. The guest path is nearly ready but has a specific hole —
  /usr/share/edk2/x64 ships OVMF_CODE.secboot.4m.fd but NO pre-enrolled
  OVMF_VARS.secboot, so the secboot firmware code is present and useless without
  a generated variable store. This one item is exactly what separates 'the UKI
  booted under QEMU' from 'the UKI's signature was verified', and both tools
  that close it are packaged. It also determines that PCR 7 on this profile
  attests to an unsigned configuration, which is why D63 exists.
- **D63** Does M20's swtpm attestation slice re-blocker onto the new M24 harness
  instead of M11, and which PCRs does the unseal policy bind? Recommended: Keep
  M20 blocked on M11 as its own BLOCKED-until clause states, but pre-prove the
  swtpm and tpm2-tools quote path on the M24 harness so M20 executes rather than
  debugs when M11 lands. Bind the unseal policy to PCR 11 (the UKI) and PCR 0/4,
  not PCR 7. Why: The quote half would genuinely run on an M24 scratch fixture,
  but the /var unseal half needs the real image and M20's clause names it, so
  moving the blocker would change the milestone's meaning to buy one rank. The
  PCR choice is forced by measurement rather than preference: with host Secure
  Boot disabled and no guest keys enrolled, a policy bound to PCR 7 would attest
  to 'Secure Boot off' — a policy that passes while proving nothing. PCR 11 and
  0/4 are the honest local subset. tpm2-tools 5.8 must be pinned either way;
  neither tpm2_pcrread nor tpm2 is installed today.
- **D64** Does planning/hardware-profile.json become a normative gate input that
  milestones cite, and what is required before any needs_hardware milestone may
  claim qualification rather than development evidence? Recommended: Yes:
  milestones cite the profile by its recorded_on date and per-capability
  evidence_command, and no needs_hardware milestone may flip to a qualification
  claim until a second, differently-configured machine is recorded and the
  milestone's own acceptance is met. The profile's existing evidence_class
  string is the binding text. Why: This re-rank promotes six milestones on the
  strength of one machine's capabilities, which is exactly the situation where
  'it passed here' erodes into 'it is qualified'. The profile file already
  carries the right language — role 'reference development machine' and an
  evidence_class stating that a passing check does not qualify hardware, does
  not close a blocked gate and is not release evidence — but nothing currently
  obliges a milestone to cite it. Making the citation explicit is what keeps the
  promotion honest, and it is why every hardware milestone above carries a
  development-evidence-only criterion in its own exit_criteria_additions rather
  than relying on a rule stated once elsewhere.
- **D65** Which Node major does the UI toolchain admit at M04, given that the
  reference profile runs Node v26.8.2 while the M01 drift register records 'Node
  20 vs 22'? (D42 relates.) Recommended: Resolve D42 with an exact pin against a
  current LTS decision; do not inherit either register value and do not silently
  adopt v26.8.2 because it happens to be installed. Record the reference-profile
  values (Node v26.8.2, pnpm 10.29.3, Playwright chromium-1228 cached) in the
  template matrix beside the chosen pin. Why: The machine runs a third version
  that appears nowhere in the register, so the row cannot be closed by picking a
  side — the evidence has moved past both options. M01's register explicitly
  marks these as proposal data to be resolved rather than inherited, and M04's
  gate would otherwise pass on whatever the workstation happens to have. The
  Playwright browser cache is already populated at chromium-1228, which makes it
  particularly easy for an unpinned gate to look green for the wrong reason.
  **Decision (2026-09-13):** The UI toolchain admits the latest Node major (26.x
  as installed on the reference profile) and tracks it forward with Renovate,
  rather than pinning an older line.
- **D66** Is bpf/scx_cake.bpf.c an Aegis original or a fork of upstream
  scx-scheds, given that the distribution already ships the binary? Recommended:
  Record the provenance explicitly in M19: either pin the upstream scx-scheds
  commit the object derives from, or state that it is an Aegis original that
  merely shares a name. (D40 relates.) Why: `command -v scx_cake` returns
  /usr/bin/scx_cake from extra/scx-scheds 1.1.3-2, already installed on the
  reference profile — the scheduler P07 proposes ships as a distribution
  package. M01's drift register already carries an unresolvable kernel-space
  eBPF dependency question (D40) and flags unpinned upstream projects. Compiling
  an object with the same name as an installed distribution binary, without
  recording which one M19 is actually verifying, is the kind of ambiguity the
  register exists to prevent; it would also make the M19 verifier log impossible
  to attribute.
- **D67** May M19's struct_ops boundary load displace the scheduler currently
  holding /sys/kernel/sched_ext/root/ops on the reference host? Recommended:
  Yes, as a deliberately scheduled disruptive step with recorded detach and
  reattach, not as a background test. The pre-existing root/ops value must be
  captured, the stubbed scheduler attached, then the original restored and
  re-verified. Why: Only one scx scheduler can hold root/ops at a time, and the
  reference host currently runs `ghostbrew` with switch_all=1 and nr_rejected=0
  — so M19's boundary criterion ('a struct_ops load with all handlers stubbed
  succeeds where the host exposes sched_ext') necessarily takes over CPU
  scheduling on the maintainer's working machine for the duration. That is
  executable and it is the right test, but it is not something to discover
  mid-run. Deciding it in advance also produces the attach/detach evidence the
  criterion should have carried all along.

- **D69** What is the dependency and toolchain version policy for a long-running
  project? Decision (2026-09-13): track the latest upstream releases across
  toolchains, editions and crates, and refresh pins rather than freeze them. The
  first component lands on Rust edition 2024 with the current stable toolchain
  and current crate releases; Renovate proposes the moves and the gates prove
  them.

- **D70** Where does the kernel come from? Decision (2026-09-13, amended the
  same day): Aegis builds the kernel in this repository while Nucleus is a
  scaffold, for the same reason D56 keeps image-definition validation here while
  Imago is a scaffold. A repository cannot defer construction to a producer that
  cannot yet produce. The build is milestone M26: a pinned source, a tracked
  configuration fragment expressing what the M18 schema requires, and a
  read-back of the produced configuration from inside a guest. It is development
  evidence and never a release artifact. Kernel construction returns to Nucleus,
  which owns it under docs/integration/stack.md, once Nucleus returns real
  artifacts against the M09 contract.
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

The verbatim quotes are held in the private generation result
(`notebook-result.json`). This public document reproduces none of them.
