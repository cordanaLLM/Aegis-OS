# Aegis OS activation roadmap

Status: reviewed draft; decisions D01-D20 recorded by the maintainer on
2026-09-13

## Outcome and scope

Aegis OS is an image-based Linux OS with seventeen subsystems: the concept's
sixteen (P01-P16) and P17 aegis-scaena, the display runtime that ADR-0003 adds
under decision D75. They are Rust host daemons, kernel-space eBPF programs,
mkosi/systemd-repart/systemd-sysupdate image definitions, a native Rust desktop
shell (ADR-0004) and Svelte UI packages. P17 is the one change to the concept's
subsystem set; ADR-0001, ADR-0002 and ADR-0004 supersede parts of the P04, P11
and P05 blueprints. This roadmap orders repository preparation and component
activation so that each step yields verifiable evidence at the lowest cost.

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
the subsystems beyond what an ADR records (ADR-0003 adds P17); activation of
imported CI, release or integration workflows (they suppress failures and print
simulated success, see REQ-CI-01, REQ-CI-02 and REQ-BOOT-02); any claim derived
from proposal narrative or benchmark figures; imported dependency versions
treated as active pins; owners or dates beyond repository names and dates
present in the sources.

Current verified state at revision time:

- LICENSE, LICENSES/, LICENSING.md and REUSE.toml are committed (a2c9626).
- The component inventory enrichment and a governance-only CI gate are committed
  (42a23c8).
- `origin` resolves to <https://github.com/cordanaLLM/Aegis-OS.git>, read back
  with `git ls-remote`. The hosted ruleset `praetor-main-protection` is active
  on `main` and reads back (`gh api repos/cordanaLLM/Aegis-OS/rulesets`) as
  deletion and non-fast-forward protection, linear history, signed commits,
  pull requests with no required approval (`review_mode: single_maintainer` in
  `.standards.yaml`), and five required status checks: `Verification gate`,
  `Platform Neutrality (Linux)`, `Platform Neutrality (macOS)`,
  `Platform Neutrality (Windows)` and `Documentation Governance`.
- Every milestone and epic is mirrored as a GitHub milestone and a `roadmap`
  issue; M27 is GitHub milestone 28, and E27-1 to E27-3 and E16-3 are issues
  #125 to #128. E11-5 and E20-3, which D84 adds, are issues #139 and #140; #52
  and #53 carry the E11-1 and E11-2 text of D84 and D85, and #79 and #80 (E24-1,
  E24-2) are closed with M24; E16-4 and E11-6, which D91 adds, are issues #147
  and #148 (read back 2026-09-28). E11-7 and E10-4, which D92 adds, and E10-5,
  which D94 adds, are issues #149 to #151; #49 to #51 (E09-1 to E09-3) carry the
  D92 text and are closed with M09. M28 and M29, which ADR-0004 adds, are GitHub
  milestones 29 and 30, and their epics E28-1 to E28-6 and E29-1 to E29-4 are
  issues #156 to #165. The M16 milestone with #47, #48, #128 and #147 (E16-1 to
  E16-4), the M11 milestone with #52 to #55, #139, #148 and #149, and the M27
  milestone with #125 to #127 were re-synced to the ADR-0004 register on
  2026-09-29, their rank lines included (read back that day with `gh issue view`
  and the milestones API). Rank lines in the other milestone descriptions and
  issue bodies are not kept current; ranks are read from
  `planning/roadmap.json`, which stays the source of truth.
- cordanaLLM/imago and cordanaLLM/nucleus resolve. imago decodes both M18
  payloads at the commit M09 pins (16f964b until D107, 987b95a since), nucleus
  verifies `build/kernel-requirement.json` against the kconfig fragments of its
  `realtime` stream at the commit M09 pins (0a4eac9 under D106, 82aa6b7 after
  the re-pin of 2026-09-29, 852be74 since D107), imago has published no image,
  and nucleus's first kernel release, of 2026-09-29, is M10's to verify (M09,
  D92, D106, D107). The private readiness matrix is a superseded 2026-09-13
  snapshot.

## Method

Dependency DAG built from the seventeen activation cards in
planning/components.json (the concept's sixteen plus P17, which D75 and
ADR-0003 add): subsystem edges from the subsystem graph (export-062),
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
- M27: the VA-API binding's build tools (bindgen, libclang, pkgconf and the
  libva headers, D80) and the Wayland and socket crates M27 proposes
  (smithay-client-toolkit, rustix; D78 permits Wayland crates in P17)
- M16: accesskit, the data model only (D101), and eslint, eslint-plugin-svelte
  and svelte-eslint-parser for the D100 lint in the pinned M04 container
- M28: gpui, git-pinned to one zed-industries/zed commit, and accesskit_unix
  (D102), with the C libraries they reach (libxkbcommon, libwayland-client) and
  the at-spi2 bus launcher and registry its AT-SPI cases start
- M29: the pinned guest image and its session services (a layer-shell
  compositor, xdg-desktop-portal, at-spi2-core, a StatusNotifierWatcher); QEMU
  is M24's

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
| 9 | M05 | Evolution loop logic: P13 Tellus SCI and P16 Athena lifecycle | done | small | no | no | not-hardware | M14 | M19, M21 |
| 10 | M06 | Agent execution chain logic: P09 Minerva and P10 Vesta | done | small | no | no | not-hardware | M14 | M08, M22 |
| 11 | M08 | Leaf slices dependent on the agent chain: P11 Ludus and P14 Hephaestus | done | small | no | no | not-hardware | M06 | M25 |
| 12 | M07 | Real-time control plane: P04, P07 and P08 logic | done | medium | no | no | not-hardware | M02 | M19, M23, M27 |
| 13 | M19 | eBPF objects loaded through the verifier on the host kernel | done | medium | no | no | full | M05, M07 | M10 |
| 14 | M23 | P07 and P08 latency fixtures on a realtime kernel guest | done | medium | yes | no | full | M07, M26 | M10 |
| 15 | M24 | Local boot harness over an externally supplied artifact | done | large | yes | no | full | M15 | M11 |
| 16 | M04 | UI accessibility harness: P12 Concordia tokens | done | medium | no | no | not-hardware | M02 | M16, M11 |
| 17 | M09 | Cross-repository contract pin: one local request/result pair | done | small | no | yes | partial | M18 | M11, M10 |
| 18 | M16 | P05 Forum shell state and lifecycle with stubbed IPC | done | small | no | no | not-hardware | M04, M14 | M28 |
| 19 | M27 | P17 Scaena first slice: VA-API frames to a layer surface over SCM_RIGHTS | done | medium | yes | no | full | M07 | M28, M29 |
| 20 | M21 | Workstation hardware slices: RAPL counters and KVM sandboxing | done | small | yes | no | full (privileged read) | M05 | - |
| 21 | M25 | GPU DMA-BUF sharing and VFIO passthrough slices | ready | medium | yes | no | full | M08, M17 | M12 |
| 22 | M22 | P10 microVM sandbox measurements on KVM | ready | medium | yes | no | full | M06 | - |
| 23 | M28 | P05 native shell on gpui: layer surface, AT-SPI tree, frame time | ready | large | yes | no | full | M16, M27 | M29 |
| 24 | M29 | P05 live session in a VM: portal, AT-SPI2 and StatusNotifierWatcher | blocked | medium | yes | no | full | M28, M27 | - |
| 25 | M11 | Minimal image build with artifact, signature and boot evidence | ready | medium | yes | yes | partial | M09, M24, M04 | M13, M20, M12 |
| 26 | M10 | eBPF objects re-verified against the Nucleus-pinned kernel in a VM | ready | medium | yes | yes | partial | M09, M19, M23 | M12 |
| 27 | M20 | TPM2 attestation slice on swtpm: audit-record signing and /var unseal | blocked | medium | yes | yes | full | M11, M14 | - |
| 28 | M12 | GPU-backed slices: DMA-BUF, VFIO and P2PDMA paths | blocked | large | yes | yes | partial | M25, M10, M11 | - |
| 29 | M13 | Release signing and remote delivery (stack.md step 5) | blocked | medium | no | yes | partial | M11 | - |

The reference profile column records what the machine in
`planning/hardware-profile.json` can evidence for that milestone. A pass there
is development evidence only; see `docs/roadmap/hardware.md`.

M11 and M10 are `ready` by rule (5): every milestone they are blocked by is
done. M11 still carries an external BLOCKED-until criterion that holds: it
waits for Imago to return an image/UKI and its product result. M10's held until
2026-09-29, when nucleus published release `v7.2.8-realtime-lusoris1`; M10's
run verified and recorded it and repeated the M19 loads on it, and M10 stays
open because that kernel cannot attach `action_gate` (decision D107,
`docs/build/nucleus-kernel.md`). `ready` is a register state for both, not an
unblocking.

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
- Dated 2026-09-29, appended after done (D103, D104, D105 and a defect found by
  probe); no exit criterion or epic text changes, and criterion 2's 'accepted
  architectures' now reads all-of. The array form: KernelRequirement::decode,
  and ProductInputManifest::decode likewise, accepted a payload written as a
  JSON array of its values in declaration order, because serde's derived decoder
  implements visit_seq and deny_unknown_fields has no key to refuse there -- a
  malformed payload criterion 1 and E18-2 should have refused. Every struct of
  both contracts now decodes from a JSON object only, at the top and nested
  (src/payload.rs); tests/array_form.rs sends the array form of both payloads
  and of every nested struct, and a one-element array led by another version,
  which the object-only version peek alone decides, and its negative cases fail
  against the decoder of 061bbde. D103: required-by admits exactly the
  identifiers of the form nucleus's ADR-0007 states, an upper-case letter then
  [A-Z0-9-], at most 128 bytes, with the bare REQ- refused; the REQ- form binds
  the payloads Aegis issues (RequirementId::is_aegis_requirement,
  tests/issued_payloads.rs). D104: a listed architecture is a promise, so a
  profile reports each other listed architecture as ArchitectureUnverified and
  MAX_UNMET grows by MAX_ARCHITECTURES, which tests/reference_profile.rs holds
  by building the reachable maximum, four rows plus two for each of MAX_FEATURES
  features; both reviewed payloads list x86-64 alone, so exit criteria 6 to 8
  hold unchanged. D105: build/kernel-requirement.schema.json and
  build/product-input.schema.json are generated from the types with schemars
  1.2.2 and held to them by tests/json_schema.rs; tools/test_payload_schemas.py
  validates the three reviewed payloads against them. `cargo test --locked
  --all-features` passes on the pinned toolchain, and criterion 5 still holds
  for the crate: it contacts no producer.
- Dated 2026-09-29, appended after done (D107); no exit criterion and no epic
  text changes. build/kernel-requirement.json gains one row after
  CONFIG_BPF_LSM: CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS, built-in, probe
  kernel-config, required by REQ-P06-05. A BPF LSM program attaches through a
  BPF trampoline, which patches the -mfentry nop the function tracer compiles
  in, and CONFIG_BPF_LSM does not depend on the tracer in Kconfig; M10 found
  action_gate failing to attach with -EBUSY on nucleus v7.2.8-realtime-lusoris1,
  which has no function tracer. The payload is fourteen rows, 2232 bytes, sha256
  92d74206ee5a4cc46bc9a8c209855c4a3d07a9ed47b003963fc73697ac8776ce (was
  d796c408b4db5dd9e5f22d35c109a8dccadcdb14f3f68ce7de2cddc594d47adb);
  build/kernel-requirement.reference.json and both JSON Schemas are unchanged,
  and no crate source changes. tests/kernel_requirement.rs requires the new
  symbol among the rows P06 needs, tests/array_form.rs counts fourteen features,
  tests/kernel_fragment.rs holds the re-rendered
  build/kernel/50-aegis-requirement.config byte for byte, and
  tools/test_payload_schemas.py validates the new payload against
  build/kernel-requirement.schema.json. Criterion 7 holds as before, the
  reference profile refusing the product requirement on CONFIG_PREEMPT_RT alone:
  planning/hardware-profile.json records
  CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS=y, read on 2026-09-29 with the
  profile's evidence command extended by that symbol, and every other symbol
  read as recorded. That value is 7.2.8-1-cachyos's, as the profile's
  kernel.added_note says: 7.2.4-1-cachyos, the recorded release, is no longer on
  the host, so the symbol was not read on it. The consumers moved with the
  payload: imago re-vendored the bytes (its pull request 52, 987b95a) and
  nucleus enables the tracer on every stream (its pull request 47, 852be74),
  both pinned in M09's entry of this date, and M26's configuration carries the
  row (M26's entry of this date). `cargo test --locked --all-features` passes on
  the pinned toolchain inside `make verify-all` (M09's entry of this date names
  the run).

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

Rank 9. State: done. Cost: small. Owner repository: cordanaLLM/Aegis-OS.
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

Rank 10. State: done. Cost: small. Owner repository: cordanaLLM/Aegis-OS.
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
  reference profile can supply either Firecracker 1.17.0 or QEMU 11.1.1
  microvm, and D58 requires every measurement to record which VMM produced it.
  Both are present on the reference profile -- `firecracker --version` prints
  Firecracker v1.17.0 (package firecracker 1.17.0-1.1) and
  `qemu-system-x86_64 --version` prints QEMU emulator version 11.1.1 -- which is
  what planning/hardware-profile.json already records, so the criterion is the
  field and not the installation.

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

Rank 11. State: done. Cost: small. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: no. Needs external contract: no. Reference profile:
not-hardware. Blocked by: M06. Unblocks: M25.

Exit criteria:

- aegis-ludus and aegis-hephaestus have manifests, lock entries and
  positive/negative/boundary tests; the Rust toolchain from M02 is reused
- Assertions are refactored to Result per HISS-07
- P11 Ludus integrates no Steamworks SDK (D12, ADR-0002); a negative test proves
  the crate builds without it. The image half of ADR-0002 is not claimed here:
  this repository builds no image and that gate stays blocked, so a crate test
  asserting an image property would be an unqualified readiness claim
- Interface contracts are typed for P09 to P14 (VERIFY_CODE_CAD, direction per
  the M01 register), P14 to P15 (geometry viewport descriptor) and P11 to P02
  (transaction receipt, TPM2 signing stubbed), each with a negative test for a
  malformed payload. The P11 to P04 rich-presence contract is deliberately not
  typed: decision D48 is open on whether that integration survives ADR-0002's
  reasoning at all, and the sources name no library to build a schema against,
  so typing one would argue for one reading of an open decision
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
  bound is accepted. Negative: a request naming no STEP source errors; whether a
  named path resolves to a file is the loader's question and is not asked here,
  because this crate reads no filesystem. Boundary: 500000 elements are accepted
  and 500001 rejected, and the iteration bound is honoured. CAD and solver
  versions are recorded as unpinned.
- **E08-3 P11 and P14 interface contracts**. Requirements: REQ-P14-05,
  REQ-P14-06, REQ-P11-06, REQ-P11-04. Acceptance: Positive: the descriptors
  round-trip. Negative: a receipt without a signature field is rejected.
  Boundary: a viewport descriptor at the mesh bound is accepted.

### M07 - Real-time control plane: P04, P07 and P08 logic

Rank 12. State: done. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: no. Needs external contract: no. Reference profile:
not-hardware. Blocked by: M02. Unblocks: M19, M23, M27.

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

Rank 13. State: done. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: no. Needs external contract: no. Reference profile: full.
Blocked by: M05, M07. Unblocks: M10.

Exit criteria:

- Toolchain admission: clang (BPF target), bpftool and libbpf (or aya, per the
  M01 register) are selected through the template matrix with pinned versions
  before any compile
- action_gate, scx_cake and kepler_power compile warning-free with clang
  -target bpf
- The objects load through the BPF verifier on the host kernel with CAP_BPF
  **and CAP_PERFMON**; the host kernel version and config (BPF LSM, sched_ext,
  BTF) are recorded as observed facts, not assumed. The register said CAP_BPF
  alone and that was an assumption: on the reference kernel CAP_BPF by itself
  refused all three program types before the verifier ran, with `BPF program
  load failed: -EPERM` for the LSM, tracepoint and struct_ops program types
  alike, and adding CAP_PERFMON made all three load. The recorded set is the
  one that ran, applied with `sudo setpriv --bounding-set=-all,+bpf,+perfmon`
  to the caller's own uid, never as root
- Negative: removing the ringbuf NULL check or an unroll bound makes the
  verifier reject the object. Boundary: a struct_ops load with all handlers
  stubbed succeeds where the host exposes sched_ext
- This is a non-qualifying local fixture: a host or stock kernel cannot close
  the Nucleus-kernel verification in M10
- The pinned tool versions are recorded from the reference profile before the
  first compile: clang 22.1.8 (`clang -print-targets` listing bpf, bpfeb,
  bpfel), bpftool v7.8.0, and libbpf **1.7.0** -- not the v1.8 this register
  carried. The two are different artefacts and the register read the wrong one:
  `pkg-config --modversion libbpf` prints 1.7.0 and
  /usr/include/bpf/libbpf_version.h declares major 1 minor 7, which is the
  shared library the loader links and which reports itself as v1.7 at runtime,
  whereas the `using libbpf v1.8` line of `bpftool version` is the libbpf
  bpftool was statically built against inside core/bpf 7.2.5-1. Both readings
  are recorded in docs/roadmap/toolchain-admission.md, distinctly, and llvm-
  strip from the same clang toolchain is admitted alongside them.
- The verifier log is retained for every load, positive and negative, and the
  negative case is proven by the log rejecting the object — not by a non-zero
  exit code alone: removing the ringbuf NULL check or an unroll bound must
  produce a named verifier rejection in the log.
- The scx_cake struct_ops boundary load explicitly attaches AND detaches on the
  host kernel, and the pre-existing scheduler is recorded and restored, with
  nr_rejected and switch_all captured at each point:
  /sys/kernel/sched_ext/root/ops reads
  **`rusty_1.1.3_x86_64_unknown_linux_gnu`** before the test -- not the
  `ghostbrew` this register carried -- `aegis_cake_stub` during it, and
  `rusty_1.1.3_x86_64_unknown_linux_gnu` again after. Those two counters are NOT
  members of root/, which holds exactly `events` and `ops`; they are top-level
  sched_ext attributes one directory up, and listing only root/ is what made
  them look absent. /sys/kernel/sched_ext/switch_all read 1 before, **0 for the
  whole hold window** and 1 after; /sys/kernel/sched_ext/nr_rejected read 0 at
  every point; /sys/kernel/sched_ext/enable_seq read 19, 20 and 21. Which
  readings may be subtracted differs by counter and is not one blanket claim:
  the `SCX_EV_*` lines of root/events and nr_rejected both belong to the
  scheduler instance -- the enable path sets nr_rejected to zero -- while
  enable_seq is incremented on every enable and never reset, so its pair IS a
  difference and the gate prints it. Only one scx scheduler can hold root/ops --
  confirmed by an attach refused with `Device or resource busy` while the
  machine's own scheduler held it -- so this is a deliberate disruptive step,
  not a background one (D67).
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

Rank 20. State: done. Cost: small. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: yes. Needs external contract: no. Reference profile: full
(privileged read). Blocked by: M05. Unblocks: nothing.

Exit criteria:

- Hardware: a host with readable RAPL counters and KVM; the host model and
  kernel are recorded
- Toolchain admission: Firecracker (and the jailer, if used) is selected through
  the template matrix with a pinned version before any microVM run
- Unblocking evidence: measured RAPL energy deltas replace the simulated wattage
  in the M05 engine, and one AF_VSOCK candidate evaluation round-trips, proving
  the sandbox path works. The measured microVM boot time and memory footprint
  are M22's under decision D71: this milestone proves the path, M22 measures it
  and records which VMM produced each figure.
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
  negative test that must fail. Decision (maintainer, 2026-09-29): the counter
  is read as root through passwordless sudo -n, read-only and scoped to exactly
  one command, `sudo -n cat /sys/class/powercap/intel-rapl:0/energy_uj`, each
  under a deadline; the zone enumeration and max_energy_range_uj are
  world-readable and are read without sudo; no other command ever runs under
  sudo; the gate prints SKIP with a reason where sudo -n is unavailable; and an
  unprivileged read of energy_uj is kept as the negative test.
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

Evidence (the 2026-09-29 disclosure, the scope and the closing summary; every
entry is in `planning/roadmap.json`, and the runs are on
`docs/build/workstation.md`):

- Disclosure, in the shape M18 recorded, because a milestone must say where its
  evidence differs from the letter of its bar: one exit criterion was amended
  during this delivery and before the state moved to done, and it is not a
  relaxation. Criterion 8 asked which privileged path the reader takes; it now
  records the answer the maintainer gave on 2026-09-29 (root through one scoped
  `sudo -n cat`, the unprivileged read kept as the negative test), and nothing
  else in it changed. No other criterion, epic or the cheapest exit was
  rewritten, and eight points are recorded instead. (1) Criterion 5 names the
  reference kernel 7.2.4-1-cachyos; the reference host runs a rolling kernel and
  read 7.2.8-1-cachyos on every run, which is the kernel recorded beside the
  readings: the figure is not relabelled, and D73 treats the reference host as
  provenance. (2) Criterion 11 says the measurement runs as root; exactly one
  process does, the `cat` sudo starts, and the reader,
  crates/aegis-tellus-rapl's binary, runs as the invoking user and reads no
  file: the gate hands it the text. Only the package-0 counter is read, because
  the decision admits one command; core is enumerated and its counter is not
  read. (3) Criterion 2's template matrix is
  docs/roadmap/toolchain-admission.md, which states that it is that matrix. The
  jailer is not used: it needs root to chroot and change user, which the
  decision does not admit, so every microVM runs as the invoking user under
  Firecracker's own seccomp filters. The distribution package firecracker
  1.17.0-1.1 on the reference profile is not run; the gate runs the release
  binary it fetched and pinned. (4) E21-2's boundary ran 64 microVMs at once on
  real KVM, each with one vCPU and 64 MiB, started one at a time, each answering
  one evaluation before the next started, about 4 GiB of guest memory in all; 64
  MiB is the smallest power of two the pinned kernel boots with this initramfs
  (48 MiB and 56 MiB guests panicked out of memory before init on 2026-09-29, 60
  MiB and 64 MiB reached the listener), a configuration value and not a
  footprint measurement. (5) E21-2's round-trip is AF_VSOCK at the guest end
  only: Firecracker mediates between AF_UNIX on the host and AF_VSOCK in the
  guest with its own virtio-vsock device model (docs/vsock.md at v1.17.0), so
  the host's vhost_vsock is not in the path. M22's criterion that names
  /dev/vhost-vsock and CONFIG_VHOST_VSOCK=m as the mechanism describes a
  vhost-based monitor, not Firecracker, and is M22's to settle. (6) The guest
  kernel is Firecracker's own CI build, vmlinux-6.18.44 from
  firecracker-ci/20260909-a8e1c3830545-0, pinned by the sha256 computed at
  admission because the bucket publishes none; neither the M26 kernel nor the
  Nucleus release carries CONFIG_VSOCKETS or CONFIG_VIRTIO_MMIO. (7) The
  rollover formula is at most one counter unit short of the true increment on a
  wrap, 15.258 uJ on the reference profile, because the kernel's range is one
  raw unit below the modulus (drivers/powercap/intel_rapl_common.c); the
  boundary tests record that edge rather than hide it. (8) The criteria name no
  crate, and M21 adds one to the lock, socket2 0.6.5, whose SockAddr::vsock is
  the safe AF_VSOCK address constructor the workspace's forbid lint needs; it is
  admitted in docs/roadmap/toolchain-admission.md.
- Scope (criteria 4 and 10, 2026-09-29): host-fixture measurements on the
  reference profile, development evidence only: a pass qualifies no hardware,
  does not close the hardware gate, is not release evidence, and re-measurement
  inside an Aegis image remains a later acceptance. No boot time and no memory
  footprint is recorded; D71 leaves both to M22, and the scaffold's literals
  stay Unmeasured. Nothing here is evidence about isolation strength, and
  AF_VSOCK pricing (REQ-P10-03) is implemented nowhere. P10, P13 and P16 stay
  proposals in planning/components.json, whose blockers now record the runs;
  their candidate rows keep manifest_present false. M21 unblocks nothing; M22
  stays ready on M06.
- Done, and each part by the entry named: criterion 1 by the host entry;
  criterion 2 by the toolchain entry; criteria 3 and 5 and E21-1's positive by
  run r20260929T200422-2476, and criterion 5's host model and kernel kept beside
  the readings by run r20260929T205659-3afc; criterion 6 and E21-1's boundary by
  the rollover entry and run r20260929T201527-d65a; criterion 7 by the
  enumeration entry; criteria 8 and 11 and E21-1's negative by the
  privileged-read entry, as the disclosure records; criterion 9 by the accuracy
  entry; criterion 3's round-trip and E21-2 by run r20260929T200422-2476 and, on
  the final revision, r20260929T205659-3afc; E21-3 by its deferral; criteria 4
  and 10 by the scope entry. Each rests on cargo test and tools/ tests inside
  make verify-all or on the recorded runs, and none on simulated output.

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

Rank 15. State: done. Cost: large. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: yes. Needs external contract: no. Reference profile: full.
Blocked by: M15. Unblocks: M11.

Split from M11 after the reference profile was recorded, so a locally verifiable
slice no longer waits behind one that is not.

Exit criteria:

- Toolchain admission before any boot: QEMU 11.1.1, the edk2 OVMF images present
  on the reference profile (OVMF_CODE.4m.fd, OVMF_VARS.4m.fd,
  OVMF_CODE.secboot.4m.fd) and swtpm 0.10.2 are selected through the template
  matrix with pinned versions
- The harness is parameterised over an externally supplied bootable artifact
  named by a pin file, and is proven here on a pinned upstream distribution
  image; under D85, verifying an Imago return's signature before boot is M11's,
  which adds that pin scheme once M09 pins the signature form of the Imago
  result. Aegis constructs no image here; image construction stays with
  cordanaLLM/imago per docs/integration/stack.md
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
- ukify 262 (262-1-arch, re-read 2026-09-28; ukify 261 on 2026-09-13),
  sbsigntools 0.9.5, erofs-utils 1.9.4 and virt-firmware 26.9 are installed on
  the reference profile for the guest-side key store and unified kernel image
  signing
- The guest key store is generated from the shipped OVMF variables with
  virt-fw-vars and the unified kernel image is signed with sbsigntools, so no
  host firmware setting is changed; the host's own Secure Boot state is recorded
  but is not this milestone's subject

Cheapest exit: Boot a pinned upstream image headless under QEMU with OVMF and
swtpm, and retain the console log and PCR readback. No image is constructed.

Evidence (the D84 and D85 disclosures and the closing summary; every entry is
in `planning/roadmap.json`, and the run is on `docs/build/boot-harness.md`):

- Disclosure, in the shape M18 recorded, because a milestone that edits its own
  bar must say so where the bar is judged: epic E24-2 was rewritten in
  planning/roadmap.json during this delivery, under decision D84 (2026-09-28),
  before the state moved to done. Its acceptance read 'Positive: PCR values are
  read back from swtpm, and the verity root hash matches. Negative: a modified
  root image fails verity and does not boot to the established state. Boundary:
  a PCR policy that omits PCR 11 fails to unseal /var. The imported script is
  not used.' For this milestone that is a narrowing, and it is disclosed as one:
  the verity and unseal halves are not dropped but moved, the first to M11 as an
  exit criterion and epic E11-5 (REQ-P02-01) and the second to M20 as an exit
  criterion and epic E20-3 (REQ-P02-02), each with its own disclosure entry,
  because a pinned upstream image carries neither a verity root nor a TPM-sealed
  /var and criterion 2 forbids constructing an image here. What E24-2 keeps is
  strengthened rather than restated: the PCR read-back is from inside the guest
  and tied to the boot by a nonce, and PCR 7 is shown to change with the
  firmware's Secure Boot state. D84 changed none of the nine exit criteria;
  criterion 2 was restated later under D85, which the next entry discloses.
  docs/roadmap/README.md now renders criteria 8 and 9, which its M24 section had
  omitted; that restores the mirror and changes no bar.
- Disclosure, in the shape M18 recorded, because a milestone that edits its own
  bar must say so where the bar is judged: exit criterion 2 and epic E24-1 were
  rewritten in planning/roadmap.json during this delivery, under decision D85
  (2026-09-28), before the state moved to done. Criterion 2 read 'The harness is
  parameterised over an externally supplied bootable artifact: a pinned upstream
  distribution image or an Imago return. Aegis constructs no image here; image
  construction stays with cordanaLLM/imago per docs/integration/stack.md'.
  E24-1's acceptance read 'Positive: the supplied artifact's digest and
  signature verify locally before any boot, whether it is a pinned upstream
  distribution image or an Imago return. Negative: a tampered digest or a bad
  signature is refused before boot, not during it. Boundary: a producer version
  exactly at the floor is accepted and one below is refused. Under decision D72
  this milestone owns the harness and the boot evidence; consuming and verifying
  an actual Imago result is M11's.' For this milestone that is a narrowing, and
  it is disclosed as one: the Imago-return half is not dropped but moved to M11,
  as an exit criterion and as added sentences of E11-1, whose requirements
  (REQ-P01-01, REQ-P01-06, REQ-P01-08) are E24-1's, with its own disclosure
  entry there. It moved because the harness's pin schema
  aegis.m24.boot-artifact-pin.v1 implements one signature scheme,
  gpg-clearsigned-checksum, and load_pin refuses any other
  (tools/test_boot_harness.py holds that refusal), while the Imago return's
  signature form is not pinned: imago's proposed result schema
  imago.p01.product-result.v1 (its ADR-0020 at 16f964b) carries image-digest and
  a signature-ref string, is to be agreed in M09, which waits on producer
  builds, and nothing produces a result yet. A scheme added here would guess a
  contract M09 has not pinned. The negative and boundary halves of E24-1 and the
  rest of criterion 2 are unchanged and judged as written, and so are the other
  eight exit criteria and E24-2 as D84 scoped it. D85 also records that M11's
  sixth criterion and E11-2, which failed that milestone on any change to the
  harness, now except that one pin scheme; that relaxation is disclosed on M11.
- Done, as D84 and D85 scope the bar, and each part by the entry named: criteria
  1 and 8 by the toolchain entry (every tool read back before any boot, the
  three OVMF images admitted by sha256, the D69 comparison on the admission
  page); criterion 2 and E24-1's positive half by the upstream-artifact entry
  (the CHECKSUM signature by the pinned fingerprint, digest and version read
  from the signed text only, the bytes hashed before every boot, `--pin`, no
  image constructed); E24-1's negative and boundary halves and criterion 5's
  first half by the refusal entry (the tampered copy refused at the digest by
  the verification and by the boot entry, with no file written and no swtpm or
  QEMU started, BADSIG on the altered CHECKSUM, 44-1.7 admitted and 44-1.6
  refused); criteria 3 and 4 by the positive entry (swtpm attached, PCR 0, 4, 7
  and 11 read inside the guest and retained with the console log and the exit
  status, the host's disabled Secure Boot recorded beside them); criterion 5's
  second half and criterion 6 by the timeout entry (the 5 s miss with QEMU still
  running and stopped by the harness; 179, 180 and 181 s reported as reached,
  reached, missed); criterion 7 by the scope entry and the gate's own last line;
  criterion 9 by the key-store and signed-UKI entry; E24-2 by the positive,
  contrast and REQ-BOOT-02 entries. Each rests on the recorded run
  r20260928T195251-1a11 or on tools/test_boot_harness.py inside
  `make verify-all`, and none on simulated output.

Epics:

- **E24-1 Externally supplied artifact verified before boot**. Requirements:
  REQ-P01-01, REQ-P01-06, REQ-P01-08. Acceptance: Positive: the supplied
  artifact's digest and signature verify locally before any boot, for a pinned
  upstream distribution image. Negative: a tampered digest or a bad signature is
  refused before boot, not during it. Boundary: a producer version exactly at
  the floor is accepted and one below is refused. Under decision D72 this
  milestone owns the harness and the boot evidence; consuming and verifying an
  actual Imago result is M11's. Under decision D85 the Imago-return half of the
  positive case moved to M11 (its exit criterion and E11-1), which adds that pin
  scheme once M09 pins the signature form of the Imago result (the signature-ref
  of imago.p01.product-result.v1).
- **E24-2 Real boot evidence replaces the simulated script**. Requirements:
  REQ-BOOT-01, REQ-BOOT-02, REQ-P01-05. Acceptance: Positive: PCR 0, 4, 7 and 11
  are read back from inside a guest booted headless under KVM on the swtpm
  instance the harness attached, with that boot's own nonce beside them.
  Negative: a report without the boot's nonce is refused, and the imported
  integration script and any simulated output never count as boot evidence
  (REQ-BOOT-02). Boundary: each PCR is recorded as extended or at its reset
  value, as read, and PCR 7 is shown to change with the firmware's Secure Boot
  state on the same bytes. Under decision D84 the dm-verity acceptance moved to
  M11 (E11-5) and the /var unseal without PCR 11 to M20 (E20-3), because a
  pinned upstream image carries neither a verity root nor a TPM-sealed /var.

### M04 - UI accessibility harness: P12 Concordia tokens

Rank 16. State: done. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: no. Needs external contract: no. Reference profile:
not-hardware. Blocked by: M02. Unblocks: M16, M11.

Exit criteria:

- Toolchain admission: Node, pnpm, Playwright browsers and Svelte are selected
  through the template matrix with pinned versions (D10: latest stable versions
  at activation, fast adoption through Renovate, sveltesentio adopted now at its
  published per-package versions) before any UI gate runs; package manifests and
  lockfiles are committed for ui/concordia-tokens
- The Playwright and axe-core suite runs headless in a container with zero
  violations on the default state; it fails when the focus outline is removed;
  the focus width and contrast boundary follows D16
- D17 (Bootstrap fork versus no monolithic CSS) is recorded; no `|| true` in the
  accessibility gate
- The admitted Node, pnpm, Playwright and Svelte versions are committed as
  lockfiles and the admission explicitly resolves the M01 drift-register row:
  the reference profile runs Node v26.10.0 (v26.8.2 on 2026-09-13) and pnpm
  10.29.3, neither of which is the register's Node 20 or Node 22, so the row is
  closed with an exact pin and not inherited (D65).
- The Playwright browser revision is pinned to the browser revision bundled by
  the pinned @playwright/test and container image digest, and the gate must fail
  rather than silently download a different revision. The reference profile's
  host browser cache held chromium-1243 when observed on 2026-09-28
  (chromium-1228 on 2026-09-13); it is written by Praetor's figure engine and is
  never the evidence (D90).
- The Node major is the latest release line (26.x on the reference profile) and
  is tracked forward by Renovate rather than pinned to an older line (D65)
- D81: the gate runs the WCAG 2.2 AA tag set (wcag2a, wcag2aa, wcag21a,
  wcag21aa, wcag22aa) and asserts from its own results that the target-size rule
  executed; its report names the EN 301 549 V4.1.1 clause and the V3.2.1 clause
  for each criterion an executed rule evaluated, with 'none in V3.2.1' for
  11.2.5.8 (target-size), and lists 11.2.4.11 and 11.2.5.7, which no rule in the
  admitted axe-core version evaluates (4.13.0 has none), as not covered by this
  gate. The gate also covers REQ-P12-08, whose WCAG 2.2 AA pairing the tag set
  matches; D81 does not re-rule that row's edition.
- D76: every colour, stroke and motion value in the committed token file is a
  custom property, so the optional heads-up theme can override it without a
  second token source; a stub override that sets the focus-ring width below the
  D16 2px floor fails the existing boundary test (REQ-P12-10).

Cheapest exit: Build the token file plus one Svelte component and run the axe
suite in a container. No compositor and no daemons.

Evidence (the D90 and D89 disclosures and the closing summary; every entry is in
`planning/roadmap.json`, and the run is on
`docs/build/accessibility-harness.md`):

- Disclosure, in the shape M18 recorded, because a milestone that edits its own
  bar must say so where the bar is judged: exit criterion 5 was rewritten in
  planning/roadmap.json during this delivery, under decision D90 (2026-09-28),
  before the state moved to done. It read 'The Playwright browser revision is
  pinned to the version actually exercised; the reference profile has
  chromium-1228 cached, and the gate must fail rather than silently download a
  different revision.' It now pins the browser revision bundled by the pinned
  @playwright/test and container image digest, and records chromium-1243 as the
  host-cache revision observed on 2026-09-28. This is a correction and not a
  relaxation: the gate still fails rather than download another revision, and
  the pin now binds to the lockfile and the image digest instead of to a cache
  that Praetor's figure engine writes, whose own Playwright pin moved it from
  chromium-1228 to chromium-1243 on 2026-09-27. D65's text in
  docs/roadmap/README.md is restated the same way, and the README's M04 section
  mirrors the new criterion. No other exit criterion and neither epic's text
  changed.
- Disclosure, in the shape M18 recorded: E04-2 lists REQ-P12-01 (UKI compilation
  gated on an accessibility pass), REQ-P12-02 (the portal's settings over D-Bus)
  and REQ-P12-03 (AT-SPI2) while M04's cheapest exit has no compositor and no
  daemons. Under decision D89 (2026-09-28) M04 covers their CSS level only, with
  Playwright's reducedMotion and forcedColors emulation, each labelled in the
  test title, the gate output and docs/build/accessibility-harness.md as browser
  emulation. A third labelled test reads Chromium's accessibility tree; D89 does
  not name it, and it counts towards nothing. The portal half of REQ-P12-02, the
  AT-SPI2 half of REQ-P12-03 and the UKI gating of REQ-P12-01 are not met by
  M04, and no emulation counts as daemon evidence. D89 assigns them to the
  milestones that own the shell and the image, M16 and M27, but no milestone
  carries them yet: M16's epics list none of the three and its D77 criterion
  keeps the portal settings and AT-SPI2 on D-Bus mocks, M27 is the P17 VA-API
  slice, and M11 says only that the accessibility gate attaches when the UI
  enters an image. Until a recorded decision adds them to a milestone's criteria
  and epics they are open and owned by no milestone, listed with P12's
  activation blockers in planning/components.json and P12's row in
  docs/roadmap/inventory.md (recorded with D89 in docs/roadmap/README.md).
  E04-2's text is unchanged; this narrows what its three requirement ids are
  evidence of here, and it is disclosed as a narrowing.
- Done, as D89 and D90 scope the bar, and each part by the entry named: criteria
  1, 4 and 6 by the toolchain entry (Node 26.10.0 and pnpm 12.6.0 exact and
  locked, the drift row closed, Renovate tracking forward); criterion 2 by the
  suite entry (headless in the pinned container, zero violations, the stripped
  outline failing, the D16 width and 3:1 boundaries); criterion 3 by the D17 and
  suppression entry; criterion 5 by the browser entry; criterion 7 by the D81
  entry; criterion 8 by the D76 entry; E04-1 and E04-2 by their entries. Each
  rests on the recorded runs or on tools/test_a11y.py inside `make verify-all`,
  and none on simulated output: the emulation results are labelled as such and
  count as nothing more (D89).

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

Rank 18. State: done. Cost: small. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: no. Needs external contract: no. Reference profile:
not-hardware. Blocked by: M04, M14. Unblocks: M28.

Exit criteria:

- crates/aegis-forum-shell is a written-out workspace member with a Cargo.lock
  entry and [lints] workspace = true, so unsafe_code = "forbid" is inherited, on
  the Rust toolchain M02 admitted; its package name is the component name,
  aegis-forum-shell, which the activation binding in tools/verify_preparation.py
  (verify_evidence_binding) requires of a component's manifest. accesskit 0.25.1
  (MIT OR Apache-2.0), the data model only, is declared once in
  [workspace.dependencies], locked, and recorded in
  docs/roadmap/toolchain-admission.md before any gate runs. The crate's hygiene
  test reads its own dependency closure (D78) and refuses a GUI toolkit, a
  windowing or Wayland crate, a D-Bus crate, an AT-SPI adapter and an async
  runtime there: gpui, winit, wayland-client, zbus, atspi, accesskit_unix and
  tokio. No Node, pnpm, Playwright or npm package serves the shell (D101).
- The typed process lifecycle is D96's and is unit-tested with compositor,
  Justitia and Tellus inputs stubbed: the forward chain Eligible but Inactive ->
  Activated -> Rate-Limited -> Quarantined -> Deleted in the source's order
  (REQ-P05-03), the two recovery edges Rate-Limited -> Activated and Quarantined
  -> Eligible but Inactive, every live state -> Deleted, and Deleted terminal. A
  process is quarantined after QUARANTINE_LIMIT = 3 consecutive rate-limited
  windows, a named constant.
- The two inbound consumers decode the producers' Rust contract types directly,
  with no schema file, no generated binding and no copy in a second language:
  the DecisionRequest of aegis_justitia::contracts::decision_request (M14), and
  a CarbonTelemetry update that this milestone types in crates/aegis-tellus,
  where P13's schemas live, as aegis.p13-p05.carbon-telemetry.v1, because no
  telemetry contract exists there yet
  (crates/aegis-tellus/src/contracts/graph.rs records the payload as M16's). D31
  stays open, so the payload carries neither an emission interval nor a delivery
  deadline. The P05/P12 token-edge direction (D05) is recorded; under D101 the
  shell consumes the tokens as Rust theme constants that M28 generates from the
  M04 token file.
- D77 framing: DISPATCH_DECISION_REQUEST from P06 and EMIT_CARBON_TELEMETRY from
  P13 each arrive as one line-delimited JSON-RPC 2.0 message on a mocked AF_UNIX
  stream (a socketpair) whose every read has a deadline and every line a byte
  bound (HISS-02); a line that is not valid JSON-RPC 2.0 is answered with a
  JSON-RPC error and the stream continues; the StatusNotifierWatcher, the portal
  settings and AT-SPI2 stay on D-Bus mocks, and M29 runs them live.
  SYNC_DESKTOP_SHELL from P04 keeps the Unix socket stream the graph records.
  D32 (2026-09-29) adopts /run/aegis/compositor.sock as its endpoint and under
  100 microseconds as its per-hop target; the crate records both as named
  constants and no test here asserts either, because no real P04 socket exists:
  a contract test asserts them once one does, and until it is measured the
  figure is a target, not a claim (ADR-0001).
- D74 canvas model: the node registry, camera and QuadTree cull bounds are part
  of the tested shell state, and the accessibility tree is exported as an
  accesskit::TreeUpdate built from that state, with no toolkit and no platform
  adapter. Culling changes only what is painted: with 1,000 seeded nodes and 10
  in view the TreeUpdate still carries all 1,000 nodes in focus order, each with
  its role, name and bounds (REQ-P05-11); focusing a culled node moves the
  camera until the node is in view (REQ-P05-09); and a keyboard-only walk over
  the model reaches every node and invokes every action, while Escape and Tab
  leave every instrument (REQ-P05-09, REQ-P05-10).
- REQ-P12-06 (D91, re-mapped by D101): the shell's accessibility check is a tree
  check over the exported TreeUpdate, not an axe-core scan. In the default state
  and on the seeded canvas every node carries a role and a non-empty name, a
  planted unlabelled node fails the check and is named, and the empty canvas
  exports a labelled empty state. It runs inside make verify-all through cargo
  test. The M04 gate is unchanged and keeps scanning the P12 token component
  only; this milestone runs no browser, container or Node toolchain for the
  shell.
- D100: ESLint enforces HISS-01, HISS-04 and HISS-08 on the JavaScript and
  Svelte under ui/ (ui/concordia-tokens today) until cordanaLLM/praetor#589
  ships a JavaScript and Svelte scanner: eslint, eslint-plugin-svelte and
  svelte-eslint-parser admitted with exact pins, a flat configuration with
  max-lines-per-function 60, complexity 10, max-statements 50, no-eval,
  no-implied-eval, no-new-func and a local rule that bans direct self-recursion,
  zero warnings and no inline configuration, run offline in the pinned M04
  container inside make verify-a11y. Positive: the committed JavaScript and
  Svelte lint clean. Negative: one planted violation per rule family fails the
  gate. Boundary: a 60-line function passes and a 61-line one fails. Mutual
  recursion is not detected: there is no call-graph check.
- Scope: the crate paints nothing, creates no window or surface, opens no real
  socket and contacts no D-Bus daemon, so a pass is model-level evidence and
  closes no accessibility gate for the running shell, which M28 checks over
  AT-SPI and M29 in a live session. The rendered-pixel cases E16-3 carried until
  2026-09-29 -- the measured focus ring at the minimum, 1.0 and maximum camera
  scale, the 1.5 px scaled-layer negative and focus under a panel -- are M28's
  (E28-4).

Cheapest exit: Unit-test the Rust lifecycle, the canvas model and its
accesskit::TreeUpdate export, and the two consumers typed on the producers' Rust
contracts, over a socketpair framed as line-delimited JSON-RPC 2.0, with D-Bus
mocks only for the platform interfaces and no toolkit, display or bus.

Evidence (the 2026-09-29 disclosure, the scope and the closing summary;
every entry is in `planning/roadmap.json`, and the runs are on
`docs/build/forum-shell.md` and `docs/build/accessibility-harness.md`):

- Disclosure, in the shape M18 recorded, because a milestone that edits its own
  bar must say so where the bar is judged: before any delivery, on 2026-09-29
  under decisions D101 (the whole P05 shell is native, ADR-0004), D96, D100 and
  D32, every exit criterion, the cheapest exit and all four epics were
  rewritten, and two criteria were added. Criterion 1 read 'ui/forum-shell has
  its package manifest and lockfile on the toolchain admitted in M04'; it now
  names crates/aegis-forum-shell on the Rust toolchain and admits accesskit
  0.25.1. Criterion 2 read 'The shell state and typed process lifecycle are
  unit-tested with compositor, Justitia and Tellus inputs stubbed'; it now names
  D96's edges and limit. Criterion 3 read 'The DecisionRequest consumer is typed
  against the M14 schema; the P05/P12 token-edge direction (D05) is recorded';
  it now types both consumers on the producers' Rust types and adds the
  CarbonTelemetry contract to aegis-tellus. Criterion 4 read 'D77 framing: the
  two stubbed inbound edges D77 moves off D-Bus, DISPATCH_DECISION_REQUEST from
  P06 and EMIT_CARBON_TELEMETRY from P13, each arrive as one line-delimited
  JSON-RPC 2.0 message on a mocked AF_UNIX stream; a line that is not valid
  JSON-RPC 2.0 is answered with a JSON-RPC error and the stream continues; the
  StatusNotifierWatcher, the portal settings and AT-SPI2 stay on D-Bus mocks;
  SYNC_DESKTOP_SHELL from P04 keeps the Unix socket stream the graph records,
  whose endpoint and budget stay open under D32.'; it drops the lead-in naming
  the two edges as stubbed ones D77 moves off D-Bus, makes the mocked stream a
  socketpair whose every read has a deadline and every line a byte bound
  (HISS-02), adds that M29 runs the D-Bus interfaces live, and replaces its last
  clause, 'whose endpoint and budget stay open under D32', with D32's endpoint
  and target, which no test here asserts. Criterion 5 read 'D74 canvas state:
  the canvas node registry, camera and QuadTree cull bounds are part of the
  tested shell state, and culling changes only what is painted: with 1,000
  seeded nodes and 10 in view the exported accessibility snapshot still lists
  all 1,000 in focus order (REQ-P05-11), and moving focus to a culled node moves
  the camera to reveal it (REQ-P05-09).'; it now reads 'D74 canvas model', the
  snapshot is an accesskit::TreeUpdate built from the model with no toolkit and
  no platform adapter, whose nodes each carry a role, name and bounds, 'to
  reveal it' becomes 'until the node is in view', and the keyboard walk with
  Escape and Tab (REQ-P05-10) joins it. Criterion 6 read 'REQ-P12-06 (D91): the
  M04 accessibility gate also scans ui/forum-shell in the same digest-pinned
  container, with the D81 tag set and no impact filter, and reports zero
  violations and zero incomplete on the shell's default state; the shell reuses
  M04's toolchain admission and builds no second harness'; it is now the
  AccessKit tree check. The cheapest exit read 'Unit-test the Svelte store and
  lifecycle logic with mocked sockets, the two edges D77 moves framed as
  line-delimited JSON-RPC 2.0, and D-Bus mocks only for the platform
  interfaces.' E16-3, titled 'Canvas state and D77 framing', carried REQ-P05-13
  and read 'Positive: a keyboard-only walk over a seeded canvas reaches every
  node and invokes every action, ending in the same store state as the pointer
  path; from each instrument Escape restores focus to its node and Tab reaches
  the next shell region; at the minimum, 1.0 and maximum camera scale the
  measured focus ring is at least 2 px and 3:1; and a DecisionRequest and a
  telemetry update each arrive as one JSON-RPC 2.0 line. Negative: culling
  implemented by removing nodes from the snapshot fails the 1,000-node check, an
  instrument with only a pointer handler fails the walk, a transcluded fragment
  that swallows Tab fails, a focus ring drawn inside the scaled layer (1.5 px at
  scale 0.5) fails, a focused node fully under a panel fails, and a line with
  jsonrpc other than "2.0" is refused with an error while the stream continues.
  Boundary: a node exactly on the cull boundary and a node straddling the
  viewport edge are both listed; a 2.0 px focus ring passes and 1.9 px fails; a
  partly covered focused node passes; Escape from the deepest nested instrument
  returns focus to canvas level in exactly that many presses; a disabled
  single-letter shortcut no longer fires; an empty canvas exposes a labelled
  empty state.' Its rendered-pixel cases, the measured focus ring at the
  minimum, 1.0 and maximum camera scale, the focus ring drawn inside the scaled
  layer, the focused node fully under a panel, the 2.0 px and 1.9 px ring and
  the partly covered focused node, need rendered pixels and move unchanged in
  substance to M28's E28-4 with REQ-P05-13, so the shell's bar keeps them and
  M16's no longer does. The rest of E16-3 is reworded for the model: the walk
  ends in the same state rather than the same store state; the snapshot becomes
  the accesskit::TreeUpdate, so culling by dropping or hiding nodes in it fails,
  and boundary nodes are exported rather than listed; an instrument with only a
  pointer action, not a pointer handler, fails the walk; an invalid jsonrpc line
  is answered with a JSON-RPC error rather than refused with an error. Two
  boundaries are added, focusing a culled node moves the camera until it is in
  view and a line at the byte bound is accepted while one byte longer is
  refused, and the empty-canvas boundary moves to E16-4, which already carried
  it. E16-4, titled 'Forum Shell accessibility scan (D91)', read 'Positive: the
  shell's default state and its seeded canvas scan with zero violations and zero
  incomplete under the D81 tag set. Negative: a planted violation in the shell,
  an unlabelled control, fails the gate. Boundary: the empty canvas is scanned
  as well and passes with its labelled empty state.' The tree check that
  replaces it is narrower and this entry says so: it asserts a role and a name
  on every node and the labelled empty state, not the WCAG 2.2 AA rule set
  axe-core applies to a DOM, because the native shell has no DOM; ADR-0004
  records it as a negative consequence, and REQ-P05-08 moves from E16-1 to E16-4
  with it. E16-1's acceptance read 'Positive: each lifecycle transition in the
  source order succeeds. Negative: a transition from Deleted is rejected.
  Boundary: Rate-Limited to Quarantined at the limit is exercised exactly.' and
  E16-2's read 'Positive: DecisionRequest and Tellus telemetry payloads parse.
  Negative: an unknown schema version is rejected. Boundary: a telemetry update
  with zero watts renders without error.'; both are strengthened, with D96's
  edges and limit and with decoding into the producers' types. The added
  criteria are the D100 lint and the scope statement. blocked_by keeps M04,
  whose token file is the single source the shell's theme constants derive from
  (D05, D101), and M14, and M16 now unblocks M28. This entry is not evidence
  that any criterion is met.
- Scope (criterion 8, 2026-09-29): model-level evidence on the reference
  profile. The crate paints nothing, creates no window or surface, opens no real
  socket and contacts no D-Bus daemon; the only sockets are the socketpairs its
  tests create, and tests/stubbed_effects.rs sweeps the sources for socket, bus,
  process, thread, clock and file identifiers and finds none. A pass closes no
  accessibility gate for the running shell, which M28 checks over AT-SPI and M29
  in a live session, and the rendered-pixel cases are M28's E28-4. P05 stays a
  proposal in planning/components.json: its blockers now name what the crate
  holds and what M28 and M29 add, and its candidate row C05 keeps
  manifest_present false, which verify_candidates() reads as an activation
  claim. M16 unblocks M28, which stays blocked on M27.
- Done, and each part by the entry named: criterion 1 by the crate entry;
  criterion 2 and E16-1 by the lifecycle entry; criterion 3 and E16-2 by the
  consumers entry; criterion 4 by the framing entry; criterion 5 and E16-3 by
  the canvas entry; criterion 6 and E16-4 by the tree-check entry; criterion 7
  by the D100 entry; criterion 8 by the scope entry. Each rests on cargo test or
  on the recorded gate runs inside make verify-all, and none on simulated
  output.

Epics:

- **E16-1 Forum shell crate and the D96 lifecycle with stubs**. Requirements:
  REQ-P05-01, REQ-P05-02, REQ-P05-03, REQ-P05-04, REQ-P05-05, REQ-P05-07,
  REQ-P04-07. Acceptance: Positive: each forward transition in the source order
  succeeds, and Rate-Limited to Activated and Quarantined to Eligible but
  Inactive succeed. Negative: every transition out of Deleted is refused, as is
  a transition the table does not list, such as Eligible but Inactive to
  Quarantined. Boundary: the third consecutive rate-limited window quarantines
  the process and the second does not; a window within budget between two
  rate-limited ones restarts the count.
- **E16-2 Consumers typed on the producers' Rust contracts, and the token-edge
  decision**. Requirements: REQ-GRAPH-01, REQ-P13-04, REQ-P06-08. Acceptance:
  Positive: a DecisionRequest and a CarbonTelemetry update each decode into the
  producer crate's own Rust type and update the shell state. Negative: an
  unknown schema version is refused with a typed error. Boundary: a telemetry
  update with zero watts decodes and updates the state without error.
- **E16-3 Canvas model, AccessKit tree export and D77 framing**. Requirements:
  REQ-P05-09, REQ-P05-10, REQ-P05-11, REQ-P17-04. Acceptance: Positive: a
  keyboard-only walk over a seeded canvas model reaches every node and invokes
  every action, ending in the same state as the pointer path; from each
  instrument Escape restores focus to its node and Tab reaches the next shell
  region; 1,000 seeded nodes with 10 in view export 1,000 nodes in focus order
  in the accesskit::TreeUpdate; and a DecisionRequest and a telemetry update
  each arrive as one JSON-RPC 2.0 line. Negative: culling implemented by
  dropping or hiding nodes in the TreeUpdate fails the 1,000-node check, an
  instrument with only a pointer action fails the walk, a transcluded fragment
  that swallows Tab fails, and a line with jsonrpc other than "2.0" is answered
  with a JSON-RPC error while the stream continues. Boundary: a node exactly on
  the cull boundary and a node straddling the viewport edge are both exported;
  focusing a culled node moves the camera until it is in view; Escape from the
  deepest nested instrument returns focus to canvas level in exactly that many
  presses; a disabled single-letter shortcut no longer fires; a line at the byte
  bound is accepted and one byte longer is refused.
- **E16-4 Forum Shell accessibility tree check (D91, re-mapped by D101)**.
  Requirements: REQ-P12-06, REQ-P05-08. Acceptance: Positive: the default state
  and the seeded canvas export a TreeUpdate in which every node has a role and a
  non-empty name. Negative: a planted unlabelled node fails the check, and the
  failure names it. Boundary: the empty canvas exports its labelled empty state
  and passes.

### M25 - GPU DMA-BUF sharing and VFIO passthrough slices

Rank 21. State: ready. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
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

Rank 22. State: ready. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
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

- **E22-1 Measured microVM boot time and footprint, each naming its VMM**.
  Requirements: REQ-P10-01, REQ-P10-04, REQ-P10-05. Acceptance: Positive: a
  measured boot time and memory footprint replace the scaffold literals that
  M06 carried as unmeasured, and each figure records which VMM produced it.
  Negative: a figure carrying no VMM identity is refused rather than recorded,
  and Firecracker and QEMU microvm numbers are never presented as
  interchangeable (D58). Boundary: the sandbox VMM's device passthrough limits
  are read from the pinned release rather than assumed, and Firecracker's
  absence of PCI passthrough sends that work to the QEMU harness rather than
  being worked around.
- **E22-2 AF_VSOCK transport measured on the admitted VMM**. Requirements:
  REQ-P10-03, REQ-P16-04. Acceptance: Positive: one candidate evaluation
  round-trips over the measured transport with /dev/vhost-vsock and
  CONFIG_VHOST_VSOCK=m recorded as the mechanism, and the figure names its
  VMM. Negative: the transport is not assumed from the round-trip M21 proved
  -- a measurement taken on a different VMM than the one recorded is refused.
  Boundary: a pass here is development evidence on the reference profile and
  closes no hardware gate.

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
  PREEMPT_RT, and measuring what that buys is M23. The guest's userspace is the
  host's own bash, mount, uname, gzip and sleep with their library closure;
  only the kernel under test is built here. The image, kernel-artifact, boot,
  hardware and release gates remain blocked
- Dated 2026-09-29, appended after done (D107); no exit criterion and no epic
  text changes. build/kernel/50-aegis-requirement.config is rendered again by
  KernelRequirement::config_fragment from the D107 payload and carries
  CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS=y (REQ-P06-05) after CONFIG_BPF_LSM,
  fourteen assignments. The symbol has no prompt, and its chain was read from
  the pinned linux-7.2.5 source, hashed against the pin: kernel/trace/Kconfig
  makes it a def_bool depending on DYNAMIC_FTRACE_WITH_REGS ||
  DYNAMIC_FTRACE_WITH_ARGS and on HAVE_DYNAMIC_FTRACE_WITH_DIRECT_CALLS,
  DYNAMIC_FTRACE_WITH_REGS and DYNAMIC_FTRACE_WITH_ARGS are def_bools over
  DYNAMIC_FTRACE, DYNAMIC_FTRACE depends on FUNCTION_TRACER, and FUNCTION_TRACER
  has no default and sits inside if FTRACE; under config X86, arch/x86/Kconfig
  selects HAVE_FUNCTION_TRACER, HAVE_DYNAMIC_FTRACE,
  HAVE_DYNAMIC_FTRACE_WITH_REGS and HAVE_DYNAMIC_FTRACE_WITH_DIRECT_CALLS, and
  x86_64_defconfig sets CONFIG_DEBUG_KERNEL=y, which defaults FTRACE on, and no
  FUNCTION_TRACER. build/kernel/10-base-support.config therefore gains
  CONFIG_FTRACE=y, CONFIG_FUNCTION_TRACER=y and CONFIG_DYNAMIC_FTRACE=y with
  that chain named above them, and with what BPF_LSM itself depends on: FTRACE,
  through BPF_EVENTS, which sits inside if FTRACE, but none of FUNCTION_TRACER,
  DYNAMIC_FTRACE or DYNAMIC_FTRACE_WITH_DIRECT_CALLS (kernel/bpf/Kconfig); it
  still assigns no payload symbol; tools/test_kernel_build.py requires the
  three. Negative, by hand over the pinned tree: the support fragment of before
  D107 with the new requirement fragment produced CONFIG_FTRACE=y,
  '# CONFIG_FUNCTION_TRACER is not set' and no DYNAMIC_FTRACE line, which the
  gate's own check refuses with 'CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS:
  REQ-P06-05 requires built-in, observed unrecorded'. `make verify-kernel` ran
  three times, each reading 14 requirement rows, passing all seven cases and
  exiting 0: into a fresh AEGIS_KERNEL_BUILD_DIR (2026-09-29, 21:13:22 to
  21:15:52 UTC: the tarball downloaded, its sha256 and signature verified and
  extracted anew, the bzImage built in 112 s), over the same tree (21:23:38 to
  21:26:30 UTC: a 16,954,368-byte bzImage and 19 modules as 7.2.5-aegis-m26 in
  142 s), and over the same tree again after the last edit of any file the gate
  reads, which narrowed the support fragment's comment on what BPF_LSM depends
  on and changed no assignment, and after this change was rebased onto main at
  a28be4e (21:53:55 to 21:57:34 UTC: a 16,954,368-byte bzImage and 19 modules in
  126 s); a direct `python3 tools/verify_kernel_build.py` between the first two
  gave the same. In the last two runs the guest's own /proc/config.gz was
  byte-identical to the produced .config, 5,545 lines, sha256
  90e08c39b2bcbacb257cb15e86d4e5d0fd82270105cd4b9d0c80d29f52088585, with
  CONFIG_FTRACE, CONFIG_FUNCTION_TRACER, CONFIG_DYNAMIC_FTRACE and
  CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS all y, and
  kernel/readback-negative-missing-option refused the host's configuration on
  CONFIG_PREEMPT_RT alone, the host carrying the new row. The rebuild reads the
  configuration back and attaches nothing; that this configuration attaches
  action_gate is M10's controlled comparison. docs/build/kernel.md records the
  runs.

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

Rank 14. State: done. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: yes. Needs external contract: no. Reference profile: full.
Blocked by: M07, M26. Unblocks: M10.

Split from M07 after the reference profile was recorded, so a locally verifiable
slice no longer waits behind one that is not.

Exit criteria:

- Toolchain admission: QEMU 11.1.1, cyclictest 2.10 from rt-tests 2.10-1.1 and
  the realtime kernel source are selected through the template matrix with
  pinned versions. The kernel source row is the M26 pin reused unchanged, not a
  distribution package: the original wording said 'the realtime kernel
  package', and no package is admitted, downloaded or installed here
- D70 is applied: the guest kernel is an interim source for the latency fixture
  only. the guest kernel is the one this repository builds in M26 while Nucleus
  is a scaffold, with its configuration read back from inside the guest; the
  kernel the product ships is built by Nucleus against the M18 schema once
  Nucleus is real
- D57 is recorded with the realtime kernel obtained without modifying the
  reference host. M26's kernel satisfies that by construction rather than by a
  download-only procedure, so the distribution-package path this criterion
  originally described is not performed: no linux-rt package is downloaded,
  installed or booted, the image is a file under AEGIS_KERNEL_BUILD_DIR that no
  package owns, and no /lib/modules entry exists for its release
- The guest kernel's configuration is read back from inside the virtual machine
  and confirms CONFIG_PREEMPT_RT, while the same probe on the reference host
  confirms it is not set there
- Positive: the latency fixture produces measured figures for the tier
  thresholds on the guest. Negative: the same fixture on the non-realtime host
  is recorded as not satisfying the determinism claim. Boundary: a run at the
  threshold and one step beyond it are reported differently
- A pass here is development evidence on the reference profile and closes no
  hardware gate

Cheapest exit: Boot the realtime kernel M26 already built in a guest and run
the admitted latency tool there, then run it again on the reference host and
record that the host is not realtime. The wording said 'the distribution
realtime kernel' before D70 moved kernel construction into this repository; no
distribution kernel is downloaded or booted.

Epics:

- **E23-1 Realtime guest kernel provenance and configuration read-back**.
  Requirements: REQ-P07-01, REQ-P07-02, REQ-P08-08. Acceptance: Positive: the
  guest boots the kernel this repository builds in M26 and reports
  CONFIG_PREEMPT_RT from inside itself, with the release string identifying
  that build and no other. Negative: the same probe on the reference host
  reports the option not set, so the two readings cannot be confused. Boundary:
  the reference host is never modified -- any distribution realtime package is
  downloaded only, with no installation and no bootloader entry (D57), and the
  guest kernel is an interim source that returns to Nucleus once Nucleus builds
  against the M18 schema (D70).
- **E23-2 Latency fixture for the P07 and P08 tier thresholds**. Requirements:
  REQ-P07-01, REQ-P07-06, REQ-P08-02, REQ-P08-08. Acceptance: Positive: the
  fixture produces measured figures for the tier thresholds on the realtime
  guest, replacing the Declared values M07 recorded as unmeasured. Negative:
  the same fixture run on the non-realtime host is recorded as not satisfying
  the determinism claim, rather than as a lower number. Boundary: a run at a
  threshold and one step beyond it are reported differently. Every figure names
  the kernel that produced it, and a pass is development evidence that closes
  no hardware gate.

Evidence:

- Disclosure, in the shape M18 recorded, because a milestone that edits its own
  bar must say so where the bar is judged: two of this milestone's six exit
  criteria, and its cheapest exit, were rewritten in planning/roadmap.json
  during this delivery and before the state moved to done. None is a relaxation.
  Criterion 1 read 'Toolchain admission: QEMU 11.1.1 and the realtime kernel
  package are selected through the template matrix with pinned versions'; it now
  admits cyclictest 2.10 from rt-tests 2.10-1.1 as well -- the tool the fixture
  actually runs, which the original omitted -- and names the kernel row as M26's
  source pin reused unchanged rather than a package, so strictly more is pinned
  than before and nothing less. Criterion 3 read 'D57 is recorded with the
  realtime kernel obtained without modifying the reference host: the
  distribution package is downloaded only (no installation, no bootloader entry)
  and its kernel image and modules are passed to the guest'; D70 moved kernel
  construction into this repository, so that procedure was not performed and the
  criterion now records satisfaction by construction. D57's actual constraint --
  that the reference host is not modified -- is preserved and met more strictly,
  because nothing is downloaded at all rather than downloaded and left
  uninstalled. The cheapest exit read 'Boot the distribution realtime kernel in
  a guest and run the latency fixture there, recording that the reference host
  itself is not realtime'; it names the kernel M26 already built for the same
  reason, and adds the second run on the host that criterion 5 requires. No
  other criterion changed: 2 and 4 to 6 are judged exactly as they were written.
  The convention is prose rather than a gate, and that is stated rather than
  implied: make verify-all checks that planning/roadmap.json and
  docs/roadmap/README.md agree on rank and state, and checks nothing about
  whether a rewritten criterion carries a disclosure row.
- D57 satisfied by construction, and the path not taken is named rather than
  invented: the realtime kernel is the bzImage M26 builds from the pinned,
  signature-verified linux-7.2.5 with CONFIG_PREEMPT_RT=y. D57 recommended a
  distribution linux-rt package booted download-only as a guest kernel; D70
  then put kernel construction in this repository while Nucleus is a scaffold,
  so that procedure is not performed and this milestone records why instead of
  describing a download it never made. Three checks the developer account can
  actually run stand behind it, each re-run by the gate: `pacman -Qo <bzImage>`
  prints 'error: No package owns ...', `pacman -Qq linux-rt` prints "error:
  package 'linux-rt' was not found", and /lib/modules holds
  ['6.18.50-1-cachyos-lts', '7.2.4-arch1-2', '7.2.5-1-cachyos'] with
  7.2.5-aegis-m26 not among them. What is NOT checked is said rather than
  implied: /boot is mode 0700 root on this profile, so the gate cannot
  enumerate bootloader entries as the developer account and makes no claim
  about them
- In place of that enumeration, a narrower statement that is mechanical:
  ALLOWED_PROGRAMS in tools/test_latency_fixture.py is the complete set of
  programs the gate may start -- qemu-system-x86_64, cyclictest, cpio, ldd,
  bash, mount, cat, grep, gzip and pacman -- and the test resolves it from the
  gate's own syntax tree rather than from a list someone maintained by hand. It
  is asserted as an EQUALITY against the union of the resolved call sites and
  the TOOLCHAIN rows, so the allowlist can carry neither an unlisted program
  nor a dead entry. The resolution is per function, and the first form of it
  was wrong in the direction that hides things rather than reports them:
  resolving argv bindings across the whole module made the toolchain version
  calls resolve to another function's literal vector and look decided, which
  silently dropped four of the ten programs. Scoped to one function, the sweep
  now reports two call sites it cannot name -- run(), the gate's own deadline
  wrapper, and read_version(), which runs one TOOLCHAIN row -- and the test
  names both and resolves the second from the table. pacman appears only as -Qo
  and -Qq, which are queries, and the same test asserts the gate names no
  bootctl, modprobe, insmod or pacman -S anywhere
- D70 applied and its limit stated where the claim is made: the guest kernel is
  an interim source for this fixture only. It is the kernel make verify-kernel
  builds while Nucleus is a scaffold; the kernel the product ships is built by
  Nucleus against the M18 schema once Nucleus returns real artifacts against
  the M09 contract. Nothing here builds, packages, signs, installs or publishes
  a kernel; this milestone consumes M26's output and adds no second
  construction path
- The configuration is read back from inside the virtual machine, and the host
  is read with the same probe -- one file, not two implementations.
  tools/guest/aegis-preempt-probe.sh is copied into the guest's initramfs and
  executed there, and executed by the gate on the host; it decompresses the
  running kernel's own /proc/config.gz and prints the matching line verbatim
  rather than a verdict, so the two readings compare as text. The interpreter
  is the same program too: the guest's /bin/sh is a copy of this host's bash,
  and the gate runs the probe under bash rather than sh for that reason. Guest,
  from inside the machine: 'AEGIS-M23-UNAME-R 7.2.5-aegis-m26',
  'AEGIS-M23-UNAME-V #1 SMP PREEMPT_RT Sun Sep 13 18:55:36 CEST 2026',
  'CONFIG_PREEMPT_RT=y'. Host, same file: uname -r 7.2.4-1-cachyos and '#
  CONFIG_PREEMPT_RT is not set'. The gate also refuses a guest that reports the
  host's own release
- The read-back is evidence for the current run and was falsified as such. The
  gate deletes the report file before the emulator starts and hands the guest a
  fresh nonce on its own kernel command line as `aegis.nonce=<value>`; the
  guest prints it back and the gate refuses any other value. Both halves were
  exercised together by patching the guest to print a fixed nonce, which
  produced 'FAIL latency/guest-preempt-rt ... the report carries nonce
  'hand-typed-0001', not this run's
  'aegis-1539807-34e0b85bea9f557fa3fdf0c09731e848'; it was not written by this
  boot and proves nothing about it'. The guest reports on a second serial line
  so a kernel printk cannot splice itself into the JSON, and powers itself off
  through magic SysRq, so the gate's 600 s deadline is a failure signal rather
  than the normal exit path
- The tool is the admitted standard one, not a fixture written here: cyclictest
  2.10 from rt-tests 2.10-1.1 (cachyos-extra-znver4), read back from
  `cyclictest --help` -- which prints 'cyclictest V 2.10'; --version is not an
  option it accepts -- and from `pacman -Qi rt-tests`. It is the only host-side
  installation this milestone made and it is admitted in
  docs/roadmap/toolchain-admission.md with a floor of 2.10, because the gate
  parses the --json payload and refuses one whose resolution_in_ns is not 1,
  and that output shape was observed on 2.10 and on nothing else. QEMU 11.1.1
  and the guest userspace tools are admitted in the same table, which
  tools/test_latency_fixture.py compares as sets against the gate's own
  TOOLCHAIN list
- Methodology, stated so it can be disputed. One SCHED_FIFO thread sleeps on a
  periodic clock_nanosleep and records, each cycle, the difference between the
  wakeup it was programmed for and the wakeup it got; the maximum over the run
  is the worst-case wakeup latency, and that sentence is carried in the type as
  MeasurementTool::measures so a figure cannot be read as something else at its
  use site. One argument vector runs on both machines -- `--default-system
  --mlockall --priority=95 --interval=200 --distance=0 --threads=1 --affinity=1
  --loops=50000 --nsecs --quiet` -- and the two runs are compared through the
  argument vectors cyclictest records in its own JSON, which is evidence that
  they matched rather than an assertion. Three arguments are decisions:
  --default-system stops cyclictest writing /dev/cpu_dma_latency, which on the
  host would be a modification (D57), at the stated cost that idle states are
  not suppressed and the figures include C-state exit latency; --priority=95 is
  REQ-P08-02's RLIMIT_RTPRIO rather than cyclictest's customary 99, reachable
  because the account's ulimit -r reads 99; --nsecs puts the figures in the
  unit the P07 edges use
- Positive, measured on the guest and replacing what M07 recorded as
  unmeasured: 50000 cycles, min 1510 ns, mean 9664 ns, max 273969 ns on
  7.2.5-aegis-m26 (PREEMPT_RT). Against the P07 tier edges and the P08 target,
  read out of the crates by the gate rather than restated in it:
  BURST_CRITICAL_NS 100000 ns is worst-case-exceeds; BURST_INTERACTIVE_NS
  2000000 ns, BURST_FRAME_NS 8000000 ns and TARGET_RTL_LATENCY_NS 5000000 ns
  are each satisfied. The critical tier was NOT attained and is reported as not
  attained: REQ-P07-01's deterministic tier-0 response is still not
  demonstrated, and no friendlier threshold was chosen to make it look so
- Negative, and it is not a smaller number. The same argument vector on the
  reference host gave 50000 cycles, min 440 ns, mean 1449 ns, max 267461 ns on
  7.2.4-1-cachyos (PREEMPT_DYNAMIC) -- a LOWER worst case than the realtime
  guest -- and every one of its four verdicts is kernel-not-realtime.
  DeterminismVerdict::of checks the kernel before the attainment, so a host
  figure never reaches the numeric comparison; the falsifier is a host reading
  of zero nanoseconds, the best conceivable figure, which
  crates/aegis-calliope/tests/measured_figures.rs pins as still refused, and
  the gate re-checks the same thing at run time. Ranking the two by figure
  would also be unstable, not merely wrong: five runs recorded on 2026-09-13, a
  sample of that day's runs rather than all of them, gave guest maxima 273969,
  214129, 139079, 203358 and 290108 ns against host maxima 267461, 192784,
  151594, 184724 and 257986 ns, so the host's was the lower one four times and
  the higher one once, while the verdicts did not move in any of them.
  docs/build/latency.md carries the table
- Boundary: a worst case one nanosecond below a threshold, exactly on it, and
  one nanosecond past it are three outcomes with three names --
  attained/satisfied, at-edge/worst-case-at-edge, exceeded/worst-case-exceeds.
  The edge is its own outcome because the classifier it reports against is
  strict: scx_cake.bpf.c writes `if (ewma < BURST_CRITICAL_NS)`, so a burst of
  exactly 100000 ns is not critical, and a fixture calling that 'attained'
  would claim a tier Tier::classify would not agree with.
  crates/aegis-lictor/tests/determinism_fixture.rs checks both sides at the
  same edge. BURST_CRITICAL_NS is pinned to 100_000 with assert_eq! in both
  crates and in tools/test_latency_fixture.py before anything is evaluated
  against it, so no boundary test compares a constant with itself. The boundary
  values are evaluated through TierAttainment::evaluate, which takes two
  integers and carries no provenance, so exercising the edge does not mint
  three figures nobody observed
- A measured figure and a declared one cannot be confused, and the separation
  is in the type system rather than in prose. `Declared<T>` (M07) and
  `Measured<T>` (M23) are distinct types in aegis-calliope with no conversion
  in either direction; Measured::new demands a KernelIdentity -- a release
  string plus a PreemptionModel, both read back from inside the machine that
  produced the figure -- and a MeasurementTool, neither of which a
  `Declared<T>` has anything to supply. Their renderings differ in the same
  direction: '5 (declared, unmeasured)' against '273969 (measured by cyclictest
  on 7.2.5-aegis-m26, PREEMPT_RT)'. tests/declared_literals.rs keeps M07's
  three constants unusable as integers and additionally sweeps the crate's own
  sources for six spellings of a conversion between the wrappers; a planted
  `impl From<Declared<u64>> for Measured<u64>` was reported as
  `measured.rs:513: impl From<Declared` and failed the suite.
  TARGET_RTL_LATENCY_NS stays Declared, because a threshold is not made into an
  observation by having an observation compared against it. The pattern is
  M05's Provenance in crates/aegis-tellus/src/power.rs: a figure carries how it
  was obtained, and the rule deciding the label is written down and tested
- One reading of the tool is a false negative and is recorded as such rather
  than quietly dropped: cyclictest's JSON carries "realtime": 0 on BOTH
  machines, including the guest that reports CONFIG_PREEMPT_RT=y from its own
  configuration. cyclictest reads /sys/kernel/realtime, a path the out-of-tree
  realtime patch set added and mainline does not. Proved at the right scope
  rather than by one grep: kernel_attrs[] in kernel/ksysfs.c of the pinned
  linux-7.2.5 is the complete attribute list of /sys/kernel and holds nine
  entries -- fscaps, uevent_seqnum, cpu_byteorder, address_bits, uevent_helper,
  profiling, vmcoreinfo, rcu_expedited, rcu_normal -- none named realtime, and
  a whole-tree search of the source for KERNEL_ATTR_RO(realtime),
  KERNEL_ATTR_RW(realtime) and __ATTR_RO(realtime) returns nothing. The field
  reports the absence of a file, not the absence of PREEMPT_RT, and this
  fixture uses the kernel's own configuration instead
- Gate placement, stated rather than assumed: the gate is `make verify-latency`
  (tools/verify_latency_fixture.py, six cases, no '|| true' anywhere) and make
  verify-all does NOT invoke it. Two reasons recorded on the target itself: it
  consumes make verify-kernel's output, so on a checkout that has not built a
  kernel it can only stand down; and the CI runner has no /dev/kvm, no emulator
  and no rt-tests, so wiring it in would add a step that can only skip. Both
  stand-down paths were exercised: with AEGIS_KERNEL_BUILD_DIR pointed at an
  empty directory it printed 'SKIP: .../bzImage does not exist; run `make
  verify-kernel` first; the latency fixture gate did not run.' and exited 0,
  and with cyclictest off PATH it printed 'SKIP: cyclictest is not on PATH;
  ...' and exited 0. An exit 0 from this target is therefore evidence only when
  the case lines are above it. What CI does re-run is
  tools/test_latency_fixture.py (44 cases: the verdict rule, the reading it is
  fed, the recorded admission, the thresholds read out of the crates and the
  program allowlist),
  crates/aegis-calliope/tests/measured_figures.rs (11 cases) and
  crates/aegis-lictor/tests/determinism_fixture.rs (7 cases), all inside make
  verify-all
- HISS-02 on the gate's own external commands, counted rather than asserted: an
  AST sweep in tools/test_latency_fixture.py finds every run()/subprocess call
  site in tools/verify_latency_fixture.py and requires each to carry a
  deadline, and the test fails if the sweep finds no call site at all rather
  than passing on an empty set. The whole-guest deadline is 600 s and the host
  measurement's is 300 s, against a ten-second measurement, so a wedged
  emulator or a cyclictest that never returns fails the gate instead of hanging
  it. One run of the full gate takes about 23 s on the reference profile
- A host kernel change cannot leave a stale figure attributed to a kernel
  nobody is running: REFERENCE_HOST records 7.2.4-1-cachyos and the gate
  compares it with os.uname().release on every run, failing with 'the recorded
  host reading describes a kernel that is no longer running' if they differ.
  The cost is recorded with the decision -- a routine host kernel update fails
  make verify-latency until the constant and docs/build/latency.md are
  re-recorded. The machine name cyclictest writes into its own JSON as
  sysinfo.nodename is dropped by the parser and never reaches the repository,
  because planning/hardware-profile.json forbids recording machine identifiers;
  tools/test_latency_fixture.py asserts it does not survive parsing
- Superseded in part by decision D73 (2026-09-27), and disclosed here because
  it changes how this milestone's gate judges a run: the host-kernel rule in
  the preceding entry is withdrawn. The reference host runs a rolling kernel,
  and its first update was answered by writing 7.2.5-1-cachyos beside the
  7.2.4-1-cachyos figure without measuring again (PR #119, superseded), which
  is the attribution the rule existed to prevent. REFERENCE_HOST now stays the
  kernel the recorded host figure was measured on;
  latency/host-not-preempt-rt is decided by the probe each run executes on the
  running kernel, and prints both releases when they differ. The same decision
  stops latency/guest-measured requiring a satisfied tier edge, which exit
  criterion 5 does not ask for: on 2026-09-27, with the reference host under
  other work, six of seven printed guest runs exceeded all four edges (worst
  cases 4943092 to 22007327 ns). The guest release, its PREEMPT_RT reading and
  a non-empty sample are still required, and the boundary case still shows the
  rule reaching satisfied. tools/test_latency_fixture.py covers the host case
  as passing, drifted, realtime, failed-probe and absent-release, and the guest
  case with every edge exceeded; the drifted host and the all-exceeded guest
  both fail against the previous gate
- Two measured surface floors were re-read rather than left where they were,
  because adding a module moved them:
  crates/aegis-calliope/tests/public_surface.rs now collects 236 names against
  204 for a keyword-only sweep, so MINIMUM_SURFACE rises from 194 to 230, and
  crates/aegis-lictor/tests/public_surface.rs collects 170 against 144, so its
  floor rises from 154 to 164 and FIELD_ARM_NAMES from 21 to 26. Leaving the
  calliope floor at 194 is what the_field_arm_carries_the_floor caught: a
  keyword-only sweep would have cleared it on its own, so dropping the
  public-field arm would no longer have failed the gate
- Gates re-run on the reference profile, each exit 0: `cargo fmt --check`,
  `cargo build --locked`, `cargo test --locked --all-features`, `cargo clippy
  --locked --all-targets --all-features -- -D warnings`, `RUSTDOCFLAGS='-D
  warnings' cargo doc --locked --no-deps`, `make verify-all`, `make
  verify-latency`, `python3 tools/rank_roadmap.py`, `python3 -B -m unittest
  discover -s tools -p 'test_*.py'`, `npx markdownlint-cli2@0.23.2`, `yamllint
  .`, `flake8 .`, `reuse lint`, `black --check --line-length 100 tools
  .config/agent/hooks`, and `git status --porcelain Cargo.lock` empty. The two
  suites were counted on this revision rather than carried forward, because a
  count is a reading like any other: `cargo test --locked --all-features`
  reports 1390 passing Rust tests -- 1375 `#[test]` functions across crates/
  plus 15 doctests, the same total `cargo test --locked --all-features --
  --list` enumerates and the same total the per-binary `test result: ok.` lines
  sum to -- and `python3 -B -m unittest discover -s tools -p 'test_*.py'`
  reports 256. Both are readings of this tree at this revision and of no other.
- Scope: development evidence on the reference profile recorded in
  planning/hardware-profile.json, which this milestone did not change --
  realtime_kernel stays present:false, because that is the correct reading of
  the HOST and the negative half of the fixture depends on it staying true. A
  pass here closes no hardware gate and qualifies nothing: one machine, one
  ten-second run per kernel, on a workstation that was simultaneously carrying
  a development session. The guest figure is a composite rather than an
  isolated measurement of what PREEMPT_RT buys, because the guest's virtual CPU
  is scheduled by a host that is not realtime -- the means say so plainly and
  never crossed, 8329 to 9664 ns in the guest against 1048 to 2701 ns on the
  host across five runs. No burst duration was measured, no PipeWire graph was
  started, no audio device was opened and no eBPF program was loaded. The
  image, kernel-artifact, boot, hardware and release gates remain blocked

### M27 - P17 Scaena first slice: VA-API frames to a layer surface over SCM_RIGHTS

Rank 19. State: done. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: yes. Needs external contract: no. Reference profile: full.
Blocked by: M07. Unblocks: M28, M29.

The first milestone of P17 aegis-scaena, added by D75 and ADR-0003. It is the
cheapest slice that yields runtime evidence for the D75 and D77 paths: each
decoded frame is checked against a content oracle on its surface, and that
surface travels as a DMA-BUF over SCM_RIGHTS and the compositor composites it.
On the reference profile that compositor is the host session's KDE KWin, so
every pass is the client half only (D79). The first slice decodes baseline
Motion-JPEG and one MPEG-2 intra frame, the codec the maintainer chose on
2026-09-28 (D80); H.264, HEVC and AV1 follow once a bitstream parser is
admitted. M27 does not block M12: on 2026-09-28 the maintainer kept it a leaf
rather than a blocker of M12 (D79), and whether the built image re-runs its
display path is decided when M12 is planned. Since 2026-09-29 it blocks M28 and
M29, which ADR-0004 plans, so it is no longer a leaf. No milestone plans a P04
that serves the protocols it uses (D79).

Exit criteria:

- Toolchain admission before any gate runs (D80): cros-libva from git rev
  59384456ac2ae78c0c3e5515f41ef1efd9b802cf (package 0.0.13, BSD-3-Clause),
  smithay-client-toolkit 0.21.1 with default-features = false and rustix 1.1.5
  with the net, fs and event features (event polls the Wayland connection fd
  under a deadline) are declared once in [workspace.dependencies] and locked;
  bindgen 0.70.1 (cros-libva's build dependency at that revision), libclang
  (clang 22.1.8), pkgconf 3.0.7 and the libva 2.24.1 headers (VA-API 1.24.0) are
  recorded in docs/roadmap/toolchain-admission.md with a floor and a reference
  value read back before the gate runs; the Verification gate's runner
  (ubuntu-24.04, whose apt libva-dev is 2.20.0) provides libva headers at or
  above the admitted floor, either by building libva at the reference version or
  by showing that the pinned revision compiles against apt's 2.20.0, which then
  sets the floor; the gate reads that version back, `cargo build --locked`
  covers the crate there, and `cargo tree --locked -p aegis-scaena` shows the
  git source at that revision. The second major versions this resolves beside
  the workspace's thiserror 2 and syn 3 (thiserror 1 through cros-libva, syn 2
  through bindgen) are named in the admission.
- crates/aegis-scaena is a written-out workspace member with a Cargo.lock entry
  and [lints] workspace = true, so unsafe_code = "forbid" is inherited, and its
  hygiene test finds no `unsafe` token in its sources. The workspace
  rust-version is raised only as far as the highest rust-version the resolved
  graph declares, read back from `cargo metadata --locked`
  (smithay-client-toolkit 0.21.1 declares 1.86 against today's 1.85); the three
  tests that assert `rust-version = "1.85"` literally,
  crates/aegis-hephaestus/tests/manifest_hygiene.rs:262,
  crates/aegis-ludus/tests/manifest_hygiene.rs:248 and
  crates/aegis-minerva/tests/manifest_hygiene.rs:199, move to the new value in
  the same change, and rust-toolchain.toml stays at 1.98.1. The two tests that
  compare lock-entry dependency lines literally,
  the_lock_entry_names_exactly_five_dependencies
  (crates/aegis-athena/tests/manifest_hygiene.rs:161) and
  the_lock_entry_names_exactly_three_dependencies
  (crates/aegis-tellus/tests/manifest_hygiene.rs:162), change in the same
  change: once cros-libva locks thiserror 1 beside thiserror 2, Cargo writes
  each thiserror 2 dependency line version-qualified ("thiserror 2.x"), so both
  strip the version suffix Cargo appends before they compare package names, and
  keep their exact name lists.
- D78 lands in the same change as P17's first Wayland or VA-API dependency, not
  before: the six lock-wide negative sweeps,
  no_hash_crate_but_the_one_d02_admits_is_resolved
  (crates/aegis-athena/tests/manifest_hygiene.rs:88),
  the_lock_file_contains_no_md5_implementation
  (crates/aegis-justitia/tests/manifest_hygiene.rs:25),
  the_lock_resolves_no_multimedia_or_async_crate
  (crates/aegis-calliope/tests/manifest_hygiene.rs:190,
  crates/aegis-compositor/tests/manifest_hygiene.rs:202,
  crates/aegis-lictor/tests/manifest_hygiene.rs:197) and
  no_new_third_party_crate_is_resolved
  (crates/aegis-tellus/tests/manifest_hygiene.rs:141), keep their forbidden
  lists verbatim and read their own crate's dependency closure instead of the
  whole lock. The closure comes from `cargo metadata --format-version 1 --locked
  --offline --all-features` without --filter-platform, starts at the one
  workspace member with the crate's name, follows every
  resolve.nodes[].deps[].pkg edge whatever its dep_kinds, marks a node visited
  when it is pushed so the loop is bounded by resolve.nodes.len() iterations,
  and matches package names exactly. It fails closed on a spawn error, a
  non-zero exit, a missed deadline (HISS-02), unparsable output, a null resolve,
  or zero or several start nodes, and the offline failure names `cargo fetch
  --locked` as the cure. P17's own sweep refuses servo, webrender, mozjs,
  mozjs_sys, wgpu, wgpu-hal, smithay, wayland-server, tokio, zbus, zenoh,
  iceoryx2, md5, md-5, blake3 and blake2 in its closure. Of the six, only the
  compositor sweep (wayland-backend) and the lictor sweep (memmap2) refuse a
  name P17's closure resolves today. What D78 gives up is recorded with it: no
  test forbids md5 or tokio workspace-wide any more, and each guarantee holds
  per crate.
- The D77 planes are typed and tested without hardware inside make verify-all,
  over a socketpair and a memfd: schema aegis.p08-p17.decoded-frame.v1, owned by
  aegis-scaena as the consumer under the M14 rule
  (crates/aegis-compositor/src/chain.rs:7-10), carries schema, correlation_id,
  fourcc, modifier, width, height and planes[{offset, pitch}] in one
  line-delimited JSON-RPC 2.0 request with exactly one fd in SCM_RIGHTS, sent
  with rustix sendmsg and received with recvmsg and MSG_CMSG_CLOEXEC into an
  OwnedFd; every receive and every send has a deadline (SO_RCVTIMEO,
  SO_SNDTIMEO), a send that misses it is refused with a typed error, and every
  line has a byte bound (HISS-02). The attach step refuses an fd whose fstatfs
  magic is not DMA_BUF_MAGIC 0x444d4142, a JSON line that names a
  file-descriptor number is refused, and after 1,000 refused messages the
  process's open-descriptor count, read from /proc/self/fd, equals its baseline;
  that case is the only test in its own integration-test binary
  (crates/aegis-scaena/tests/fd_leak.rs), so no concurrent test perturbs the
  count.
- Positive on the reference profile: `make verify-display`, a new target outside
  make verify-all like verify-latency, runs with LIBVA_DRIVER_NAME=iHD set for
  that process only, on the render node whose dev_t equals the compositor's
  zwp_linux_dmabuf_v1 main device (the Arc A380 today). The codec is the one the
  maintainer chose on 2026-09-28, baseline Motion-JPEG plus one MPEG-2 intra
  frame (D80). Decode is checked against content, not exit status. The one intra
  frame of cros-libva's libva_utils_mpeg2vldemo test data decodes to CRC-32
  0xa5713e52 over its visible NV12 lines, the value cros-libva asserts at the
  pinned revision (lib/src/lib.rs, crc_nv12_image); that value is upstream's and
  is not re-recorded here: a driver that yields another fails the case, which
  names the driver version, and adopting a new value is a recorded decision.
  Each of the 60 frames of a committed baseline Motion-JPEG fixture (sha256
  pinned in the test) decodes through VAProfileJPEGBaseline and VAEntrypointVLD,
  and its content is checked against the fixture rather than against an earlier
  run of the same decoder: the fixture's generator draws each frame's index as a
  row of black and white 16 x 16 luma blocks on the MCU grid between one white
  and one black guard block, and the test reads the blocks back from the decoded
  surface with create_image before that surface is exported, thresholding each
  block's mean luma at 128, so a blank frame, a frame out of order or a frame
  whose blocks do not read back fails, and a planted zero-filled surface fails.
  The per-frame CRC-32 the test records when M27 first passes is a regression
  pin only; a driver update that changes one is re-measured and recorded with
  the driver version, the D73 pattern. Each checked surface is exported with
  export_prime (NV12, composed layers), passed over the socketpair and attached
  to a zwlr_layer_shell_v1 surface through zwp_linux_dmabuf_v1 create: 60
  created events, 0 failed events, a wp_presentation presented event for every
  committed frame, and every received fd has the (st_dev, st_ino) of the
  exported one; Aegis code never maps the received fd. Every Wayland wait (the
  registry roundtrip, the first configure, each created or failed event and each
  presented or discarded event) runs under its own deadline, polled on the
  connection fd, and a missed deadline fails the case and names the event it
  waited for (HISS-02). The decode wait has a deadline too: cros-libva's sync
  wraps vaSyncSurface, which takes no timeout, and vaSyncSurface2 is reachable
  only through its generated unsafe bindings, so before it calls sync or
  create_image the test polls Surface::query_status until VASurfaceReady under a
  deadline with a bounded number of polls, and a missed deadline fails the case
  and names the surface (HISS-02). The run prints rustc, libva, the VA vendor
  string, the compositor it ran against and the modifier the driver chose.
- Negative on the reference profile, each refused before anything is attached:
  (a) the session's own LIBVA_DRIVER_NAME=nvidia, because the VA vendor string
  does not name the iHD driver, and a decode's exit status alone does not show
  which GPU decoded; (b) a decode node whose dev_t differs from the compositor's
  zwp_linux_dmabuf_v1 main device (the amdgpu render node today), resolved
  through /sys/class/drm and never from a hard-coded renderD number; (c) a
  format and modifier pair missing from the compositor's feedback for the
  surface, such as NV12 with INTEL_4_TILED_DG2_RC_CCS, refused client-side with
  a typed error while the next valid frame still presents. The test asserts the
  client-side refusal, not the compositor's reply: linux-dmabuf-v1.xml:248-249
  requires the invalid_format protocol error from version 4, and a client test
  does not rest on one compositor's conformance. Because a compositor cannot
  detect a modifier that misstates the layout, the descriptor carries the
  modifier export_prime returned, unchanged, and a test pins that.
- Boundary: 1 and 4 planes are accepted and 0 and 5 refused; a plane whose
  offset + pitch x plane height equals the object size read with lseek(SEEK_END)
  is accepted and one byte more refused; an attach before the layer surface's
  first ack_configure is refused by the surface state machine; the first and the
  sixtieth fixture frame both present; a line exactly at the byte bound is
  accepted and one byte longer refused; a silent peer fails the receive at its
  deadline, a peer that stops reading fails the send at its deadline, and a
  Wayland peer that accepts the connection and never answers the registry
  roundtrip fails at its deadline.
- Fixture licensing: the Motion-JPEG fixture is generated for Aegis, its ffmpeg
  command, which draws the index blocks, recorded as provenance, and carries the
  project licence. The MPEG-2 frame data is third-party (BSD-3-Clause through
  cros-libva, adapted from libva-utils, MIT), and because verify_licensing()
  holds every REUSE.toml annotation to exactly EUPL-1.2 and CC-BY-SA-4.0, that
  file carries file-level SPDX headers with its upstream copyright, the
  LICENSES/ texts it names are added, and `reuse lint` passes.
- Labelling (D79): every recorded pass names the compositor that served it and
  says client half only; nothing here is evidence that P04 serves
  zwlr_layer_shell_v1 or zwp_linux_dmabuf_v1. No Servo, wgpu, SpiderMonkey or
  iceoryx2 crate is resolved, the layer surface takes no keyboard focus
  (keyboard_interactivity none), and a pass is development evidence on the
  reference profile that closes no hardware or accessibility gate. HISS-03 is
  not claimed on the frame path, and the reason is known rather than unmeasured:
  cros-libva at the pinned revision heap-allocates for every decoded picture
  (Picture::new boxes its state, lib/src/picture.rs:118; each JPEG and MPEG-2
  parameter buffer is a Box, lib/src/buffer/jpeg_baseline.rs:53 and
  lib/src/buffer/mpeg2.rs:74). No per-frame allocation count is produced unless
  an allocation counter is admitted, because a counting global allocator needs
  an unsafe GlobalAlloc implementation the crate may not contain, and D83
  records that allocation as a scoped deviation limited to the pinned decoder
  binding: P17's own frame loop allocates nothing per frame.
- The crate texts that name M12 as the display, layer-shell or pacing milestone
  are corrected in the same change:
  crates/aegis-compositor/src/surface.rs:17-18,
  crates/aegis-compositor/src/lib.rs:42-43,
  crates/aegis-compositor/src/pacing.rs:40-41,
  crates/aegis-compositor/tests/pacing_constant.rs:16-17,
  crates/aegis-hestia/src/contracts/graph.rs:46-47,
  crates/aegis-hestia/src/contracts/mod.rs:19-20 and the admitted_at texts at
  crates/aegis-compositor/src/decision.rs:294-295 and :324 say that M27
  exercises the client half against the host compositor and that no milestone
  yet plans a P04 that drives a display or serves zwlr_layer_shell_v1 (D79), and
  the assertion at crates/aegis-compositor/tests/decision_register.rs:214 moves
  to the new text.
- Without the reference profile's capabilities the gate prints `SKIP: <reason>;
  the display slice gate did not run.` and exits 0, the convention
  verify-latency uses, so an exit 0 is evidence only with the case lines above
  it; docs/build/display.md records the run that is evidence.

Cheapest exit: Decode the reference MPEG-2 frame and the fixture stream once on
the reference profile with the driver and device pinned, export, pass and attach
them, and test the descriptor codec, the SCM_RIGHTS path and the surface state
machine on a socketpair and a memfd inside make verify-all.

Evidence (the 2026-09-29 disclosure, the scope and the closing summary; every
entry is in `planning/roadmap.json`, and the run is on `docs/build/display.md`):

- Disclosure, in the shape M18 recorded, because a milestone must say where its
  evidence differs from the letter of its bar: no exit criterion, epic or the
  cheapest exit was rewritten by this delivery, and six points are recorded
  instead. (1) Criterion 2's parenthetical names 1.86 as the value the rule
  would give; the rule gives 1.87, because accesskit 0.25.1, admitted by M16
  after the criterion was written, declares it, and 1.87 is what the workspace
  declares. (2) The runner half of criterion 1 and of E27-3's positive, cargo
  build --locked on the Verification gate's runner against the libva it builds,
  rests on the ci.yml steps run unmodified in an ubuntu:24.04 container on
  2026-09-29; the runner's own evidence is the required Verification gate check
  of the pull request carrying this change, without which it does not merge. (3)
  E27-2's boundary 'a decoded surface that does not reach VASurfaceReady fails
  at its deadline and names the surface' is exercised on a status source that
  never leaves VASurfaceRendering (tests/decode_rules.rs); the Arc A380 finished
  every decode within its first polls, and nothing provoked a stuck surface on
  the hardware. (4) HISS-03: wayland-client allocates per protocol object, three
  per frame, a third-party allocation D83 does not name, recorded beside D83 and
  not waived. (5) The gate also skips when logind reports the session locked,
  which criterion 11 does not list: the first runs on 2026-09-29 found the
  session locked, when the compositor presents no client surface and the first
  frame's presented event cannot arrive. (6) Criterion 3 asks the six sweeps to
  keep their forbidden lists verbatim and to match package names exactly, and
  for justitia's sweep those two cannot both hold: before M27 it searched
  Cargo.lock for four substrings, "md5", "md-5", name = "md5 and name = "md-5,
  and the last two, having no closing quote, also refused any package whose name
  merely starts with md5 or md-5 (md5-asm, say). Its forbidden list is now the
  exact names md5 and md-5; that prefix coverage is given up with D78's
  exact-name rule, and tests/manifest_hygiene.rs records it in
  a_prefixed_md5_name_is_not_a_hit beside a_planted_md5_crate_fails_the_sweep.
- Scope (criterion 9, 2026-09-29): development evidence on the reference profile
  for the client half only. The compositor is the host session's KDE KWin, not
  P04, which serves no Wayland protocol; no milestone yet plans one that does,
  and a pass closes no hardware, accessibility or release gate. P17 stays a
  proposal in planning/components.json: its blockers now record the run and what
  is still open (the runner's own libva evidence, the engine M28 admits, a P04
  that serves the protocols), and its candidate row C18 keeps manifest_present
  false, which verify_candidates() reads as an activation claim. M27 unblocks
  M28, which turns ready; M29 stays blocked on M28.
- Done, and each part by the entry named: criterion 1 by the toolchain entry,
  its runner half as the disclosure records; criterion 2 by the member entry;
  criterion 3 and E27-3 by the D78 entry; criterion 4 and E27-1 by the planes
  entry; criteria 5, 6 and 7 and E27-2 by run r20260929T151938-0d35 and, on the
  final revision, r20260929T170629-d8c5, on which the ready poll preceded every
  sync and create_image; criterion 8 by the licensing entry; criterion 9 by the
  labelling and scope entries; criterion 10 by the M12-texts entry; criterion 11
  by the gate entry. Each rests on cargo test and tools/ tests inside make
  verify-all or on the recorded runs, and none on simulated output.

Epics:

- **E27-1 D77 planes over a socketpair: decoded-frame descriptor, JSON-RPC 2.0
  lines and SCM_RIGHTS**. Requirements: REQ-P17-04, REQ-P17-07. Acceptance:
  Positive: a descriptor and one memfd round-trip over a socketpair, and the
  received fd, taken with MSG_CMSG_CLOEXEC into an OwnedFd, has the sender's
  (st_dev, st_ino). Negative: the attach step refuses the memfd because its
  fstatfs magic is not DMA_BUF_MAGIC; a message carrying zero or two fds for one
  descriptor is refused and the surplus fd is closed; a line whose jsonrpc
  member is not "2.0" is refused; a line that names a file-descriptor number is
  refused. Boundary: 1 and 4 planes accepted, 0 and 5 refused; a plane ending
  exactly at the end of its object accepted and one byte further refused; a line
  at the byte bound accepted and one byte over refused; a silent peer fails at
  its receive deadline; a peer that stops reading fails the send at its
  deadline; a Wayland peer that never answers the registry roundtrip fails at
  its deadline; after 1,000 refused messages the open-descriptor count read from
  /proc/self/fd equals its baseline, in a test binary that holds only that case.
- **E27-2 VA-API frames to a layer surface on the reference profile, driver and
  device pinned**. Requirements: REQ-P17-02, REQ-P17-03. Acceptance: Positive:
  the reference MPEG-2 frame decodes to CRC-32 0xa5713e52 and each of the 60
  fixture frames reads back the index blocks drawn into it on the Arc A380 with
  the iHD driver, and each fixture frame yields a created event and a presented
  event on a zwlr_layer_shell_v1 surface of the host compositor. Negative: the
  session's LIBVA_DRIVER_NAME=nvidia, a decode node that is not the compositor's
  main device, and an unadvertised format and modifier pair are each refused
  before anything is attached, and a planted zero-filled surface fails the
  content check. Boundary: an attach before the first ack_configure is refused;
  the first and the sixtieth frame both present; a presented event that does not
  arrive fails at its deadline and names the event; a decoded surface that does
  not reach VASurfaceReady fails at its deadline and names the surface; the
  recorded pass names the compositor and says client half only (D79).
- **E27-3 Workspace membership, lock sweeps on each crate's own closure and
  toolchain admission (D78, D80)**. Requirements: REQ-WS-01. Acceptance:
  Positive: `cargo build --locked` builds aegis-scaena on the reference profile
  and on the Verification gate's runner, and each of the six re-scoped sweeps
  reports on the pre-M27 lock exactly what the lock-wide sweep reported.
  Negative: a forbidden crate planted in a crate's own closure (tokio as a
  dev-dependency of aegis-compositor) fails that crate's sweep; a failing,
  timed-out or node-less `cargo metadata` fails closed; a manifest that omits
  aegis-scaena from the written-out member list fails the membership test.
  Boundary: wayland-backend in Cargo.lock but outside aegis-compositor's closure
  passes; a name that only shares a prefix (tokio-macros) is not a hit; the
  workspace rust-version equals the highest rust-version the resolved graph
  declares, not one release more, and the three tests that pinned 1.85 read the
  new value; the athena and tellus lock-entry tests pass with thiserror 1 and 2
  both locked and keep their exact name lists.

### M28 - P05 native shell on gpui: layer surface, AT-SPI tree, frame time

Rank 23. State: ready. Cost: large. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: yes. Needs external contract: no. Reference profile: full.
Blocked by: M16, M27. Unblocks: M29.

The P05 shell on the toolkit D102 chose, planned on 2026-09-29 under D101 and
ADR-0004. It puts M16's canvas model on gpui as a layer surface behind the P17
surface contract this milestone fixes, exports the tree live over AT-SPI,
generates the Concordia theme constants, measures the focus ring on rendered
pixels and measures, rather than assumes, the 1,000-node frame and tree-update
times.

Exit criteria:

- Toolchain admission before any gate runs (D102): gpui is declared once in
  [workspace.dependencies] as a git dependency on the zed-industries/zed
  repository with rev = the one commit this milestone chooses, recorded in
  docs/roadmap/toolchain-admission.md with the date it was read, and locked;
  `cargo tree --locked` shows the git source at that revision, and a manifest
  that names a branch or a tag instead of a rev fails the admission test.
  accesskit_unix is admitted at the version that commit resolves (0.22 at zed
  main on 2026-09-29, against the current 0.24.0), and a version behind the
  current release is recorded as D69 drift with its reason. Either accesskit
  resolves to one version in the P05 closure, or the conversion between gpui's
  accesskit (0.24.0 at zed main on 2026-09-29) and the 0.25.1 M16 exports is
  written and tested; the admission says which. The C libraries gpui reaches,
  libxkbcommon through the xkbcommon crate and libwayland-client through
  wayland-backend's dlopen, are recorded with a floor and a reference value read
  back before the gate runs. The licence of every package the pin resolves is
  read from `cargo metadata --locked` and recorded, and any that is not a
  permissive licence is named for a maintainer decision before a gate counts.
- gpui enters through a workspace member of its own that depends on
  crates/aegis-forum-shell, so the M16 model crate keeps its toolkit-free
  closure and its tests keep running with no display, GPU or bus. That member is
  written out, has [lints] workspace = true and contains no `unsafe` token in
  its sources, and every other crate's closure sweep (D78) reports exactly what
  it reported before this milestone.
- The shell maps as a zwlr_layer_shell_v1 surface through gpui's layer-shell API
  (crates/gpui/src/platform/layer_shell.rs), and this milestone fixes the P17
  surface contract the shell sits behind, which ADR-0003 decision 3 and D82 left
  unfixed: MOUNT_SHELL_CANVAS, P05 to P17, is one line-delimited JSON-RPC 2.0
  request over AF_UNIX (D77) whose schema, owned by aegis-scaena as the consumer
  under the M14 rule, names the layer-surface parameters the shell maps with
  (the fields of gpui's LayerShellOptions: namespace, layer, anchor, exclusive
  zone, exclusive edge, margin and keyboard interactivity). Every read has a
  deadline and every line a byte bound (HISS-02). A compositor that does not
  advertise zwlr_layer_shell_v1 yields gpui's LayerShellNotSupportedError, and
  the case names it.
- The AccessKit tree reaches AT-SPI live: gpui's accesskit_unix adapter (zbus
  and atspi, no C accessibility library) exports the running shell's tree, and a
  test reads it back with the atspi crate on a private D-Bus session with the
  at-spi2 bus launcher and registry, their versions read back before the gate
  runs and every call under a deadline (HISS-02). REQ-P17-06's exposure suite
  runs there: interactive roles, accessible names, focus tracking, actions,
  hidden state for unrendered nodes, bounds and incremental updates. With 1,000
  seeded nodes and 10 in view all 1,000 are reachable over AT-SPI (REQ-P05-11),
  and every node has a role and a name, the live half of M16's tree check
  (REQ-P12-06, D101).
- Concordia tokens as Rust theme constants (D05, D101): a generator reads
  ui/concordia-tokens/concordia-tokens.css, the single token source M04
  committed, and emits the Rust constants the shell compiles. A constant edited
  by hand, or a token changed without regenerating, fails make verify-all, and
  the D16 focus-ring token (3 px, 2 px floor) arrives through the generator,
  which refuses a value below the floor.
- Rendered pixels, moved from M16's E16-3 (REQ-P05-13): measured on the painted
  surface rather than computed from the model, the focus ring at the minimum,
  1.0 and maximum camera scale is at least 2 px and 3:1; a ring drawn inside the
  scaled layer (1.5 px at scale 0.5) fails; a focused node fully under a panel
  fails; 2.0 px passes and 1.9 px fails; a partly covered focused node passes.
- Measured, not claimed (ADR-0001): with 1,000 seeded nodes the frame time and
  the AccessKit tree-update time are measured on the reference profile over a
  recorded number of frames and recorded with their distribution, the GPU, the
  driver, the compositor and the zed commit. No figure is claimed before it is
  measured, and a target, if one is set, is a recorded decision. Before the
  frame-time case counts, this milestone records whether HISS-03 binds gpui's
  per-frame work -- gpui rebuilds its AccessKit tree every frame
  (crates/gpui/src/window/a11y.rs) -- the question D83 answered for P17's
  decoder.
- `make verify-shell`, a new target outside make verify-all like verify-display,
  runs the live cases (surface, AT-SPI, pixels and timing) on the reference
  profile; without its capabilities it prints `SKIP: <reason>; the native shell
  gate did not run.` and exits 0, so an exit 0 is evidence only with the case
  lines above it. The token generator, the surface-contract schema and every
  case that needs no display run inside make verify-all.
  docs/build/native-shell.md records the run that is evidence.
- Labelling (D79): a run against the host session's compositor (KDE KWin on the
  reference profile) is development evidence for the client half only. Nothing
  here is evidence that P04 serves zwlr_layer_shell_v1, a pass closes no
  hardware, accessibility or release gate, and the portal, the AT-SPI2 bridge of
  a real session and the real StatusNotifierWatcher are M29's.

Cheapest exit: Map one gpui layer surface on the reference profile that paints
the M16 canvas model, read its AccessKit tree back over AT-SPI with the atspi
crate on a private session bus, measure the 1,000-node frame and tree-update
times, and test the token generator and the surface-contract schema inside make
verify-all without a display.

Epics:

- **E28-1 gpui git pin, accesskit_unix and the shell's workspace member (D102,
  D78)**. Requirements: REQ-WS-01. Acceptance: Positive: `cargo build --locked`
  builds the shell member on the reference profile and on the Verification
  gate's runner, and `cargo tree --locked` shows gpui's git source at the pinned
  revision. Negative: a manifest that names a branch or a tag for gpui instead
  of a rev fails the admission test, as does a resolved package whose licence is
  not recorded; gpui planted in the M16 model crate's closure fails that crate's
  sweep. Boundary: accesskit_unix at the version the pin resolves is recorded
  beside the current release (D69); every other crate's closure sweep reports
  exactly what it reported before this milestone.
- **E28-2 Layer surface and the P17 shell-surface contract (ADR-0003 decision
  3)**. Requirements: REQ-P17-02, REQ-P17-04. Acceptance: Positive: a
  MOUNT_SHELL_CANVAS request round-trips over a socketpair as one JSON-RPC 2.0
  line, and the shell maps a zwlr_layer_shell_v1 surface with its parameters on
  the host compositor and receives its first configure. Negative: a line whose
  jsonrpc member is not "2.0", an unknown schema version and an unknown layer
  are each refused with a typed error; a compositor without zwlr_layer_shell_v1
  yields LayerShellNotSupportedError and the case names it. Boundary: a line at
  the byte bound is accepted and one byte longer refused; a silent peer fails at
  its read deadline; a first configure that does not arrive fails at its
  deadline and names the event.
- **E28-3 Live AccessKit tree over AT-SPI (REQ-P17-06)**. Requirements:
  REQ-P17-06, REQ-P05-11, REQ-P12-06. Acceptance: Positive: a fixture button is
  exposed over AT-SPI with role button, a name and a click action, the focused
  node is reported focused, and with 1,000 seeded nodes and 10 in view all 1,000
  are reachable, each with a role and a name. Negative: the same button exposed
  as a generic container fails; a planted unlabelled node fails and is named;
  culling that removes nodes from the tree fails the 1,000-node check. Boundary:
  a hidden subtree is reported hidden; a client started after the surface maps
  still receives the full tree; an AT-SPI call that does not answer fails at its
  deadline.
- **E28-4 Focus ring measured on rendered pixels (moved from E16-3)**.
  Requirements: REQ-P05-13. Acceptance: Positive: at the minimum, 1.0 and
  maximum camera scale the focus ring measured on the painted surface is at
  least 2 px and 3:1. Negative: a ring drawn inside the scaled layer (1.5 px at
  scale 0.5) fails; a focused node fully under a panel fails. Boundary: a 2.0 px
  ring passes and 1.9 px fails; a partly covered focused node passes.
- **E28-5 Concordia tokens as Rust theme constants (D05, D101)**. Requirements:
  REQ-P12-05, REQ-P05-06. Acceptance: Positive: the generated constants equal
  the values in ui/concordia-tokens/concordia-tokens.css, and the shell compiles
  against them. Negative: a hand-edited constant, or a token changed without
  regenerating, fails make verify-all. Boundary: a focus-ring token at the 2 px
  D16 floor is accepted and 1 px is refused by the generator.
- **E28-6 1,000-node frame time and tree-update time, measured (ADR-0001)**.
  Requirements: REQ-P05-11. Acceptance: Positive: with 1,000 seeded nodes the
  frame time and the AccessKit tree-update time are measured over the recorded
  number of frames on the reference profile and recorded with their
  distribution, the GPU, the driver, the compositor and the zed commit.
  Negative: a run whose tree held fewer than 1,000 nodes, or that ran with the
  accessibility adapter inactive, is refused as a measurement. Boundary: a run
  of exactly the recorded frame count is accepted and one frame short is
  refused; no target is asserted unless a recorded decision sets one.

### M29 - P05 live session in a VM: portal, AT-SPI2 and StatusNotifierWatcher

Rank 24. State: blocked. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: yes. Needs external contract: no. Reference profile: full.
Blocked by: M28, M27. Unblocks: nothing.

The live-session P05 milestone D91 waited for, planned on 2026-09-29 (ADR-0004).
It runs the M28 shell in a real session inside a VM, so the portal, the AT-SPI2
bridge and the StatusNotifierWatcher are real daemons rather than the D-Bus
mocks M16 uses.

Exit criteria:

- A live session in a VM on KVM: a pinned distribution guest image, not an Aegis
  image (M11 builds that), boots under QEMU with M24's admission reused and runs
  a Wayland compositor that advertises zwlr_layer_shell_v1, xdg-desktop-portal
  with a settings backend, at-spi2-core and a StatusNotifierWatcher on the
  session bus, each version read back from the guest before the cases run. The
  guest image is pinned by sha256 and refused on a mismatch before it boots, and
  every wait for a guest service has a deadline (HISS-02). The shell is the one
  M28 builds, unchanged.
- REQ-P12-02, the portal half D89 and D91 left open: in the live session the
  shell reads org.freedesktop.portal.Settings over D-Bus at mount and applies a
  change of the accessibility preferences it reads, the contrast and
  reduced-motion settings REQ-P12-02 and D76 name, within one frame of the
  change signal and without a restart. The setting names are read from the
  guest's xdg-desktop-portal interface description when the milestone is
  delivered, not assumed here.
- REQ-P12-03, the AT-SPI2 half D89 and D91 left open: the shell's tree reaches
  the session's at-spi2 registry, and an assistive-technology client started
  after the shell maps, reading through the atspi crate, receives the full tree
  and the focus events of a keyboard walk over the canvas.
- REQ-P05-05's D-Bus half: on mount the shell registers with the session's real
  StatusNotifierWatcher, and the watcher lists it. The compositor-socket half
  stays with D32's contract test, which waits for a real P04 socket.
- `make verify-session`, a new target outside make verify-all, runs the cases;
  without KVM or the pinned guest image it prints `SKIP: <reason>; the live
  session gate did not run.` and exits 0, so an exit 0 is evidence only with the
  case lines above it. docs/build/live-session.md records the run that is
  evidence.
- Labelling: a pass is development evidence in a VM on the reference profile.
  The guest's compositor is not P04, so D79's client-half rule applies; the
  guest is not the Aegis image; and a pass closes no hardware, accessibility or
  release gate. The image's accessibility precondition is M11's (REQ-P12-01),
  not this milestone's claim.

Cheapest exit: Boot one pinned distribution guest with a layer-shell compositor,
xdg-desktop-portal, at-spi2-core and a StatusNotifierWatcher under QEMU with
KVM, run the M28 shell in it, change one portal setting, read the tree back
through the session's registry and list the watcher's items.

Epics:

- **E29-1 Portal settings drive the shell (REQ-P12-02, portal half)**.
  Requirements: REQ-P12-02. Acceptance: Positive: the shell reads the portal's
  accessibility settings at mount, and a change in the portal backend is applied
  within one frame of the change signal, with no restart. Negative: a portal
  that does not answer within its deadline leaves the shell on its defaults, and
  the case records the timeout rather than a value. Boundary: changing the
  setting and changing it back returns the shell to its original state; a change
  made before the shell maps is applied at mount.
- **E29-2 AT-SPI2 bridge in the live session (REQ-P12-03, AT-SPI2 half)**.
  Requirements: REQ-P12-03, REQ-P17-06. Acceptance: Positive: an
  assistive-technology client started after the shell maps receives the full
  tree through the session's at-spi2 registry, and the focus events of a
  keyboard walk over the canvas. Negative: with the registry stopped the case
  fails and names the registry, rather than passing on the shell's own export.
  Boundary: a client started before the shell maps and one started after it
  receive the same tree; a focus event that does not arrive fails at its
  deadline.
- **E29-3 Real StatusNotifierWatcher (REQ-P05-05)**. Requirements: REQ-P05-05.
  Acceptance: Positive: on mount the shell registers with the session's
  StatusNotifierWatcher, and the watcher lists it. Negative: with no watcher on
  the bus the registration fails with a typed error and the shell keeps running.
  Boundary: a watcher that appears after the shell mounts is registered with
  once, not twice.
- **E29-4 Pinned guest session on KVM**. Requirements: REQ-P12-02, REQ-P12-03,
  REQ-P05-05. Acceptance: Positive: the pinned guest boots under QEMU with KVM,
  and the compositor, the portal, the at-spi2 registry and the watcher each
  answer on the session bus with their versions read back. Negative: a guest
  image whose sha256 differs from the pin is refused before it boots. Boundary:
  a service that does not appear within its deadline fails the case and names
  the service.

### M09 - Cross-repository contract pin: one local request/result pair

Rank 17. State: done. Cost: small. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: no. Needs external contract: yes. Reference profile: partial.
Blocked by: M18. Unblocks: M11, M10.

Exit criteria:

- The M18 schemas are consumed by the producer that reads them, and acceptance
  is recorded only from that consumption, not from a fixed symbol list:
  cordanaLLM/imago at the pinned commit decodes build/product-input.json (`imago
  aegis validate`, pkg/aegis) and build/kernel-requirement.json (`imago kernel
  requirement validate`, pkg/kernel). cordanaLLM/nucleus consumes no Aegis
  payload -- its AGENTS.md ('The contract with imago') names its inbound as
  imago's own kernel/requirement.json and its outbound as the
  imago.nucleus.kernel-artifact.v1 manifest -- and is recorded by identity and
  that documented outbound role only (D92)
- Each payload is run locally through imago built from the pinned commit, with
  the binary's build provenance read back, for a positive result (accepted, and
  printed back field by field), a negative result (a rejected payload whose
  error carries the payload's correlation-id) and a boundary result (an empty
  feature list rejected explicitly with ErrEmptyRequirement's 'feature list is
  empty'; retries.max-attempts and the packages count exactly at imago's bound
  accepted and one above refused). Under D92 the result legs moved: the Imago
  product result (imago.p01.product-result.v1) to M11 and the Nucleus kernel
  result to M10
- Canonical producer identities are confirmed with the verifying command and its
  output retained: `git ls-remote --heads
  https://github.com/cordanaLLM/imago.git` and the same for nucleus. Both
  returned 'Repository not found' at the 2026-09-13 reference-profile probe and
  resolve from 2026-09-16 (imago main 16f964b and nucleus main 8672247 on
  2026-09-27); any non-canonical identity still in builder workflows is recorded
  as the blocker
- Simulated output is not accepted as a result; no hosted dispatch is claimed
- Producer-side decoders, fixtures and results live in the producer
  repositories: imago decodes the Aegis payloads from fixtures byte-identical to
  build/product-input.json and build/kernel-requirement.json (imago #32, #35),
  and nucleus publishes the imago.nucleus.kernel-artifact.v1 manifest shape
  (nucleus #11). Aegis tracks its own payloads only, and nothing is published to
  cordanaLLM/imago or cordanaLLM/nucleus from here
- The producer commits exercised are pinned in the evidence from each producer's
  main branch, so the run is reproducible and is never mistaken for hosted
  acceptance. The 2026-09-13 pins (imago 4f116fc, nucleus 78ca8f2) were
  local-only commits that never reached either producer and are not reused.

Cheapest exit: Run the pair against the pinned local checkouts without any
hosted dispatch.

Evidence (the 2026-09-27 and D92 disclosures, the scope, the closing summary
and the dated D106, re-pin and D107 entries of 2026-09-29; every entry is in
`planning/roadmap.json`, and the runs are on `docs/build/contract-pair.md`):

- Disclosure, in the shape M18 recorded, because a milestone that edits its own
  bar must say so where the bar is judged: before any delivery, three exit
  criteria were rewritten on 2026-09-27 and a fourth was merged into one of
  them, because their premise -- that the producer repositories do not resolve
  -- stopped being true. None is a relaxation. The identity criterion and the
  retained-command criterion were merged: the `git ls-remote` command stays
  required, and its 2026-09-13 result stays recorded as history. The dogfooding
  criterion is replaced because the producers now carry the consuming side
  themselves (imago #32 and #35, nucleus #11). The local pins imago 4f116fc and
  nucleus 78ca8f2 are dropped because neither commit ever reached its producer.
  Observed, and not a criterion: neither producer builds anything yet. imago's
  pkg/aegis stops at acceptance, with binding an accepted request to an executor
  left to later work, nucleus refuses to emit a kernel artifact it did not
  compile while its forge compiles no kernel (nucleus #18, open), and neither
  repository has a release. E09-1's and E09-2's positive halves therefore wait
  on producer work; their negative and boundary halves are reachable against the
  producers' decoders now.
- Disclosure, in the shape M18 recorded, because a milestone that edits its own
  bar must say so where the bar is judged: under decision D92 (2026-09-28),
  during this delivery and before the state moved to done, exit criteria 1 and 2
  and the acceptance of E09-1 and E09-2 were rewritten and E09-2's title
  changed. Criterion 1 read 'The M18 schemas are proposed to cordanaLLM/imago
  and cordanaLLM/nucleus; acceptance is recorded only when each producer
  consumes the payload, not a fixed symbol list'. It now records acceptance from
  the one producer that consumes the payloads, imago, and records nucleus by
  identity and outbound role; that narrows 'each producer', and it is disclosed
  as a narrowing: nucleus reads no Aegis payload at 8672247, so no nucleus
  acceptance exists to record. Criterion 2 read 'One request/result pair is
  executed locally against pinned local imago and nucleus checkouts, with a
  positive result, a negative result (rejected payload with a correlated error)
  and a boundary result (empty requirement list rejected explicitly)'. It now
  runs each payload through the pinned imago and moves the result legs out: this
  is a relaxation of what M09 proves, disclosed as one, and neither leg is
  dropped -- the Imago product result is M11's new criterion and E11-7, the
  Nucleus kernel result M10's reworded first criterion and E10-4. E09-1 read
  'Positive: an accepted request returns image digest, signature reference and
  boot-evidence fields. Negative: a malformed manifest is rejected with a
  correlated error. Boundary: a retry at the bound is recorded, and one above is
  refused.'. Its positive sentence moved verbatim to E11-7; its negative stands;
  its boundary is replaced by the bound imago enforces, retries.max-attempts 10
  accepted and 11 refused, with the packages count at 256 and 257 added. No
  producer executes a retry today, so a retry that is executed and recorded is
  observed nowhere, and D92 assigns it to no milestone: that is a narrowing,
  disclosed as one. E09-2 was titled 'Nucleus consumption of the kernel
  requirement payload' and read 'Positive: a result returns kernel
  version/config digest, artifact digest and provenance. Negative: an
  unsatisfiable feature is rejected with a correlated error. Boundary: an empty
  requirement list is rejected explicitly.'. Its positive sentence moved
  verbatim to E10-4; its consumer is now imago's pkg/kernel, the one reader of
  the payload; its negative reads 'a feature imago cannot accept' for 'an
  unsatisfiable feature', because imago checks a feature's shape and vocabulary
  and has no kernel to satisfy it against, and no producer checks the
  requirement against a built kernel; rejecting a feature a built kernel does
  not satisfy, with the requirement's correlation-id, was left to no milestone
  by D92 and is assigned to M10 by D94 (its criterion and E10-5), so it moved
  rather than narrowed; its boundary stands and gains the one-feature
  acceptance. E09-3's text is unchanged; under D92 its 'retained pair' is each
  request and imago's recorded response to it. Criteria 3 to 6 are unchanged.
  M09's rank moved 22 -> 17 because a done milestone keeps its place among the
  done ones, by the ranking rule, not by a decision.
- Scope: consumption evidence on the reference profile. No product result,
  image, UKI or kernel came back from either producer, and none was built here;
  the imago binary is a decoder built from source to run the contract, not a
  release artifact. P01 and P02 stay proposals in planning/components.json;
  their activation blockers now read the result and artifact contracts, the
  request half being pinned here. M11 and M10 become ready under the register
  rule, every milestone in their blocked_by now done, but each keeps an external
  BLOCKED-until criterion that still holds -- M11 waits for Imago to return an
  image/UKI and the product result (imago#46), M10 for a Nucleus-published
  kernel manifest (nucleus #18, #20) -- so 'ready' there is a register state and
  not an unblocking.
- Done, as D92 scopes the bar, and each part by the entry named: criteria 1 and
  5 by the consumption entry; criterion 2, E09-1 and E09-2 by the recorded-run
  entry; criteria 3 and 6 by the identity entry; criterion 4 and E09-3 by the
  simulated-output entry. Each rests on the recorded runs or on
  tools/test_contract_pair.py inside `make verify-all`, and none on simulated
  output.
- Dated 2026-09-29, appended after done (D106); no exit criterion and no epic
  text changes. D92 recorded cordanaLLM/nucleus by identity only because at
  8672247 it read no Aegis payload. At 0a4eac93f29fef432bfa9d892ad236568ce2f482
  (nucleus pull request 35; its main on 2026-09-29, read with `git ls-remote
  --heads`) it does: its versions.json binds the label aegis-os to
  cordanaLLM/Aegis-OS build/kernel-requirement.json and the realtime stream, and
  scripts/verify_kernel_requirement.py decodes the document as
  crates/aegis-fabrica-defs does and holds every feature against the kconfig
  fragments of that stream. build/contract/producers.pin.json, now schema
  aegis.m09.contract-pin.v2, pins that commit with the verifier, the label, the
  bound stream, the report schema nucleus.kernel-requirement-report.v1 and the
  evidence level declared. `make contract-fetch` (run f20260929T084215-2847)
  clones nucleus into a fresh directory beside imago, and the gate adds
  contract/nucleus-checkout -- the checkout is the pinned commit with nothing it
  lacks, the verifier is present and versions.json binds exactly one row to
  aegis-os -- and four cases that run the verifier offline with the gate's
  python3 and -I and assert on its --report-json, never on its text:
  nucleus/accepted (the committed requirement, with --sha256 and
  --correlation-id bound to its own bytes and id: exit 0, PASS, held by
  realtime, no reason), nucleus/correlated-refusal (a planted built-in
  CONFIG_AEGIS_CONTRACT_UNSET: exit 1, FAIL, every reason opening with
  aegis-m18-kernel-requirement-0001:, and the report recording the symbol unset
  and unmet on realtime x86_64), nucleus/empty-features-refused (no feature:
  exit 1, REJECTED NoFeatures; one feature: exit 0, PASS) and
  nucleus/dispatch-binding-refused (the sha256 or the correlation id of
  build/kernel-requirement.reference.json: exit 1, REJECTED DigestMismatch and
  CorrelationMismatch). Run r20260929T085636-c155 of `make verify-contract`
  passed all nineteen cases, the fourteen imago cases unchanged. The evidence
  level is declared, the fragments as nucleus merges them and not a built
  configuration, so this is not the check of a feature against a built kernel
  D94 gives M10 (E10-5), and no Nucleus kernel result came back (E10-4). E09-2's
  text stays as written, naming imago's pkg/kernel, and its evidence grows by
  these runs; criterion 1's and E09-2's statements that nucleus consumes or
  reads no Aegis payload are read as of 8672247, as D92's dated correction
  records.
- Dated 2026-09-29, gates run on the D103 to D106 revision after its review,
  each exit 0: `make contract-fetch` (run f20260929T084215-2847: both ls-remote
  calls, fresh depth-1 clones of imago and nucleus, and an imago binary with the
  same sha256 as on 2026-09-28,
  71186e11e2589ad86d6dae318b8fa337752ed89d0e7de7911ddf53dd8d5a67bd); `make
  verify-contract` (run r20260929T092808-d300, nineteen PASS lines); and `make
  verify-all` with the contract pair gate running (run r20260929T092828-797e),
  praetorctl built from the ci.yml pin d2b6a3b958004d0408bb6e04a41de68e5778fbc4.
  Replays against copies of the cache, each exit 1 with the payload cases not
  run: an identity record for the old nucleus pin 8672247 beside the pinned
  nucleus checkout failed contract/identity (run r20260929T085243-2c5e); an
  untracked scripts/zz_planted.py (run r20260929T085243-9938) and a
  versions.json edited to bind aegis-os to lts (run r20260929T085243-a0d5)
  failed contract/nucleus-checkout; and the cache a fetch from before D106 left,
  whose record names 8672247 and which holds no nucleus checkout, failed
  contract/identity with no SKIP line (run r20260929T092142-aeb6). As first
  delivered the gate printed SKIP and exited 0 on that cache, the absent
  checkout hiding the wrong record; review found it, and the record is now read
  before any absent piece. tools/test_contract_pair.py covers each nucleus
  decision with the reports nucleus wrote, abridged by dropping fields and rows
  and nothing else, kept as bytes; the one variant nucleus did not write, an
  unbound stream that sets the planted symbol, is built inside its test and
  named synthetic.
- Dated 2026-09-29, appended after done; no exit criterion and no epic text
  changes. The nucleus pin moves from 0a4eac93f29fef432bfa9d892ad236568ce2f482
  to 82aa6b7a3c68a42a6330370c81ec642482014c9f, cordanaLLM/nucleus main on
  2026-09-29 (read with `git ls-remote https://github.com/cordanaLLM/nucleus.git
  refs/heads/main`), after its pull requests 36 (30253b9: signed kernel sources
  and a resolved configuration) and 37 (82aa6b7: every leg compiled and its
  artifacts gated before signing; it closes nucleus issues 18 and 31). At
  82aa6b7 versions.json still binds aegis-os to cordanaLLM/Aegis-OS
  build/kernel-requirement.json on realtime with dispatched false, and
  scripts/verify_kernel_requirement.py is still standard-library Python; it now
  imports its sibling scripts/versions_query.py by putting its own directory on
  sys.path, which python3 -I otherwise hides. Without --resolved-config its
  report keeps the schema nucleus.kernel-requirement-report.v1, evidence_level
  declared and every document field; each stream row gains an evidence field,
  declared here. The first gate run at 82aa6b7 passed (`make contract-fetch` run
  f20260929T101312-03f2, `make verify-contract` run r20260929T101320-e64d), and
  the next failed contract/nucleus-checkout (run r20260929T102148-b707): under
  python3 -I alone the sibling import wrote
  `scripts/__pycache__/versions_query.cpython-314.pyc` into the checkout. The
  gate now runs the verifier with -I -B, and tools/test_contract_pair.py runs a
  sibling-importing stand-in with -B (no bytecode) and without it (bytecode
  written). Gates run on this revision, each exit 0: `make contract-fetch` (run
  f20260929T102410-28e2: nucleus main equals the pin, a fresh depth-1 clone at
  82aa6b7, and the imago binary again
  71186e11e2589ad86d6dae318b8fa337752ed89d0e7de7911ddf53dd8d5a67bd); `make
  verify-contract` twice in a row (runs r20260929T102414-7f8a and
  r20260929T102414-cc48, nineteen PASS lines each, the nucleus checkout still
  clean after the first; the nucleus rows as under D106: PASS held by realtime,
  the planted symbol refused with the correlation id, NoFeatures, DigestMismatch
  and CorrelationMismatch); and `make verify-all` with the contract pair gate
  running (run r20260929T102617-ece2), praetorctl 892dc1035ae9, the ci.yml pin
  892dc1035ae93de4e956a5d9a33202557ad28202. The -B runs wrote reports
  byte-identical to run r20260929T101320-e64d's, and the reports
  tools/test_contract_pair.py keeps were re-captured from them, abridged as
  before; beyond nucleus_revision the fields they keep are unchanged. The
  evidence level stays declared. The resolved level is M10's input, not M09's:
  E10-4 and E10-5 read the release asset kernel-realtime-x86_64.config, which
  the imago.nucleus.kernel-artifact.v1 manifest pins by kernel.config_digest,
  and nucleus has published no release (its releases API lists none and
  publish-release.yml has never run, read 2026-09-29). nucleus's own
  verify-requirements run 36540473760 on 82aa6b7 reports this document (Aegis-OS
  631a1b5, sha256 d796c408b4db) held by realtime at both levels; it ran on
  nucleus's runner, not in this gate, and is neither a built kernel nor a
  release, so it closes nothing in M10.
- Dated 2026-09-29, appended after done (D107); no exit criterion and no epic
  text changes. build/kernel-requirement.json gains the row
  CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS built-in, probe kernel-config,
  required by REQ-P06-05 (M18's entry of this date), so its fixture digest in
  build/contract/producers.pin.json moves from
  d796c408b4db5dd9e5f22d35c109a8dccadcdb14f3f68ce7de2cddc594d47adb to
  92d74206ee5a4cc46bc9a8c209855c4a3d07a9ed47b003963fc73697ac8776ce, and both
  producer pins move with it, each its producer's main on 2026-09-29, read with
  `git ls-remote --heads`. imago moves from
  16f964b4dafadac2b1f0a662c7dcbb4b7bb29bee to
  987b95a432e5c9aac1b4885b97fa2d3ff1b9d311, its pull request 52 and the only
  commit between the two: it re-vendors
  pkg/kernel/testdata/aegis-kernel-requirement.json with exactly these bytes,
  moves two test counts from 13 to 14 and closes imago issue 51; go.mod still
  declares go 1.27.1, and pkg/aegis/aegis.go still bounds MaxRetryAttempts at 10
  and MaxPackages at 256. nucleus moves from
  82aa6b7a3c68a42a6330370c81ec642482014c9f to
  852be742eb173700d5ef93b0c6f867b855f9c640, its pull request 47, which sets
  CONFIG_FTRACE, CONFIG_FUNCTION_TRACER, CONFIG_DYNAMIC_FTRACE and
  CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS in kconfig/security-hardened.config
  for every stream (its ADR-0011). At 852be74 versions.json still binds aegis-os
  to cordanaLLM/Aegis-OS build/kernel-requirement.json on realtime with
  dispatched false, and the verifier's command line, exit codes and report keys
  are unchanged; it now sets sys.dont_write_bytecode itself (nucleus pull
  request 41), and the gate keeps -I -B. At 82aa6b7 the verifier refuses the new
  payload: run by hand from a separate checkout, it printed FAIL with
  'aegis-m18-kernel-requirement-0001: realtime x86_64:
  CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS (required-by REQ-P06-05) requires
  built-in, observed unrecorded' and exited 1. Gates run on this revision, each
  exit 0: `make contract-fetch` into a fresh cache (run f20260929T230809-e9ad:
  both mains equal the pins, fresh depth-1 clones, and an imago binary stamped
  v0.0.0-20260929210208-987b95a432e5 and vcs.time 2026-09-29T21:02:08Z, sha256
  81c0f6d1277bca9592c46920c61cf79a44e024535a65207c0cede1587057727d, which
  replaces 71186e11e258... because the module version and vcs.time go stamps
  change with the commit); `make verify-contract` twice (runs
  r20260929T230816-7c75 and r20260929T233252-ba31, nineteen PASS lines each:
  contract/fixtures-identical over the 2232-byte copy,
  kernel-requirement/accepted listing fourteen features, nucleus/accepted PASS
  held by realtime on sha256 92d74206ee5a, the planted symbol refused with the
  correlation id on sha256 ef59a2280044, NoFeatures, DigestMismatch and
  CorrelationMismatch; the two wrote byte-identical reports and left both
  checkouts clean); `make verify-all` with the contract pair gate running (run
  r20260929T233538-b915), with praetorctl built from
  8d0344e917f16a2b7e23b1ec5258133ddba783f2, the ci.yml pin before this change
  was rebased onto main at a28be4e; and, after the rebase, `make
  verify-contract` again (run r20260929T235617-b7ab, nineteen PASS lines,
  reports byte-identical to the two above) and `make verify-all` with the
  contract pair gate and every other gate running (run r20260929T235928-32d1),
  with praetorctl built from 9e855dabefac0f78f4cec59dc88db7a8d588b87f, the
  ci.yml pin there. A copy of a cache fetched for the old pins (run
  r20260929T233158-d82f) failed contract/identity for both producers, exit 1,
  and no other case ran. What imago printed for the product input, the kernel
  requirement's header and the empty feature list is byte for byte what 16f964b
  printed. The nucleus reports tools/test_contract_pair.py keeps were
  re-captured from run r20260929T230816-7c75, abridged as before; beyond
  nucleus_revision and the two document digests, the fields they keep are
  unchanged. The evidence level stays declared, and M10's release stage now runs
  imago 987b95a.

Epics:

- **E09-1 Imago consumption of the product input manifest**. Requirements:
  REQ-P01-01, REQ-P01-02, REQ-P01-03, REQ-P01-04, REQ-P01-06, REQ-P01-10.
  Acceptance: Positive: imago at the pinned commit accepts
  build/product-input.json and prints back the build request it maps it to,
  every field as sent. Negative: a malformed manifest is rejected with a
  correlated error. Boundary: retries.max-attempts exactly at imago's bound
  (MaxRetryAttempts, 10) is accepted and one above is refused, and the packages
  count likewise at MaxPackages (256) and one above. The result half -- image
  digest, signature reference and boot-evidence fields -- moved to M11's E11-7
  under D92.
- **E09-2 Imago consumption of the kernel requirement payload (re-scoped from
  Nucleus by D92)**. Requirements: REQ-P01-09, REQ-P07-01, REQ-P06-05,
  REQ-P13-02. Acceptance: Positive: imago at the pinned commit (pkg/kernel)
  accepts build/kernel-requirement.json and
  build/kernel-requirement.reference.json and lists every feature each declares.
  Negative: a feature imago cannot accept -- a state other than built-in or
  module, an unknown probe, a duplicated symbol -- is rejected with an error
  carrying the payload's correlation-id. Boundary: an empty requirement list is
  rejected explicitly (ErrEmptyRequirement), and a one-feature list is accepted.
  cordanaLLM/nucleus reads no Aegis payload and is recorded by identity and its
  outbound role only; the kernel result -- kernel version/config digest,
  artifact digest and provenance -- moved to M10's E10-4 under D92.
- **E09-3 Pair evidence and identity status**. Requirements: REQ-BOOT-02,
  REQ-GOV-02. Acceptance: The retained pair shows correlation id and exact
  revisions. Simulated output is refused. Identity status is recorded.

### M11 - Minimal image build with artifact, signature and boot evidence

Rank 25. State: ready. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: yes. Needs external contract: yes. Reference profile: partial.
Blocked by: M09, M24, M04. Unblocks: M13, M20, M12.

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
  (D20); eBPF objects enter through M10 and M12
- REQ-P12-01 (D91, re-mapped by D101): the integration adapter requests an
  image/UKI build from Imago, and accepts its return, only for a commit on which
  the M04 accessibility gate reported PASS on the P12 token component and, once
  M16 delivers it, the P05 shell's AccessKit tree check (E16-4) passed; a FAIL,
  or a SKIP for a missing engine, image or offline store, stops the request
  before it is sent. The image still carries no UI (D20): both checks are build
  preconditions, not image content
- The A/B sysupdate transfer is exercised once between root-a and root-b, and
  observed transitions are compared with the M15 state machine
- The M24 harness is reused unchanged except for the one pin scheme D85 moves
  here, and the only new inputs are the Imago artifact and its pin; a criterion
  states that no second boot apparatus is built here.
- The artifact's digest and signature are verified with the pinned cosign (2.6.3
  on the reference profile) before the boot runs, and a tampered-digest negative
  case is exercised.
- The Secure Boot position is restated explicitly so it cannot be lost between
  milestones: a QEMU/OVMF boot proves the image boots, not that it boots signed,
  unless the guest VARS store was enrolled per D62; the reference profile's host
  firmware cannot verify it (SecureBoot 0; SetupMode 1 with no PK, KEK or db
  enrolled, re-read 2026-09-28).
- One criterion states that one boot on one developer workstation is development
  evidence only: it does not qualify hardware, does not close the hardware or
  release gate, and the retained logs must say so on their face.
- The image half of ADR-0002 is checked here, because M08 could not check it:
  that milestone proved the crate builds without the Steamworks SDK and this
  repository built no image (DSP-26). A negative test sweeps the image content
  list and the built image for the SDK package, its shared library and a
  vendored copy of its source, and fails on any of them. It is a check over
  those names and not a proof that no proprietary component is present.
- Moved here from M24's E24-2 by D84: the dm-verity root hash of the Imago image
  matches the value its signed release records, and a modified root image fails
  verity and does not boot to the established state. M24's pinned upstream image
  carries no verity root, and M24 constructs no image, so the acceptance can
  only be judged on this milestone's artifact.
- Moved here from M24's criterion 2 and E24-1 by D85: the M24 harness verifies
  the Imago return's digest and signature before any boot. Once the signature
  form of the Imago result is pinned (the signature-ref of
  imago.p01.product-result.v1, which D92 moved from M09 to this milestone with
  the result itself), this milestone adds that form to the harness's pin schema
  as a second signature scheme, with its own positive, negative and boundary
  cases, and the gpg-clearsigned-checksum scheme M24 proved keeps verifying the
  upstream pin unchanged. M24 proved the upstream kind only, and a scheme added
  before that form is pinned would guess its contract.
- Moved here from M09's E09-1 by D92: for the request M09 proved imago accepts,
  Imago returns an imago.p01.product-result.v1 result carrying the request's
  correlation-id, the image digest, the signature reference and the
  boot-evidence reference, and imago's Result.Validate accepts it. Nothing
  produces that result at the imago commit M09 pinned (16f964b; its ADR-0020
  says 'Nothing produces a result yet'), and cordanaLLM/imago#46, opened
  2026-09-28, tracks the emitter. The signature form D85 waits for is this
  result's signature-ref, so it is pinned here and not in M09.
- D53 (decided 2026-09-28): the image determinism seed is derived from the
  released revision, never a fixed constant. Positive: two builds of the same
  revision carry identical partition identifiers. Negative: an image whose
  partition identifiers derive from an all-zero seed is refused. Boundary: two
  revisions that differ only in their last commit carry different partition
  identifiers

Cheapest exit: No cheaper exit exists: this is the first real artifact. Keep it
to one image and one boot, and retain every log.

Evidence:

- Disclosure, in the shape M18 recorded, because a milestone that edits its own
  bar must say so where the bar is judged: before any delivery, one exit
  criterion and epic E11-5 were added on 2026-09-28 under decision D84. None is
  a relaxation: they add the dm-verity acceptance that M24's E24-2 carried until
  then -- 'the verity root hash matches' and 'a modified root image fails verity
  and does not boot to the established state' -- because a pinned upstream image
  has no verity root and M24 constructs no image, and REQ-P02-01 moves with it.
  E11-5's boundary (one flipped bit) is new and strictly narrower than the
  negative it sits beside. Nothing else in this milestone changed under D84, and
  the next entry discloses what D85 changed later; M11 was blocked on M09 and
  M24 when D84 was recorded, M24 has since closed, and this entry is not
  evidence that any criterion is met.
- Disclosure, in the shape M18 recorded, because a milestone that edits its own
  bar must say so where the bar is judged: before any delivery, on 2026-09-28
  under decision D85, one exit criterion was added, and criterion 6, the
  acceptance of E11-1 and E11-2, the title of E11-2 and the last sentence of
  E11-5 were rewritten. The added criterion and E11-1's added sentences are not
  relaxations: they take over the half of M24's criterion 2 and E24-1 that M24
  could not prove, verifying 'an Imago return' before boot, because the
  signature form of the Imago result is to be pinned in M09 and nothing produces
  a result yet. E11-1 read 'Positive: digest and signature verify locally.
  Negative: a tampered image digest or bad signature is rejected. Boundary: a
  producer version exactly at the floor is accepted, and one below is
  rejected.'; those sentences stand unchanged. Criterion 6 and E11-2 are
  relaxed, and are disclosed as such: criterion 6 read 'The M24 harness is
  reused unchanged and the only new input is the Imago artifact; a criterion
  states that no second boot apparatus is built here.' and E11-2 read 'Positive:
  the Imago artifact is fed to the M24 harness with no change to the harness,
  and the harness reports the same evidence shape it reported for a pinned
  upstream image. Negative: no second boot apparatus is built here -- a change
  to the harness fails this milestone rather than being absorbed into it.
  Boundary: under decision D72 the real boot evidence is M24's epic and the A/B
  transfer is E11-4's; what this milestone adds is the artifact and the proof
  that the harness needed nothing new to accept it.' Both now allow exactly one
  change to the harness, the pin scheme for the signature form M09 pins, with
  its own positive, negative and boundary cases, and still fail this milestone
  on any other change or on a second boot apparatus; without that exception the
  moved acceptance could not be met by any milestone. E11-2's title read 'The
  Imago artifact runs through the unchanged M24 harness' and E11-5's last
  sentence ended 'runs on the unchanged M24 harness (E11-2).'; both now name the
  same one exception and relax nothing beyond it, and the reference-profile
  rationale, which records why M24 was split out, keeps its wording and gains a
  dated sentence saying so. Nothing else changed. M24 is done, so M09 is the one
  blocker left, and this entry is not evidence that any criterion is met.
- Disclosure, in the shape M18 recorded, because a milestone that edits its own
  bar must say so where the bar is judged: before any delivery, on 2026-09-28
  under decision D92, one exit criterion and epic E11-7 were added and the D85
  criterion and E11-1's acceptance were reworded. The added criterion and E11-7
  are not relaxations: they take over E09-1's positive half, 'an accepted
  request returns image digest, signature reference and boot-evidence fields',
  which M09 could not prove because nothing produces imago.p01.product-result.v1
  at imago 16f964b (cordanaLLM/imago#46); E11-7's negative and boundary are new
  and use the bounds imago's Result type declares. The D85 criterion read '...
  Once M09 pins the signature form of the Imago result (the signature-ref of
  imago.p01.product-result.v1), this milestone adds that form ... and a scheme
  added before M09 pins the form would guess its contract.', and E11-1 read '...
  through the pin scheme this milestone adds once M09 pins the signature form of
  the Imago result: ...'; both now say the form is pinned here, with the result
  itself, and nothing else in either changed. The reference-profile rationale's
  D85 sentence, ending 'which M11 adds once M09 pins it', keeps its wording and
  gains a dated sentence saying the form is pinned here under D92. M09 is done,
  so every milestone in blocked_by is done and the register rule makes M11
  ready. That is a register state, not an unblocking: the first exit criterion,
  'BLOCKED until: Imago accepts the M09 manifest and returns an image/UKI with
  digest and signature', still holds -- imago accepts the manifest (M09) and
  returns nothing -- and this entry is not evidence that any criterion is met.
- Disclosure, in the shape M18 recorded, because a milestone that edits its own
  bar must say so where the bar is judged: before any delivery, on 2026-09-29
  under decision D101 (ADR-0004), the REQ-P12-01 criterion and E11-6's
  acceptance were reworded. The criterion read 'REQ-P12-01 (D91): the
  integration adapter requests an image/UKI build from Imago, and accepts its
  return, only for a commit on which the M04 accessibility gate reported PASS; a
  FAIL, or a SKIP for a missing engine, image or offline store, stops the
  request before it is sent. The image still carries no UI (D20): the gate is a
  build precondition, not image content', and E11-6 read 'Positive: with the M04
  gate passing on the commit, the build request is sent and its record names the
  accessibility run id. Negative: a planted accessibility violation stops the
  request before it is sent. Boundary: a gate that printed SKIP stops it too; a
  skip is not a pass.' Both now add the P05 shell's AccessKit tree check, which
  replaces the scan of ui/forum-shell D91 had planned, as a second precondition
  once M16 delivers it. Neither is a relaxation: the M04 precondition is
  unchanged and one is added. Nothing else in this milestone changed; it stays
  ready by the register rule while its external BLOCKED-until criterion holds,
  and this entry is not evidence that any criterion is met.

Epics:

- **E11-1 Imago result consumed and verified**. Requirements: REQ-P01-01,
  REQ-P01-06, REQ-P01-08. Acceptance: Positive: digest and signature verify
  locally. Negative: a tampered image digest or bad signature is rejected.
  Boundary: a producer version exactly at the floor is accepted, and one below
  is rejected. Under decision D85 this runs in the M24 harness before any boot,
  through the pin scheme this milestone adds once the signature form of the
  Imago result is pinned here (E11-7, D92): the Imago return's digest and
  signature verify before the boot, a tampered digest or a bad signature is
  refused before boot and not during it, and the upstream pin M24 recorded still
  verifies unchanged. This half moved from M24's criterion 2 and E24-1, whose
  requirements it shares.
- **E11-2 The Imago artifact runs through the M24 harness, changed only by the
  D85 pin scheme**. Requirements: REQ-P01-05, REQ-P01-01. Acceptance: Positive:
  the Imago artifact is fed to the M24 harness with no change to the harness
  except the pin scheme for its signature form that D85 moves here (E11-1), and
  the harness reports the same evidence shape it reported for a pinned upstream
  image. Negative: no second boot apparatus is built here -- any other change to
  the harness fails this milestone rather than being absorbed into it. Boundary:
  under decision D72 the real boot evidence is M24's epic and the A/B transfer
  is E11-4's; what this milestone adds is the artifact, that one pin scheme and
  the proof that the harness needed nothing else to accept it.
- **E11-3 Analyzer gate without suppression in the image path**. Requirements:
  REQ-CI-01. Acceptance: Positive: the M03 gate passes on the image inputs.
  Negative: an ignored-key diagnostic fails the image acceptance. Boundary: no
  step carries failure suppression.
- **E11-4 A/B transfer exercised**. Requirements: REQ-P01-04, REQ-P02-04,
  REQ-P02-08. Acceptance: Positive: the second slot is written read-only and
  boots. Negative: a transfer with a bad signature is discarded. Boundary:
  watchdog expiry before bless rolls back once.
- **E11-5 dm-verity root verified on the real image**. Requirements: REQ-P02-01.
  Acceptance: Positive: the verity root hash the harness reads back from the
  booted Imago image matches the value its signed release records. Negative: a
  modified root image fails verity and does not boot to the established state.
  Boundary: a root image identical to the signed one except for one flipped bit
  fails exactly as a wholesale modification does. This acceptance moved from
  M24's E24-2 under D84 and runs on the M24 harness, changed only as E11-2
  allows.
- **E11-6 UKI compilation gated on the accessibility pass (D91, D101)**.
  Requirements: REQ-P12-01. Acceptance: Positive: with the M04 gate and, once
  M16 has delivered it, the shell's AccessKit tree check both passing on the
  commit, the build request is sent and its record names the run of each.
  Negative: a planted accessibility violation in either stops the request before
  it is sent. Boundary: a gate that printed SKIP stops it too; a skip is not a
  pass.
- **E11-7 Imago product result for the M09 request (moved from E09-1 by D92)**.
  Requirements: REQ-P01-01, REQ-P01-06. Acceptance: Positive: an accepted
  request returns image digest, signature reference and boot-evidence fields:
  imago emits an imago.p01.product-result.v1 document for the request M09's gate
  proves it accepts, its correlation-id is the request's, and imago's
  Result.Validate accepts it at the pinned commit. Negative: a result whose
  image-digest is not 'sha256:' and 64 lowercase hex digits is refused by
  Result.Validate, and one whose correlation-id is not the request's is refused
  by the integration adapter, each with a correlated error. Boundary: a
  signature-ref and a boot-evidence-ref of exactly 512 characters (imago's
  MaxReferenceLen) are accepted, and 513 are refused. Moved from M09's E09-1
  under D92; cordanaLLM/imago#46 tracks the emitter.

### M10 - eBPF objects re-verified against the Nucleus-pinned kernel in a VM

Rank 26. State: ready. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
Needs hardware: yes. Needs external contract: yes. Reference profile: partial.
Blocked by: M09, M19, M23. Unblocks: M12.

Exit criteria:

- BLOCKED until: Nucleus publishes an imago.nucleus.kernel-artifact.v1 manifest
  for a kernel it built, and that kernel's version/config digest, artifact
  digest and provenance are verified and recorded (moved here from M09's E09-2
  by D92). At the nucleus commit M09 pinned (8672247) the forge compiles no
  kernel (nucleus #18) and verify-requirements checks a hardcoded symbol list
  rather than the Aegis kernel requirement (nucleus #20), so no such manifest
  exists
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
- D94: every feature of the Aegis kernel requirement
  (build/kernel-requirement.json) is checked against the Nucleus kernel's own
  config inside the VM before any M19 object loads, and a feature that config
  does not satisfy is rejected with the requirement's correlation-id and the
  symbol named; this is the check M09's E09-2 negative could not run, since
  imago has no built kernel to check against (D92)
- One criterion restates that the M19 host-kernel fixture does not close this
  milestone and that a pass here on the reference profile's VM is development
  evidence only, closing neither the hardware nor the release gate.

Cheapest exit: Boot the Nucleus kernel directly in QEMU/KVM with a minimal
initramfs and repeat the M19 loads. The M19 host-kernel fixture does not close
this milestone.

Evidence (the D92 disclosure of 2026-09-28, the disclosure of 2026-09-29, the
E10-1 finding and the gate; every entry is in `planning/roadmap.json`, and the
run is on `docs/build/nucleus-kernel.md`):

- Disclosure, in the shape M18 recorded, because a milestone that edits its own
  bar must say so where the bar is judged: before any delivery, on 2026-09-28
  under decision D92, the first exit criterion was reworded and epic E10-4
  added. The criterion read 'BLOCKED until: the M09 pair delivers a Nucleus
  kernel whose version/config digest is recorded'. It now waits on a
  Nucleus-published imago.nucleus.kernel-artifact.v1 manifest for a built
  kernel, because D92 closed M09 on the consumption contract and moved E09-2's
  positive half, 'a result returns kernel version/config digest, artifact digest
  and provenance', here as E10-4, whose negative and boundary are new. Neither
  is a relaxation: the kernel M10 boots is still a Nucleus kernel with its
  digests recorded. The reference-profile rationale, which ends 'the only
  genuinely external input left here is the Nucleus kernel M09 has to obtain',
  keeps its wording and gains a dated sentence saying that kernel is now this
  milestone's own first criterion and E10-4. M09 is done, and with M19 and M23
  done the register rule makes M10 ready. That is a register state, not an
  unblocking: the first exit criterion still holds, because nucleus has
  published no kernel manifest (nucleus #18, #20), and this entry is not
  evidence that any criterion is met.
- Disclosure, in the shape M18 recorded, because a milestone must say where its
  evidence differs from the letter of its bar (2026-09-29). No exit criterion,
  epic or the cheapest exit was rewritten by this delivery, and M10 is not done.
  Eight points are recorded. (1) Criterion 4 and E10-1's positive are not met:
  action_gate loads through the Nucleus kernel's verifier and does not attach
  (the E10-1 entry); since D107, dated 2026-09-29, E10-5's positive is not met
  on that release either: the check criterion 7 names refuses it against the
  fourteen-row requirement before any load (the criterion 7 entry). (2)
  Criterion 7 says the requirement is checked against the kernel's config inside
  the VM: the readback guest prints its own
  /proc/config.gz, tools/kernel_requirement_check.py decides every row on that
  text on the host, and the load guest hashes /proc/config.gz again and loads
  nothing unless the sha256 is the one checked; no Python runs in the guest. (3)
  M19's loader gained a tp-attach mode and a --map-set option
  (bpf/loader/aegis_bpf_probe.c); its four M19 modes are unchanged and
  tools/test_bpf_objects.py passes. (4) The objects are M19's sources compiled
  by M19's gate functions, against a vmlinux.h dumped from the Nucleus kernel's
  own BTF, extracted from the verified image, not the host's. (5) Guest loads
  run under setpriv as uid 65534 with M19's capability set -all,+bpf,+perfmon;
  PID 1 is root, so M19's sudo is absent, and tracefs is mounted group-owned by
  gid 65534 because CAP_PERFMON does not bypass file permissions. (6) The
  runtime probes the requirement names for powercap and the IOMMU groups are not
  read in the guest: QEMU presents neither RAPL counters nor an IOMMU. (7)
  cosign 2.6.3 is admitted by M10, having been admitted nowhere before, and held
  below 3 because the reference profile's second copy, 3.1.3, deprecates the
  --offline flag the gate passes; it verifies SHA256SUMS against the pinned
  identity, tag commit and tag ref, while imago checks only that the bundle is
  present. (8) tools/kernel_requirement_check.py reimplements in Python the
  configuration and identity half of KernelRequirement::unmet (the criterion 7
  entry); it does not decide the capability rows the Rust half reads from a
  profile.
- E10-1 NOT met (2026-09-29), runs r20260929T195935-62a3 and
  r20260929T203240-505f: the negative holds, M19's unchecked-pointer variant
  rejected with 'R7 invalid mem access 'ringbuf_mem_or_null''. The positive does
  not: action_gate loads (load_rc=0) and its attach fails with 'failed to
  attach: -EBUSY'. A BPF LSM program attaches through a BPF trampoline, which on
  x86 patches the five-byte nop -mfentry leaves at the hook's entry;
  __bpf_arch_text_poke in arch/x86/net/bpf_jit_comp.c returns -EBUSY when the
  bytes there are not that nop, and the Nucleus kernel is built with
  CONFIG_FUNCTION_TRACER not set, so no such nop exists, although
  CONFIG_BPF_LSM=y and Kconfig does not make BPF_LSM depend on the tracer. The
  hypothesis was checked: two kernels from M26's verified linux 7.2.5 source and
  M26's configuration, one with CONFIG_FTRACE, CONFIG_FUNCTION_TRACER and
  CONFIG_DYNAMIC_FTRACE on and otherwise differing only in the options those
  select and the host's pahole version, booted through the same boot() with the
  same initramfs, objects, loader and capabilities: the unchanged one failed
  with -EBUSY and the other attached, the marker exec reached the ring buffer
  and it detached with marker_seen=1 (kept under
  ~/.cache/aegis-nucleus-kernel/control-function-tracer-20260929). The Nucleus
  kernel satisfies build/kernel-requirement.json as written, and so would M26's,
  which has the same gap; the requirement does not ask for the trampoline. What
  would close E10-1 is decision D107 (decided 2026-09-29: add the requirement
  row), recorded in docs/roadmap/README.md, not taken here: a requirement row
  for CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS, carried through M09's pinned
  fixtures once imago and nucleus vendor the new bytes, and a Nucleus release
  built to it. (Dated 2026-09-29, after D107 was recorded:
  build/kernel-requirement.json now carries the row, M09 pins imago 987b95a and
  nucleus 852be74, which vendor and hold it, and M26's configuration carries it
  too; the statements above that the requirement does not ask for the
  trampoline, that the Nucleus kernel satisfies it and that M26 has the same gap
  describe the payload of these runs. Run r20260929T232643-b28e ran the gate
  against the same release with the new requirement, into a fresh cache and with
  imago 987b95a: the release stage passed,
  nucleus/requirement-satisfied-before-any-load rejected
  CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS with the correlation id, the
  requirement stage's negative and boundary cases failed with it because the
  configuration already carries that unmet row, and no M19 object was loaded.
  E10-1 now waits for a Nucleus release built to the new requirement, which
  nucleus's ADR-0011 names v7.2.8-realtime-lusoris2.)
- The gate (2026-09-29): make verify-nucleus-kernel runs
  tools/verify_nucleus_kernel.py, outside make verify-all like verify-latency
  and verify-boot, and make nucleus-kernel-fetch is its one networked step. A
  host without the reference profile's capabilities prints a SKIP line that
  names the reason and ends 'the Nucleus kernel gate did not run.', and exits 0:
  not Linux (macOS and Windows name their platform), no read-write /dev/kvm, no
  /proc/config.gz, a tool missing or outside its admitted range, no M09 contract
  cache, or no fetched release, observed on 2026-09-29 with an empty cache
  directory, and with a PATH naming /usr/bin/cosign 3.1.3, which the gate and
  the fetch both refuse. A cache that is present but wrong is a FAIL, observed
  the same day with symlinked assets and an altered SHA256SUMS.
  tools/test_nucleus_kernel.py (82 tests) and
  tools/test_kernel_requirement_check.py (30 tests), inside make verify-all,
  hold the pin, the pre-boot refusal, the stage order, the signature's identity
  and revision binding, the release floor, the D94 rules, the report parser, the
  capability diff, the load checks, the skip lines, cosign's admitted range, the
  fetch's refusal of other bytes, the admission and the programs the gate may
  start. docs/build/nucleus-kernel.md records the run: r20260929T195935-62a3 was
  the first to reach every case, and r20260929T203240-505f, started at 20:32:40
  local time after the gate's files were last edited at 20:32:20, is the
  recorded run, with the same eighteen outcomes. This is development evidence on
  the reference profile only: the M19 host-kernel fixture does not close M10,
  and a pass here would close neither the hardware nor the release gate.

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
- **E10-4 Nucleus kernel result recorded (moved from E09-2 by D92)**.
  Requirements: REQ-P01-09, REQ-P07-01. Acceptance: Positive: a result returns
  kernel version/config digest, artifact digest and provenance: a
  Nucleus-published imago.nucleus.kernel-artifact.v1 manifest for a kernel
  Nucleus built is accepted by `imago kernel artifact verify` at a pinned imago
  commit against the release assets it names, and its kernel.release,
  kernel.config_digest, each artifact's sha256 and the provenance revision are
  recorded before any M19 object loads on that kernel. Negative: a manifest
  whose artifact digest does not match the downloaded artifact is refused before
  the kernel boots. Boundary: a kernel whose release is exactly the kernel
  requirement's abi.minimum-release (6.12) is accepted, and one below it is
  refused. Moved from M09's E09-2 under D92.
- **E10-5 Kernel requirement satisfied by the built Nucleus kernel (D94)**.
  Requirements: REQ-P07-01, REQ-P01-09. Acceptance: Positive: every feature of
  the kernel requirement is satisfied by the Nucleus kernel's config and each
  symbol's state is recorded. Negative: a requirement feature the config does
  not satisfy, such as a required symbol set to n, is rejected with the
  requirement's correlation-id and that symbol named. Boundary: a feature
  required as a module is satisfied by m, and a feature required built-in is not
  satisfied by m.

### M20 - TPM2 attestation slice on swtpm: audit-record signing and /var unseal

Rank 27. State: blocked. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
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
  pinned through the template matrix before any quote is taken. Both are present
  on the reference profile -- swtpm 0.10.2-1.1 and tpm2-tools 5.8-1.1, read back
  from `swtpm --version` and `tpm2_pcrread --version` -- so what this criterion
  still requires is the pin, not the installation.
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
- tpm2-tools 5.8 is installed on the reference profile; /dev/tpmrm0 is group tss
  and the developer account is a member (re-read 2026-09-28), so the PCR read
  runs unprivileged
- PCR 7 is measured inside a guest booted against an enrolled variable store, so
  the value attests a key set rather than recording that Secure Boot was off
- Moved here from M24's E24-2 by D84: an unseal attempt under a PCR policy that
  omits PCR 11 fails to unseal /var on the real M11 image, whose /var is
  TPM-sealed. M24's pinned upstream image has no sealed /var, so the acceptance
  can only be judged here.

Cheapest exit: Use the M11 VM with swtpm. No physical TPM is required for this
milestone.

Evidence:

- Disclosure, in the shape M18 recorded, because a milestone that edits its own
  bar must say so where the bar is judged: before any delivery, one exit
  criterion and epic E20-3 were added on 2026-09-28 under decision D84. None is
  a relaxation: they add the boundary M24's E24-2 carried until then -- 'a PCR
  policy that omits PCR 11 fails to unseal /var' -- because a pinned upstream
  image has no TPM-sealed /var, and E20-2 already cites the same REQ-P02-02. M24
  does not pre-prove D63's tpm2-tools quote path: its harness reads PCRs through
  sysfs, so the criterion recording that pre-proof is still open here. Nothing
  else in this milestone changed; it stays blocked on M11 and M14, and this
  entry is not evidence that any criterion is met.

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
- **E20-3 PCR 11 is required to unseal /var**. Requirements: REQ-P02-02.
  Acceptance: Positive: /var unseals under the enrolled policy that binds PCR 0,
  4 and 11 (D63). Negative: a PCR policy that omits PCR 11 fails to unseal /var.
  Boundary: a policy that differs from the enrolled one only by dropping PCR 11
  is refused, so PCR 11 is load-bearing rather than incidental. This acceptance
  moved from M24's E24-2 under D84.

### M12 - GPU-backed slices: DMA-BUF, VFIO and P2PDMA paths

Rank 28. State: blocked. Cost: large. Owner repository: cordanaLLM/Aegis-OS.
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

Rank 29. State: blocked. Cost: medium. Owner repository: cordanaLLM/Aegis-OS.
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

- Cross-repository contracts are verified for consumption only. M09 pins
  imago's decoding of both M18 payloads (D92), but imago builds no image yet, so
  the result M11 waits for depends on producer work Aegis cannot do:
  cordanaLLM/imago issue 46. nucleus published its first kernel on 2026-09-29,
  and M10 verified it; D107 put the BPF trampoline's prerequisite into the
  kernel requirement the same day, and what M10 now waits for is a Nucleus
  release built to it.
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
  **Decision (2026-09-16, supersedes the maturity condition):** The maturity
  condition is met and sveltesentio is adopted now, not evaluated as a
  candidate. `golusoris/sveltesentio` publishes nineteen `@sveltesentio/*`
  packages to npm under MIT, versioned per package (`@sveltesentio/ui` 0.5.0,
  `@sveltesentio/shell` 0.2.0, `@sveltesentio/core` 0.3.0 at this decision), so
  a consumer pins a released package version rather than a repository revision.
  The toolchain rule is unchanged: Node LTS and pnpm are still selected through
  the template matrix, and each `@sveltesentio/*` package is pinned in the UI
  component's manifest and lockfile before any UI gate runs. This settles the
  toolchain question only; it admits no package that a component has not
  declared, and the accessibility and UI gates remain blocked on their own
  evidence.
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
  Outcome at M23: the constraint was met and the mechanism was superseded. D70
  put kernel construction in this repository while Nucleus is a scaffold, so the
  guest boots the kernel M26 builds and **no linux-rt package is downloaded,
  installed or booted**. The host-unmodified half of D57 therefore holds by
  construction rather than by a download-only procedure, and `pacman -Qq
  linux-rt` reports the package is not installed.
- **D58** Is Firecracker or QEMU microvm the sandbox VMM for P10 and the new
  M22? Recommended: Pin Firecracker 1.17.0 (firecracker 1.17.0-1.1) for the
  headline boot-time and footprint numbers, with QEMU 11.1.1 microvm as a
  recorded fallback. Every measurement must record which VMM produced it, and
  the two sets are never presented as interchangeable. Why: both monitors are
  installed on the reference profile, re-probed on 2026-09-13 -- `command -v
  firecracker` prints /usr/bin/firecracker and exits 0, `firecracker --version`
  prints Firecracker v1.17.0 from package firecracker 1.17.0-1.1, QEMU 11.1.1 is
  installed and /usr/share/edk2/x64/MICROVM.4m.fd is on disk -- so both
  measurements are available today at zero install cost, which is what
  planning/hardware-profile.json records. The numbers are not equivalent, and
  M21's original criterion says measured boot time and footprint
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
  **Host reading (2026-09-28):** SecureBoot still reads 0, but SetupMode now
  reads 1 and no PK, KEK, db or dbx variable exists; the maintainer reports
  clearing the keys before a firmware update, with no exact date.
  `planning/hardware-profile.json` records the reading and the report. The
  decision is unchanged: Secure Boot evidence stays guest-side, host enrolment
  stays an operator action, and PCR 7 on the host still attests to an unsigned
  configuration.
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
  values (Node v26.8.2, pnpm 10.29.3) in the template matrix beside the chosen
  pin, and pin the browser revision bundled by the pinned `@playwright/test` and
  container image digest (restated under D90). Why: The machine runs a third
  version that appears nowhere in the register, so the row cannot be closed by
  picking a side — the evidence has moved past both options. M01's register
  explicitly marks these as proposal data to be resolved rather than inherited,
  and M04's gate would otherwise pass on whatever the workstation happens to
  have. The Playwright browser cache was already populated (chromium-1228 on
  2026-09-13), which makes it particularly easy for an unpinned gate to look
  green for the wrong reason. **Decision (2026-09-13):** The UI toolchain admits
  the latest Node major (26.x as installed on the reference profile) and tracks
  it forward with Renovate, rather than pinning an older line. **Recorded at M04
  (2026-09-28):** the accessibility gate pins Node 26.10.0 by the sha256 of its
  release tarball; the 26.x line enters Active LTS on 2026-10-28 (the
  nodejs/Release schedule, read 2026-09-28), and D42 is settled by this
  decision. The host browser cache is written by Praetor's figure engine, held
  chromium-1243 on 2026-09-28, and is never the evidence (D90). Renovate counts
  a Node release as unstable until its line reaches LTS, so once 26.x is LTS it
  would not offer the next Current line; `renovate.json` sets `ignoreUnstable`
  to false for `node` in `ui/concordia-tokens`, so the pin is offered each new
  release line when it is released.
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
  **Decision (2026-09-13, M19):** Aegis original. `bpf/scx_cake.bpf.c` shares a
  name with `/usr/bin/scx_cake` from `extra/scx-scheds 1.1.3-2` and derives
  from neither it nor its upstream `https://github.com/sched-ext/scx`: no line
  is copied, no commit is vendored, and it is not a fork. It descends from the
  imported P07 proposal sketch under `.workingdir/prepared/scaffold/`,
  rewritten rather than imported -- the sketch's `enqueue` classified a task
  and then dispatched it nowhere, which attached would stall every runnable
  task until the watchdog ejected it. The provenance is recorded in the
  object's own header and asserted by `tools/test_bpf_objects.py`.
- **D67** May M19's struct_ops boundary load displace the scheduler currently
  holding /sys/kernel/sched_ext/root/ops on the reference host? Recommended:
  Yes, as a deliberately scheduled disruptive step with recorded detach and
  reattach, not as a background test. The pre-existing root/ops value must be
  captured, the stubbed scheduler attached, then the original restored and
  re-verified. Why: Only one scx scheduler can hold root/ops at a time, and the
  reference host runs exactly one at any moment — so M19's boundary criterion
  ('a struct_ops load with all handlers stubbed succeeds where the host exposes
  sched_ext') necessarily takes over CPU scheduling on the maintainer's working
  machine for the duration. That is
  executable and it is the right test, but it is not something to discover
  mid-run. Deciding it in advance also produces the attach/detach evidence the
  criterion should have carried all along.
  This clause carried readings that M19 found to be wrong, and they are
  corrected here as well as in the register: `root/ops` read
  `rusty_1.1.3_x86_64_unknown_linux_gnu` at M19 time, not `ghostbrew`. M19's own
  first correction -- that `switch_all` and `nr_rejected` do not exist -- was
  itself wrong and is withdrawn: `/sys/kernel/sched_ext/root/` holds exactly
  `events` and `ops`, but both counters are top-level `sched_ext` attributes one
  directory up, and only `root/` had been listed. They are captured at each
  point of the sequence, and `switch_all` read `0` for the whole hold window,
  which is the kernel's own measurement that no task was switched to the stub.
  **Decision (2026-09-13, M19):** Yes, as recommended, and it ran. The sequence
  is recorded in `docs/build/bpf.md`: the restore path was proven first, the
  scheduler was released through `org.scx.Loader`, the stub was attached for a
  bounded one-second window, detached, and the original restored and
  re-verified **by name**. The case is opt-in behind `--allow-scheduler-takeover`,
  so it is never a background step. It also uncovered that the reference host
  has two scheduler supervisors enabled at once, which is why the restore check
  compares names rather than accepting that something is attached.

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
- **D71** Which milestone owns the Firecracker and AF_VSOCK sandboxing epic,
  M21 or M22? Options: M21, whose title already claims KVM sandboxing
  alongside the RAPL counters; M22, whose title is the microVM measurement
  itself; or both, with the epic split so each owns a half. Why: E21-2 and
  E22-2 are byte-identical, including their acceptance text and their
  REQ-P10-01/REQ-P10-03/REQ-P16-04 citations, so two milestones claim the same
  work and whichever runs second would either duplicate it or silently skip
  it. Found by sweeping the register for epics sharing an acceptance text.
  Separately and not in question: E22-1 duplicates E21-1's RAPL telemetry epic
  while M22's own criteria never mention RAPL, so that copy is simply wrong
  and belongs to M21.
  **Decision (2026-09-13):** Split by what each milestone does. M21 proves the
  sandbox path -- a microVM boots, one AF_VSOCK candidate evaluation
  round-trips, an over-limit request is refused -- and keeps E21-2. M22
  measures it: E22-1 is the boot time and memory footprint with each figure
  naming its VMM per D58, and E22-2 is the AF_VSOCK transport measured on the
  admitted VMM. M22's copy of the RAPL epic is gone; that work was only ever
  M21's. M21's unblocking-evidence criterion now defers the measured boot time
  and footprint to M22 rather than claiming them too, so the two milestones no
  longer overlap on the figures either.

- **D72** Which milestone owns the Imago-result and boot-evidence epics, M11
  or M24? Options: M24, which builds the harness over an externally supplied
  artifact and states that artifact may be a pinned distribution image or an
  Imago return; M11, which supplies the Imago artifact and states that the M24
  harness is reused unchanged and no second boot apparatus is built there; or
  a split where M24 owns the harness half and M11 owns the artifact half. Why:
  E11-1/E24-1 and E11-2/E24-2 are byte-identical pairs, so the two milestones
  claim the same acceptance while their criteria describe a deliberate
  division of labour. Found by the same sweep as D71.
  **Decision (2026-09-13):** Split by where the work happens. M24 owns the
  harness and the real boot evidence: E24-1 becomes verification of whatever
  artifact is supplied, upstream image or Imago return, and E24-2 keeps the
  boot evidence. M11 owns the Imago result: E11-1 is unchanged, and E11-2
  becomes the claim its own criterion already made -- that the harness is
  reused unchanged and no second boot apparatus is built. The A/B transfer
  stays E11-4's, which already held it. This keeps M24 deliverable now on a
  pinned upstream image rather than waiting on Imago.

- **D73** Does a host kernel update fail `make verify-latency` until the
  recorded host reading is re-recorded, and must the guest run satisfy a tier
  edge? Options: keep both rules and take a new measurement after every host
  update; keep them and accept relabelling the recorded release; or treat
  REFERENCE_HOST as provenance, decide the host case from each run's probe,
  and report the guest's satisfied edges as the measured outcome. Why: the
  reference host runs a rolling kernel (7.2.4, 7.2.5 and 7.2.8 within two
  weeks), and the first update was answered by relabelling the 7.2.4 figure
  as 7.2.5 without a measurement (PR #119). Separately, host load pushed the
  guest's worst case past the 8 ms frame edge in six of seven printed runs on
  2026-09-27, failing a requirement that M23's exit criteria do not state.
  **Decision (2026-09-27):** The third option. A recorded figure keeps the
  kernel that produced it; `tools/verify_latency_fixture.py` prints the
  recorded and running releases side by side and fails the host case only on
  what its probe reads. The guest case still requires the M26 release, a
  PREEMPT_RT reading and a non-empty sample. Re-recording REFERENCE_HOST means
  a new measurement, recorded under the kernel that produced it.

### Decisions from the desktop environment specification (2026-09-27)

Source: `.workingdir/aegis-desktop-environment-specification.md` (private,
sha256 prefix `a41f6c9e825d`, cited elsewhere as source id `desktop-spec`), a
notebook export (version 2.0.0-DEV) and therefore proposal data; this section
calls it the specification. On 2026-09-27 the maintainer selected four of its
pillars for the design and decided D74 to D77 below, revising D75 on 2026-09-28.
D78 to D80 record what that revision required, D81 sets the accessibility target
of the new requirements, D82 was open until 2026-09-29, when ADR-0004 recorded
its answer, and D83 scopes HISS-03 on P17's frame path. The change to the
concept is ADR-0003 (public:docs/adr/0003-display-runtime-p17-scaena.md
`706676855b65`): P17 aegis-scaena is recorded as a proposal, and nothing is
activated. Three rows of the specification's status table are wrong:
`crates/aegis-ipc` and `crates/praetor` are marked implemented and neither
exists (Praetor is the separate Go repository cordanaLLM/praetor), and there is
no `crates/aegis-shell`; the recorded shell is P05 aegis-forum-shell. The
specification's seL4 memory-limit example does not apply to a Linux system. Not
selected: 6DOF hand terminals with bare-hand gesture input, and TEE attestation
beyond TPM2 (TDX, SEV-SNP, CCA).

- **D74** Does P05 Forum shell adopt the specification's post-WIMP spatial
  canvas: interaction instruments and surrogate objects in place of menus and
  modal dialogs, QuadTree viewport culling, space-scale pan and zoom with
  focus+context magnification, and DOM-fragment transclusion? Options: the
  canvas as P05's primary surface; the canvas as one workspace inside a
  conventional P05 shell; P05 as recorded. Recommended: the canvas as one P05
  workspace, built against the M16 state and lifecycle contract, with P12
  requirements written before activation: keyboard and screen-reader reach for
  every instrument and surrogate, reduced motion for pan, zoom and
  magnification, and a stable layout for screen-magnifier users. Why: P05 has no
  UI code yet (`ui/` holds only its README) and M16 is blocked on M04 and M14;
  under D10's 2026-09-16 decision its UI builds on `@sveltesentio` packages,
  which require Svelte 5 or later. Every UI component carries a P12 edge
  (REQ-P12-01, REQ-P12-03, REQ-P12-05, REQ-P12-06), and the specification names
  none. `@sveltesentio/shell` 0.2.0 provides device-class layout and D-pad
  focus, not a spatial canvas, so the canvas would be built here.
  **Decision (2026-09-27):** The first option, not the recommendation: the
  spatial canvas is P05's primary surface and replaces the conventional shell
  entirely (ADR-0003). The recommendation's preconditions were not part of the
  choice; the accessibility rows the canvas needs are derived from the D81
  target and recorded in `docs/roadmap/requirements.md`: REQ-P05-09 to
  REQ-P05-15 cover keyboard reach, no focus trap, paint-only culling, instrument
  and surrogate semantics, focus under camera zoom, pointer alternatives, and
  text resize with a linear mode for magnifier users. As planning that follows
  the answer, not as part of it, M16 holds the canvas state and tests four of
  those rows. REQ-P05-11 makes culling paint-only, which overrides the
  specification's culling of off-screen nodes from layout, so every node stays
  reachable by assistive technology. Which engine renders the canvas is D82,
  still open. Addendum (2026-09-29, ADR-0004): D82 is decided, native Rust with
  AccessKit on gpui (D102). Under the re-scoped M16, M16 tests three of those
  rows on the model (REQ-P05-09 to REQ-P05-11), and REQ-P05-13 moves to M28,
  which renders pixels.
- **D75** Does Aegis build a Rust-native display runtime (Servo, WebRender,
  wgpu) that mounts shell surfaces through `zwlr_layer_shell_v1` and imports
  hardware-decoded video as DMA-BUF without copies? Options: a new component on
  the `servo` crate (0.6.0, 2026-09-25) and wgpu (30.0.1); a conventional
  webview for the shell, with zero-copy video only in the native P04/P08 path;
  defer until M25 and M12 demonstrate a display path. Recommended: defer, and
  evaluate the Servo option at M25, where DMA-BUF sharing is first demonstrated.
  wgpu imports single-plane DMA-BUF (gfx-rs/wgpu#9366, merged 2026-04-09) but
  not the multi-planar NV12 buffers VA-API and NVDEC produce (gfx-rs/wgpu#9801,
  open). Why: no recorded component owns a web runtime. Servo records Linux
  assistive-technology detection as unreliable (servo/servo#46834, open),
  against REQ-P12-03 and REQ-P12-08. The specification names smithay for surface
  binding, which D08 checked and did not admit at M07 (0.7.0 is still current);
  a layer-shell client needs `wayland-client` and `smithay-client-toolkit`
  (0.21.1), which no decision has checked.
  **Decision (2026-09-28, revising the answer of 2026-09-27):** A new component,
  P17 aegis-scaena, in which the compositor composes (ADR-0003). The 2026-09-27
  answer, one Servo, WebRender and wgpu scene built now, did not survive
  verification: webrender 0.70 renders through OpenGL (gleam) and not wgpu, wgpu
  cannot import the multi-planar NV12 buffers decoders produce
  (gfx-rs/wgpu#9801), Servo runs its own event loop, which is not Send, and its
  own tokio runtime, and both paths need unsafe code the workspace forbids with
  no per-crate override (error E0453, reproduced). Instead the shell engine and
  each video stream own separate Wayland surfaces, and decoded video reaches the
  compositor as DMA-BUF descriptors over the D77 SCM_RIGHTS path to
  `zwp_linux_dmabuf_v1`, which composites it without a copy. The no-Servo video
  slice comes first, within the ban on unsafe code, and the shell engine is
  chosen later behind a fixed surface contract (D82). As planning that follows
  the answer, not as part of it, that slice is M27: VA-API decode on the Arc
  A380 (iHD, the device the compositor imports from) with the driver and render
  node pinned per run, export through cros-libva (D80), the fd sent with rustix
  `sendmsg` over a socketpair with one JSON-RPC 2.0 descriptor line, and the
  receiver attaching it to a smithay-client-toolkit layer surface, with no wgpu;
  SpiderMonkey provenance is deferred with the engine. The graph of record is
  unchanged, so D38's recommendation to keep the sixteen-node shape of
  export-062 still holds: P17 and its four edges are recorded beside that graph,
  marked 'not in export-062' in `docs/roadmap/requirements.md`, not inside it.
- **D76** Is the specification's heads-up visual language the design language
  for P12 Concordia: security states as colour and frame style (amber dashed for
  unverified, cyan double line while a gate runs, blue with corner brackets once
  verified, crimson hazard frame on fault), scanline, chromatic-aberration and
  Fresnel depth shaders, and diegetic panels anchored in the canvas? Options:
  Concordia's default theme; an optional theme over Concordia's base tokens,
  which M04 creates; decline. Recommended: an optional theme, with each state
  told apart by frame shape and text as well as colour, every shader and
  animation behind the desktop's reduced-motion preference, which REQ-P12-02
  already requires the UI to follow through `org.freedesktop.portal.Settings`,
  and the fault flash held under WCAG 2.3.1's three-flash limit. Why: M04 owns
  the only accessibility gate (axe-core and Playwright, the D16 focus ring,
  D17's rule against monolithic CSS). The specification cites MIL-STD-1472H for
  its colours, and that citation has not been checked against the standard.
  Which recorded state feeds which frame is part of this decision.
  **Decision (2026-09-27):** As recommended: an optional theme over the
  Concordia base tokens M04 creates, never the default. Each state is told apart
  by frame shape and text as well as colour, every shader and animation sits
  behind the reduced-motion preference REQ-P12-02 reads from
  `org.freedesktop.portal.Settings`, and the fault flash stays under the
  three-flash limit of WCAG 2.2 SC 2.3.1 (REQ-P12-10 to REQ-P12-13). As planning
  that follows the answer, not as part of it, M04 keeps every token overridable
  so the theme needs no second token source. Which recorded state feeds which
  frame stays open until the theme has a milestone, and the MIL-STD-1472H
  citation stays unchecked.
- **D77** Do shell, compositor and daemons talk over line-delimited JSON-RPC 2.0
  on Unix sockets for control, with video frames and telemetry kept off the
  sockets? Options: that control plane with iceoryx2 (0.10.0, MIT OR Apache-2.0)
  shared memory as the data plane; that control plane with DMA-BUF descriptors
  passed as `SCM_RIGHTS` messages as the data plane; typed per-edge payloads as
  the crates define them today, with a transport chosen per edge at activation.
  Recommended: the DMA-BUF data plane, decided at M16, whose IPC is stubbed
  today, with iceoryx2 evaluated only if a high-rate stream appears that is not
  a GPU buffer. Why: P04 models a tiered `af_unix` mesh (M07), and P08 and P15
  already carry DMA-BUF descriptor types with no transport;
  `crates/aegis-hestia/src/overlay.rs` already names `SCM_RIGHTS` as how a
  descriptor travels. `@sveltesentio/ipc-sockmap` 0.2.0 frames length-prefixed
  messages on a Node-to-Go edge, not JSON-RPC, so it does not supply this
  contract.
  **Decision (2026-09-27; internal edges 2026-09-28):** As recommended:
  line-delimited JSON-RPC 2.0 over AF_UNIX for control, DMA-BUF descriptors
  passed as `SCM_RIGHTS` messages for data, and iceoryx2 only if a high-rate
  stream appears that is not a GPU buffer. On 2026-09-28 the maintainer moved
  the three Aegis-internal D-Bus edges the question named to JSON-RPC:
  DISPATCH_DECISION_REQUEST (P06 to P05), EMIT_CARBON_TELEMETRY (P13 to P05) and
  SPATIOTEMPORAL_TASK_SHIFT (P13 to P07). The platform D-Bus interfaces stay on
  D-Bus: the freedesktop portal (REQ-P12-02), AT-SPI2 (REQ-P12-03) and the
  StatusNotifierWatcher (REQ-P05-05). Wayland keeps its own protocol for P17 to
  P04. The option chosen on 2026-09-27 deferred the formal decision to M16; the
  2026-09-28 answer recorded the edge move "as part of D77's closure", so D77
  closes here, and M16 applies the framing rather than deciding it. M27 is the
  first milestone that exercises the planes, and M16's stubs for the two moved
  edges it consumes use the framing. D77 does not rule on the edges it did not
  name: SYNC_DESKTOP_SHELL keeps the Unix socket stream the graph records, with
  its endpoint and budget still open under D32, the Zenoh-recorded P04 Tier-2
  mesh and FOCUS_SWITCH_NOTIFY are unchanged, and ACTION_GATE_INTERCEPT (P06 to
  P09), whose transport stays open under D26 (DSP-03), is not moved by D77.
- **D78** How do the lock-wide negative sweeps stay meaningful once one crate
  needs a dependency another crate's sweep refuses? Options: keep every sweep
  lock-wide and keep Wayland and VA-API crates out of the workspace; give P17
  its own workspace and lock; re-scope each sweep to its own crate's dependency
  closure. Why: smithay-client-toolkit 0.21.1 resolves wayland-backend and
  memmap2, which `crates/aegis-compositor/tests/manifest_hygiene.rs` and
  `crates/aegis-lictor/tests/manifest_hygiene.rs` refuse anywhere in
  `Cargo.lock` although neither crate would depend on them, and weakening those
  tests without a record would be evasion.
  **Decision (2026-09-28):** As recommended, the third option. Every crate's
  lock-wide negative sweep reads that crate's own dependency closure from `cargo
  metadata --locked`, so P17 may depend on Wayland and VA-API crates while every
  other crate keeps its guarantee. The change belongs to M27, the milestone that
  first adds such a dependency; M27's exit criteria name the six sweeps, the
  closure rule and the fail-closed causes, and show that on the pre-M27 lock
  each re-scoped sweep reports exactly what the lock-wide sweep reported. What
  the decision gives up: after the re-scope no test forbids md5 or tokio
  workspace-wide, so each guarantee holds per crate, and P17's own sweep carries
  D02's hash exclusion and the async-runtime exclusion into the new crate.
- **D79** What does a run against the host session's compositor count as?
  Options: development evidence for the client half only; no evidence until P04
  serves the protocols. Why: the reference profile's compositor is KDE KWin
  6.7.5, which advertises `zwlr_layer_shell_v1` version 5 and
  `zwp_linux_dmabuf_v1` version 5, while P04 serves no Wayland protocol and its
  tests refuse a server library.
  **Decision (2026-09-28):** As recommended, the first option. A run against the
  host session's compositor is development evidence on the reference profile,
  labelled client half only. P04 serving layer shell and DMA-BUF import is
  separate work that no milestone yet plans, and such a run closes no hardware
  gate.
  **M27 as a leaf (2026-09-28):** As recommended, M27 stays a leaf and unblocks
  no milestone; whether M12 re-runs its display path in the built image is
  decided when M12 is planned.
  **M27 no longer a leaf (2026-09-29):** M28 and M29, planned under ADR-0004,
  both wait on M27, so it unblocks them; the answer above about M12 is
  unchanged.
- **D80** Which VA-API binding does P17 use? Options: cros-libva git-pinned at
  the merge of chromeos/cros-libva#37; the single-author fork `libva` 0.1.4;
  bindings generated in this repository. Why: cros-libva 0.0.13 from crates.io
  (2024-12-06), the latest release, fails to compile against the reference
  profile's libva 2.24.1, and #37, which fixes that, is merged but unreleased.
  Every option needs bindgen, libclang, pkg-config and the libva headers at
  build time.
  **Decision (2026-09-28):** As recommended, the first option: cros-libva at git
  revision 59384456ac2ae78c0c3e5515f41ef1efd9b802cf (#37, merged 2026-09-01),
  refreshed to a crates.io release once one carries the fix (D69). Its build
  tools are proposed rows in `docs/roadmap/toolchain-admission.md` and are
  admitted by M27.
  **Codec (2026-09-28):** As recommended, M27 decodes baseline Motion-JPEG
  through VAProfileJPEGBaseline plus one MPEG-2 intra frame, the fixture
  generated once with ffmpeg and pinned by sha256. cros-libva has no bitstream
  parser, and cros-codecs 0.0.6, which has one, requires exactly cros-libva
  0.0.12 and so cannot use the git pin; H.264, HEVC and AV1 follow once a parser
  is admitted. A consequence recorded beside the answer, not put to the
  maintainer: cros-libva at that revision heap-allocates for every decoded
  picture, so M27 does not claim HISS-03 on the frame path, and whether that
  allocation is accepted or removed is not decided.
  **libva on the Verification gate (2026-09-29):** The maintainer decided that
  the runner meets the admitted libva floor by building intel/libva 2.24.1
  from its release tarball, pinned by the sha256 the GitHub release publishes,
  with meson and ninja, cached between runs, in `.github/workflows/ci.yml`
  before `make verify-all`, with `PKG_CONFIG_PATH` pointing the crate build at
  it. The runner's apt libva is 2.20.0 on ubuntu-24.04 and 2.23.0 on
  ubuntu-26.04, both below the floor; the runner image is not changed and the
  floor is not lowered. `docs/roadmap/toolchain-admission.md` records the
  tarball URL, its sha256 and the build flags.
- **D81** Which accessibility standard do the new canvas, heads-up theme and
  display-runtime requirements target? Options: EN 301 549 V4.1.1 (2026-09),
  which reflects WCAG 2.2, with WCAG 2.2 AA; EN 301 549 V3.2.1 (2021-03), which
  reflects WCAG 2.1, with WCAG 2.1 AA. Why: D74 makes an unconventional canvas
  the primary surface, where the WCAG 2.2 additions on focus visibility,
  dragging and target size apply directly. Context recorded beside the question,
  not put to the maintainer: REQ-P12-08 pairs V3.2.1 with WCAG 2.2 AA, which no
  single edition does; D81 does not re-rule that row.
  **Decision (2026-09-28):** As recommended, V4.1.1 and WCAG 2.2 AA are the
  target of the new rows. Every standards-derived requirement also names the
  V3.2.1 (WCAG 2.1) clause, which stays the harmonised legal baseline until
  V4.1.1 is cited in the Official Journal, and says 'none in V3.2.1' for the
  WCAG 2.2 additions V4.1.1 carries into clause 11 (11.2.4.11, 11.2.5.7,
  11.2.5.8, 11.3.3.7 and 11.3.3.8; 11.3.2.6 is Void). Where V3.2.1 splits a
  software clause into open and closed functionality, the row names the
  open-functionality sub-clause (for example 11.2.1.1.1).
- **D82** Which engine renders P05's canvas behind P17's surface contract?
  Options: Servo (servo 0.6.0, MPL-2.0: paints through OpenGL with webrender
  0.70 and surfman, starts its own tokio runtime, fetches a prebuilt
  SpiderMonkey at build time unless the archive is pinned, and records Linux
  assistive-technology detection as unreliable in servo/servo#46834); WPE WebKit
  through its WPEPlatform API (C and GObject); a native Rust toolkit that paints
  the canvas itself and exposes AccessKit. Recommended: decide once P17's
  surface contract for the engine is fixed, which no milestone plans yet (M27
  fixes only the decoded-frame descriptor and the layer-surface attach path), on
  the engine's own accessibility exposure (REQ-P17-06) and on the no-unsafe
  rule, not on Chromium results. Why: D74 makes the canvas
  the primary surface, so the engine's exposure decides whether the shell can be
  used with a screen reader at all, and the D75 revision removed the engine from
  the first slice so that this can be decided on evidence.
  **Decision (2026-09-29):** The third option: a native Rust toolkit that paints
  the canvas itself and exposes AccessKit, so no web engine, Servo or WPE
  WebKit, is admitted for P05. D101 widens the choice from the canvas to the
  whole shell and D102 names the toolkit, gpui, which M28 admits (ADR-0004,
  public:docs/adr/0004-native-p05-shell-on-gpui.md `246799c4d9d1`).
- **D83** Does HISS-03 (no dynamic heap allocation in hot loops) bind the
  third-party decoder that P17's frame path calls? Options: a scoped deviation,
  where HISS-03 binds Aegis-authored code and the decoder's own allocation is
  recorded; the allocation must be removed before M27 can pass; decide at M27
  activation once allocations can be counted. Why: cros-libva at the revision
  D80 pins heap-allocates for every decoded picture (`Picture::new` boxes its
  state, and each JPEG and MPEG-2 parameter buffer is a `Box`), and counting
  allocations needs an unsafe `GlobalAlloc` the crate may not contain.
  **Decision (2026-09-28):** As recommended, a scoped deviation. HISS-03 binds
  the code Aegis writes: P17's own frame loop allocates nothing per frame.
  cros-libva's per-picture allocation is a named deviation limited to the
  decoder binding D80 pins, and it is reviewed whenever that binding is replaced
  or re-pinned. The deviation grants nothing to Aegis-authored code and waives
  no other invariant.
  **Applied at M27 (2026-09-29):** P17's frame loop in `crates/aegis-scaena`
  (the receive, the attach checks, the format admission, the import, the
  commit and the waits) builds only `Copy` values, lands lines in caller-owned
  buffers and control messages in stack buffers, and
  `tests/allocation_bounds.rs` fails to compile if a heap-owning field enters
  one of them; no per-frame count is produced, because a counting allocator
  needs an unsafe `GlobalAlloc`. The run found a second third-party allocation
  on the frame path that D83 does not name: wayland-client, through
  smithay-client-toolkit (D75), allocates for every protocol object it
  creates, and each frame creates a `zwp_linux_buffer_params_v1`, the
  `wl_buffer` its `created` event announces and a `wp_presentation_feedback`,
  which M27's per-frame `created` and `presented` events require. It is
  recorded with M27's evidence as outside D83's scope, not waived; whether
  D83 names that binding too is the maintainer's to decide.

### Decisions from the M24 delivery (2026-09-28)

The maintainer decided D84 on 2026-09-28, before M24's implementation, from the
research that preceded it, and D85 and the recorded timeout the same day, after
the delivery's review and its real runs. D84 and D85 change milestones'
acceptance; the implementation choices recorded with D84, the timeout among
them, change none, and are written down so they are not reopened.

- **D84** Which of E24-2's acceptance halves can M24 prove on a pinned upstream
  image? Options: keep the real swtpm PCR read-back and the ban on the imported
  simulated script in M24, and move the dm-verity acceptance to M11 and the /var
  unseal without PCR 11 to M20, where a real image exists; build a scratch
  verity and LUKS2 fixture from the upstream image here; or keep M24 open until
  an Imago return carries a verity root and a sealed /var. Why: E24-2 asked for
  a matching verity root hash, a modified root that fails verity, and a PCR
  policy without PCR 11 that fails to unseal /var. A pinned upstream cloud image
  has no verity root and no TPM-sealed /var, M24's second criterion forbids
  constructing an image here, and D63 already says the unseal half needs the
  real image. **Decision (2026-09-28):** the first option. E24-2 keeps PCR 0, 4,
  7 and 11 read back from inside a guest booted on swtpm, tied to that boot by a
  nonce, and the rule that the imported integration script and any simulated
  output never count (REQ-BOOT-02). The verity acceptance is now an exit
  criterion and epic E11-5 of M11 (REQ-P02-01), and the unseal acceptance an
  exit criterion and epic E20-3 of M20 (REQ-P02-02); each carries a disclosure
  entry in its evidence. M24 does not take D63's pre-proof of the tpm2-tools
  quote path: the harness reads PCRs through sysfs, and the quote stays M20's.

  **Recorded with D84 (2026-09-28).** The artifact is Fedora-Cloud-Base-UEFI-UKI
  44-1.7, because it boots a systemd-stub UKI and so exercises PCR 11 and the
  UKI signing criterion; Ubuntu 26.04 boots shim and GRUB and would leave both
  untested. The pin is the compose URL, the sha256 and the signing-key
  fingerprint in `build/boot/artifact.pin.json`; the bytes are cached outside
  the repository and hashed on the exact file booted, immediately before the
  boot, and every boot writes to a throwaway overlay. Guest Secure Boot is a
  per-run variable store generated with virt-fw-vars from the shipped
  `OVMF_VARS.4m.fd`, with a custom PK and KEK and the Microsoft UEFI CA in db so
  the distribution-signed image boots unmodified on `OVMF_CODE.secboot.4m.fd`,
  plus a separate case that signs the UKI with sbsign and the custom key; the
  host firmware is never written, and its state is read from efivarfs. "Within
  the recorded timeout" is inclusive: a prompt at exactly the timeout passes.
  The one-second boundary is tested as a pure rule on synthetic times, and live
  boots only measure the margin the recorded timeout keeps, which is the lesson
  of D73: a live edge under host load is not a gate.

  **Recorded timeout (2026-09-28, after the delivery's runs):** As recommended,
  the recorded login-prompt timeout is raised from 120 s to 180 s, still
  inclusive. Why: a quiet host reaches the prompt in about 15.5 s, but a
  `make verify-boot` run while other work held the load average at 89 saw it
  after 65.7 s, the slowest boot measured, which left 54.3 s under 120 s; at
  180 s the same boot keeps 114.3 s. The pin, the gate's tests, whose boundary
  is now 179, 180 and 181 s, and every page that stated 120 s now state 180 s.
  The negative case keeps its deliberately short 5 s, which is below the
  firmware and kernel start-up alone.

  M15's last criterion still names M24 as the consumer of the A/B transition
  trace, and stays as M15 recorded it. Under D72 the transfer is E11-4's, so the
  consumer is M11 running on this harness; a pinned upstream image has no A/B
  slots, and nothing in M24 compares the trace. `docs/build/ab-lifecycle.md`
  carries a dated note saying so.
- **D85** How does M24 close, when its criterion 2 and E24-1 also ask the
  harness to verify an Imago return before boot and the signature form of the
  Imago result is not pinned yet? Alternatives on record from M24's review:
  close M24 on the upstream-image kind it proves and move the Imago-return
  verification to M11; keep M24 ready until M09 pins the signature form and
  this harness gains that scheme with its own cases; or add M09 to M24's
  blockers. Why: every other part of M24 is met by real runs. The harness's pin
  schema has one signature scheme, `gpg-clearsigned-checksum`, and refuses any
  other; imago's proposed result schema `imago.p01.product-result.v1` (its
  ADR-0020 at 16f964b) carries an image digest and a `signature-ref` string
  whose form M09 is to pin, M09 waits on producer builds, and nothing produces
  a result yet. **Decision (2026-09-28):** the first option, as recommended.
  M24 is done for the upstream-image kind it proves. Verifying an Imago
  return's signature before boot moves to M11, which adds that pin scheme once
  M09 pins the signature form of the Imago result (the `signature-ref` of
  `imago.p01.product-result.v1`). M24's criterion 2 and E24-1 are restated for
  the upstream kind, and M11 gains an exit criterion and the moved acceptance
  in E11-1, whose requirements are E24-1's; each milestone carries a disclosure
  entry in its evidence. A consequence recorded beside the answer, not put to
  the maintainer: adding a pin scheme is a change to the harness, which M11's
  sixth criterion and E11-2 forbade, so both now except that one scheme and
  still fail M11 on any other change, and M11 discloses the relaxation. M11
  stays blocked; M09 is its one unfinished blocker.

### Decisions from the M04 delivery (2026-09-28)

The maintainer decided D86 to D90 on 2026-09-28, before M04's implementation,
from the research that preceded it, and settled two open drift-register
decisions the same day: D42 is superseded by D65, whose 26.x line enters Active
LTS on 2026-10-28, and D43 is decided for pnpm at its latest stable 12.x, pinned
through `packageManager` (both recorded in place in
`docs/roadmap/inventory.md`). D89 and D90 change a milestone's acceptance and
M04 discloses both in its evidence; D86, D87 and D88 record how M04 is
delivered.

- **D86** Does M04 advance P12 in `planning/components.json`, or only record
  milestone evidence? Options: evidence only, P12 staying a proposal as the
  components of M17, M05, M06 and M08 did; activate P12 in M04 and extend
  `tools/verify_preparation.py` to read a `package.json` manifest; defer
  activation to a later milestone once that extension exists. Why: no M04
  criterion asks for a status change, and the validator binds activation to a
  Cargo manifest, which a `package.json` is not. **Decision (2026-09-28):**
  evidence only. P12 stays a proposal and the validator is unchanged; P12's
  blockers are restated with what M04 delivered.
- **D87** Which `@sveltesentio/*` packages does M04 declare? Options: none, with
  plain CSS custom properties and one Svelte 5 component, M16 adopting packages
  when the shell needs them; `@sveltesentio/core` and `@sveltesentio/testing`;
  `@sveltesentio/ui` for its tokens. Why: D10 admits no package a component has
  not declared. The published `@sveltesentio/testing` 0.1.0 declares
  `@sveltesentio/core` 0.1.0 as an exact peer and exports TypeScript sources a
  Playwright spec cannot import from `node_modules`, `@sveltesentio/ui` 0.5.0
  brings Tailwind, in tension with D17, and no package carries the D16 width or
  the D76 stroke and motion tokens. **Decision (2026-09-28):** none in M04.
  **For M16 (2026-09-29):** none. The question D87 left to M16, which
  `@sveltesentio/*` packages the shell declares, is answered with no package,
  and D101 makes it moot the same day: the shell is native Rust and declares no
  npm package.
- **D88** Where does the accessibility gate run? Options: inside `make
  verify-all` with a guard that prints a skip reason when no container engine is
  found or the digest-pinned image or the offline dependency store is not
  present locally, the gate itself never pulling and running the container with
  networking disabled, a separate fetch target pulling the image by digest and
  filling an offline store, and CI running that fetch before `make verify-all`;
  a separate target with its own CI job, outside `make verify-all`; inside `make
  verify-all`, pulling on demand. Why: the CI runner has a container engine, so
  unlike the kernel and boot gates this one can run there, and keeping the pull
  out of the gate is what lets it honour REQ-P12-04's disabled network.
  **Decision (2026-09-28):** the first option. The fetch target is `make
  a11y-fetch`, which `.github/workflows/ci.yml` runs before `make verify-all`.
- **D89** How does M04 treat REQ-P12-01 (UKI compilation gated on an
  accessibility pass), REQ-P12-02 (the portal's settings over D-Bus) and
  REQ-P12-03 (AT-SPI2), which E04-2 lists while M04's cheapest exit has no
  compositor and no daemons? Options: cover their CSS level with Playwright's
  reduced-motion and forced-colours emulation, labelled as browser emulation in
  the output and the evidence, and record the portal, AT-SPI2 and UKI halves as
  not met by M04; build a portal stub and an AT-SPI2 probe in M04; drop the
  three from E04-2. Why: emulation is not the desktop, and counting it as portal
  or AT-SPI2 evidence would be simulated output. **Decision (2026-09-28):** the
  first option. The portal, AT-SPI2 and UKI halves stay with the milestones that
  own the shell and the image, M16 and M27, and no emulation counts as daemon
  evidence.

  **Recorded with D89 (2026-09-28, at the delivery's review).** No milestone
  carries the three halves yet. M16's epics list none of REQ-P12-01 to
  REQ-P12-03, and its D77 criterion keeps the portal settings and AT-SPI2 on
  D-Bus mocks; M27 is the P17 VA-API slice, whose epics list REQ-P17 and REQ-WS
  rows only; and M11 says only that the accessibility gate attaches when the UI
  enters an image. REQ-P12-06's scan of the Forum Shell is in the same position,
  since M04 scans the P12 component only. Until a recorded decision adds them to
  a milestone's criteria and epics, they are open and owned by no milestone;
  P12's activation blockers in `planning/components.json` and P12's row in
  `docs/roadmap/inventory.md` list them. M04 also runs a third labelled test
  that reads Chromium's accessibility tree; D89 does not name it, it is not
  AT-SPI2 evidence, and it counts towards nothing. D91 is the recorded decision
  this note waits for: it assigns REQ-P12-06 and REQ-P12-01, and keeps the
  portal and AT-SPI2 halves open.
- **D90** M04's fifth criterion named chromium-1228 as the revision cached on
  the reference profile. Options: restate it as the browser revision bundled by
  the pinned `@playwright/test` and container image digest; fix it in a separate
  change; leave the text and note the drift in the evidence. Why: the host cache
  is written by Praetor's figure engine, whose own pin moved it to chromium-1243
  on 2026-09-27, so a criterion tied to it moves with another repository.
  **Decision (2026-09-28):** the first option, in this delivery. The criterion
  in `planning/roadmap.json`, its mirror below and D65's text now name the
  browser revision bundled by the pinned `@playwright/test` and container image
  digest; 1243 is recorded as the host-cache revision observed on 2026-09-28,
  the host cache is never the evidence, and M04's evidence carries a disclosure
  entry in the shape M18 recorded.

### Decisions after the M04 delivery (2026-09-28)

- **D91** Which milestones own what D89 left open: the UKI gating of REQ-P12-01,
  the portal half of REQ-P12-02, the AT-SPI2 half of REQ-P12-03 and
  REQ-P12-06's scan of the Forum Shell? Options: split by the milestone that
  builds each thing, REQ-P12-06 to M16, REQ-P12-01 to M11, and the portal and
  AT-SPI2 halves kept open until a live-session P05 milestone is planned; leave
  all four open; move all four to M16; split as the first option but with
  REQ-P12-01 in M13. Why: M16 creates `ui/forum-shell` on the toolchain M04
  admitted, so scanning it is the same gate over one more package; M11 is where
  Aegis first requests an image/UKI from Imago, so it is the first place a build
  can be refused; the portal and AT-SPI2 halves need real D-Bus daemons in a
  running session, which M16's D77 criterion keeps on mocks and no planned
  milestone provides. **Decision (2026-09-28):** the first option. M16 gains a
  criterion and epic E16-4 for REQ-P12-06. M11 gains a criterion and epic E11-6
  for REQ-P12-01: the integration adapter requests an image/UKI build from
  Imago, and accepts its return, only for a commit on which the M04 gate
  reported PASS, and a SKIP stops it as a FAIL does. M11 is now blocked by
  M04, which is done, so M11's state and the computed order are unchanged. This
  amends D20 in one clause only: D20 kept the accessibility gate off M11
  because it would have put the Node/Playwright admission on the first-artifact
  path, and M04 has since made that admission. The minimal image still carries
  no UI and no kernel-attached eBPF programs. The portal half of REQ-P12-02 and
  the AT-SPI2 half of REQ-P12-03 stay open and owned by no milestone, listed
  with P12's activation blockers, until a live-session P05 milestone is
  planned.

  **Recorded 2026-09-29 (ADR-0004).** The live-session P05 milestone this
  decision waited for is M29, which owns the portal half of REQ-P12-02, the
  AT-SPI2 half of REQ-P12-03 and REQ-P05-05's real StatusNotifierWatcher. D101
  removes the premise that M16 creates `ui/forum-shell` on the M04 toolchain:
  E16-4 is now an AccessKit tree check over the shell's exported tree, not the
  M04 gate over a package, and E11-6 gates the build request on the M04 PASS
  and, once M16 delivers it, that tree check.

### Decisions from the M09 delivery (2026-09-28)

The maintainer decided D92 and D93 on 2026-09-28, before M09's implementation,
from the research that preceded it. D92 changes the acceptance of M09, M11 and
M10, and each of the three discloses it in its evidence; D93 records where M09's
gate runs.

- **D92** What does M09 close on, when imago decodes both Aegis payloads but
  nothing produces imago's result type and nucleus reads no Aegis payload?
  Options, from M09's research: close M09 on the consumption contract that is
  provable against the pinned producers and move the result legs to the
  milestones that already wait for them, the Imago product result to M11 and the
  Nucleus kernel result to M10, the way D84 and D85 moved M24's halves to M11;
  keep M09 ready until imago emits `imago.p01.product-result.v1` and nucleus
  publishes a kernel manifest; drop the result legs from the roadmap. Why: at
  imago 16f964b both validators accept the real payloads, refuse tampered ones
  with the payload's correlation id and refuse an empty feature list with
  ErrEmptyRequirement, against fixtures byte-identical to this repository's; but
  imago's ADR-0020 records that nothing produces a result, nucleus's forge
  compiles no kernel (nucleus #18) and its verify-requirements checks a
  hardcoded list (nucleus #20), and nucleus's own AGENTS.md names imago, not
  nucleus, as the payload's reader. M11 already waits for Imago's image/UKI and
  for the signature form D85 moved there, and M10 for a Nucleus kernel.
  **Decision (2026-09-28):** the first option. M09 closes on consumption: imago
  at 16f964b4dafadac2b1f0a662c7dcbb4b7bb29bee consumes
  `build/product-input.json` through `imago aegis validate` and
  `build/kernel-requirement.json` through `imago kernel requirement validate`,
  each with a positive acceptance, a negative rejection carrying the payload's
  correlation-id and, for the kernel requirement, a boundary rejection of an
  empty feature list. E09-1's positive half, the Imago product result
  (image-digest, signature-ref and boot-evidence-ref of
  `imago.p01.product-result.v1`), moves to M11 as a criterion and epic E11-7.
  E09-2's positive half, the Nucleus kernel result, moves to M10, whose first
  criterion now waits on a Nucleus-published `imago.nucleus.kernel-artifact.v1`
  manifest for a built kernel, and to epic E10-4. E09-2 is re-scoped to its real
  consumer, imago's `pkg/kernel`; nucleus is recorded by identity, with `git
  ls-remote`, and by its documented outbound role only. E09-1's boundary, 'a
  retry at the bound is recorded, and one above is refused', is replaced by a
  bound imago enforces in `pkg/aegis`: exactly at the bound accepted, one above
  refused. Every rewritten criterion and acceptance is disclosed in the evidence
  of the milestone it belongs to, in the shape M18 recorded.
  cordanaLLM/imago#46, filed 2026-09-28, tracks the missing product-result
  emitter. M11 and M10 change state by the register rule, ready once every
  blocker is done, and each says in its text that its external BLOCKED-until
  criterion still holds, so 'ready' is not read as unblocked.

  **Recorded with D92 (2026-09-28, at the delivery).** Two bounds are run:
  `retries.max-attempts` at 10 and 11, the bound imago's ADR-0020 records as an
  acceptance criterion of its own step-aegis-schema-pin ('a retry count at the
  bound is accepted while one above it is refused'), and the packages count at
  256 and 257. imago's bounds are wider than the Aegis crate's -- 256 packages
  and 10 attempts against 64 and 5, and a backoff from 0 s rather than 1 s -- so
  every payload Aegis can emit is inside them, and the gate sends one at the
  Aegis maxima. D53, the image determinism seed in `docs/roadmap/inventory.md`,
  asked to be decided before M09; the pinned product input carries no seed and
  no producer was asked to reproduce an image, and the maintainer decided it at
  this delivery: the seed derives from the released revision, recorded in place
  in `docs/roadmap/inventory.md` and carried by an M11 criterion.

  Two pointers written before D92 are read, not rewritten. M24 is done, and its
  criterion 2 and E24-1, which say M11 'adds that pin scheme once M09 pins the
  signature form of the Imago result', keep their wording and now read as M11
  pinning that form itself, with the result (E11-7); the D85 record above keeps
  its wording for the same reason, and M11's and M10's reference-profile
  rationales gain dated D92 sentences. E09-2's negative, 'an unsatisfiable
  feature is rejected with a correlated error', is re-scoped to a feature imago
  cannot accept; rejecting a feature a built kernel does not satisfy, with the
  requirement's correlation-id, was left to no milestone by D92 and D94 assigns
  it to M10. The executed retry stays with no milestone, and M09 discloses it as
  a narrowing.

  **Corrected (2026-09-29, by D106).** D92's premise that nucleus reads no Aegis
  payload held at `8672247` and stopped holding at nucleus
  `0a4eac93f29fef432bfa9d892ad236568ce2f482` (pull request 35), whose
  `scripts/verify_kernel_requirement.py` decodes `build/kernel-requirement.json`
  and holds it against the kconfig fragments of the `realtime` stream. The
  closure stands: M09 closed on imago's consumption, and this adds a second
  consumer rather than removing one. What changes is the gate, which runs
  nucleus's verifier as well, and M09's record, which appends dated evidence and
  rewrites no criterion; the statements that nucleus consumes, or reads, no
  Aegis payload in its first criterion and in E09-2 are read as of `8672247`.
  The Nucleus kernel result stays M10's (E10-4), and so does rejecting a feature
  a built kernel does not satisfy (E10-5, D94): nucleus's verdict is at its
  `declared` level, the fragments as merged, and no kernel was built.
- **D93** Where does the contract gate run? Options: the D88 pattern -- a fetch
  target, `make contract-fetch`, as the only networked step, which runs `git
  ls-remote` against both producers and keeps the output, clones imago at the
  pinned commit into a cache outside the repository and builds it with
  `GOTOOLCHAIN=local CGO_ENABLED=0 go build -trimpath -buildvcs=true` so that
  `go version -m` carries the revision, and an offline gate inside `make
  verify-all` that skips with a printed reason when go, git or the cached
  checkout or binary is absent and fails when the cache is present but wrong,
  with CI running the fetch before `make verify-all`; a separate target outside
  `make verify-all`, as the kernel and boot gates are; a gate that clones and
  builds on demand. Why: the gate needs only go and git, which the CI runner
  has, so like the accessibility gate and unlike the kernel and boot gates it
  can run where CI runs; keeping the network out of the gate lets it run
  offline, and building the binary in the fetch is what gives the
  simulated-output check a provenance to compare against. **Decision
  (2026-09-28):** the first option. `make contract-fetch` and `make
  verify-contract`, the gate wired into `make verify-all`, and
  `.github/workflows/ci.yml` running the fetch before the gate on the Go that
  `actions/setup-go` installs from Praetor's `go.mod`. That Go satisfies imago's
  `go 1.27.1` under `GOTOOLCHAIN=local`: Praetor declares `go 1.27`, which
  setup-go resolves to the newest 1.27 patch, go1.27.1 on 2026-09-28, so the
  workflow's Go setup is unchanged.
- **D94** Who rejects a kernel-requirement feature that a built kernel does not
  satisfy, which E09-2's negative named and D92 left to no milestone? Options:
  M10, which boots the Nucleus kernel and reads its config inside the VM, gains
  a criterion and epic E10-5; leave it unowned as a disclosed narrowing. Why:
  imago checks a feature's shape and vocabulary but has no kernel to check it
  against, and M10 is the first milestone holding a Nucleus kernel's config, so
  it is the first place the check can run. **Decision (2026-09-28):** the first
  option. M10 checks every feature of `build/kernel-requirement.json` against
  the Nucleus kernel's config before any M19 object loads, and rejects an
  unsatisfied feature with the requirement's correlation-id and the symbol
  named.
- **D95** Criterion 3 of M09 makes a non-canonical identity in a producer's
  builder workflows a blocker. At nucleus 8672247 no workflow names one, but
  other files do: `scripts/setup-remote-repository.sh` creates `lusoris/nucleus`,
  `mkdocs.yml` and `SUPPORT.md` name `lusoris.github.io/nucleus`,
  `docker/Dockerfile.builder` labels its source `lusoris/lusoris-kernel-forge`,
  and ADR-0004 and `docs/packaging.md` push to `ghcr.io/lusoris/kernels`. How
  are they treated? Options: non-blocking, recorded, with one nucleus issue
  listing them; non-blocking, recorded only; a blocker, reading the builder
  container as a builder identity. Why: no workflow builds or runs that
  container, pushes to that registry or runs the script, so none is on a path
  that produces an artifact today; they become blocking once a workflow uses
  them. `@lusoris` in CODEOWNERS and MAINTAINERS is the maintainer's account and
  the `-lusoris1` localversion is a kernel name, so neither is a repository
  identity. **Decision (2026-09-28):** the first option;
  cordanaLLM/nucleus#28 lists each file and line.

### Decisions of 2026-09-29

The maintainer decided D96 to D102 on 2026-09-29, together with D32 and D82,
which were open until then. D96 to D100 were decided first, for a JavaScript
shell, before M16's implementation; D101 then made the whole P05 shell native
the same day, which withdrew D97, D98 and D99 before any delivery and kept D96
and D100. D82, D101 and D102 are recorded in ADR-0004
(public:docs/adr/0004-native-p05-shell-on-gpui.md `246799c4d9d1`); D82's answer
is appended to its entry above, D32's to its entry in
`docs/roadmap/inventory.md`, and D87's answer for M16 to its entry above. M16
and M11 change under them and disclose it in their evidence, and M28 and M29 are
new.

- **D96** Which transitions does the P05 typed lifecycle admit, and at what
  limit does a rate-limited process become quarantined? REQ-P05-03 names five
  states, Eligible but Inactive, Activated, Rate-Limited, Quarantined and
  Deleted, and export-014 `659691a2d7d0` (section 6.3) refers to a figure the
  export does not contain, so no source records an edge or a limit. Options: the
  forward chain in the source's order with two recovery edges and a named limit;
  the forward chain alone; any state to any state behind a guard. Why: E16-1's
  acceptance asks for each transition in the source order to succeed, a
  transition out of Deleted to be refused and the Rate-Limited to Quarantined
  limit to be exercised exactly, which needs edges and a number the sources do
  not give. **Decision (2026-09-29):** the first option, kept under D101. The
  forward chain Eligible but Inactive to Activated to Rate-Limited to
  Quarantined to Deleted, plus Rate-Limited to Activated when the process is
  back within its budget and Quarantined to Eligible but Inactive on release.
  Every live state may go to Deleted, and Deleted is terminal. A process is
  quarantined after `QUARANTINE_LIMIT` = 3 consecutive rate-limited windows, a
  named constant; E16-1's boundary runs the transition at exactly 3 and its
  absence at 2. No other edge is admitted, an idle return from Activated
  included.
- **D97** What runs the shell's unit tests? Options: `node:test`, built into the
  admitted Node 26.10.0; Vitest; Playwright alone. Why: no JavaScript unit
  runner was admitted, and a new runner is a new admission row, Renovate rule
  and lockfile entry. **Decision (2026-09-29):** `node:test` on the pinned Node.
  **Withdrawn (2026-09-29, by D101)** before any delivery: the shell has no
  JavaScript, and its unit tests are Rust tests that `cargo test` runs.
- **D98** How is the DecisionRequest consumer typed against the M14 schema when
  no machine-readable schema exists? Options: golden fixtures that the Rust
  contract serializes and the JavaScript consumer validates; a JSON Schema
  generated with schemars and checked with ajv; TypeScript types. **Decision
  (2026-09-29):** Rust golden fixtures. **Withdrawn (2026-09-29, by D101)**
  before any delivery: the native shell decodes the producers' Rust types
  directly, so no consumer in a second language needs a fixture. The
  CarbonTelemetry contract those fixtures would have needed is still M16's, in
  `crates/aegis-tellus`.
- **D99** Does `ui/forum-shell` get its own lockfile beside M04's, or do the UI
  packages share one? Options: one pnpm workspace under `ui/` with one lockfile
  and one offline store; a lockfile per package with the gate keyed per package;
  the shell inside `ui/concordia-tokens`. **Decision (2026-09-29):** one
  workspace. **Withdrawn (2026-09-29, by D101)** before any delivery: there is
  no `ui/forum-shell`, so `ui/concordia-tokens` keeps M04's lockfile, pnpm
  settings and toolchain pin where M04 committed them.
- **D100** How is HISS enforced on the JavaScript and Svelte under `ui/`, which
  `praetorctl audit` does not scan? cordanaLLM/praetor#589, filed 2026-09-29,
  records that praetorctl's HISS scanners cover Go, Python and Rust only: a
  planted recursion, an `eval` and a 73-line function in JavaScript all pass the
  audit. Options: ESLint with the HISS limits, run offline in the pinned
  accessibility container until praetor ships a scanner; wait for praetor;
  hand-written checks in `tools/`. Why: HISS-01, HISS-04 and HISS-08 bind all
  code. **Decision (2026-09-29):** ESLint, kept under D101 for the JavaScript
  and Svelte that remain, `ui/concordia-tokens` today: eslint,
  eslint-plugin-svelte and svelte-eslint-parser admitted with exact pins and
  grouped in Renovate as M04's tools are, a flat configuration with
  `max-lines-per-function` 60, `complexity` 10, `max-statements` 50, `no-eval`,
  `no-implied-eval`, `no-new-func` and a local rule that bans direct
  self-recursion, zero warnings, run in `make verify-a11y` offline in the pinned
  container with one planted violation per rule family. Mutual recursion is not
  detected: there is no call-graph check. The implementation lands with M16's
  delivery, whose criteria carry it, and stands until praetor#589 ships a
  scanner.
- **D101** Does D82's native choice cover the canvas alone or the whole P05
  shell? The register still modelled the chrome as Svelte in a Wry webview:
  REQ-P05-02, REQ-P05-08, M16's `ui/forum-shell` package and D91's axe-core scan
  of it. The whole shell was the recommended answer. Why: a DOM scanner cannot
  reach a native surface, so a native canvas inside a Svelte and Wry chrome
  would leave two engines and two accessibility trees in one shell, and
  REQ-P17-01 records that WebKitGTK and Wry wrap the browser in GTK container
  windows that meet realization timing locks with `zwlr_layer_shell_v1` on
  Wayland. **Decision (2026-09-29):** the whole shell (ADR-0004). The canvas and
  the chrome are one Rust program on one toolkit, whose AccessKit tree reaches
  AT-SPI through accesskit_unix; Svelte and the Wry webview leave the P05 shell,
  and no `ui/forum-shell` package is created. The Concordia tokens stay the
  single token source and also emit Rust theme constants (M28). The shell's
  accessibility check moves from axe-core and Playwright to an AccessKit tree
  check, M16 on the exported `accesskit::TreeUpdate` and M28 over AT-SPI.
  REQ-P05-02, REQ-P05-08, REQ-P12-01 and REQ-P12-06 keep their source text and
  gain re-mapping notes. M04 is done and its evidence stands: its gate covers
  the P12 token component, CSS and one Svelte 5 component, and never covered the
  shell. As planning that follows the answer, from the maintainer's answers of
  the same day: M16 becomes the Rust crate `crates/aegis-forum-shell`, named
  after the component because the activation binding in
  `tools/verify_preparation.py` requires that of a component's manifest, with
  D96's lifecycle, D77's framing, consumers typed on the producers' Rust types
  and the canvas model with its tree export; the E16-3 cases that need rendered
  pixels move to M28; M28 (the native shell on gpui) and M29 (a live session in
  a VM) are added; and E11-6 gates on the M04 PASS and, once M16 delivers it,
  the shell's tree check. D101 decides P05 only: P15 Hestia's WebKitGTK (wry)
  micro-frontend host (REQ-P15-01) and the P14 to P15 Svelte micro-frontend
  (REQ-P14-06) are not re-ruled.
- **D102** Which native toolkit paints the shell? Options: gpui
  (zed-industries/zed, Apache-2.0), git-pinned; egui with an Aegis-written
  layer-shell host; Xilem and Masonry; a spike of gpui against egui. Why, read
  on 2026-09-29: gpui is the only candidate that ships a public
  `zwlr_layer_shell_v1` API of its own
  (`crates/gpui/src/platform/layer_shell.rs`) and wires accesskit_unix directly
  rather than through winit, it has a `canvas` element and a
  `TransformationMatrix` for a camera, and Zed uses it in production. Its risks:
  its crates.io release, 0.2.2 of 2025-10-22, is eleven months old, so the pin
  is a git revision; at zed `main` it resolves accesskit 0.24.0 and
  accesskit_unix 0.22 against the current 0.25.1 and 0.24.0; it links
  libxkbcommon, a C library, for Linux keymaps; and no published benchmark ties
  1,000 nodes to a frame time. Disqualified on evidence: iced, Floem and Makepad
  have no AccessKit or AT-SPI path, Freya depends on Skia (C++), and Slint is
  licensed GPL-3.0-only or under proprietary licences, enables AccessKit only
  behind a feature, and no pan-and-zoom canvas container was found in it.
  **Decision (2026-09-29):** gpui, git-pinned (ADR-0004). The commit is chosen
  and admitted at M28, not here, and refreshed to a crates.io release once one
  carries what the shell needs (D69), the pattern D80 set for cros-libva; until
  then no gpui or accesskit_unix dependency is admitted, and
  `docs/roadmap/toolchain-admission.md` lists both as proposed.

### Decisions of 2026-09-29 (producer contracts)

The maintainer decided D103 to D106 on 2026-09-29, answering the questions a
session working in cordanaLLM/nucleus raised while nucleus pull request 35 made
nucleus read `build/kernel-requirement.json`: its ADR-0007, merged as Proposed
at nucleus `0a4eac93f29fef432bfa9d892ad236568ce2f482`, diverges from the schema
owner in two places and asks this repository to settle both. D103 to D105 change
the M18 schema crate and D106 the M09 gate; M18 and M09 are done, and each
appends dated evidence and rewrites no criterion. Recorded with them is a defect
the same review found and a probe confirmed, the array form, which is not a
decision and is fixed rather than ruled on.

- **D103** What may `required-by` hold? Options: any identifier of the form
  nucleus's ADR-0007 states -- an upper-case letter, then upper-case letters,
  digits and hyphens, at most 128 bytes, the bare `REQ-` refused -- with the
  `REQ-` form enforced only on the payloads Aegis issues; keep the `REQ-` rule
  in the schema and ask imago to rename its identifiers; admit a per-producer
  list of prefixes. Why: imago's `kernel/requirement.json` at `16f964b` names
  its flavours (`FLAVOR-BASE`, `FLAVOR-K8S-NODE`, `FLAVOR-AI-INFER`,
  `FLAVOR-DOCKER`) under the schema id
  `aegis.p01-nucleus.kernel-requirement.v1`, and `RequirementId` refused every
  one (cordanaLLM/imago#48); nucleus's verifier accepts them for every source
  and names the divergence as a question to this repository. A per-producer list
  would put producer vocabulary in the schema. **Decision (2026-09-29):** the
  first option. `RequirementId` (`crates/aegis-fabrica-defs/src/field.rs`)
  admits `[A-Z][A-Z0-9-]*` of at most 128 bytes and refuses the bare `REQ-`,
  exactly nucleus's form, so the schema and nucleus admit the same identifiers;
  `tools/test_payload_schemas.py` compares the published pattern with nucleus's
  `(?!REQ-$)[A-Z][A-Z0-9-]*` over every value of up to six characters from a
  small alphabet. The `REQ-` form now binds the payloads this repository
  publishes, not the decoder: `RequirementId::is_aegis_requirement` and
  `crates/aegis-fabrica-defs/tests/issued_payloads.rs` hold every `required-by`
  in `build/kernel-requirement.json`, `build/kernel-requirement.reference.json`
  and `build/product-input.json` to `REQ-` and to a row in
  `docs/roadmap/requirements.md`. imago#48's flavour identifiers decode under
  the schema id they claim; imago's decoder still diverges from the owner in the
  other directions #48 lists, which is imago's to close.
- **D104** Is an architecture a requirement lists a promise or an option?
  Options: all-of and fail-closed, so a conforming kernel is built for every
  listed architecture and each build provides every feature; any-of, so a kernel
  for one listed architecture conforms. Why: `KernelRequirement::unmet` accepted
  a profile of any listed architecture (`unmet_identity`) and the field was
  documented as the architectures a kernel 'may be built for', while nucleus's
  verifier builds for, and requires, every listed one and asked which reading
  the owner meant. Both live documents list `x86-64` alone, so no payload
  changes either way. **Decision (2026-09-29):** all-of, fail-closed.
  `KernelRequirement::architectures` is documented as the architectures a kernel
  must be built for, all of them, and the check against a profile reports each
  listed architecture the profile is not for as `Unmet::ArchitectureUnverified`,
  so one profile proves a requirement only when it lists exactly that profile's
  architecture; a repeated architecture is one promise. `MAX_UNMET` grows by
  `MAX_ARCHITECTURES` to stay above every reachable count;
  `the_reachable_maximum_of_unmet_rows_is_reported_whole` builds that count,
  four rows plus two for each of `MAX_FEATURES` features, and every row comes
  back, where the old bound would drop one. The boundary -- exactly the
  profile's architecture met, a repeat of it met, one other architecture one
  unverified row -- is in
  `crates/aegis-fabrica-defs/tests/reference_profile.rs`.
- **D105** How does a consumer in another language validate the two contracts?
  Options: JSON Schemas generated from the Rust types with schemars, committed
  under `build/` with a test that fails when they drift; hand-written JSON
  Schemas; no schema, the crate as the only definition. Why: nucleus's ADR-0007
  records that Aegis publishes no JSON Schema and re-implements the decoder
  field by field from the crate's source, imago#48 shows imago's decoder
  diverging from the owner in both directions, and cordanaLLM/praetor#607
  records the fleet-level gap of checking a consumer's decoder against the
  owner's at all. A hand-written schema would be a third definition to drift.
  **Decision (2026-09-29):** the first option. schemars 1.2.2 is admitted
  (`docs/roadmap/toolchain-admission.md`: MIT, its newest release, the lock's
  checksum the one crates.io publishes, and five dependencies under MIT or MIT
  OR Apache-2.0). `build/kernel-requirement.schema.json` and
  `build/product-input.schema.json` are JSON Schema 2020-12: every struct is
  `type: object` with `additionalProperties: false` and its `required` fields,
  every bounded field carries its anchored `pattern` and `maxLength`, and the
  list and range bounds are the decoder's constants.
  `crates/aegis-fabrica-defs/tests/json_schema.rs` fails on drift and names the
  regeneration command, and `tools/test_payload_schemas.py` reads both as a
  consumer does. What a schema cannot state -- a repeated symbol, an inverted
  release -- stays the decoder's. `docs/build/product-input.md` names the paths
  for consumers.
- **D106** nucleus now reads the kernel requirement: how does M09 record it,
  when D92 closed M09 with nucleus recorded by identity only because it read no
  Aegis payload? Options: extend the M09 contract gate -- the fetch clones
  nucleus at a pinned commit and the offline gate runs its verifier and asserts
  on its JSON report -- with dated M09 evidence and a dated correction of D92;
  open a milestone for the nucleus edge; record the change and leave the gate
  alone. Why: nucleus pull request 35, merged as
  `0a4eac93f29fef432bfa9d892ad236568ce2f482`, made
  `scripts/verify_kernel_requirement.py` decode `build/kernel-requirement.json`
  and hold it against the kconfig fragments of the `realtime` stream its
  `versions.json` binds this repository to; the consumption contract is exactly
  what M09 pins, so a second consumer belongs in the same gate, and M09 is done,
  so its bar must not move. **Decision (2026-09-29):** the first option.
  `build/contract/producers.pin.json` moves nucleus to that commit, nucleus's
  `main` on 2026-09-29 read with `git ls-remote --heads`, and records the
  verifier, the label `aegis-os`, the bound stream `realtime`, the report schema
  `nucleus.kernel-requirement-report.v1` and the evidence level `declared`.
  `make contract-fetch` clones nucleus beside imago; the gate checks the
  checkout and its binding, runs the verifier with the gate's `python3` and
  `-I`, and asserts on its `--report-json`, never its text: PASS for `aegis-os`
  with `--sha256` and `--correlation-id` bound, a planted built-in symbol
  nucleus does not set refused FAIL with the correlation id in every reason, an
  empty feature list REJECTED `NoFeatures` and one feature PASS, and the digest
  or correlation id of another document REJECTED `DigestMismatch` and
  `CorrelationMismatch` (`docs/build/contract-pair.md`). The `declared` level is
  recorded as such: the fragments as nucleus merges them, not a built
  configuration, so M10's E10-4 and E10-5 are unchanged. M09 appends dated
  evidence and E09-2's text stays, its evidence growing by these runs; D92
  carries a dated correction.

**Recorded with them (2026-09-29): the array form.** serde's derived decoder
reads a struct from a JSON array of its values in declaration order as readily
as from an object, and `deny_unknown_fields` has no key to refuse in an array,
so `KernelRequirement::decode` returned `Ok` for the reviewed requirement
written as `[schema, correlation-id, architectures, abi, features]`, and the
product input manifest decoded the same way; nucleus's ADR-0007 named it as its
second divergence and refuses the form. It is refused now for every struct of
both contracts, at the top and nested, without recursion: each struct's
`Deserialize` asks the deserializer for a map and passes only the map's entries
to the decoder serde derives on a private field mirror
(`crates/aegis-fabrica-defs/src/payload.rs`), and the version peek reads objects
only, so an array led by another version is `Malformed` rather than
`UnknownVersion`. `crates/aegis-fabrica-defs/tests/array_form.rs` sends the
array form of both payloads and of every nested struct, and a one-element array
led by another version, the case the object-only peek alone decides, and fails
against the decoder of `061bbde`.

### Questions from the M10 run (2026-09-29)

- **D107** Does the Aegis kernel requirement ask for what a BPF LSM program
  needs to attach, and not only for `CONFIG_BPF_LSM`? Options: add a
  `build/kernel-requirement.json` row, `CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS`
  built-in, probe `kernel-config`, required by REQ-P06-05, carried through
  M09's pinned fixtures once imago and nucleus vendor the new bytes, and ask
  nucleus for a release built to it; ask nucleus to enable the function tracer
  without changing the requirement; or record E10-1's positive as unmet on this
  kernel and leave both unchanged. Recommended: the first option. Why: M10's run
  of 2026-09-29 (`docs/build/nucleus-kernel.md`) found the Nucleus kernel
  `7.2.8-lusoris1-realtime` satisfies all thirteen features and still cannot
  attach `action_gate`: the load succeeds and the attach returns `-EBUSY`,
  because the BPF trampoline patches the `-mfentry` nop at the hook's entry and
  the kernel is built with `CONFIG_FUNCTION_TRACER` not set. A controlled
  comparison on M26's source and configuration, differing only in the function
  tracer, attached with it and failed without it. `CONFIG_BPF_LSM` does not
  depend on the tracer in Kconfig, so a requirement that names only
  `CONFIG_BPF_LSM` admits a kernel that cannot run a BPF LSM program; M26's own
  kernel has the same gap. The payload is the contract both producers vendor,
  so the change is the maintainer's, and M10 stays open until a release built to
  it passes E10-1.
  **Decision (2026-09-29):** the first option. `build/kernel-requirement.json`
  gains the `CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS` row in its own change,
  which merges once imago and nucleus vendor the new bytes, and nucleus is asked
  for a release built to it. M10 stays open until that release passes E10-1.
  **Recorded (2026-09-29):** imago pull request 52, merged as
  `987b95a432e5c9aac1b4885b97fa2d3ff1b9d311`, re-vendors the new bytes (2232
  bytes, sha256
  `92d74206ee5a4cc46bc9a8c209855c4a3d07a9ed47b003963fc73697ac8776ce`) and closes
  imago issue 51; nucleus pull request 47, merged as
  `852be742eb173700d5ef93b0c6f867b855f9c640`, sets `CONFIG_FTRACE`,
  `CONFIG_FUNCTION_TRACER`, `CONFIG_DYNAMIC_FTRACE` and
  `CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS` for every stream (its ADR-0011).
  Both were their producer's `main` when the row landed here:
  `build/kernel-requirement.json` gains it, and M09 pins both commits and the
  new digest, with `make contract-fetch` run f20260929T230809-e9ad and `make
  verify-contract` run r20260929T230816-7c75 passing all nineteen cases; nucleus
  at `82aa6b7` refuses the new payload on that row. M26's
  `50-aegis-requirement.config` is rendered again, `10-base-support.config`
  gains the tracer the row follows from, and `make verify-kernel` rebuilt the
  kernel and read the row back from the guest, all seven cases passing. The
  reference profile records the row as `y`, read on `7.2.8-1-cachyos`. M10's
  gate against `v7.2.8-realtime-lusoris1` with the new requirement (run
  r20260929T232643-b28e) stops at the requirement case on that row and loads
  nothing; M10 stays open until a release built to it passes E10-1, which
  nucleus's ADR-0011 names `v7.2.8-realtime-lusoris2`. The dated entries of M18,
  M09 and M26 record the runs.

## Evidence

Bundle sha256: 8186bf0336e16764216396a147c536d96b3933901f5b2b81a4d0d3b74ffa25c6.
Sources cited by requirements (id, sha256):

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
- desktop-spec a41f6c9e825d7ef8bc51d88ef4f3f020a88e3afa7bbda2f8778e107ff5653884
  (private, 11183 bytes, outside the notebook bundle)
- en301549-v4.1.1
  636914c5c18dc58cf7716b8935502f1471de05caf5e1e6ae32469eed929cb824, ETSI EN
  301 549 V4.1.1 (2026-09),
  <https://www.etsi.org/deliver/etsi_en/301500_301599/301549/04.01.01_60/en_301549v040101p.pdf>
- en301549-v3.2.1
  1eee3a1841a94567da8e59f3b19a782ce9ab081c386b6a2a763b8cde13ff5b49, ETSI EN
  301 549 V3.2.1 (2021-03),
  <https://www.etsi.org/deliver/etsi_en/301500_301599/301549/03.02.01_60/en_301549v030201p.pdf>
- wcag-2.2 6e3c5fe397257cae509a2fb4752b73062cf8cbeb92c2cec618989b17e4cf7057,
  W3C Recommendation 12 December 2024,
  <https://www.w3.org/TR/2024/REC-WCAG22-20241212/>

The verbatim quotes are held in the private generation result
(`notebook-result.json`). This public document reproduces none of them. The
three standards are public and were re-hashed from the URLs above on
2026-09-28.
