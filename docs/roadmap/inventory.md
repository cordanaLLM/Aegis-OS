# Component inventory and decision register (M01)

Status: reviewed planning data

## Purpose and limits

This register reconciles the sixteen subsystems against the imported planning
sources and records what each one was missing before it could be activated. It
is planning data only.

- It records **no** build, boot, image, hardware, accessibility or release
  evidence, and its existence closes none of those gates. They remain blocked
  milestones.
- It installs nothing and executes no imported script or workflow. Every
  imported artefact named here stays inactive.
- The machine-readable form is `planning/candidates.json`. It is validated by
  `verify_candidates()` in `tools/verify_preparation.py`, which runs inside
  `make verify-all`.
- Every row cites the planning source by export id and sha256. The source bundle
  digest is `8186bf0336e16764216396a147c536d96b3933901f5b2b81a4d0d3b74ffa25c6`;
  all 62 source digests were recomputed before this register was written.
- The subsystem graph `export-062` (`1ce919ed54bb`) stays the graph of record.
  No decision recorded here edits it; superseded strings inside it are annotated
  instead.
- Quoted spans from private sources stay at or under 200 characters, and no
  private path appears in this document.

## Implementation candidates

Seventeen candidate rows cover all sixteen components: twelve Rust candidates,
three UI candidates, and the two declarative-definition candidates (P01, P02)
whose Rust crates are authorised but unnamed. P15 appears twice because decision
D09 gives it both a Rust crate and a UI package.

Two facts hold for every row without exception, and both were checked against
the tracked tree rather than inferred:

1. **Recorded on 2026-09-13 at milestone M01, and superseded in part by M02.**
   When this register was written, no `Cargo.toml`, `Cargo.lock`,
   `package.json` or JavaScript lockfile was committed anywhere, and `crates`
   and `ui` tracked only their README files. Milestone M02 then activated P06:
   the workspace root `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` and
   `crates/aegis-justitia/` are committed, and the P06 row below records that
   manifest and lockfile.
2. **Recorded at M01, extended by M14 for the P06 consumer contracts.** The
   P06 row's residual gaps no longer include the three consumer schemas: the
   action proposal from P09, the decision request to P05 and the signed audit
   record to P16 are typed, versioned Rust structs under
   `crates/aegis-justitia/src/contracts/`, and the hardened unit is a reviewed
   contract file under `crates/aegis-justitia/contracts/`. No transport is
   implemented, so the eBPF and TPM2 gaps in the same row are untouched.
3. **Every other component still records no manifest and no lockfile.** A
   `git ls-files` query for manifest and lockfile names returns only the
   workspace root files and the P06 crate; no other reserved directory tracks
   anything but its README. `verify_candidates()` fails closed if a row claims
   otherwise while its component is still a proposal.

"In proposed workspace" refers to membership of the ten-member Cargo workspace
declared by the imported proposal manifest (`export-006`, `6e694e01e136`). It is
not a repository fact: that workspace is proposal data and is not committed.

| Component | Language | Proposed path | In proposed workspace | Source id and sha256 (first 12) | Missing artefacts |
| --- | --- | --- | --- | --- | --- |
| P01 aegis-fabrica | declarative-config + proposed Rust crate | `build/ (mkosi, repart and sysupdate definition inputs)`; `crates/<undecided>/ definition parser (D15; no source records a crate name)` | no (no crate proposed) | export-010 1748925bf81e; export-049 4d9043f4af30; export-053 d85a1f6ceb0e; export-054 41d6d121b142; export-055 c915f281532f; export-063 b284bb77c19f | crate name and path for the D15 definition parser; crate Cargo.toml; workspace Cargo.toml and Cargo.lock; positive, negative and boundary tests; Imago request and artefact contracts; pinned image and kernel inputs |
| P02 aegis-janus-vallum | declarative-config + proposed Rust crate | `build/repart.d and build/sysupdate.d (reviewed, M03)`; `crates/aegis-janus-lifecycle/ A/B lifecycle (D15, created in M15)` | no (no component crate) | export-011 528da609dab7; export-053 d85a1f6ceb0e; export-054 41d6d121b142; export-055 c915f281532f; export-063 b284bb77c19f | a P02 component crate (aegis-janus-lifecycle models the lifecycle and is not the daemon); real signature, dm-verity and sysupdate effects, all stubbed in M15; TPM2 and secure-boot hardware or emulator evidence; real artefact and boot evidence |
| P03 aegis-vulcan | rust | `crates/aegis-vulcan/src/` validation slice and P03 consumer descriptors (M17); `crates/aegis-vulcan/tests/` | no (joined the repository workspace at M17, settling D22) | export-012 9b502c76509b; export-038 fe2cb01cfd13 | a P03 component daemon (the crate validates the arithmetic and the bounds and is not the driver); real BAR mapping, VFIO container, IOMMU domain and NVMe or GPU peer-to-peer transfer; a transport for either consumer descriptor, stubbed until M12; IOMMU, VFIO, NVMe and GPU peer-to-peer evidence |
| P04 aegis-compositor | rust | `crates/aegis-compositor/src/main.rs`; `build/mkosi.extra/usr/lib/systemd/system/aegis-compositor.service` | yes | export-013 7f4c22813332; export-027 f19640d7a7da; export-028 d74a93eac654 | crates/aegis-compositor/Cargo.toml; workspace Cargo.toml and Cargo.lock; positive, negative and boundary tests; compositor dependency pins (D08 and ADR-0001 select pure Rust; export-007 still says wlroots); frame-pacing constant (M07) |
| P05 aegis-forum-shell | svelte | `ui/forum-shell/src/App.svelte` | not applicable | export-014 659691a2d7d0; export-043 e3dbfa226e54; export-023 a0d06b6b8e6c | ui/forum-shell/package.json; JavaScript lockfile; accessibility test wiring (export-023 is proposal data, quarantined); UI toolchain pin (D10; open decisions D42 and D43); Concordia token consumption contract (D05) |
| P06 aegis-justitia | rust | `crates/aegis-justitia/src/main.rs`; `crates/aegis-justitia/tests/` | yes | export-015 fcbe2caed363; export-031 5ff683328148; export-047 971948673346; export-024 50a41ecd0b74 | crates/aegis-justitia/Cargo.toml; workspace Cargo.toml and Cargo.lock; library code outside #[cfg(test)] (export-047 defines its engine inside the test module); hash-algorithm trait boundary (D02); eBPF compile and verifier evidence for action_gate (M19, M10) |
| P07 aegis-lictor | rust | `crates/aegis-lictor/src/main.rs`; `bpf/scx_cake.bpf.c` | yes | export-016 cfa58b5b23b8; export-032 30d67bc52290; export-056 39af243568ad | crates/aegis-lictor/Cargo.toml; workspace Cargo.toml and Cargo.lock; eBPF toolchain admission (M19; open decision D40); positive, negative and boundary tests; kernel sched_ext evidence (D20 keeps eBPF out of the first image) |
| P08 aegis-calliope | rust | `crates/aegis-calliope/src/main.rs` | yes | export-017 afbc0af8056d; export-026 c2f1e433cd32 | crates/aegis-calliope/Cargo.toml; workspace Cargo.toml and Cargo.lock; PipeWire dependency pin (none is declared in export-006); positive, negative and boundary tests; PipeWire and DMA-BUF runtime evidence |
| P09 aegis-minerva | rust | `crates/aegis-minerva/src/main.rs` | yes | export-034 213a95fc0d02 | crates/aegis-minerva/Cargo.toml; workspace Cargo.toml and Cargo.lock; blueprint-level requirement source: the bundle holds no P09 report; action-proposal schema typed against P06 at M14 (`aegis_justitia::contracts::proposal`, direction settled by D03); the P09 crate that would send one is still absent; positive, negative and boundary tests |
| P10 aegis-vesta | rust | `crates/aegis-vesta/src/main.rs` | yes | export-018 5dbc6d071bbb; export-037 ce490c88081f | crates/aegis-vesta/Cargo.toml; workspace Cargo.toml and Cargo.lock; Wasm runtime selected through the template matrix (D06); both P06 gating paths are recorded and round-trip-tested at M14 (`aegis_justitia::contracts::graph::SandboxAdmissionPath`; D04 stays unresolved) and the behavioural contract tests remain M06 work; thread-per-core runtime decision (open decision D46) |
| P11 aegis-ludus | rust | `crates/aegis-ludus/src/main.rs` | yes | export-019 b0aa6e54ed57; export-033 531cbdf98eb5 | crates/aegis-ludus/Cargo.toml; workspace Cargo.toml and Cargo.lock; removal of the Steamworks path before activation (D12 and ADR-0002 exclude it); a named rich-presence library (open decision D48); TPM2 evidence for transaction signing |
| P12 aegis-concordia | css design tokens | `ui/concordia-tokens/concordia-tokens.css` | not applicable | export-020 1748f49bb7f8; export-042 411fb8c2d731; export-023 a0d06b6b8e6c | ui/concordia-tokens/package.json; JavaScript lockfile; accessibility test package (export-023 is quarantined proposal data); focus-ring width applied per D16 (3px token, 2px boundary minimum); confirmation that no monolithic CSS library is introduced (D17) |
| P13 aegis-tellus | rust | `crates/aegis-tellus/src/main.rs`; `bpf/kepler_power.bpf.c` | yes | export-036 25813d240733; export-048 293357bac7d3 | crates/aegis-tellus/Cargo.toml; workspace Cargo.toml and Cargo.lock; blueprint-level requirement source: the bundle holds no P13 report; telemetry payload schema for the P16 consumer; positive, negative and boundary tests |
| P14 aegis-hephaestus | rust | `crates/aegis-hephaestus/src/main.rs` | yes | export-021 6a3cda152e6c; export-029 427996186520 | crates/aegis-hephaestus/Cargo.toml; workspace Cargo.toml and Cargo.lock; CAD and solver dependency pins (absent from the export-006 dependency table); licence decision for GPL-licensed solver bindings; positive, negative and boundary tests |
| P15 aegis-hestia | rust | `crates/aegis-hestia/src/` storage and vector logic, the D09 boundary payload and the P04 registration (M17); `crates/aegis-hestia/tests/` | no (joined the repository workspace at M17, settling D22) | export-022 46cea660df63; export-030 689d175667d6 | a P15 component daemon (the crate models the store and the overlay and is neither); a real PGlite instance, its WebAssembly runtime and the Btrfs @pglite subvolume; a Wayland connection and a wlr-layer-shell binding, stubbed until M12; real hardware or emulator evidence |
| P15 aegis-hestia | svelte | `ui/hestia-app/src/App.svelte` | not applicable | export-045 359466152a37; export-022 46cea660df63 | ui/hestia-app/package.json; JavaScript lockfile; accessibility test; UI toolchain pin (D10). The Rust side of the D09 boundary exists from M17 (`aegis_hestia::HestiaView`); nothing on this side consumes it yet |
| P16 aegis-athena | rust | `crates/aegis-athena/src/main.rs` | yes | export-025 6b23723ddb76 | crates/aegis-athena/Cargo.toml; workspace Cargo.toml and Cargo.lock; blueprint-level requirement source: the bundle holds no P16 report; hash-algorithm trait so that D02 supersedes the ledger named in export-007; signed audit-record schema (M14) |

Boundary rows, stated explicitly because they are the ones a workspace-wide
build would silently skip:

- **P03 `aegis-vulcan`** and the Rust half of **P15 `aegis-hestia`** are not
  members of the proposed workspace and appear in no imported directory map. A
  workspace-wide build invocation would not cover them (REQ-WS-01). Their
  membership was open decision D22, **settled at M17 by acting on it**: both
  are now written-out members of the repository's own workspace root
  `Cargo.toml`, and `Cargo.lock` carries an entry for each, so a
  workspace-wide `cargo` invocation reaches them. The statement about the
  *proposed* workspace is unchanged: `export-006` still lists ten members and
  neither of these is among them.
- **P01** and **P02** have no proposed crate at any path: absent from the
  proposed workspace, absent from the imported directory map, and outside the
  P03-P16 range that `crates/README.md` reserves. Decision D15 authorises new
  crates under `crates/`; their names and split are open decision D21.
- **P09**, **P13** and **P16** rest on a single source each. The bundle holds no
  blueprint report for them: it spans export-001 to export-063 with export-008
  absent, 62 sources in total, and the import record contains no report for
  these three. This is open decision D24.

## Contradictions and their status

All 28 edges of the graph of record were compared against both architecture
documents in both directions, and the four named value conflicts were traced
through every source that states them. 27 disputes are registered: 12 are closed
by a recorded decision and 15 remain open.

### Resolved

| ID | Subject | Side A | Side B | Decision | Affected contract | Milestone |
| --- | --- | --- | --- | --- | --- | --- |
| DSP-01 | P05/P12 design-token edge direction | export-062 `1ce919ed54bb`: Graph of record: edge P05_Forum -> P12_Concordia, relation CONSUMES_DESIGN_TOKENS, transport CSS custom properties and ARIA bridges. | export-002 `7e0c95f4ea05`: Architecture document draws the arrow the other way: P12 supplies accessibility design tokens to P05. | D05: P12 Concordia produces the design tokens and P05 Forum consumes them; the graph edge identifier is kept with the direction annotated (M16). | Concordia design-token and ARIA bridge contract consumed by the Forum shell; producer identity. | M16 (direction recorded); token harness M04 |
| DSP-02 | P06/P09 action-gate edge direction | export-062 `1ce919ed54bb`: Graph of record: edge P06_Justitia -> P09_Minerva, relation ACTION_GATE_INTERCEPT, pre-execution risk-tier gate and approval routing. | export-002 `7e0c95f4ea05`: Architecture document draws P09 -> P06: Minerva raises the pre-execution interception request. | D03: propose/intercept semantics. P09 proposes and P06 intercepts and decides; the graph edge identifiers are kept (typed in M14). | Action-proposal schema between P09 Minerva and P06 Justitia (allow, block or escalate). | M14 (typed in `aegis_justitia::contracts::proposal`; the direction is a single-variant enum, so the opposite reading does not decode); engine M02 |
| DSP-04 | P06 -> P10 syscall-interception edge existence | export-002 `7e0c95f4ea05`: Draws a P06 -> P10 syscall-interception edge carried by the action_gate eBPF program. | export-062 `1ce919ed54bb`: No P06 -> P10 edge exists among the 28 edges; Vesta is reached only from P09 by capsule execution. | D04: recorded as unresolved in M14; both the direct P06 to P10 path and the transitive path through P09 are contract-tested in M06 before the graph is changed. | Whether Justitia gates Vesta syscalls directly or transitively through P09; Vesta admission path. | M14 (recorded in code as `D04_SANDBOX_GATE`, unresolved, with both paths round-trip-tested), M06 (both paths contract-tested behaviourally) |
| DSP-07 | Ledger hash algorithm: three algorithms for one ledger | export-062 `1ce919ed54bb`: Graph of record names a BLAKE3 checkpoint ledger as the P06 -> P16 audit transport. | export-015 `fcbe2caed363`: The P06 audit-record schema field for the previous record is a SHA-256 hex digest of the previous signed block. | D02: SHA-256 hash chain behind a hash-algorithm trait; MD5 excluded; BLAKE3 only through a later recorded decision. The graph string is annotated, not edited, at M01. | Signed audit-record schema (P06 to P16) and the Athena ledger hash-chain trait. | M02 (recorded), M05 (ledger), M14 (schema typed; `HashAlgorithm` admits SHA-256 only, so a record naming another algorithm does not decode) |
| DSP-08 | Focus-indicator width: 3px token against a 2px stroke | export-042 `411fb8c2d731`: Token file sets a focus-ring width of 3px with a 2px offset, and raises the width again inside the forced-colors block. | export-020 `1748f49bb7f8`: Report requires a focus indicator with a minimum 3:1 contrast ratio and a 2px stroke width, and declares a 2px token. | D16: a 3px focus-ring token with a 2px minimum enforced by the boundary test. The forced-colors override must not be read as a violation. | Concordia focus-ring token contract and the accessibility boundary assertion consumed by the P05 shell. | M04 (accessibility harness boundary test) |
| DSP-09 | CSS framework: framework fork against token-only delivery | export-004 `15831276a058`: Section prose describes Concordia as a fork of a monolithic CSS framework with improved accessibility. | export-004 `15831276a058`: The trade-off matrix in the same source records design tokens as chosen and monolithic CSS libraries as discarded. | D17: design tokens without a monolithic CSS library; the framework-fork statement is superseded. Both sides are the same source id, which is a source-internal conflict. | ui/concordia-tokens dependency surface and the P05/P12 token contract (tokens against framework classes). | M04 |
| DSP-10 | Boot kernel identity: distribution package against a produced kernel artefact | export-049 `4d9043f4af30`: The image content package list names a real-time distribution kernel package with no version. | public:docs/integration/stack.md `dbc7e10aecaa`: The shared ownership table assigns kernel configuration, patch selection and artefact production to the Nucleus repository. | D07: both. The image boots a pinned distribution real-time kernel by default and a Nucleus artefact overrides it once M09 delivers one. The version floor stays open (D55). | Aegis-side kernel requirement payload (LSM, sched_ext, BTF, RAPL, VFIO/IOMMU, KVM; ABI; architectures) and the image content list. | M18 (schema); consumed at M09 and M11; eBPF re-verification M10 |
| DSP-23 | P04 compositor implementation: mandated C library against a pure-Rust scaffold | export-013 `7f4c22813332`: The report rejects higher-level abstractions and mandates a C-based compositor library for absolute control of the event loop. | export-027 `f19640d7a7da`: The only code artefact is pure Rust with no foreign-function binding to that library and no version pinned anywhere. | D08: pure Rust compositor; the C library mandate is superseded (ADR-0001). No C compositor toolchain is admitted. | P04 registry and IPC state-machine backend, and the admitted toolchain (a C toolchain against a Rust-only workspace). | M07 |
| DSP-24 | P15 Hestia location: Rust crate against UI package | export-030 `689d175667d6`: The file header places Hestia under crates/aegis-hestia as a Rust daemon for the productivity and application engine. | export-007 `84f43472c536`: The directory map lists only ui/hestia-app for P15 and carries no crates/aegis-hestia row. | D09: both. A Rust crate for the storage and vector logic and a Svelte package for the UI, with a typed boundary (M17). | P15 component path in the inventory and the typed boundary between the Rust logic and the Svelte package. | M01 (recorded), applied at M17 |
| DSP-25 | P10 Wasm capsule runtime against the language-boundary rule | export-037 `ce490c88081f`: A Rust daemon describes its capsules in terms of a Go-implemented Wasm runtime; the graph repeats that runtime in the transport string. | public:docs/integration/stack.md `dbc7e10aecaa`: Shared package reuse must respect language boundaries: a Go library is not a direct dependency of a Rust daemon without an adapter and contract tests. | D06: a Rust-native WebAssembly runtime selected at M06 through the template matrix; no Go runtime and no protocol adapter. The graph string is annotated, not edited. | Vesta capsule-execution contract and the crate dependency set; also the P09 -> P10 transport string. | M06 |
| DSP-26 | P11 proprietary SDK against the subsystem's own open-source constraint | export-019 `b0aa6e54ed57`: The report wraps proprietary C++ SDK headers through a binding generator to isolate the runtime from that ABI. | export-004 `15831276a058`: The trade-off matrix records free and open-source compliance as the declared constraint for the same subsystem. | D12: the proprietary SDK is excluded from the Aegis image; P11 covers only free and open-source integration paths (ADR-0002). | P11 crate dependency set and the image content list; a licence decision before any SDK is vendored. | M08 (negative test proves crate and image build without it) |
| DSP-27 | P02 security model: reversible consolidation against permanent hardening | export-004 `15831276a058`: The trade-off matrix poses reversible structural consolidation against permanent hardening for the P02 security model. | export-011 `528da609dab7`: Root-hash verification must make the read-only root filesystem bit-for-bit identical to the signed release before it is trusted. | D13: reversible structural consolidation, reconciled with root-hash integrity in M15. | P02 A/B candidate lifecycle state machine against the root-integrity requirement. | M15 |

Two resolved disputes leave a string in the graph of record that the decision
supersedes: the audit transport still names the algorithm D02 rejected, and the
capsule transport still names the runtime D06 rejected. The graph is not edited
at M01, so both are annotated here and must not be read back as requirements.

### Open

| ID | Subject | Side A | Side B | Affected contract | Milestone | Carried as |
| --- | --- | --- | --- | --- | --- | --- |
| DSP-03 | P06/P09 action-gate transport: three transports named for one edge | export-062 `1ce919ed54bb`: Transport recorded as the eBPF action_gate program combined with D-Bus. | export-002 `7e0c95f4ea05`: Communication matrix records a Unix domain socket with a sub-500 microsecond budget. | P06/P09 wire contract: bus name, socket path, eBPF map or ring-buffer layout, and the sub-500 microsecond budget. | M14 settled only D03's direction half and left this transport question untouched; eBPF half re-verified in M19 and M10 | D26 |
| DSP-05 | P09/P14 code-CAD verification edge direction | export-062 `1ce919ed54bb`: Graph of record: P09_Minerva -> P14_Hephaestus, relation VERIFY_CODE_CAD, parametric constraint and dimension checks. | export-002 `7e0c95f4ea05`: Architecture document draws P14 -> P09 for the same solver-backed verification. | Code-CAD verification request and result schema; which side owns the solver invocation. | M06 (P09 router and solver); P14 slice M08 | D27 |
| DSP-06 | P15/P02 local-database storage edge existence | export-002 `7e0c95f4ea05`: Draws P15 -> P02 for local database storage on the dedicated Btrfs subvolume. | export-062 `1ce919ed54bb`: P15_Hestia has exactly one outbound edge, to P04 for overlay registration; no P15 -> P02 edge exists. | Hestia storage contract against the P02 var-partition subvolume, snapshot and encryption ownership. | M17 (Hestia slice); definitions M03, lifecycle M15 | D28 |
| DSP-11 | P08/P11 remote-play capture edge direction | export-062 `1ce919ed54bb`: Graph of record: P08_Calliope -> P11_Ludus, relation REMOTE_PLAY_CAPTURE, a sub-5 millisecond encoded video stream. | export-002 `7e0c95f4ea05`: Architecture document draws P11 -> P08 for the same buffer-sharing capture stream. | Capture contract: which subsystem opens the stream and owns the sub-5 millisecond encode budget. | M07 (P08) and M08 (P11); GPU path M12 | D29 |
| DSP-12 | P12/P15 accessibility-enforcement edge direction | export-062 `1ce919ed54bb`: Graph of record: P12_Concordia -> P15_Hestia, relation ENFORCE_A11Y_STANDARDS, conformance assertions. | export-003 `13af15ffc316`: Master diagram draws the Hestia application as a consumer of Concordia, the reverse direction. | Ownership of the accessibility assertion gate for the Hestia surface: producer enforcing or application consuming. | M04 (harness) and M17 (Hestia slice) | D30 |
| DSP-13 | P13 -> P05 carbon-telemetry budget: delivery deadline against emission interval | export-002 `7e0c95f4ea05`: Communication matrix records a D-Bus signal with a sub-2 millisecond figure for power and carbon updates. | export-003 `13af15ffc316`: Communication matrix records the same signal interface with a one-second interval. | Telemetry signal contract: emission period against delivery deadline, and the shell status-bar refresh assertion. | M05 (producer); consumer M16 | D31 |
| DSP-14 | P04 -> P05 compositor/shell endpoint and budget stated by one source only | export-062 `1ce919ed54bb`: Graph of record: P04_Mercurius -> P05_Forum over a Unix socket stream, with no endpoint path and no budget. | export-003 `13af15ffc316`: The only source naming the socket path and a sub-100 microsecond budget; export-002 omits the edge entirely. | Compositor and shell IPC contract: socket path, framing, and which budget the contract test asserts. | M07 (producer) and M16 (consumer with stubbed IPC) | D32 |
| DSP-15 | P06 -> P05 decision-request edge: absent from one document, budget exceeds the gate | export-002 `7e0c95f4ea05`: Communication matrix records a D-Bus decision-request prompt with a sub-1 millisecond figure. | export-003 `13af15ffc316`: No Justitia to shell edge appears in the master diagram or the matrix; Justitia connects only to Minerva and the TPM. | Versioned decision-request schema and whether the human-oversight prompt sits inside the sub-500 microsecond interception gate. | M14 (schema typed in `aegis_justitia::contracts::decision_request`; the latency question is untouched because no transport exists); consumer typed in M16 | D33 |
| DSP-16 | P09 -> P10 capsule execution: transport, endpoint and latency disagree | export-062 `1ce919ed54bb`: Graph of record: a virtual-socket transport named together with a Go Wasm runtime; the P10 node budget is a sub-125 millisecond microVM boot. | export-003 `13af15ffc316`: Communication matrix records the same virtual-socket family with a concrete address and a sub-1 millisecond budget. | Vesta capsule-dispatch contract: address and port, fallback, and whether the budget covers a warm call or a cold boot. | M06 | D34 |
| DSP-17 | P07 -> P09 VRAM swap: transport disagreement and a different endpoint | export-062 `1ce919ed54bb`: Graph of record: transport is the kernel GPU scheduler priority path plus a dynamic VRAM allocator, with no bus call. | export-002 `7e0c95f4ea05`: Communication matrix adds D-Bus with a sub-100 microsecond budget and an explicit inference-pause payload. | Broker to agent pause and swap control contract: a bus method on the agent daemon or a scheduler-only action. | M07 (broker); GPU evidence M12 | D35 |
| DSP-18 | P03 -> P09 weight-streaming endpoint: subsystem target against hardware target | export-062 `1ce919ed54bb`: Graph of record: P03_Vulcan -> P09_Minerva over a peer-to-peer DMA path; export-002 agrees with this direction. | export-003 `13af15ffc316`: Draws the peer-to-peer path from storage to the GPU and from P03 to the GPU, with no P09 endpoint. | Weight-streaming contract: whether Minerva requests a stream from Vulcan or only observes a programmed hardware path. | M17 (P03 validation crate); DMA evidence M12 | D36 |
| DSP-19 | Eight graph-of-record edges appear in neither architecture document | export-062 `1ce919ed54bb`: Graph-only edges: P03->P15, P06->P16, P07->P04, P07->P08, P11->P02, P11->P04, P14->P15, P15->P04. | export-002 `7e0c95f4ea05`: Draws sixteen edges and names none of the eight; export-003 lists six matrix rows and names none of them either. | Eight interface contracts with a single supporting source each; none can be typed as a two-source-agreed edge. | M07, M08, M14, M17 (per edge) | D37 |
| DSP-20 | Five edges present in the graph and one document but absent from the other | export-062 `1ce919ed54bb`: Carries P08->P04, P10->P16, P13->P07, P16->P13 and P06->P05; export-002 carries the same five. | export-003 `13af15ffc316`: None of the five relations appears in the master diagram or the matrix. | M05 evolution loop, M06 sandbox evaluation, M07 buffer-sharing stream; P06->P05 is tracked separately in DSP-15. | M05, M06, M07 | D37 |
| DSP-21 | Node set: hardware, kernel and transport nodes outside the sixteen-subsystem graph | export-062 `1ce919ed54bb`: Declares sixteen nodes and 28 edges; every node is a subsystem, and hardware and transports appear only as attribute strings. | export-003 `13af15ffc316`: Adds CPU, GPU, storage, TPM, three eBPF programs, a mesh transport and a sidecar protocol as first-class nodes. | Owner of the TPM attestation contract (P02 or direct hardware access) and of sidecar dispatch (P04 or an unowned transport). | M20 (attestation slice), M07 (compositor IPC registry) | D38 |
| DSP-22 | Btrfs subvolume set for the var partition | export-055 `c915f281532f`: The partition definition declares five subvolumes for the var partition. | export-003 `13af15ffc316`: The storage model prose lists only four of the same five subvolumes. | P02 var-partition definition validated offline, and the database subvolume the Hestia storage dependency rests on. | M03 (definitions validated offline), M15 (A/B lifecycle) | D39 |

DSP-20 needs no decision of its own: two of three sources agree and the graph of
record carries the five edges. It is registered so the third source's silence is
not later read as a contradiction, and it is folded into D37 with the graph-only
edges.

## Toolchain and version drift

56 rows. Every one is proposal data: no manifest, lockfile or toolchain pin is
committed, the imported workflows are inactive, and nothing was installed to
produce this register. Each "current upstream" value was read from the project's
own release channel on 2026-09-13; where a value could not be read from an open
source it is reported as unknowable rather than guessed.

| Item | Proposed pin | Source | Current upstream | Drift verdict | Pinned at |
| --- | --- | --- | --- | --- | --- |
| tokio async runtime | 1.38 with all features | export-006 6e694e01e136 | 1.53.1, published 2026-07-20 (crates.io) | outdated: fifteen minor releases behind; the caret pin would resolve to the current release | M02, re-locked per crate at each consuming milestone |
| monoio thread-per-core runtime | 0.2 with the bytes feature | export-006 6e694e01e136 | 0.2.4, published 2024-08-20; newest release upstream | current pin, stale upstream: no release in over two years, which is a maintenance risk for the thread-per-core mandate | M06 and M07 (runtime selected at activation) |
| aya user-space eBPF loader | 0.12 | export-006 6e694e01e136 | 0.14.0, published 2026-06-24 (crates.io) | outdated: two breaking pre-1.0 minors behind; the pin will not upgrade under semantic versioning | M19 (eBPF toolchain admission) |
| aya kernel-space eBPF crate | 0.1 under the old crate name | export-006 6e694e01e136 | the old crate name does not exist on crates.io; the successor crate is at 0.2.1, published 2026-06-30 | unresolvable: the dependency cannot be fetched at all, so the proposal workspace would fail on its first build; delete rather than bump | M19 |
| memmap2 memory-mapping wrapper | 0.9 | export-006 6e694e01e136 | 0.9.11, published 2026-06-22 (crates.io) | current: the 0.9 series is still newest | M17 (P03 validation crate) |
| zenoh mesh IPC | 1.0 | export-006 6e694e01e136 | 1.10.1, published 2026-09-07 (crates.io) | outdated: ten minor releases behind within the 1.x line; the most actively released dependency in the set | M07 (the proposal pin is not inherited) |
| serde | 1.0 with derive | export-006 6e694e01e136 | 1.0.229, published 2026-07-18 (crates.io) | current | M02 |
| serde_json | 1.0 | export-006 6e694e01e136 | 1.0.151, published 2026-07-20 (crates.io) | current | M02 |
| tracing | 0.1 | export-006 6e694e01e136 | 0.1.44, published 2025-12-18 (crates.io) | current | M02 |
| tracing-subscriber | 0.3 with env-filter | export-006 6e694e01e136 | 0.3.23, published 2026-03-13 (crates.io) | current | M02 |
| anyhow | 1.0 | export-006 6e694e01e136 | 1.0.104, published 2026-07-18 (crates.io) | current | M02 |
| thiserror | 1.0 | export-006 6e694e01e136 | 2.0.20, published 2026-08-08 (crates.io) | outdated across a major boundary: the caret pin can never reach the current line | M02 (decided once, workspace-wide) |
| Rust edition | 2021 | export-006 6e694e01e136 | 2024 is the newest stabilised edition (Rust Edition Guide) | outdated by one edition; cheapest to change before any crate exists | M02 |
| workspace package version | 1.0.0 | export-006 6e694e01e136 | no workspace manifest is committed and no release exists; the tracked VERSION file reads 0.1.0 | not drift but an aspirational release number that would misstate maturity | M02 (initial version) and M13 (release versioning) |
| project identity URL | a lowercase repository path | export-006 6e694e01e136 | the configured canonical remote uses the repository's own letter case | mismatched: the case difference redirects silently and would produce wrong crate metadata and documentation links | M02, re-checked at M13 |
| workspace licence | EUPL-1.2 | export-006 6e694e01e136 | the committed REUSE metadata declares EUPL-1.2 for technical material and CC BY-SA 4.0 for prose | current: the proposal value agrees with the committed technical layer; it carries no prose-layer declaration | M02 |
| Rust toolchain channel | unpinned floating stable channel in the guide and the imported CI | export-007 84f43472c536 | stable 1.98.1, dated 2026-09-01 (static.rust-lang.org channel manifest) | unpinned: no version drift but no reproducibility, and no pinned version before a cargo gate runs | M02, reused by later Rust milestones |
| eBPF linker | unpinned cargo install with no version | export-007 84f43472c536 | 0.11.1, published 2026-09-07 (crates.io) | unpinned floating install; needed only if the Rust eBPF path is chosen over the C path | M19 |
| C compiler and toolchain for eBPF | a floor of major version 18 only | export-007 84f43472c536 | 23.1.1, released 2026-09-08 (upstream releases) | outdated floor, five majors behind current; a floor is not a pin, and the imported Makefile invokes the compiler with no version guard | M19 |
| image build tool | a floor of version 24 or later | export-007 84f43472c536 | version 27, released 2026-08-27; the reference profile runs mkosi 27 from the distribution package extra/mkosi 27-1, read back with `mkosi --version` and `pacman -Qi mkosi` | resolved at M18: the inherited v24 floor is superseded by the exact pin mkosi 27 (extra/mkosi 27-1) recorded in docs/roadmap/toolchain-admission.md, and build/mkosi.conf carries MinimumVersion=27 so mkosi itself refuses an older mkosi | M18 (D14 and D56); admitted |
| image distribution and release | a rolling distribution with the release set to latest | export-049 4d9043f4af30 | the named distribution is rolling and publishes no numbered release; latest resolves to whatever a mirror serves at build time | resolved at M18 for the definition: build/mkosi.conf pins Snapshot=2026/09/13, which mkosi 27 resolves against the Arch archive, and the product input manifest refuses an unpinned snapshot at decode time. A snapshot fixes every package version at once, so no per-package version syntax is needed; whether the pinned snapshot builds is M11 work | M18 (D18); build proof at M11 |
| image output version string | a hardcoded 1.0.0 in the output filename | export-049 4d9043f4af30 | no image or release exists; the release train is gated and M13 is blocked | not assessable as drift: a hardcoded version that no release process produces, and the imported release workflow uses a different filename | M11 (naming) and M13 (release) |
| boot kernel package | a bare real-time kernel package name with no version | export-049 4d9043f4af30 | the distribution package resolves to a rolling real-time kernel; upstream publishes separate stable, mainline and longterm lines | resolved at M18 for the identity: the product input manifest records the boot kernel source and the pinned default package linux-rt, and the dated snapshot fixes which linux-rt build that name resolves to. D70 as amended makes M26 the current producer; the artefact itself is M26 and M10 work | M18 (D07 and D18); re-verified at M26 and M10 |
| target kernel floor | a floor plus an unreleased target version | export-016 cfa58b5b23b8 | the named target version is not released; upstream lists it only as a release candidate, with an earlier stable and longterm line available | replaced at M18 by a feature payload: build/kernel-requirement.json states the Kconfig symbols, states and probes a conforming kernel must satisfy, with a checked minimum release of 6.12 and the unreleased target release recorded but never checked against a profile, because a check against a version that does not exist would refuse every kernel that does | M18 (feature payload, not a version list); consumed by M26 |
| Node.js runtime | a hardcoded major in the imported CI against a different major in the guide | export-041 e05ddc9466fc | the CI major reached end of life on 2026-04-30; the guide major is in maintenance until 2027-04-30; the current active long-term line is two majors newer | outdated on both sides: neither imported value is a supported long-term release | M04 (D10); consumed by M16 |
| UI package manager | unpinned install script with no version | export-007 84f43472c536 | 12.4.1 (npm dist-tag latest) | unpinned, and contradicted: the imported CI and Makefile assume a different package manager entirely | M04 |
| UI framework | major version only, no minor or patch anywhere | export-007 84f43472c536 | 5.57.0 (npm dist-tag latest) | current major, unpinned minor: the reactive syntax used by the imported components remains valid | M04; manifest at M16 |
| browser test runner | unpinned; named in five sources with no version | export-041 e05ddc9466fc | 1.63.0, released 2026-09-04 | unpinned: browser binaries are pinned per runner release, so an unpinned runner means unpinned browsers | M04 |
| accessibility rule engine | unpinned | export-023 a0d06b6b8e6c | 4.13.0, released 2026-08-05 | unpinned, and a correctness problem: the imported standards file makes a full pass a hard gate, so the gate's definition of pass would change between runs | M04 |
| release-workflow toolchain action | a major tag on an action namespace | export-052 78920f933421 | the referenced repository returns HTTP 404; no published action exists at that name | nonexistent: unresolvable, and a name-squatting exposure in a job holding write permissions; replace, do not re-pin | M13 |
| checkout action | a major tag rather than a commit digest | export-041 e05ddc9466fc | 7.0.1, released 2026-07-20 | outdated by three majors and tag-pinned; a mutable tag is the standard supply-chain weakness | M13 |
| Node setup action | a major tag | export-041 e05ddc9466fc | 7.0.0, released 2026-07-14 | outdated by three majors, tag-pinned, and carries the end-of-life Node major as its input | M04 and M13 |
| Rust cache action | a major tag | export-041 e05ddc9466fc | 2.9.2, released 2026-08-06 | current major, tag-pinned rather than digest-pinned; the only imported action whose major is still live | M13 |
| community Rust toolchain action | a moving branch reference | export-041 e05ddc9466fc | the repository publishes one release tag and is conventionally referenced by named branch refs | unpinned by design: both the action code and the installed Rust version float | M02 and M13 |
| release publishing action | a major tag | export-052 78920f933421 | 3.0.3, released 2026-08-30 | outdated by one major, tag-pinned; it publishes a non-draft release, so it must stay inactive until M13 | M13 |
| provenance generator workflow | a semantic tag on a reusable workflow | export-052 78920f933421 | 2.1.0, released 2025-02-24 | outdated by one minor; the tag form is the documented practice for this reusable workflow, so only the version is at issue | M13 |
| artefact signing tool | unpinned distribution package plus a legacy experimental flag | export-052 78920f933421 | 3.1.3, released 2026-08-06 | unpinned, and the experimental flag belongs to a superseded major line; the pin must name the upstream version, not a distribution package | M13, with M11 admission |
| systemd components | unpinned package names only | export-049 4d9043f4af30 | 261.3, released 2026-09-10; the workstation runs the same major | unpinned with no floor, although the definition syntax has changed across recent majors; the imported CI suppresses the validator's exit code | M03 (floor) and M18 |
| compositor C library | unpinned; named as an image package with no version in any source | export-049 4d9043f4af30 | 0.20.2, tagged 2026-07-07 upstream | unpinned and superseded: D08 and ADR-0001 select a pure Rust compositor, so the image package entry must be removed at M18 | none in Aegis; removal at M18 |
| audio server stack | unpinned; four package names and many prose mentions, none with a version | export-049 4d9043f4af30 | 1.6.8, tagged 2026-07-09 upstream | unpinned; the proposal's latency claim depends on settings that have changed across the current major line | M07 logic slice; real pin at M18 |
| microVM hypervisor | unpinned | export-018 5dbc6d071bbb | 1.17.0, released 2026-09-10 | unpinned; no admission is needed before the hardware-gated milestone | M21 |
| Go Wasm runtime | unpinned | export-037 ce490c88081f | 1.12.0 under a renamed upstream organisation; the old path now redirects | unpinned and superseded by D06; also barred by the language-boundary rule. The organisation rename is itself a drift signal | none; excluded at M06 |
| Wasm plugin framework | unpinned | export-037 ce490c88081f | 1.30.0, released 2026-06-04 | unpinned; it survives D06 because its Rust SDK works with a Rust-native runtime | M06 |
| embedded WASM database | unpinned | export-022 46cea660df63 | 0.5.8, published 2026-08-26 | unpinned and pre-1.0, so every minor is potentially breaking; it carries the local persistence layer | M17, with the JavaScript half at M04 |
| proprietary game-platform SDK | unpinned | export-019 b0aa6e54ed57 | not verifiable from an open source: the download and version listing sit behind a partner login | unknowable, and moot: D12 and ADR-0002 exclude it from the image; the licence terms also conflict with the technical layer | none; excluded |
| chat-platform presence integration | unpinned, and no concrete library is named | export-019 b0aa6e54ed57 | not determinable: the sources name no library, and the legacy project publishes no current release identity | unknowable: a missing-dependency decision rather than version drift | M08 (open decision D54) |
| mesh generator | unpinned | export-021 6a3cda152e6c | 4.15.2 (current advertised download series) | unpinned; its licence interacts with the technical layer, so a linking decision is owed before any binding is admitted | M08 at the earliest |
| CFD solver | unpinned, and the fork is not identified | export-021 6a3cda152e6c | one line is at a dated current release; the other line could not be read in this session and is reported as unverified | unpinned and ambiguous: no pin is expressible until the fork is chosen | M08 at the earliest |
| Rust B-Rep CAD kernels | unpinned; neither appears in the proposal dependency table | export-021 6a3cda152e6c | one kernel's registry release is from 2024-09-20 while its repository was pushed on 2026-09-07; the other is at 0.1.9 from 2026-02-12 | unpinned, with a registry-against-repository divergence, so the pin must say whether it targets the published crate or a revision | M08 |
| SMT solver | unpinned | export-034 213a95fc0d02 | 5.1.0, released 2026-08-16; the 5.x line succeeded the 4.x line in July 2026 | unpinned, and upstream crossed a major boundary within the last two months, so any remembered 4.x assumption is stale | later than M06, which admits no solver binding |
| power-telemetry project | unpinned; the probe is reimplemented locally | export-061 e7d07dd56ca7 | 0.11.4, released 2026-02-16, in a slowing cadence | unpinned and pre-1.0; the version matters only if the upstream project is adopted rather than the local probe | M05, M19 and M21 |
| scheduler framework and scheduler program | unpinned; the program is a local object with no toolchain version | export-056 39af243568ad | 1.1.3, released 2026-08-19; the kernel feature itself has been mainline since the 6.12 line | unpinned, and effectively a kernel pin in disguise because the interface tracks the kernel | M19 (local fixture) and M10 (against the pinned kernel) |
| emulator, firmware and TPM emulation | unpinned package names plus a hardcoded firmware path | export-007 84f43472c536 | the distribution packages are at current versions for the targeted distribution | unpinned; the hardcoded firmware path in the image definition is a portability defect rather than version drift | M11, shared with M10 |
| TPM tooling | unpinned package name only | export-049 4d9043f4af30 | the distribution package is at its current version | unpinned; the library choice is a contract input for the signed audit-record schema | M20, recorded at M18 |
| remaining image packages | unpinned bare package names for nine further packages | export-049 4d9043f4af30 | each resolves to whatever the rolling repository serves at build time; no version is claimed by the source | unpinned; drift is not assessable without a chosen snapshot | M18 (D18) |
| compositor frame-pacing constant | a loop throttle that contradicts the same file's refresh-rate comment and render budget | export-027 f19640d7a7da | not an upstream artefact: the stated refresh rate implies a frame period roughly seventy times longer than the constant | internally inconsistent rather than outdated; an internal pin, included because the epic's acceptance covers it | M07 |

Three of these are hard failures rather than stale numbers, and they are the
ones to act on first:

1. The kernel-space eBPF dependency cannot be resolved at all: the crate name in
   the proposal manifest does not exist in the registry, so the proposed
   workspace would fail on its first build. It must be deleted or replaced,
   never bumped (open decision D40).
2. The toolchain action referenced by the imported release workflow returns HTTP
   404, inside a job that holds write permissions. Beyond being unverified
   (REQ-REL-02), an unregistered namespace referenced from a privileged workflow
   is a name-squatting exposure. The workflow is inactive, so nothing is at risk
   today; the reference must be replaced at M13, not re-pinned.
3. The runtime major hardcoded in the imported CI reached end of life on
   2026-04-30, and the major named in the imported guide is in maintenance only.
   Neither imported value is a supported long-term release (open decision D42).

Two further observations: the image package list still names the compositor
library that ADR-0001 superseded, which argues for authoring that list fresh at
M18 rather than carrying the imported file forward; and one CAD kernel's
registry release trails its repository by about two years, so the M08 pin must
state whether it targets the published release or a revision.

## Imported artefacts quarantined

8 imported artefacts are held as proposal data. None is tracked in this
repository: every tracked file was hashed and none carries any of these digests.
Two tracked paths share a name with an imported artefact and are different files
by digest. The repository's own workflows contain no failure suppression and pin
every action to a commit digest, so the quarantine holds and M01 changes no
tracked workflow.

### imported continuous-integration workflow (`export-041`, `e05ddc9466fc`)

Claims: Three jobs: a Rust format, lint and test gate called a Power-of-Ten
check; an image synthesis and headless boot verification; and an accessibility
conformance gate.

| Defect | Evidence |
| --- | --- |
| Failure suppression in both definition validators | Each validation step ends in a shell true fallback, so a malformed partition geometry or transfer definition reports success (REQ-CI-01). |
| Failure suppression in dependency installation | Dependency install and browser install both end in a true fallback, so the job stays green with nothing installed. |
| Conformance reported without executing the check | The accessibility suite itself ends in a true fallback, so a gate named for a conformance standard can never fail (REQ-CI-02). |
| Unpinned third-party actions | Every action is referenced by a mutable tag or branch rather than a commit digest; one toolchain action floats on a branch. |
| Toolchain drift hardcoded | A now end-of-life Node major is hardcoded, conflicting with the imported guide's own major (REQ-CI-03). |
| Simulated boot output pulled into CI | The boot step executes the imported integration script, which prints boot, measurement and integrity success without starting an emulator (REQ-BOOT-02). |
| No pinned toolchain for the image job | The Rust channel resolves at run time and image packages install unversioned from the runner distribution, so the job is not reproducible. |
| No eBPF gate | No job compiles or verifier-loads any of the three eBPF programs, although the workflow calls itself an invariant check. |

Replacement: No whole-file replacement: the Rust gate is authorable at M02, the
accessibility gate at M04, the eBPF gate at M19, and image synthesis with a real
boot gate at M11.

### imported release and attestation workflow (`export-052`, `78920f933421`)

Claims: On any version tag push: build, synthesize a boot image, sign it,
generate provenance and publish a non-draft, non-prerelease public release with
the image, its signature and an integrity artefact.

| Defect | Evidence |
| --- | --- |
| A tag push alone can create a release | No manual gate, no environment approval and no artefact precondition; release delivery is blocked until M13 (D11). |
| Publishes immediately and publicly | Draft and prerelease are both false with generated notes, so a single accidental tag becomes a real published release (REQ-REL-01). |
| Unverified third-party action with write permissions | The toolchain action namespace resolves to HTTP 404 inside a job holding contents, identity-token and packages write (REQ-REL-02). |
| Unpinned third-party actions | Every action floats on a mutable tag, including the reusable provenance workflow and the action that creates the release. |
| Excessive standing permissions | Write permissions are granted at workflow level to every step, including the unverified action, rather than scoped to the publishing step. |
| Signing without verification | Keyless signing is enabled through an experimental flag; no signature is verified, no certificate identity policy is asserted and no signer attestation is retained. |
| Provenance that cannot be valid | The subject list is a single hash over a file set where encoded per-artefact digests are expected, and a reusable workflow is invoked from a step position. |
| Publishes an artefact never produced | The release includes an integrity artefact that no step in the workflow produces, and no boot or measurement evidence is collected anywhere. |

Replacement: M13 (release signing and remote delivery), and not before an M11
artefact exists and the unverified action reference is replaced with a
digest-pinned, verified action.

### imported integration and sanity script (`export-046`, `9d566b66ab93`)

Claims: States that it evaluates partition geometries, boot of the unified image
and daemon interfaces, and validates a headless boot with measurement, integrity
and volume unlock.

| Defect | Evidence |
| --- | --- |
| Simulated boot and measurement output | The branch prints a simulation notice and sleeps; no emulator, firmware, TPM emulator or container runner is invoked anywhere in the file. |
| Boot and measurement success printed | A boot success line and specific measurement register values are printed although no TPM was read (REQ-BOOT-02). |
| Integrity verification printed | A root-hash verification line is printed with no hash computed and no device mapped. |
| Volume unlock printed | A policy-based volume unlock line is printed with no volume and no policy evaluated. |
| Success printed without executing the check | A loop prints a pass line for each of nine hardcoded crate names; no path is inspected and no build tool is invoked, so it would pass in an empty directory. |
| File existence treated as verification | Presence of one definition file is reported as geometry verified; contents are never parsed and three further definitions are never checked. |
| Unconditional green exit | The script always reaches a success banner and exits zero; the advertised daemon interface evaluation is never implemented. |

Replacement: Split: offline definition validation with real exit codes at M03; a
real boot check with retained emulator logs and measurement readback at M11;
signing at M20. Until then no gate may reference it.

### imported build orchestrator (`export-009`, `3068c85b768a`)

Claims: Targets for build, eBPF, UI, test, lint, format, accessibility, image
and boot, advertised as enforcing Power-of-Ten build invariants and zero-drift
compilation.

| Defect | Evidence |
| --- | --- |
| Simulated output inside the test gate | The test target runs the simulated integration script, so a green test run includes fabricated boot and measurement passes (REQ-P14-08). |
| Hardcoded identity presented as determinism | A fixed epoch and an all-zero seed are baked into the image target, which makes every build produce identical partition identifiers on every machine. |
| Unpinned dependency resolution | The UI build re-resolves and mutates the lockfile instead of installing from it, contradicting the zero-drift claim in the same file. |
| Incomplete eBPF gate | Only one of the three imported eBPF programs is compiled, and no verifier load is attempted. |
| Unreachable boot target | It boots an image path that no target in the file produces and passes an SSH key into the guest, an undeclared credential path into a supposedly immutable image. |
| Unpinned toolchain | The build, compiler and image tools are taken from the path with no version pin and no availability check. |
| No verification or governance gate | No target runs the repository's verification, audit or context-compile checks, so adopting the file would replace the governance gate. |

Replacement: Target by target: Rust targets at M02, accessibility at M04, a
complete eBPF target with verifier load at M19, image and boot targets at M11.
The tracked Makefile stays authoritative.

### imported repository population script (`export-057`, `76e31b8e4e4b`)

Claims: States that it initialises the canonical directory layout, places all
scaffolded files into it, and then prints a completion banner.

| Defect | Evidence |
| --- | --- |
| Failure suppression | The final permission change ends in a true fallback; under strict error handling it is the only guarded command, so the suppression is deliberate. |
| Success printed without executing the work | Every copy is guarded on source existence, with no counter, manifest check or digest verification, so the banner prints whether it copied fifty files or none. |
| Self-copy at the repository root | Five copies have identical source and destination when run from the root, which the copy tool rejects, so the run aborts mid-population or silently does nothing. |
| Overwrites governance configuration | It copies proposal standards data over the tracked governance configuration with no diff, review or backup. |
| Activates both quarantined workflows | It installs the failure-suppressing CI and the tag-triggered publishing workflow into the workflow directory in one unreviewed step. |
| References a source that does not exist | It copies a second root partition definition that exists nowhere in the import set; the existence guard swallows it and the A/B scheme is created without a B slot. |
| Creates unmanifested crate skeletons | It creates twelve crate source trees with no manifests, no workspace root and no lock, producing a tree that cannot build. |

Replacement: None: superseded outright. Each component's directory, manifest,
lock and interface contract are created by hand at the milestone that activates
it. Bulk population by script is not reintroduced.

### imported checkpoint ledger module (`export-040`, `84b1ca13cc33`)

Claims: A deterministic hash-chained audit trail said to enforce Power-of-Ten
invariants for experiment and candidate auditing, appending records linked by a
previous-record hash.

| Defect | Evidence |
| --- | --- |
| Claim contradicted by the code | The header advertises one hash algorithm and the implementation computes another; this is the ledger-hash contradiction recorded as DSP-07. |
| No algorithm boundary | The algorithm is hardcoded at the call site with no trait or injection point, so the boundary D02 requires does not exist. |
| Hardcoded system path | A system-wide absolute path is the default constructor argument; it needs elevated rights to create and is not configurable per environment. |
| Chain trusted blindly | Only the last line is parsed and its self-declared hash is adopted as the chain head; nothing recomputes a record hash or walks the chain. |
| Unhandled errors | File read, parse and property access are unguarded, so a truncated final line, the normal outcome of a crash mid-append, throws out of the constructor. |
| Non-atomic append and no signing | Appends without locking or flushing, so concurrent writers interleave; records carry no signature and the encoding order is never pinned. |
| No tests | No positive, negative or boundary test accompanies a public interface, so the three-dimensional testing invariant is unmet. |

Replacement: The trait boundary and D02 land at M02; a corrected ledger with
chain verification, canonical encoding, injected storage and three-dimensional
tests at M05; signed records at M14 and M20.

### imported accessibility specification (`export-023`, `a0d06b6b8e6c`)

Claims: An accessibility audit suite named for a European conformance standard:
a rule-engine scan asserting zero violations, and a focus-indicator test named
for a contrast ratio and a stroke width.

| Defect | Evidence |
| --- | --- |
| Conformance reported without measuring it | The focus test reads style, width and colour but asserts only that the outline style is not none; contrast is never computed and width is never compared. |
| Assertion weaker than the claim | The single substantive assertion discards the two other collected properties, so the D16 width boundary is not implemented. |
| Hardcoded development origin | Both tests navigate to a hardcoded local development origin with no managed server, fixture or readiness gate, so in CI the load fails and the suppressed job still reports a pass. |
| No negative or boundary case | Nothing asserts that removing the focus outline fails the suite, and no case exercises the minimum width, which M04 requires. |
| Coverage narrower than the conformance claim | One route in its default state is scanned behind a single selector, while conformance is claimed for the whole shell. |
| Unpinned toolchain | The test runner and rule engine are imported with no version pinned anywhere in the import set. |

Replacement: M04 (accessibility harness): headless against a managed server,
zero violations on the default state, failing when the focus outline is removed,
and implementing the D16 boundary.

### imported action-interceptor test module (`export-047`, `971948673346`)

Claims: Seven Rust unit tests covering risk-tier routing, maker-checker
separation, two-checker enforcement for high-risk actions, decision timeout
fail-closed behaviour and a global killswitch.

| Defect | Evidence |
| --- | --- |
| Tests its own mock, not product code | The engine and every type are defined inside the test-only module, so nothing ships and a release build compiles none of it; M02 requires library code. |
| Forbidden unwrapping | Six unwrap calls on clock durations and a result; a pre-epoch clock would panic the interceptor instead of failing closed, against the error-handling invariant. |
| Fail-open boundary | Expiry uses a strict comparison, so a request due in the current second is still approvable; the boundary fails open, and no test covers the equality case. |
| Missing boundary test | The only expiry case is one second past due; nothing exercises the equality edge that the strict comparison leaves open. |
| Warning hygiene | Two imports are unused, which alone fails the required deny-warnings lint gate. |
| Decision request never persisted | The constructed request is bound to a discarded name and dropped, so the returned identifier refers to no record and the approval path is unreachable from interception. |
| Test-only constant left in the engine | A short approval window is hardcoded into the engine with a comment admitting it is for testing; the timeout is policy and belongs in configuration with its own bounds. |
| High-risk path not enforced at interception | The tier match never branches on the high-risk flag, so a high-risk action declared at the lowest tier is simply allowed and the two-checker rule is never consulted. |
| Missing coverage | The killswitch test covers only the lowest tier; no test covers audit-record emission or hash chaining, and no hashing or signing boundary exists. |

Replacement: M02 (P06 promoted end-to-end): library code in the crate behind a
workspace root and a committed lock, no unwrapping, equality failing closed, and
three-dimensional tests under the lint gate.

The defects chain rather than standing alone. The population script installs
both defective workflows in one unreviewed step; the continuous-integration
workflow then executes the integration script, which prints boot, measurement
and integrity success without starting an emulator; and the release workflow
publishes on any version tag. Running the population script once would therefore
produce a pipeline that reports evidence it never gathered and can publish on a
mistyped tag. That is the concrete reason it must never be executed.

Two artefacts report conformance they never measure, which is worse than plain
suppression: a suppressed command at least leaves a failing step in the log. The
accessibility focus test is named for a contrast ratio and a stroke width but
asserts only that an outline exists, and the integration script prints a pass
line for each of nine hardcoded names with no filesystem or build check at all.

## Open decisions carried forward

Numbering continues from D20 in the roadmap overview. Thirty-five decisions are
carried forward: five from the inventory, fourteen from the contradiction
register, nine from the drift register, six from the quarantine register, and
one version floor left open by D07. Each names both sides, the contract it
affects and the milestone that must close it.

One of the five inventory decisions, D22, has since been settled: milestone M17
took the recommended option and the resolution is recorded in place below. The
entry stays listed and the numbering is unchanged, so a reader who followed a
D22 citation from elsewhere still lands on it.

### From the inventory (D21-D25)

- **D21** What are the crate names and paths for the two crates D15 authorises,
  the P01/P02 definition parser and the P02 A/B lifecycle? Options: two separate
  crates, mirroring the one-crate-per-subsystem naming of the ten proposed
  members; one shared crate holding both, since both read the same definition
  files. Affects `crates/README.md`, the future workspace member list, and
  milestones M03 and M15, which cannot write manifests until the path is
  recorded. Recommended: two separate crates. D15 is worded in the plural, and
  M03 and M15 are separate milestones with separate test obligations; one crate
  would couple their activation gates.
- **D22** Do P03 `aegis-vulcan` and the Rust half of P15 `aegis-hestia` join the
  workspace member list, or stay outside it? Options: extend the list to twelve
  members so a workspace-wide build covers every Rust candidate; keep both
  outside, because the proposal manifest (`export-006`, `6e694e01e136`) omits
  them and the imported directory map (`export-007`, `84f43472c536`) maps
  neither. Affects REQ-WS-01, the workspace manifest M02 must create, and the
  typed boundary D09 gives P15. Recommended: extend to twelve. REQ-WS-01 records
  the omission as a defect, not as an intent, and D09 already commits P15 to
  having a Rust crate. **Settled at M17:** the recommendation was taken. The
  repository's workspace member list is written out and now names
  `crates/aegis-vulcan` and `crates/aegis-hestia` alongside the three crates
  already activated, so `cargo build`, `cargo test` and `cargo clippy` at the
  workspace root reach both. Twelve members remains the eventual shape; five
  exist, because a directory joins the list when it has a manifest, a lock
  entry and positive, negative and boundary tests, and not before.
- **D23** Should `planning/components.json` candidate sources be extended to the
  declarative and eBPF artefacts that the same components already depend on?
  Options: extend, so that P01 and P02 cite the image and partition definitions,
  P07 and P13 their eBPF programs, and P05 and P12 the accessibility
  specification; leave as is, since those artefacts are covered indirectly by
  the prose and the import record. Affects the acceptance rule that a candidate
  without a source hash fails review, and the activation checklists that read
  from it. Recommended: extend. P01 and P02 currently carry a report only, while
  their recorded first slice depends on the definition files by name, so exactly
  those artefacts lack the hash binding the rule requires. This register already
  lists them.
- **D24** How is the absence of blueprint reports for P09, P13 and P16 handled
  before those components are activated? Options: accept the gap and derive
  requirements from the directory map and the subsystem graph; block activation
  until a report is authored in-repository as public planning text. Affects the
  requirement rows for those three subsystems and milestones M06, M08 and M16.
  Recommended: accept the gap at M01 but record it per component, and require
  the missing requirements to be written as public planning text before each
  component's own milestone opens. The bundle demonstrably cannot supply the
  reports.
- **D25** Which repository identity and version do the first committed manifests
  carry? Options: the proposal values, a lowercase repository path and version
  1.0.0; the repository's own values, the declared origin and the tracked
  `VERSION` file. Affects the workspace package metadata, REQ-GOV-02 and the M00
  evidence line that already flags the letter-case divergence. Recommended: the
  repository's own values. The case difference redirects silently rather than
  failing, and the repository has never released 1.0.0; both proposal values are
  recorded as drift above.

### From the contradiction register (D26-D39)

- **D26** Which transport carries the P06/P09 action gate: the eBPF program, a
  bus interface, a Unix domain socket, or an eBPF ring buffer? Three sources
  state three different pairs; all three agree on the sub-500 microsecond
  budget. Affects the wire contract typed at M14. Recommended: a bus interface
  for the proposal and decision round trip plus an eBPF map update for the
  kernel-side allow, which is the union one architecture document already
  describes end to end; record the socket form as superseded. Close alongside
  D03.
- **D27** Which direction does the code-CAD verification edge run: P09 calls
  P14, or P14 calls P09? The graph of record and one architecture document
  disagree; the other has no such edge. Affects the verification request and
  result schema and solver ownership (M06, with the P14 slice at M08).
  Recommended: apply the shape of D03 and D05. Keep the graph's edge identifier
  and direction, and state the call semantics explicitly: Minerva submits a
  parametric script and Hephaestus returns a constraint verdict.
- **D28** Does the graph of record gain a P15 to P02 storage edge for the
  database subvolume? One architecture document draws it, the graph does not,
  and the third source supports the dependency in prose only. Affects the Hestia
  storage contract against the var-partition definition (M17, with definitions
  at M03 and the lifecycle at M15). Recommended: handle it as D04 handled the
  analogous case. Do not change the graph at M01; require the M17 slice to
  declare its storage dependency and let that evidence decide. The subvolume
  already exists in the partition definition, so the dependency is real even if
  the edge is not.
- **D29** Which direction does the capture edge run between P08 and P11? Affects
  the buffer-sharing capture contract and the owner of the sub-5 millisecond
  encode budget (M07, M08, with hardware evidence at M12). Recommended: keep the
  graph's direction, with the consumer annotated as the requester, mirroring
  D05. The payload text also needs rewording when typed, because D12 excludes
  the platform it names as the consumer.
- **D30** Which direction does the accessibility-enforcement edge run between
  P12 and P15? Affects ownership of the conformance assertion gate for the
  Hestia surface (M04, M17). Recommended: resolve consistently with D05. P12
  produces tokens and assertions and P15 consumes them; keep the graph's edge
  identifier with the direction annotated, recorded as an explicit extension of
  D05 so the two token edges cannot drift apart.
- **D31** Is the P13 to P05 telemetry figure a delivery deadline or an emission
  interval? One source states a sub-2 millisecond figure, another a one-second
  interval, and the graph's node budget agrees with the interval. Affects the
  signal contract and the shell status-bar refresh assertion (M05 producer, M16
  consumer). Recommended: record both as distinct fields rather than choosing
  between them, so neither source is silently dropped.
- **D32** What are the P04 to P05 endpoint and budget, given that only one of
  three sources states either? Affects the compositor and shell IPC contract
  (M07 producer, M16 consumer with stubbed IPC). Recommended: adopt the stated
  socket path and the sub-100 microsecond figure as the IPC hop budget, and keep
  the frame envelope as a separate budget the shell must meet. Record the third
  source's silence as an omission, not a rejection.
- **D33** Is the P06 to P05 decision-request dispatch inside the sub-500
  microsecond interception gate, and what is its deadline? Affects the versioned
  decision-request schema (M14, consumed at M16). Recommended: separate the two
  budgets. The interception gate closes within its own budget by escalating, and
  the dispatch to the shell is an asynchronous follow-up with its own deadline.
  That keeps both figures true and keeps the human-oversight prompt off the
  gate's critical path.
- **D34** What endpoint and latency does the P09 to P10 capsule contract use?
  The graph names a transport family with a cold-boot node budget; another
  source names a concrete address with a warm-call budget. Affects the capsule
  dispatch contract (M06). Recommended: pin the concrete address and record two
  budgets, cold boot and warm call. Drop the runtime name from the transport
  string, since D06 already selected a Rust-native runtime.
- **D35** Does the P07 to P09 swap trigger cross a subsystem interface, or is it
  a scheduler-only action with no agent-visible call? Affects the pause and swap
  control contract (M07, with GPU evidence at M12). Recommended: require an
  agent-visible pause call in addition to the allocator action. An inference
  pause the agent daemon never observes cannot be tested, and the payload text
  names the pause explicitly. Treat the hardware-only arrow as an abstraction of
  the same mechanism.
- **D36** Does P03 expose a request interface to P09 for weight streaming, or
  only program a hardware path P09 observes? Two of three sources agree on the
  subsystem edge. Affects the streaming request contract (M17, with DMA evidence
  at M12). Recommended: keep the graph of record and require a thin request
  interface so the path is testable from the consumer; treat the hardware-only
  view as a layering diagram. Low priority: close with M17 rather than ahead of
  it.
- **D37** Are the eight graph-only edges confirmed, or dropped from the graph of
  record? They are declared edges with no supporting mention in either
  architecture document. Affects eight separate interface contracts across M05,
  M07, M08, M14 and M17. Recommended: confirm all eight by default, since the
  graph is the graph of record and single-source support is not a contradiction,
  but mark each as single-source and unconfirmed so the owning milestone must
  produce a real test before the edge counts as connected. Only one is currently
  flagged this way; extend the flag to the other seven, and to the five edges
  DSP-20 records.
- **D38** Does the graph of record gain hardware, kernel and transport nodes,
  and if not, who owns the attestation and sidecar dispatch contracts? The graph
  declares sixteen subsystem nodes; one architecture document adds hardware and
  transports as first-class nodes and reassigns both owners. Affects the
  attestation contract (M20) and the sidecar dispatch contract (M07).
  Recommended: keep the sixteen-node shape, because the contracts are written
  against subsystems, and resolve ownership explicitly: P02 brokers attestation
  quotes for P06, and P04 owns sidecar dispatch. Record the hardware nodes as a
  substrate view.
- **D39** Does the var partition carry the fifth Btrfs subvolume that one source
  lists and another omits? Affects the partition definition validated offline at
  M03 and the A/B lifecycle at M15; the same list carries the subvolume the
  Hestia storage dependency rests on. Recommended: keep the machine-readable
  definition as authoritative, since it is what the M03 gate parses, and treat
  the prose list as an incomplete summary. Record the divergence so a later
  removal is a decision rather than drift. Low priority.

### From the drift register (D40-D48)

- **D40** How is the unresolvable kernel-space eBPF dependency replaced: adopt
  the renamed successor crate, or drop the Rust kernel-side path and compile the
  three eBPF programs with a C toolchain? Affects the M19 toolchain admission
  and the unsafe-proof obligations on whichever side owns kernel-space code.
  Recommended: keep the C toolchain for M19, because the three imported programs
  are already C and M19's cheapest exit is compiling what exists; revisit the
  Rust path only if a Rust-side map definition is needed. Either way the
  unresolvable dependency line must be deleted, not bumped.
- **D41** Does the workspace adopt the current error-derive major and the newest
  stabilised Rust edition at M02, or keep the proposal values? Affects the
  workspace manifest, which propagates unchanged to every later Rust milestone.
  Recommended: adopt both at M02. Nothing depends on the old values yet, and
  this is the cheapest moment in the roadmap to make either change. Extending
  D10's latest-stable-at-activation rule to the Rust side would settle this
  class of question once instead of per crate.
- **D42** Which runtime major does the UI toolchain admit at M04, given that the
  two imported sources disagree and both named majors are now unsupported or in
  maintenance? Affects the M04 admission and M16, which consumes it.
  Recommended: the current active long-term line, per D10. The CI major is end
  of life and is not a candidate at all; the guide major buys nothing over the
  current line and expires sooner.
- **D43** Which package manager does the UI use? The imported guide prescribes
  one and the imported CI and Makefile assume another, and the choice determines
  which lockfile format M04 commits. Recommended: defer to whatever the shared
  template matrix supplies for this framework, since D10 already routes the
  stack toward the shared UI framework; choosing independently would create a
  second drift axis. If the matrix is silent, take the guide's choice and delete
  the contrary assumptions rather than carrying both.
- **D44** Does the repository adopt commit-digest pinning for third-party
  workflow actions, or keep major-tag references? Affects M13 and, by precedent,
  the repository-owned preparation gate. Recommended: digest-pin everything
  except the reusable provenance workflow, whose own documentation prescribes a
  semantic tag. That is a split policy, so it must be written down once at M13
  rather than rediscovered per workflow. Four of the six imported references are
  majors behind current and one resolves to a 404.
- **D45** Is the case-mismatched project identity fixed only in the first
  committed manifest, or swept across every identity surface? Affects
  REQ-GOV-02, crate metadata, documentation links and release metadata.
  Recommended: fix it in the manifest and record the canonical identity here,
  and do not edit the imported sources, which are immutable proposal data. The
  mismatched path redirects silently, so nothing will fail later to catch it.
- **D46** Does the thread-per-core runtime mandate survive, given that the
  mandated runtime's newest release is from August 2024? Affects M06 and M07,
  and the shape of the deadline and timeout obligations, which differ between
  runtimes. Recommended: record the maintenance gap as an explicit risk now and
  decide at M06, where the first thread-per-core code is written; M06 admits no
  asynchronous I/O work anyway. Do not silently inherit the proposal pin.
- **D47** Does the image definition pin a distribution snapshot, or does Aegis
  stop owning the package list and hand it to the image producer? D18 already
  chose the snapshot; the open part is whether the imported definition file is
  carried forward at all. Recommended: author the package list fresh at M18
  against the current decisions. The imported file still names the compositor
  library ADR-0001 superseded and hardcodes a distribution-specific firmware
  path, so it is requirements input rather than a file to inherit.
- **D48** Does the reasoning behind ADR-0002 also exclude the chat-platform
  presence integration in P11? Options: exclude it as a proprietary,
  network-dependent integration in a local-first system with no library named
  and no version verifiable; keep it as local inter-process communication to a
  user-installed client rather than a service dependency of the OS. Affects the
  rich-presence contract M08 currently requires. Recommended: decide before M08,
  because the milestone's exit criteria require typing a contract that may not
  survive the decision. If it is kept, a concrete library must be named first,
  since the sources name none.

### From the quarantine register (D49-D54)

- **D49** Is the hash-chained checkpoint ledger re-authored in Rust inside the
  shared workspace, or kept in its imported language behind a typed boundary?
  Affects the signed audit-record schema (M14) and the evolution-loop ledger
  (M05); a cross-language ledger forces a canonical encoding to be pinned before
  any hash is computed. Recommended: a Rust crate. Every consumer the roadmap
  names is Rust, the D02 trait boundary is created in Rust at M02 regardless,
  and a second language inside the audit path would need its own pinned
  toolchain admission.
- **D50** Does the corrected interceptor fail closed when a decision is due in
  the current second, changing the imported strict comparison to an inclusive
  one? Options: fail closed at equality, which is what M02's exit criteria
  already require and what the imported error text implies; keep the strict
  comparison, the ordinary reading of a deadline. Affects the decision-request
  schema and its boundary tests (M02, M14). Recommended: fail closed at
  equality, and record the one-second clock granularity as a separate
  limitation.
- **D51** Where does the approval timeout come from, given that the imported
  engine hardcodes a short test window? Options: per-tier configuration carried
  in the request and validated against compile-time bounds; a compile-time
  constant per tier with no runtime surface. Affects the decision-request schema
  and the M02 boundary tests. Recommended: configuration with hard compile-time
  bounds. The window will differ between high-risk classes, but an unbounded
  configuration value is itself an oversight bypass.
- **D52** Does the high-risk regulatory flag escalate the risk tier at
  interception, or only tighten who may approve at decision time? Affects the
  action-proposal schema (M14) and the decision logic (M02). Recommended:
  escalate at interception. As imported, a high-risk action declared at the
  lowest tier is allowed outright and never reaches the two-checker path at all,
  which defeats the control that rule exists to provide.
- **D53** Is the image determinism seed a fixed constant, or derived per build?
  Affects the M18 product input manifest and the M09 producer contract; the seed
  determines partition identifiers that the A/B lifecycle observes. Recommended:
  derive it from the released revision. An all-zero seed makes every
  installation share partition identifiers, which is a hardcoded identity rather
  than reproducibility. Decide before M09, since the seed policy is part of what
  the producer is asked to reproduce.
- **D54** When the release workflow is authored at M13, is publication gated on
  a protected environment with manual approval, or on a tag push alone? Affects
  the M13 exit criteria, the hosted readback recorded under D11, and the
  permission scoping, which currently sits at workflow level. Recommended: a
  protected environment plus an explicit dispatch trigger, with write
  permissions scoped to the publishing step only. The repository is public with
  a protected default branch, and M13 is where an irreversible external artefact
  first appears.

### Left open by an earlier decision (D55)

- **D55** What kernel version floor does the image require, given that D07
  settled ownership but not the version? One report and one architecture diagram
  name a floor plus a target release that upstream has not released; the image
  definition names a package with no version at all. Affects the kernel
  requirement payload authored at M18 and re-verified at M10. Recommended:
  express the requirement as a feature payload rather than a version list, as
  M18 already requires, and record the observed kernel version and configuration
  digest as facts at M10 instead of pinning an unreleased target.
