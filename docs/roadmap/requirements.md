# Aegis OS activation specification

Status: draft requiring review

## Requirements

Each requirement cites a source id and the first 12 hex characters of its sha256; the verbatim quote is held privately. Requirement text is limited to what the cited span states. Repository facts and inferences are kept in the roadmap and plan prose, not in the requirement text.

### P01 aegis-fabrica

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-P01-01 | mkosi.conf selects the Arch distribution with Release=latest and UKI output aegis-os-v1.0.0.efi for the Fabrica image; the distribution release is therefore not pinned. | export-049 4d9043f4af30 |
| REQ-P01-02 | The root partition is an EROFS image with dm-verity data integrity, forming the immutable A-slot. | export-054 41d6d121b142 |
| REQ-P01-03 | The /var partition is a TPM2-sealed encrypted Btrfs volume, not part of the read-only root image. | export-055 c915f281532f |
| REQ-P01-04 | sysupdate targets a dual-slot A/B root transfer, writing a read-only erofs image into the alternate slot. | export-063 b284bb77c19f |
| REQ-P01-05 | The UKI chain of trust uses a TPM2 PCR measurement strategy: PCR 0 for UEFI firmware, PCR 4 for the UKI binary, PCR 7 for Secure Boot policy and PCR 11 as the dedicated UKI verification slot. | export-010 1748925bf81e |
| REQ-P01-06 | The imported CI workflow synthesizes the UKI by running mkosi against build/mkosi.conf; the workflow is inactive proposal data. | export-041 e05ddc9466fc |
| REQ-P01-07 | The proposed Cargo workspace declares the aya crate at version 0.12 under its eBPF and kernel-bypass dependencies; imported versions are proposal data, not active pins. | export-006 6e694e01e136 |
| REQ-P01-08 | The developer toolchain contract pins a minimum mkosi version for image synthesis. | export-007 84f43472c536 |
| REQ-P01-09 | The mkosi content list pins the Arch linux-rt package as the boot kernel, which conflicts with the shared-stack rule that Nucleus produces the kernel artifact; the boot kernel identity is an open decision. Resolved by D07: distribution linux-rt by default, Nucleus artifact overrides. | export-049 4d9043f4af30 |
| REQ-P01-10 | The ESP partition definition bounds the partition size between 512M and 1G, giving a concrete boundary case for repart validation. | export-053 d85a1f6ceb0e |

### P02 aegis-janus-vallum

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-P02-01 | dm-verity root-hash verification must ensure the EROFS root filesystem is bit-for-bit identical to the signed release before it is treated as trusted, preventing corrupted or unauthenticated state from being hardened into the execution path. | export-011 528da609dab7 |
| REQ-P02-02 | The stateful /var LUKS2 volume must be enrolled to the TPM2 device bound to PCR 0, 4, 7, and 11, per the documented systemd-cryptenroll invocation. | export-011 528da609dab7 |
| REQ-P02-03 | The var partition definition declares TPM2-bound encryption (Encrypt=tpm2), a 10G minimum size and Btrfs subvolumes @var, @var-log, @flatpak, @containers and @pglite. | export-055 c915f281532f |
| REQ-P02-04 | The sysupdate transfer target is a partition spanning both root slots (root-a and root-b), not writable, erofs-formatted, with mode 0444. | export-063 b284bb77c19f |
| REQ-P02-05 | P02's security model must resolve the stated trade-off between Reversible Structural Consolidation (mature security structures reopenable for bounded maintenance) and Permanent Hardening (locks paths indefinitely, risking uncorrectable 'False Maturity'). | export-004 15831276a058 |
| REQ-P02-06 | Developer environment setup for P02-adjacent image synthesis requires systemd-ukify, systemd-repart, and mkosi version 24 or newer. | export-007 84f43472c536 |
| REQ-P02-07 | The root-a partition definition declares Verity=data, tying the EROFS root slot to its dm-verity data role. | export-054 41d6d121b142 |
| REQ-P02-08 | If the hardware watchdog expires before systemd-bless-boot sends the bless signal, an automatic A/B rollback is triggered. | export-011 528da609dab7 |

### P03 aegis-vulcan

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-P03-01 | Project Vulcan operates under an engineering mandate that restricts the whole-subsystem power budget to 20 watts. | export-012 9b502c76509b |
| REQ-P03-02 | The Vulcan IOMMU subsystem must enforce six named boundaries (Scope, Budget, Sensed State, Causal Operation, Topology, Reversibility) to maintain protection-domain integrity. | export-012 9b502c76509b |
| REQ-P03-03 | Vulcan uses Peer-to-Peer (P2P) DMA specifically to eliminate CPU host RAM staging and kernel block layer overhead on the NVMe-to-GPU path. | export-012 9b502c76509b |
| REQ-P03-04 | Mapping a PCIe BAR into user space asserts that the BAR physical address is a multiple of BAR_ALIGNMENT_BYTES (page aligned). | export-038 fe2cb01cfd13 |
| REQ-P03-05 | Direct DMA dispatch asserts that block_count is greater than zero and at most 8192 blocks. | export-038 fe2cb01cfd13 |
| REQ-P03-06 | P03 aegis-vulcan carries an isolation model of VFIO Group Isolation / IOMMU Domain and a latency budget under 10 microseconds for Direct DMA. | export-062 1ce919ed54bb |
| REQ-P03-07 | The subsystem graph's GPUDIRECT_WEIGHT_STREAMING edge carries zero-copy NVMe-to-GPU VRAM DMA weight loading over PCIe P2PDMA / CUDA GPUDirect. | export-062 1ce919ed54bb |
| REQ-P03-08 | The proposed Cargo workspace declares the memmap2 crate at version 0.9 under its eBPF and kernel-bypass dependencies. | export-006 6e694e01e136 |

### P04 aegis-compositor

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-P04-01 | Mercurius rejects libweston and Smithay for the fast runtime loop and mandates a C-based wlroots implementation to control the event loop. Superseded by ADR-0001 (pure Rust compositor). | export-013 7f4c22813332 |
| REQ-P04-02 | Tier 2 of the agent mesh is Eclipse Zenoh pub/sub with shared-memory backends targeting 5-35us latency and 50+ Gbps throughput. | export-013 7f4c22813332 |
| REQ-P04-03 | Tier 2 must use immutable-read controls to prevent retroactivity so a newly attached agent cannot back-act on upstream sensorimotor state. | export-013 7f4c22813332 |
| REQ-P04-04 | The compositor scaffold bounds the surface registry at 256 concurrently registered Wayland surfaces, labelled as a NASA JPL P10 compliance bound. | export-027 f19640d7a7da |
| REQ-P04-05 | The compositor systemd unit sets a cgroup memory limit of 256M (MemoryMax=256M). | export-028 d74a93eac654 |
| REQ-P04-06 | The compositor's user-space agent mesh uses a Modular RAG pipeline letting Retrieval, Rerank, and Memory modules be swapped per task demand. | export-004 15831276a058 |
| REQ-P04-07 | The shell protocol's focus-switch report carries the target cgroup slice so the compositor can drive resource re-allocation on focus change. | export-035 4c2146ffed32 |
| REQ-P04-08 | The compositor scaffold throttles its loop to a 100 microsecond frame time, which contradicts its own 144Hz pacing comment and the 1.5ms render budget; the pacing constant must be pinned. | export-027 f19640d7a7da |

### P05 aegis-forum-shell

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-P05-01 | Every process in the Forum shell must be assigned to a cgroup enforcing strict energy and memory limits within the 20-watt facility budget. | export-014 659691a2d7d0 |
| REQ-P05-02 | The shell requires a high-throughput IPC backbone synchronizing state between the Svelte/Wry frontend and the Rust/C backend. | export-014 659691a2d7d0 |
| REQ-P05-03 | Managed processes move through a typed lifecycle: Eligible but Inactive, Activated, Rate-Limited, Quarantined, or Deleted. | export-014 659691a2d7d0 |
| REQ-P05-04 | The shell must implement the coordination-without-a-privileged-clock D-Bus interaction model defined in Figure 10. | export-014 659691a2d7d0 |
| REQ-P05-05 | On mount the shell connects to the compositor over a Unix domain socket and to the StatusNotifierWatcher over D-Bus. | export-043 e3dbfa226e54 |
| REQ-P05-06 | Focus indicators in the shell UI must satisfy a minimum 3:1 contrast ratio and a 2px stroke. | export-023 a0d06b6b8e6c |
| REQ-P05-07 | Forum must scale cost with currently-utilized Active Capacity rather than Total Capacity, preventing idle energy bleed. | export-004 15831276a058 |
| REQ-P05-08 | The accessibility gate runs the Playwright axe-core suite from the ui/forum-shell package via 'npm run test:a11y'. | export-009 3068c85b768a |

### P06 aegis-justitia

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-P06-01 | Two-person maker-checker verification with TPM2-backed identity signatures is mandatory for high-stakes actions; no single identity may self-authorize a high-risk action. | export-015 fcbe2caed363 |
| REQ-P06-02 | The justitia-interceptor systemd unit sets IPAddressDeny=any, which the unit comment ties to the Zero-SaaS Sovereign App Engine requirement. | export-015 fcbe2caed363 |
| REQ-P06-03 | The engine must reject a decision when the primary reviewer is the same identity as the action's maker. | export-047 971948673346 |
| REQ-P06-04 | Annex III biometric high-risk actions require two distinct human checkers, neither of whom is the maker. | export-047 971948673346 |
| REQ-P06-05 | The kernel-space eBPF LSM hook must submit intercepted exec events into a ring buffer for evaluation by the aegis-justitia host daemon. | export-024 50a41ecd0b74 |
| REQ-P06-06 | The subsystem graph directs PROVIDES_TPM2_ATTESTATION from P02 Janus & Vallum to P06 Justitia over hardware TPM2 PCR, carrying PCR 0, 4, 7, 11 quotes and attestation keys for audit signing. | export-062 1ce919ed54bb |
| REQ-P06-07 | The developer guide maps crates/aegis-justitia/ to the EU AI Act Art 14 pre-execution interceptor, killswitch and TPM2 attestation. | export-007 84f43472c536 |
| REQ-P06-08 | Justitia enforces the normative baseline of German and EU law and treats accessibility (A11y-by-default) as a prerequisite for digital service sovereignty. | export-004 15831276a058 |
| REQ-P06-09 | The Justitia reference daemon computes its audit chain hash with MD5, contradicting the SHA-256/TPM2-RSA-PSS scheme in the same subsystem's schema; the ledger algorithm must be pinned before activation. | export-031 5ff683328148 |
| REQ-P06-10 | Justitia's product-liability mitigation targets the EU Product Liability Directive's stated effective date of 9 December 2026, an external deadline the roadmap must track. | export-015 fcbe2caed363 |

### P07 aegis-lictor

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-P07-01 | Lictor targets Linux 6.12+/7.3 and compiles for x86-64-v3/v4 with CONFIG_PREEMPT_RT and CONFIG_HZ_1000 for deterministic T0/T1 response. | export-016 cfa58b5b23b8 |
| REQ-P07-02 | isolcpus pins performance-critical threads (Wayland, Compositor) to physical cores as part of Locality Enforcement (P-002). | export-016 cfa58b5b23b8 |
| REQ-P07-03 | The lictor-d daemon is a zero-GC Rust service that uses the Aya eBPF library to manage the resource-broker mechanism record tuple. | export-016 cfa58b5b23b8 |
| REQ-P07-04 | scx_cake classifies tasks into 4 tiers by burst duration: Tier 0 Critical <100us covers the Wayland Compositor and PipeWire Audio on Priority Queue 0. | export-056 39af243568ad |
| REQ-P07-05 | Lictor uses eBPF-driven Fragility Probes to prevent Catastrophic Drift during kernel updates by testing shadowed structural changes in a non-authoritative path before promotion. | export-004 15831276a058 |
| REQ-P07-06 | The developer workflow compiles the kernel-space eBPF C programs scx_cake, kepler_power, and action_gate as a discrete build step. | export-007 84f43472c536 |

### P08 aegis-calliope

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-P08-01 | Calliope uses DMA-BUF zero-copy sharing between Looking Glass and OBS Studio over hardware-accelerated paths, avoiding CPU-side copies. | export-017 afbc0af8056d |
| REQ-P08-02 | PipeWire real-time audio threads must run with RLIMIT_RTPRIO = 95 as a non-negotiable priority setting. | export-017 afbc0af8056d |
| REQ-P08-03 | Third-party audio plugins must pass through a Quarantine stage of rate-limited, restricted execution before activation into the real-time graph. | export-017 afbc0af8056d |
| REQ-P08-04 | The subsystem graph directs SHARE_DMA_BUF_STREAM from P08 Calliope to P04 Mercurius over PipeWire pipewiresrc / SPA_DATA_DmaBuf. | export-062 1ce919ed54bb |
| REQ-P08-05 | The subsystem graph directs ENFORCE_REALTIME_RTPRIO from P07 Lictor to P08 Calliope over Linux cgroups v2 / RLIMIT_RTPRIO. | export-062 1ce919ed54bb |
| REQ-P08-06 | The Calliope scaffold bounds DMA-BUF buffers at 64, labelled as an explicit NASA JPL P10-2 upper bound. | export-026 c2f1e433cd32 |
| REQ-P08-07 | Calliope achieves audio/video synchronization via local phase and drift estimation instead of a privileged global master clock, to reduce synchronization energy cost. | export-004 15831276a058 |
| REQ-P08-08 | The crates/aegis-calliope Rust crate is the designated code location for PipeWire 5ms RTL audio tuning and zero-copy DMA-BUF video capture. | export-007 84f43472c536 |

### P09 aegis-minerva

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-P09-01 | The source candidate hard-codes a strict 20W power envelope budget as the biomimetic power cap constant. | export-034 213a95fc0d02 |
| REQ-P09-02 | The ALPS router must stay under a 1.5ms routing-overhead latency cap, encoded as a microsecond constant. | export-034 213a95fc0d02 |
| REQ-P09-03 | The subsystem graph directs ACTION_GATE_INTERCEPT from P06 Justitia to P09 Minerva over eBPF action_gate.bpf.c / D-Bus. | export-062 1ce919ed54bb |
| REQ-P09-04 | The subsystem graph directs GPUDIRECT_WEIGHT_STREAMING from P03 Vulcan to P09 Minerva over PCIe P2PDMA / CUDA GPUDirect. | export-062 1ce919ed54bb |
| REQ-P09-05 | The master architecture diagram documents Justitia intercepting Minerva's actions before execution. | export-003 13af15ffc316 |
| REQ-P09-06 | The trade-off guide states Minerva implements the 20W thesis by separating rapid acquisition (fast memory) from stable long-term structure (slow model). | export-004 15831276a058 |
| REQ-P09-07 | The comparative technical reference matrix records P09 Minerva's core architectural choice (Sparse Tiny Experts over Dense Foundation Models), primary impact (Efficiency), and regulatory constraint (Ethical AI). | export-004 15831276a058 |
| REQ-P09-08 | The developer guide maps the crates/aegis-minerva/ directory to the P09 Minerva Biomimetic AI subsystem, responsible for the 20W Alps SLM policy router, AgentHER trajectory relabeler, and Z3 solver. | export-007 84f43472c536 |

### P10 aegis-vesta

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-P10-01 | Vesta's MicroVM subsystem targets a <5MB memory footprint and <125ms boot time so many dormant agent sandboxes can exist without consuming energy until activated by the sparse router. | export-018 5dbc6d071bbb |
| REQ-P10-02 | All GPU/NPU workloads inside Vesta must go through the Venus protocol so accelerator memory movement is priced and tagged, preventing untraceable 'Memory Laundering' of data into the sandbox. | export-018 5dbc6d071bbb |
| REQ-P10-03 | Vesta's physical-computation boundaries for Firecracker/MicroVM integration include explicit pricing of all AF_VSOCK traffic. | export-018 5dbc6d071bbb |
| REQ-P10-04 | Vesta must use io_uring to reduce the memory-movement price of asynchronous sandboxing so it does not bottleneck the fast runtime loop. | export-004 15831276a058 |
| REQ-P10-05 | The aegis-vesta crate header lists Firecracker microVM sandboxing with under 125ms boot and under 5MB RAM footprint among its standards targets; the values are unmeasured. | export-037 ce490c88081f |
| REQ-P10-06 | Agent workload sandboxing under P10 combines Firecracker MicroVMs (sub-125ms boot, <5MB RAM) with capability-secure Extism/Wazero WebAssembly capsules. | export-003 13af15ffc316 |
| REQ-P10-07 | The subsystem graph directs SANDBOX_CANDIDATE_EVALUATION from P10 Vesta to P16 Athena over Firecracker MicroVM / AF_VSOCK, carrying isolated execution and verification of A/B system candidates. | export-062 1ce919ed54bb |
| REQ-P10-08 | The Vesta source candidate names Wazero, a Go runtime, as the Wasm capsule engine, which conflicts with the shared-stack rule that a Go library is not a direct dependency of a Rust daemon. | export-037 ce490c88081f |

### P11 aegis-ludus

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-P11-01 | bindgen wraps C++ Steamworks headers to isolate the Ludus runtime from the unstable C++ ABI and prevent hardcoded binary dependencies from breaking portability across Linux distributions. Superseded by ADR-0002 (no Steamworks in the image). | export-019 b0aa6e54ed57 |
| REQ-P11-02 | Launch-command-line tokens from GetLaunchCommandLine must be validated to mitigate command injection before entering the adaptation loop. | export-019 b0aa6e54ed57 |
| REQ-P11-03 | Video frames must remain in GPU memory (zero-copy DMA-BUF) during transition from compositor to encoder for Remote Play streaming. | export-019 b0aa6e54ed57 |
| REQ-P11-04 | Payment data and session keys must be sealed to specific TPM2 Platform Configuration Registers (PCRs). | export-019 b0aa6e54ed57 |
| REQ-P11-05 | Ludus's architectural stance is weak coupling for third-party IPC, treating any external binary connection as retroactivity risk to preserve producer stability. | export-004 15831276a058 |
| REQ-P11-06 | The subsystem graph directs ATTEST_GAME_TRANSACTION from P11 Ludus to P02 Janus & Vallum over a hardware TPM2 PCR quote, carrying PCR-sealed microtransaction receipts. | export-062 1ce919ed54bb |
| REQ-P11-07 | The Ludus scaffold bounds parsed launch command-line arguments with MAX_LAUNCH_ARGS set to 64. | export-033 531cbdf98eb5 |

### P12 aegis-concordia

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-P12-01 | Concordia gates UKI compilation on a 100% accessibility pass, running npm run test:a11y with Playwright and axe-core against EN 301 549 before release. | export-020 1748f49bb7f8 |
| REQ-P12-02 | Micro-frontends monitor org.freedesktop.portal.Settings via D-Bus so accessibility preference changes trigger immediate reactive UI updates without page refresh. | export-020 1748f49bb7f8 |
| REQ-P12-03 | Running locally on a sovereign desktop shell keeps AT-SPI2 D-Bus accessibility bridges available for assistive technology. | export-020 1748f49bb7f8 |
| REQ-P12-04 | Static accessibility validation runs a container image, referenced by a truncated digest, with networking disabled and the repository bind-mounted read-only. | export-020 1748f49bb7f8 |
| REQ-P12-05 | The focus-ring design token defines a 3px stroke width for the EN 301 549 Clause 11 focus indication rule. | export-042 411fb8c2d731 |
| REQ-P12-06 | The automated accessibility scan must check the Forum Shell UI against WCAG 2.x tags and assert zero violations. | export-023 a0d06b6b8e6c |
| REQ-P12-07 | Concordia targets coherent digital services across 22,000 public administrations through design tokens and a Bootstrap fork with better accessibility. | export-004 15831276a058 |
| REQ-P12-08 | The developer guide maps ui/concordia-tokens/ to P12 Concordia: EN 301 549 V3.2.1 / WCAG 2.2 AA design tokens and AT-SPI2 D-Bus bridges. | export-007 84f43472c536 |
| REQ-P12-09 | The trade-off matrix records Concordia's core choice as design tokens with monolithic CSS libraries as the discarded alternative, cognitive load as primary impact and EN 301 549 as constraint. | export-004 15831276a058 |

### P13 aegis-tellus

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-P13-01 | Tellus implements the ISO/IEC 21031:2024 SCI rate formula SCI = ((E*I)+M)/R in the daemon's core carbon-calculation method. | export-036 25813d240733 |
| REQ-P13-02 | The kepler_power eBPF probe monitors per-cgroup v2 CPU/DRAM RAPL and ACPI power consumption counters. | export-048 293357bac7d3 |
| REQ-P13-03 | The subsystem graph directs SPATIOTEMPORAL_TASK_SHIFT from P13 Tellus to P07 Lictor over D-Bus / kepler_power.bpf, deferring background builds and batch inference on high grid carbon. | export-062 1ce919ed54bb |
| REQ-P13-04 | The subsystem graph directs EMIT_CARBON_TELEMETRY from P13 Tellus to P05 Forum as a D-Bus signal on org.aegisos.Tellus1. | export-062 1ce919ed54bb |
| REQ-P13-05 | The subsystem graph directs EVALUATE_CANDIDATE_CARBON_SCI from P16 Athena to P13 Tellus using the ISO/IEC 21031:2024 SCI rate calculation. | export-062 1ce919ed54bb |
| REQ-P13-06 | Tellus enforces a bidding contract where modules bid against global energy prices based on their expected task-value improvement (Delta V). | export-004 15831276a058 |
| REQ-P13-07 | Section 18's comparative matrix records Tellus's chosen architecture as Kepler Power Telemetry (versus a discarded Estimated Power Model), with Calibration as primary impact and the SCI Framework as the regulatory constraint. | export-004 15831276a058 |
| REQ-P13-08 | The developer guide maps crates/aegis-tellus/ to Kepler eBPF power probes and the ISO/IEC 21031:2024 SCI carbon rate engine. | export-007 84f43472c536 |

### P14 aegis-hephaestus

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-P14-01 | Hephaestus uses a dual-engine approach, exact NURBS and polyhedral CSG, to close the Siemens Parasolid/ACIS dependency gap while keeping B-Rep fidelity and watertight meshes for solvers. | export-021 6a3cda152e6c |
| REQ-P14-02 | The Hephaestus scaffold bounds meshing at 500,000 elements to prevent out-of-memory panics. | export-029 427996186520 |
| REQ-P14-03 | Multi-physics solver execution (OpenFOAM, CalculiX, Elmer) must be isolated in cgroups v2 slices enforcing the architecture's 'Six Boundaries' resource-isolation contract. | export-021 6a3cda152e6c |
| REQ-P14-04 | Generated Code-CAD must pass Z3 symbolic solver verification as a hard gate before promotion, validating dimension validity and constraint margins. | export-021 6a3cda152e6c |
| REQ-P14-05 | The subsystem graph directs VERIFY_CODE_CAD from P09 Minerva to P14 Hephaestus over a Z3 Symbolic Solver / PAL Engine transport for parametric CAD constraint and dimension checks. | export-062 1ce919ed54bb |
| REQ-P14-06 | The subsystem graph directs RENDER_GEOMETRY_MICROFRONTEND from P14 Hephaestus to P15 Hestia over a Svelte 5 micro-frontend / PGlite OPFS transport for STEP AP242 and mesh viewports. | export-062 1ce919ed54bb |
| REQ-P14-07 | aegis-hephaestus is listed as a member crate of the proposed Rust workspace. | export-006 6e694e01e136 |
| REQ-P14-08 | The imported Makefile test target runs cargo test --workspace followed by bash integration_test.sh; the script step prints simulated output (REQ-BOOT-02) and is excluded from any activation gate. | export-009 3068c85b768a |

### P15 aegis-hestia

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-P15-01 | Hestia's WebKitGTK(wry) micro-frontend host targets a 30-50MB idle RAM footprint and ~0% idle CPU usage to minimize routing overhead vs Electron/browser stacks. | export-022 46cea660df63 |
| REQ-P15-02 | The Lictor Daemon's GPU priority tiers must guarantee VRAM residency and 0ms preemption latency for Hestia Media/UI (High Priority tier). | export-022 46cea660df63 |
| REQ-P15-03 | Background AI/RAG work is Low Priority under the Lictor scheduler and must be disallowed from VRAM during Hestia media playback (Substrate Competition), throttled by active energy price. | export-022 46cea660df63 |
| REQ-P15-04 | A Hestia feature can reach Established maturity status only after passing 6 of 9 machine-checkable structural promotion gates under Project Athena. | export-022 46cea660df63 |
| REQ-P15-05 | The PgliteVectorStore must assert it is initialized before querying, and the query limit must be strictly bounded to 1..=100 (NASA JPL P10 assertion density). | export-030 689d175667d6 |
| REQ-P15-06 | The trade-off guide claims that grounding agents in governed context via the Atlan MCP server improves agent SQL accuracy by 38%; the figure is an unverified proposal claim. | export-004 15831276a058 |
| REQ-P15-07 | P03 Vulcan feeds P15 Hestia zero-copy media ingest over a PCIe BAR / memory-mapped DMA transport (subsystem_graph.json edge). | export-062 1ce919ed54bb |
| REQ-P15-08 | The architecture document places stateful persistence on a LUKS2-encrypted Btrfs /var partition with @var, @var-log, @containers and @pglite subvolumes. | export-003 13af15ffc316 |

### P16 aegis-athena

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-P16-01 | Athena's scaffold header states a 7-step candidate lifecycle (Propose, Challenge, Decompose, Prove, Check, Publish, Invalidate) with BLAKE3 experiment ledger auditing and Pareto-gated optimization. | export-025 6b23723ddb76 |
| REQ-P16-02 | Candidate promotion is gated by a Pareto-superiority check across latency, memory, carbon rate, and null-model retention thresholds. | export-025 6b23723ddb76 |
| REQ-P16-03 | The checkpoint ledger module describes itself as a BLAKE3 hash-chained audit trail enforcing NASA JPL P10 invariants for AI experiment and candidate evaluation auditing. | export-040 84b1ca13cc33 |
| REQ-P16-04 | P10 Vesta performs isolated execution and verification of A/B system candidates for Athena over a Firecracker MicroVM / AF_VSOCK transport. | export-062 1ce919ed54bb |
| REQ-P16-05 | Athena uses reversible maturity gates to prevent 'False Maturity', where a structural change looks stable but fails under compositional stress or regressions. | export-004 15831276a058 |
| REQ-P16-06 | The architecture interface table links P16 Athena and P02 Janus through systemd-sysupdate over local D-Bus, asynchronously, for dual-slot A/B UKI update deployment. | export-003 13af15ffc316 |
| REQ-P16-07 | aegis-athena is a declared member crate of the Aegis OS Rust workspace. | export-006 6e694e01e136 |
| REQ-P16-08 | The imported Makefile lint target runs cargo clippy across the workspace and all targets with warnings denied. | export-009 3068c85b768a |
| REQ-P16-09 | The only ledger reference implementation hashes records with SHA-256 although the Athena sources name a BLAKE3 ledger; the algorithm must be pinned before the Rust port. | export-040 84b1ca13cc33 |
| REQ-P16-10 | The subsystem graph assigns Athena a 500ms candidate-check latency budget. | export-062 1ce919ed54bb |

### Boot evidence

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-BOOT-01 | The mkosi host configuration expects KVM-accelerated headless QEMU boot with OVMF firmware, so image boot evidence requires a KVM-capable host. | export-049 4d9043f4af30 |
| REQ-BOOT-02 | The imported integration script prints a UKI boot and TPM2 PCR measurement success message without executing any boot; it is simulated output and must never count as boot evidence. | export-046 9d566b66ab93 |

### Imported CI workflow (inactive)

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-CI-01 | The imported CI workflow suppresses failures of the repart schema validation step with '// true'; the imported workflow stays inactive and any activated gate must fail on analyzer errors. | export-041 e05ddc9466fc |
| REQ-CI-02 | The imported CI workflow suppresses failures of the accessibility suite with '// true'; an activated accessibility gate must not inherit this suppression. | export-041 e05ddc9466fc |
| REQ-CI-03 | The imported CI workflow installs Node.js 20 for the UI accessibility job, which conflicts with the developer guide's Node.js 22 LTS toolchain and must be resolved by a pinned UI toolchain selection. | export-041 e05ddc9466fc |

### Governance and identity

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-GOV-01 | The proposed Cargo workspace (export-006) declares EUPL-1.2 as the workspace package licence. | export-006 6e694e01e136 |
| REQ-GOV-02 | The proposed Cargo workspace declares repository https://github.com/cordanaLLM/aegis-os, which differs in letter case from the configured origin cordanaLLM/Aegis-OS; the identity must be reconciled. | export-006 6e694e01e136 |
| REQ-GOV-03 | The exported agent instructions require every agent execution path with side effects (file mutation, network transmission, configuration change, financial transaction) to pass through aegis-justitia pre-execution approval gates. | export-001 f32a74743af5 |

### Graph contradictions

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-GRAPH-01 | The dataflow architecture document draws the design-token edge from P12 Concordia to P05 Forum, opposite to the subsystem graph's P05-to-P12 edge; the direction must be reconciled before a contract is written. | export-002 7e0c95f4ea05 |
| REQ-GRAPH-02 | The dataflow architecture document asserts a P06-to-P10 syscall-intercept edge that the subsystem graph does not contain; whether Justitia gates Vesta sandbox execution is unresolved. | export-002 7e0c95f4ea05 |
| REQ-GRAPH-03 | The dataflow architecture document places Hestia's PGlite storage on a Btrfs @pglite subvolume owned by P02, an edge absent from the subsystem graph. | export-002 7e0c95f4ea05 |
| REQ-GRAPH-04 | The dataflow architecture document directs the action-gate edge from P09 Minerva to P06 Justitia, opposite to the subsystem graph; the caller/callee direction of the safety gate is unresolved. | export-002 7e0c95f4ea05 |
| REQ-GRAPH-05 | The dataflow architecture document directs the Code-CAD verification edge from P14 Hephaestus to P09 Minerva, opposite to the subsystem graph. | export-002 7e0c95f4ea05 |

### Imported release workflow (inactive)

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-REL-01 | The imported release workflow signs the synthesized UKI with cosign sign-blob and publishes a detached signature; release stays blocked until a real artifact exists and publication settings are decided. | export-052 78920f933421 |
| REQ-REL-02 | The imported release workflow references an unverified third-party toolchain action name that must be verified or replaced before any release workflow is activated. | export-052 78920f933421 |

### UI toolchain and token conflicts

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-UI-01 | The developer guide selects Node.js 22 LTS and pnpm as the UI shell toolchain. | export-007 84f43472c536 |
| REQ-UI-02 | The Concordia report's CSS token definitions set a 2px focus stroke and a 3:1 focus contrast, which conflicts with the 3px focus-ring width in concordia-tokens.css. | export-020 1748f49bb7f8 |

### Workspace structure

| ID | Requirement | Source |
| --- | --- | --- |
| REQ-WS-01 | The proposed Cargo workspace lists ten member crates beginning with aegis-compositor; the Vulcan and Hestia Rust candidates are not members and cannot build under a workspace-wide cargo invocation. | export-006 6e694e01e136 |

## Interfaces and behavior

Graph of record: export-062 (subsystem_graph.json). An edge counts as connected only after it is typed with a schema, correlation id, exact revision and bounded retries, and a result is recorded. Disputed directions are marked, and the contract milestone is named.

| Source | Target | Relation | Transport | Contract milestone | Dispute |
| --- | --- | --- | --- | --- | --- |
| P01_Fabrica | P02_Janus_Vallum | PRODUCES_UKI_IMAGE | mkosi / sbsign | M18/M09 |  |
| P02_Janus_Vallum | P06_Justitia | PROVIDES_TPM2_ATTESTATION | Hardware TPM2 PCR | M20 |  |
| P03_Vulcan | P09_Minerva | GPUDIRECT_WEIGHT_STREAMING | PCIe P2PDMA / CUDA GPUDirect | M17 |  |
| P03_Vulcan | P15_Hestia | ZERO_COPY_MEDIA_INGEST | PCIe BAR / Memory Mapping | M17 | not in export-002/003 |
| P04_Mercurius | P05_Forum | SYNC_DESKTOP_SHELL | AF_UNIX Socket Stream | M07 |  |
| P04_Mercurius | P07_Lictor | FOCUS_SWITCH_NOTIFY | Eclipse Zenoh Zero-Copy Shared Memory | M07 |  |
| P04_Mercurius | P10_Vesta | DISPATCH_MCP_SIDECARS | MCP JSON-RPC over Stdio/SSE | M07 |  |
| P05_Forum | P12_Concordia | CONSUMES_DESIGN_TOKENS | CSS Custom Properties / ARIA Bridges | M16 | D05 direction |
| P06_Justitia | P09_Minerva | ACTION_GATE_INTERCEPT | eBPF action_gate.bpf.c / D-Bus | M14 | D03 direction/transport |
| P06_Justitia | P05_Forum | DISPATCH_DECISION_REQUEST | D-Bus / Unix Socket | M14 |  |
| P06_Justitia | P16_Athena | AUDIT_RECONSTRUCTIVE_CANDIDATE | BLAKE3 Checkpoint Ledger | M14 |  |
| P07_Lictor | P04_Mercurius | PRIORITIZE_COMPOSITOR_THREAD | Linux sched_ext struct_ops / scx_cake | M07 |  |
| P07_Lictor | P08_Calliope | ENFORCE_REALTIME_RTPRIO | Linux cgroups v2 / RLIMIT_RTPRIO | M07 |  |
| P07_Lictor | P09_Minerva | DYNAMIC_VRAM_SWAP | drm_sched Priority / Dynamic VRAM Allocator | M07 |  |
| P08_Calliope | P04_Mercurius | SHARE_DMA_BUF_STREAM | PipeWire pipewiresrc / SPA_DATA_DmaBuf | M07 |  |
| P08_Calliope | P11_Ludus | REMOTE_PLAY_CAPTURE | Gamescope / PipeWire DMA-BUF | M07 |  |
| P09_Minerva | P10_Vesta | EXECUTE_WASMED_CAPSULE | AF_VSOCK / Wazero Wasm WIT | M06 |  |
| P09_Minerva | P14_Hephaestus | VERIFY_CODE_CAD | Z3 Symbolic Solver / PAL Engine | M06 | REQ-GRAPH-05 direction |
| P10_Vesta | P16_Athena | SANDBOX_CANDIDATE_EVALUATION | Firecracker MicroVM / AF_VSOCK | M06 |  |
| P11_Ludus | P02_Janus_Vallum | ATTEST_GAME_TRANSACTION | Hardware TPM2 PCR Quote | M08 |  |
| P11_Ludus | P04_Mercurius | DISPATCH_RICH_PRESENCE | Local Unix Domain Socket /tmp/discord-ipc-0 | M08 |  |
| P12_Concordia | P15_Hestia | ENFORCE_A11Y_STANDARDS | Playwright axe-core CI Gate / Svelte CSS | M04 |  |
| P13_Tellus | P07_Lictor | SPATIOTEMPORAL_TASK_SHIFT | D-Bus / kepler_power.bpf | M05 |  |
| P13_Tellus | P05_Forum | EMIT_CARBON_TELEMETRY | D-Bus Signal org.aegisos.Tellus1 | M05 |  |
| P14_Hephaestus | P15_Hestia | RENDER_GEOMETRY_MICROFRONTEND | Svelte 5 Micro-Frontend / PGlite OPFS | M08 |  |
| P15_Hestia | P04_Mercurius | REGISTER_PIP_OVERLAY | wlr-layer-shell Overlay Surface Protocol | M17 |  |
| P16_Athena | P02_Janus_Vallum | TRIGGER_SYSUPDATE_ROLLBACK | systemd-sysupdate / systemd-boot NVRAM | M05 |  |
| P16_Athena | P13_Tellus | EVALUATE_CANDIDATE_CARBON_SCI | ISO/IEC 21031:2024 SCI Rate Calculation | M05 |  |
| P06_Justitia | P10_Vesta | SYSCALL_INTERCEPT (export-002 only) | eBPF action_gate | M14 record, M06 tests | D04 not in graph of record |
| P15_Hestia | P02_Janus_Vallum | PGLITE_OPFS_STORAGE (export-002 only) | Btrfs @pglite subvolume | M17 | REQ-GRAPH-03 not in graph of record |

External producer interfaces (docs/integration/stack.md):

- Aegis -> Imago: the versioned product input manifest (repart definitions, sysupdate transfer, mkosi configuration references) is authored in M18 and proposed in M09. The result returns image/UKI digest, signature reference and boot evidence. Status: declared, unverified; the producer identity was unresolved at revision time.
- Aegis -> Nucleus: the kernel requirement payload (BPF LSM, sched_ext, BTF, RAPL, VFIO/IOMMU, KVM; ABI; accepted architectures) is authored in M18 and proposed in M09. The result returns kernel version/config digest, artifact digest and provenance. Status: declared, unverified; Nucleus currently validates a fixed symbol list, per the private readiness matrix.
- Golusoris and template-native-gpu: optional, and only for components that need a native/GPU backend (M12). The language boundary is respected: no Go library is a direct Rust dependency; otherwise a protocol/FFI adapter with contract tests is used.
- Upstream projects (systemd, wlroots, PipeWire, Firecracker, Z3, PGlite, Steamworks, CAD/solver stack): versions are not active pins. Each is selected through the template matrix at the activation that needs it.

Behavioral contracts derived from the requirements:

- Justitia decision engine (M02): input is an action proposal with maker identity, risk tier, reviewers, due timestamp and killswitch state. Output is Allow, Block or Escalate(DecisionRequest). Errors are maker-as-checker, insufficient distinct checkers, expired request (fail closed) and killswitch engaged.
- Justitia consumer contracts (M14): DecisionRequest, action proposal and signed audit record schemas are versioned. Unknown versions and unsigned proposals are rejected.
- Athena candidate lifecycle (M05): input is a candidate id and metrics (latency_ms, memory_mb, sci_carbon_rate, null_model_retention). Output is Publish or Invalidate plus a hash-chained ledger record. Errors are an empty id or a missing prior record.
- Tellus SCI engine (M05): input is energy, grid intensity, embodied carbon and functional units. Output is an SCI rate and a defer decision. Non-positive functional units are handled by fallback.
- Lictor tier classifier (M07): input is burst durations. Output is a tier per the source thresholds, using strict comparisons. A missing task context falls back to interactive.
- Compositor registry and Tier-1 socket (M07): bounded to 256 surfaces and 64 clients; overflow returns an error.
- Repart and sysupdate definitions (M03): validated with `systemd-repart --dry-run=yes` against a scratch image and `systemd-sysupdate --root=<scratch tree> --offline list`. Non-zero exits and ignored-key diagnostics both fail.
- A/B lifecycle (M15): candidate, signature check, delta acquisition, slot swap, watchdog, then bless or rollback.

## Acceptance criteria

Every public interface needs positive, negative and boundary tests (HISS-15). Criteria per milestone, in rank order, follow the epics:

### M00 (done)

- E00-1 Governance gate reported with its limits: Positive: make verify-all passes. Negative: a drifted generated client file fails `compile-context --verify`. Boundary: the report states preparation/governance scope only; simulated boot output is never cited as evidence.
- E00-2 Split licence scaffold committed: LICENSING.md and REUSE.toml record EUPL-1.2 for technical material and CC-BY-SA-4.0 for prose. The proposal Cargo workspace (export-006) also declares EUPL-1.2; no workspace manifest is committed in the repository yet.
- E00-3 Remote declared; proposal identity recorded for reconciliation: origin resolves to cordanaLLM/Aegis-OS with main present remotely. The proposal identity cordanaLLM/aegis-os (export-006) differs in letter case and is carried into the M01 register; hosted readback is not claimed.

### M01 (ready)

- E01-1 Crate and UI inventory with manifest status: Positive: 12 Rust and 3 UI candidates are listed with source id and hash. Negative: a candidate without a source hash fails review. Boundary: Vulcan and Hestia appear with status 'not a proposal-workspace member', and the P01/P02 crate location is decided.
- E01-2 Graph and source contradiction register: Each disputed edge or value names both sources and the affected contract. export-062 stays the graph of record until a decision changes it.
- E01-3 Toolchain, identity and version-drift register: Every pinned or floating toolchain is listed with its source and marked as proposal data. Nothing is installed. Each drift item becomes a decision.
- E01-4 Imported workflow quarantine: Imported CI, release and integration scripts stay inactive. Their failure-suppressing and simulated-output steps are listed as defects to fix before any activation.

### M02 (blocked)

- E02-1 Workspace root, manifest, lock and Rust toolchain admission: Positive: the crate builds under `cargo build -p aegis-justitia` and a workspace-wide invocation. Negative: an unlocked dependency change fails `cargo build --locked`. Boundary: clippy with -D warnings passes with zero warnings.
- E02-2 Decision engine with positive, negative and boundary tests: Positive: a Tier C action is allowed, and two distinct non-maker checkers are accepted. Negative: maker as checker is rejected, and a duplicate checker is rejected. Boundary: due timestamps equal to now and to now-1 both fail closed, and the killswitch blocks Tier C.
- E02-3 Audit ledger algorithm decision and trait boundary: Positive: one algorithm is recorded and a chain re-walk verifies. Negative: a single flipped byte is detected, and a missing signer returns an error. Boundary: an empty ledger verifies as the genesis state. MD5 is removed.

### M14 (blocked)

- E14-1 Action-gate direction decision and action proposal contract: Positive: a signed, well-formed proposal round-trips. Negative: an unsigned or malformed proposal is rejected. Boundary: a proposal at the maximum field lengths is accepted, and one byte over is rejected.
- E14-2 DecisionRequest and audit record contracts: Positive: both schemas round-trip. Negative: an unknown schema version is rejected. Boundary: an audit record whose previous hash is the all-zero genesis value is accepted only as the first record. The PLD effective date is tracked as an external deadline.
- E14-3 Hardened unit contract: Positive: the unit file declares IPAddressDeny=any. Negative: a review check fails if the directive is removed. Boundary: the unit is not installed or started in this milestone.

### M03 (blocked)

- E03-1 Repart analyzer gate: Positive: the dry run exits 0 with no ignored-key diagnostics. Negative: a missing Type= exits non-zero, and an unknown key fails the gate. Boundary: ESP min equal to max is accepted, and inverted min greater than max is rejected.
- E03-2 Sysupdate transfer gate: Positive: the rewritten transfer lists without error offline. Negative: the imported transfer is rejected (exit 1). Boundary: a transfer with both root slots and no writable target parses, and one with a single slot is flagged against the A/B requirement.
- E03-3 Definition parser crate: Positive: the definitions round-trip through the parser. Negative: a duplicate section or malformed size is rejected. Boundary: 512M and 1G parse exactly, and 511M below the minimum is flagged.
- E03-4 PCR measurement acceptance target: The PCR 0/4/7/11 strategy is documented as the acceptance target for M11 boot evidence. It is not claimed here.

### M18 (blocked)

- E18-1 Product input manifest schema: Positive: the manifest built from the M03 files validates. Negative: a manifest without a correlation id or exact revision is rejected. Boundary: a retry count at the bound is accepted, and one above is rejected.
- E18-2 Kernel requirement schema and kernel identity decision: Positive: the payload lists the kernel features required by P06, P07 and P13. Negative: an unknown architecture is rejected. Boundary: an empty requirement list is rejected explicitly, not accepted as 'no requirements'.
- E18-3 mkosi admission decision: Positive: if admitted, `mkosi summary` parses the Output stanza. Negative: a version below the floor is refused. Boundary: the floor version itself is accepted.

### M15 (blocked)

- E15-1 A/B lifecycle state machine: Positive: the happy path reaches Bless. Negative: a signature failure reaches Discard. Boundary: watchdog expiry exactly at the timeout takes Rollback, and one tick before does not.
- E15-2 Reversible consolidation decision: The decision is recorded, and a test shows that a reopened slot still requires a verity match before Bless.

### M05 (blocked)

- E05-1 Tellus SCI arithmetic: Positive: a hand-computed SCI matches. Negative: functional_units <= 0 falls back without NaN. Boundary: 300.0 is not deferred, 300.001 is deferred, and slices never exceed 16.
- E05-2 Athena lifecycle and Pareto gate: Positive: a passing candidate reaches Publish. Negative: exceeding any single bound reaches Invalidate with a ledger entry, and an empty id is rejected. Boundary: latency 1.5 and retention 0.99 are exercised exactly.
- E05-3 Hash-chained ledger in Rust: Positive: a chain re-walk verifies. Negative: a tampered record is detected. Boundary: the genesis record is verified, and the algorithm matches D02.
- E05-4 Evolution-loop edge contracts: Positive: typed request and response round-trip for each edge. Negative: malformed payloads are rejected. Boundary: the sysupdate call is stubbed, and an empty candidate list is handled explicitly.

### M06 (blocked)

- E06-1 Minerva router, replay and solver logic: Positive: expert registration and routing succeed. Negative: route returns None when no expert matches. Boundary: the 32nd expert is accepted and the 33rd rejected; the 128th trajectory step is accepted and the 129th rejected; relabelling flips only negative rewards.
- E06-2 Vesta bounded controllers: Positive: 64 microVMs and 128 capsules are accepted. Negative: terminating an unknown id returns false. Boundary: the 65th microVM and 129th capsule fail. The boot-time literal is marked unmeasured.
- E06-3 Wasm runtime boundary decision: The decision is recorded. Venus and AF_VSOCK pricing stay deferred hardware-backed requirements (M21).
- E06-4 Agent-chain edge contracts: Positive: typed contracts round-trip. Negative: unsigned or malformed proposals are rejected. Boundary: a capsule request at the capsule bound is accepted, and one over is rejected.

### M17 (blocked)

- E17-1 Vulcan validation crate: Positive: an aligned BAR is accepted. Negative: a misaligned BAR is rejected. Boundary: block_count 0 and 8193 are rejected, 8192 is accepted, and the ring index wraps.
- E17-2 Hestia vector-store state machine: Positive: an initialized store answers queries. Negative: a query before init fails. Boundary: limits 0 and 101 fail, and 1 and 100 pass.
- E17-3 P03 and P15 interface contracts: Positive: the descriptors round-trip. Negative: a malformed descriptor is rejected. Boundary: a descriptor at the maximum block count is accepted, and one over is rejected.

### M07 (blocked)

- E07-1 Compositor registry and Tier-1 socket bounds: Positive: 256 surfaces and 64 clients are accepted. Negative: the 257th surface is rejected and the 65th client is not admitted. Boundary: a pacing-constant test pins the chosen value.
- E07-2 wlroots and mesh design decision: The decision is recorded. The Zenoh version is selected at activation after checking current upstream, with a mocked transport test: a publish round-trips, a subscriber cannot write back, and an empty key expression is rejected.
- E07-3 Lictor EWMA tiers and broker: Positive: bursts below each threshold classify per the source comparisons. Negative: an unregistered focus PID throttles nothing. Boundary: bursts exactly at each threshold classify per the strict comparison.
- E07-4 Calliope plugin lifecycle and DMA-BUF descriptors: Positive: 32 slots are accepted. Negative: the 33rd slot and an illegal transition are rejected. Boundary: stride arithmetic is checked at 0 and at u32::MAX/4.
- E07-5 Real-time edge contracts: Positive: typed focus-switch, RTPRIO grant and DMA-BUF stream descriptors round-trip. Negative: malformed descriptors are rejected. Boundary: an RTPRIO value at the source value is accepted, and one above is rejected.

### M08 (blocked)

- E08-1 Ludus launch-argument validator: Positive: 64 args are accepted. Negative: 65 args are rejected. Boundary: an empty argument list is handled explicitly and its authentication outcome is recorded.
- E08-2 Hephaestus bounded loops: Positive: a mesh under the bound is accepted. Negative: a missing STEP path errors. Boundary: 500000 elements are accepted and 500001 rejected, and the iteration bound is honoured. CAD and solver versions are recorded as unpinned.
- E08-3 P11 and P14 interface contracts: Positive: the descriptors round-trip. Negative: a receipt without a signature field is rejected. Boundary: a viewport descriptor at the mesh bound is accepted.

### M19 (blocked)

- E19-1 action_gate LSM object: Positive: the program loads, and an exec event reaches a stub ringbuf consumer where BPF LSM is active. Negative: the unchecked-pointer variant is rejected. Boundary: if BPF LSM is not active on the host, attach is recorded as unavailable rather than passed.
- E19-2 scx_cake struct_ops object: Positive: struct_ops loads where sched_ext is present. Negative: the unbounded-loop variant is rejected. Boundary: all-stub handlers load.
- E19-3 kepler_power probe object: Positive: the tracepoint program loads. Negative: an out-of-bounds map access variant is rejected. Boundary: the hardcoded TDP literal is recorded as a non-measurement.

### M04 (blocked)

- E04-1 UI toolchain admission and lockfiles: Positive: the pinned Node and pnpm versions install from the lockfile. Negative: a lockfile mismatch fails `pnpm install --frozen-lockfile`. Boundary: the gate fails on the first violation and is not suppressed.
- E04-2 Concordia tokens and axe-core harness: Positive: zero violations on the default state. Negative: a stripped outline fails. Boundary: the focus width chosen by D16 passes, one pixel less fails; 3:1 contrast passes and 2.99:1 fails; 200% text scaling causes no overflow. The truncated container digest is recorded as non-pinnable.

### M16 (blocked)

- E16-1 Forum shell state and lifecycle with stubs: Positive: each lifecycle transition in the source order succeeds. Negative: a transition from Deleted is rejected. Boundary: Rate-Limited to Quarantined at the limit is exercised exactly.
- E16-2 Consumer contracts and token-edge decision: Positive: DecisionRequest and Tellus telemetry payloads parse. Negative: an unknown schema version is rejected. Boundary: a telemetry update with zero watts renders without error.

### M09 (blocked)

- E09-1 Imago consumption of the product input manifest: Positive: an accepted request returns image digest, signature reference and boot-evidence fields. Negative: a malformed manifest is rejected with a correlated error. Boundary: a retry at the bound is recorded, and one above is refused.
- E09-2 Nucleus consumption of the kernel requirement payload: Positive: a result returns kernel version/config digest, artifact digest and provenance. Negative: an unsatisfiable feature is rejected with a correlated error. Boundary: an empty requirement list is rejected explicitly.
- E09-3 Pair evidence and identity status: The retained pair shows correlation id and exact revisions. Simulated output is refused. Identity status is recorded.

### M11 (blocked)

- E11-1 Imago result consumed and verified: Positive: digest and signature verify locally. Negative: a tampered image digest or bad signature is rejected. Boundary: a producer version exactly at the floor is accepted, and one below is rejected.
- E11-2 Real boot evidence replaces the simulated script: Positive: PCR values are read back from swtpm, and the verity root hash matches. Negative: a modified root image fails verity and does not boot to the established state. Boundary: a PCR policy that omits PCR 11 fails to unseal /var. The imported script is not used.
- E11-3 Analyzer gate without suppression in the image path: Positive: the M03 gate passes on the image inputs. Negative: an ignored-key diagnostic fails the image acceptance. Boundary: no step carries failure suppression.
- E11-4 A/B transfer exercised: Positive: the second slot is written read-only and boots. Negative: a transfer with a bad signature is discarded. Boundary: watchdog expiry before bless rolls back once.

### M10 (blocked)

- E10-1 action_gate on the Nucleus kernel: Positive: the program loads and attaches. Negative: the unchecked-pointer variant is rejected. Boundary: BPF LSM absent from the kernel config fails the milestone with a recorded reason.
- E10-2 scx_cake on the Nucleus kernel: Positive: struct_ops attaches. Negative: the unbounded variant is rejected. Boundary: a kernel without sched_ext is recorded as a Nucleus requirement defect.
- E10-3 kepler_power on the Nucleus kernel: Positive: the tracepoint attaches. Negative: a missing tracepoint is reported, not ignored. Boundary: zero energy delta over an idle interval is recorded as a value, not an error.

### M21 (blocked)

- E21-1 RAPL-backed carbon telemetry: Positive: SCI is computed from measured energy. Negative: unreadable counters fail closed with an error, not a default value. Boundary: counter wraparound between two samples yields a correct positive delta.
- E21-2 Firecracker and AF_VSOCK sandboxing: Positive: a microVM boots and the candidate evaluation round-trips over AF_VSOCK. Negative: a microVM request above the memory limit is refused. Boundary: the 64th microVM on real KVM is accepted and the 65th refused.
- E21-3 Venus GPU pricing deferred: Recorded as deferred to M12. It is not claimed here.

### M13 (blocked)

- E13-1 Hosted ruleset/label and identity readback: Positive: rulesets and labels read back from the hosted side match .github/rulesets and .config/labels.yaml. Negative: a missing required check is reported as drift. Boundary: the proposal identity case difference is resolved or recorded.
- E13-2 Signing and provenance for a real artifact: Positive: the signature verifies against the M11 artifact. Negative: verification fails against a modified artifact, and an unverified action reference fails workflow lint. Boundary: signing an absent artifact fails rather than producing an empty signature.
- E13-3 Publication settings and consumers: Positive: one recorded consumer enables delivery. Negative: delivery is refused with zero recorded consumers. Boundary: a consumer without a pinned version is not counted.

### M20 (blocked)

- E20-1 PCR quote signs an audit record: Positive: an audit record signed with the quote verifies. Negative: a record signed under a different PCR state fails verification. Boundary: a quote over exactly PCR 0/4/7/11 is accepted, and a quote missing PCR 11 is rejected.
- E20-2 PCR-sealed /var unseal: Positive: /var unseals under the enrolled policy. Negative: a wrong PCR policy fails to unseal. Boundary: changing only PCR 7 is enough to prevent unseal.

### M12 (blocked)

- E12-1 DMA-BUF sharing paths: Positive: one measured zero-copy transfer per path. Negative: an invalid DMA-BUF fd is rejected without a CPU copy fallback being counted as success. Boundary: the 64th buffer is accepted and the 65th refused on real hardware.
- E12-2 VFIO and P2PDMA paths: Positive: one measured NVMe-to-GPU transfer. Negative: a device outside its IOMMU group is refused. Boundary: a transfer of exactly 8192 blocks succeeds, and 8193 is refused before dispatch.
- E12-3 GPU template boundary: Positive: one backend fixture result is retained, or a not-needed decision is recorded. Negative: a Go library as a direct Rust dependency is rejected. Boundary: an ABI mapping with zero functions is not accepted as a boundary.

## Constraints and unknowns

Constraints:

- The concept is preserved with no redesign. Praetor governance is canonical, and notebook exports are immutable proposal data, including their dependency versions.
- `make verify-all` verifies preparation and governance only. Build, image, boot, hardware, accessibility and release are separate blocked gates.
- Aegis owns product requirements and adapters, Imago owns image construction, and Nucleus owns kernel construction. Golusoris supplies compatible shared packages, and a Go library is never a direct dependency of a Rust daemon.
- Toolchains enter through the template matrix only as components activate. Every activation needs a manifest, a dependency lock, an interface contract and positive/negative/boundary tests.
- HISS invariants apply to activated code: bounded loops, no unwrap/expect, no recursion, zero warnings and function-size limits. Scaffold `assert!` bounds are refactored to `Result` before their negative tests count.
- A host or stock kernel, or workstation measurements, are local fixtures. They never close a milestone that requires the Nucleus kernel or an Aegis image.
- Public documents cite ids and hashes only; quotes stay private.

Unknowns (see open decisions D01-D20):

- The kernel identity and version line.
- Versions for wlroots, Mesa, PGlite, Steamworks, the CAD/solver stack, Firecracker and the Wasm engine.
- The GPU vendor for the P08/P11/P15 DMA-BUF paths.
- Whether P03 -> P15 exists outside the graph JSON.
- The exact SCI functional-unit definitions.
- The D-Bus interface schemas for Lictor and Minerva.
- The real container digest for the Concordia validator.
- Whether reports for P09, P13 and P16 exist at all; they are absent from the export.
- Whether the configured Imago/Nucleus origins will resolve, and whether builder workflow identities will be canonicalized (private readiness matrix).
- {{unresolved}}: no source states a minimum host specification for M11/M12/M20/M21 beyond KVM, OVMF, swtpm, TPM2, RAPL, an NVMe SSD and an NVIDIA/AMD GPU.
