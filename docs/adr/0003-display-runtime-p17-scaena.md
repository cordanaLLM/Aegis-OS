<!-- markdownlint-disable MD013 -->
# ADR-0003: Add P17 aegis-scaena, where the compositor composes

- **Status**: Accepted
- **Date**: 2026-09-28
- **Authors**: @lusoris (decision), roadmap decisions D74, D75 and D77

## Context

The concept this repository activates has sixteen subsystems, P01 to P16, and
`CONTEXT.md` requires an ADR to change it. A private desktop environment
specification (source id `desktop-spec`, sha256 prefix `a41f6c9e825d`, version
2.0.0-DEV, proposal data) proposed a post-WIMP spatial canvas for the shell, a
Rust-native display runtime on Servo, WebRender and wgpu that mounts surfaces
through `zwlr_layer_shell_v1` and imports hardware-decoded video as DMA-BUF,
and line-delimited JSON-RPC 2.0 between components. No recorded subsystem owns
a display runtime: P04 aegis-compositor serves no Wayland protocol yet, and P05
aegis-forum-shell has no UI code.

On 2026-09-27 the maintainer chose the canvas as P05's primary surface (D74)
and a new display-runtime component built now (D75). A check of the
single-scene design against the released crates then found four blockers:

- webrender 0.70.0, which servo 0.6.0 requires, renders through OpenGL (gleam)
  and has no wgpu dependency, so a Servo page reaches a wgpu scene only through
  a per-frame GPU copy;
- wgpu 30.0.1 imports single-plane DMA-BUF only, while VA-API and NVDEC produce
  multi-planar NV12 (gfx-rs/wgpu#9801, open);
- Servo drives its own event loop, which is not `Send`, and starts its own tokio
  runtime, so it cannot share one loop with the daemons as the specification
  assumes;
- the DMA-BUF import and window-handle calls both paths need are `unsafe`, and
  the workspace forbids `unsafe_code` with no per-crate override: a member that
  inherits the workspace lints and allows it fails with error E0453.

The maintainer therefore revised D75 on 2026-09-28 so that the compositor
composes.

## Decision

The OS design gains a seventeenth subsystem beside the concept's sixteen, P17
aegis-scaena (Project Scaena, graph node `P17_Scaena`, from the Latin for
stage), owned by Aegis.

1. The shell engine and each video stream own separate Wayland surfaces. P17
   mounts them through `zwlr_layer_shell_v1` and hands decoded video to the
   compositor as DMA-BUF through `zwp_linux_dmabuf_v1`, so the compositor
   composites it without a copy (D75).
2. Between components, control is line-delimited JSON-RPC 2.0 over `AF_UNIX`
   and data is DMA-BUF descriptors passed as `SCM_RIGHTS`; iceoryx2 is reserved
   for a future high-rate stream that is not a GPU buffer. The three
   Aegis-internal D-Bus edges put to the maintainer, DISPATCH_DECISION_REQUEST,
   EMIT_CARBON_TELEMETRY and SPATIOTEMPORAL_TASK_SHIFT, move to JSON-RPC;
   ACTION_GATE_INTERCEPT (P06 to P09, eBPF action_gate / D-Bus) is not re-ruled
   by D77, and its transport stays open under D26; the platform D-Bus interfaces
   (the freedesktop portal, AT-SPI2, the StatusNotifierWatcher) stay (D77).
3. The spatial canvas is P05's primary surface (D74). The engine that renders
   it is chosen later behind a fixed P17 surface contract (open decision D82);
   until then no web engine is admitted.
4. The first slice, milestone M27, decodes baseline Motion-JPEG and one MPEG-2
   intra frame on VA-API (the codec the maintainer chose on 2026-09-28),
   exports each surface with cros-libva, passes the fd over a socketpair and
   attaches it to a layer surface. It uses no Servo, no wgpu and no `unsafe`
   code.

## Consequences

- **Positive**: the NV12 zero-copy path, which needs no engine and no `unsafe`
  code in Aegis, comes first, and M27 is the milestone meant to produce its
  runtime evidence on the reference profile. The workspace keeps
  `unsafe_code = "forbid"`, and the engine choice (decision 3) no longer holds
  up the first slice.
- **Negative**: the inventory is no longer the concept's sixteen, so
  `tools/verify_preparation.py` pins seventeen and every sixteen-subsystem
  statement in the documentation changes. There is no single scene graph: the
  canvas and each video are separate surfaces, and an effect spanning both
  belongs to the compositor. P17 links libva and a vendor VA driver, C
  libraries reached through cros-libva's bindings; that FFI lives in the
  dependency and not in Aegis code, and ADR-0001's single memory-safety model
  stays scoped to P04. The VA-API binding is a git pin of an unreleased
  cros-libva fix (D80), refreshed to a crates.io release once one carries it.
  The P04 half is unproven: until P04 serves both protocols, a run against the
  host session's compositor is the client half only (D79). The other crates'
  lock sweeps read their own dependency closure (D78), starting in the same
  change that adds P17's first Wayland or VA-API crate to the lock.
- The specification stays proposal data. Its single Servo, WebRender and wgpu
  scene and its one shared tokio loop are recorded as superseded, not deleted.
  The graph of record, export-062, is unchanged; P17 and its edges are
  recorded beside it.

## References

- Roadmap decisions D74 to D83, `docs/roadmap/README.md` (Risks and decisions).
- Component P17 in `planning/components.json`; milestone M27 in
  `planning/roadmap.json`.
- Requirements REQ-P17-01 to REQ-P17-08, `docs/roadmap/requirements.md`.
- Maintainer answer, 2026-09-27, to "The desktop concept (Gemini notebook) spans
  seven pillars. Which ones do you actually want in Aegis's design? I'll record
  those as proposal data plus open decisions (D74+) against P04/P05/P08/P12, and
  fix the spec's false 'Implemented' claims. Nothing gets activated.": "Spatial
  Svelte shell, Rust web display engine, FUI/diegetic HUD look, UDS IPC + shared
  memory".
- Maintainer answer, 2026-09-27, to "D74 — Should P05 Forum shell adopt the
  spatial post-WIMP canvas (instruments, surrogate objects, QuadTree culling,
  focus+context zoom, DOM transclusion)?": "P05's primary surface".
- Maintainer answer, 2026-09-27, to "D75 — Should Aegis build a Rust-native
  display runtime (Servo + WebRender + wgpu, layer-shell surfaces, zero-copy
  DMA-BUF video)?": "New component now".
- Maintainer answer, 2026-09-27, to "D76 — Should the heads-up visual language
  (colour + frame-style security states, scanline/aberration/Fresnel shaders,
  diegetic panels) become the P12 Concordia design language?": "Optional theme
  (Recommended)".
- Maintainer answer, 2026-09-27, to "D77 — Which data plane should carry video
  frames and telemetry between shell, compositor and daemons (control stays
  JSON-RPC over Unix sockets)?": "DMA-BUF via SCM_RIGHTS (Recommended)".
- Maintainer answer, 2026-09-28, to "D75 as decided (Servo + WebRender + wgpu as
  one zero-copy scene) fails on four verified points. Which direction should the
  design panel (workflow 2) take?": "Compositor composes (Recommended)", the
  option described as "Shell engine and each video stream get their own Wayland
  surfaces; video decode goes over the D77 SCM_RIGHTS path straight to the
  compositor. Build the no-Servo video slice first (fits the unsafe ban, real
  evidence now); pick the shell engine later behind a fixed surface contract."
- Maintainer answer, 2026-09-28, to "What should the new display-runtime
  component be? A new component needs ADR-0003 (CONTEXT.md requires an ADR to
  change the 16-subsystem concept) plus the 16-to-17 validator change.": "P17
  aegis-scaena".
- Maintainer answer, 2026-09-28, to "D77 puts shell/compositor/daemon control
  traffic on JSON-RPC over Unix sockets. The register still routes three
  Aegis-internal edges over D-Bus (P06→P05 decision requests, P13→P05 and
  P13→P07 telemetry). Move them?": "Move internal edges (Recommended)".
- Maintainer answer, 2026-09-28, to "Which accessibility standard should the 21
  new requirements (canvas, HUD theme, display runtime) target?": "EN 301 549
  V4.1.1 / WCAG 2.2 AA (Recommended)".
- Maintainer answer, 2026-09-28, to "Every existing crate's manifest_hygiene
  test scans the whole shared Cargo.lock and forbids wayland-backend/tokio, so
  any real Wayland slice turns the required gate red. How should that be
  resolved?": "Per-crate closure (Recommended)".
- Maintainer answer, 2026-09-28, to "The first slice would run against your KDE
  KWin (P04 aegis-compositor has no Wayland server yet). Does a KWin run count
  as the milestone's evidence?": "KWin = dev evidence (Recommended)".
- Maintainer answer, 2026-09-28, to "VA-API decode needs a Rust binding.
  crates.io cros-libva 0.0.13 fails against the host's libva 2.24.1; the
  upstream fix (chromeos/cros-libva #37) is merged but unreleased. All options
  need bindgen/libclang/pkg-config admitted as toolchain.": "Git-pin cros-libva
  (Recommended)".
- Maintainer answer, 2026-09-28, to "M27's first slice needs a codec. cros-libva
  has no bitstream parser, and cros-codecs 0.0.6 (which has one) requires
  exactly cros-libva 0.0.12, so it can't use our git pin. Which codec should M27
  decode?": "Motion-JPEG now (Recommended)".
- Maintainer answer, 2026-09-28, to "Should M12 (GPU-backed slices inside the
- Maintainer answer, 2026-09-28, to "HISS-03 forbids heap allocation in hot loops, but the pinned cros-libva allocates for every decoded frame (Picture::new boxes its state; each JPEG/MPEG-2 parameter buffer is a Box). M27 currently says HISS-03 is not claimed on the frame path and the decision is open. What should the register say?": "Scoped deviation (Recommended)".
  built image) re-run M27's display path in the image, making M27 a blocker of
  M12?": "M27 stays a leaf (Recommended)".
- Upstream facts read 2026-09-27 and 2026-09-28: the crates.io dependency lists
  of servo 0.6.0 and webrender 0.70.0; `texture_from_dmabuf_fd` in wgpu-hal
  30.0.1 `src/vulkan/device.rs`; gfx-rs/wgpu#9801; `async_runtime.rs` in
  servo-net 0.6.0; error E0453 from rustc 1.98.1 in a scratch workspace.
