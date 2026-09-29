<!-- markdownlint-disable MD013 -->
# ADR-0004: Build the whole P05 shell natively in Rust on gpui

- **Status**: Accepted
- **Date**: 2026-09-29
- **Authors**: @lusoris (decision), roadmap decisions D82, D101 and D102
- **Supersedes**: ADR-0003 decision 3 in part (the open engine choice D82);
  ADR-0003 itself stays Accepted and unchanged

## Context

ADR-0003 made the spatial canvas P05's primary surface (D74) and left the
engine that renders it open as decision D82, behind a P17 surface contract no
milestone had fixed. D82 offered three engines: Servo 0.6.0, WPE WebKit through
its WPEPlatform API, and a native Rust toolkit that paints the canvas itself and
exposes AccessKit. Until D82 was decided, no web engine was admitted.

ADR-0003 ruled only on the canvas behind P17. The rest of the register still
modelled P05 as a Svelte application in a Wry webview (WebKitGTK on Linux):

- REQ-P05-02 names a "Svelte/Wry frontend" and REQ-P05-08 an accessibility gate
  that runs Playwright and axe-core from a `ui/forum-shell` package through
  `npm run test:a11y` (`docs/roadmap/requirements.md`);
- M16's criteria created `ui/forum-shell` on the Node, pnpm and Playwright
  toolchain M04 admitted, and D91 had the M04 axe-core gate scan that package
  for REQ-P12-06 (`planning/roadmap.json`);
- the P05 card in `planning/components.json` cited the Svelte scaffold
  (export-043) and asked for a UI package manifest and an axe-core test.

A DOM scanner cannot reach a native surface, so choosing a native engine for
the canvas alone would have left two engines and two accessibility trees in one
shell. REQ-P17-01 records the other cost of the webview path: WebKitGTK and Wry
wrap the browser in GTK container windows, which on Wayland meet realization
timing locks with `zwlr_layer_shell_v1`.

Facts read on 2026-09-29 (`crates.io` API and the upstream repositories):

- accesskit 0.25.1 and accesskit_unix 0.24.0 are the current releases, both
  published 2026-09-25 under MIT OR Apache-2.0. The Unix adapter implements the
  AT-SPI D-Bus interfaces through zbus and atspi, both pure Rust, so no C
  accessibility library enters the shell.
- The AccessKit README describes a toolkit that pushes a complete tree, which
  the platform adapter retains, so a node the toolkit keeps but does not paint
  stays visible to assistive technology; REQ-P05-11 asks exactly that of
  viewport culling.
- gpui (zed-industries/zed, `crates/gpui`, Apache-2.0) is the only toolkit
  surveyed that both ships a public `zwlr_layer_shell_v1` API of its own
  (`crates/gpui/src/platform/layer_shell.rs`) and wires accesskit_unix directly
  rather than through winit (`crates/gpui_linux/Cargo.toml`). Its `canvas`
  element and the `TransformationMatrix` in `crates/gpui/src/scene.rs` provide
  custom painting under a camera transform, and the Zed editor uses it in
  production.
- gpui's crates.io release is 0.2.2 of 2025-10-22, with nothing published
  since, while `crates/gpui` keeps changing on zed's `main`. At that `main` the
  workspace pins accesskit 0.24.0 and accesskit_unix 0.22, one and two minor
  releases behind the current ones.

## Decision

1. **D82: a native Rust toolkit that exposes AccessKit paints P05's canvas.**
   No web engine is admitted for P05: neither Servo nor WPE WebKit.
2. **D101: the whole P05 shell is native.** The canvas and the chrome around it
   are one Rust program on one toolkit, and its accessibility tree is
   AccessKit's, exported to AT-SPI through accesskit_unix. Svelte and the Wry
   webview leave the P05 shell, and no `ui/forum-shell` package is created. The
   Concordia tokens in `ui/concordia-tokens` (M04) stay the single token source
   and also emit Rust theme constants for the shell; the generator and the
   constants are M28's work. The shell's accessibility check moves from
   axe-core and Playwright to an AccessKit tree check: M16 runs it on the
   exported `accesskit::TreeUpdate`, and M28 runs it over AT-SPI against the
   running shell. REQ-P05-02, REQ-P05-08, REQ-P12-01 and REQ-P12-06 keep their
   source text and gain a re-mapping note.
3. **D102: the toolkit is gpui**, consumed as a git dependency on
   zed-industries/zed pinned to one commit. This ADR chooses no commit: M28
   chooses the pin and admits it, together with accesskit_unix, through the
   toolchain-admission rule, and refreshes it to a crates.io release once one
   carries what the shell needs (D69), the pattern D80 set for cros-libva.
4. **Milestones.** M16 becomes the Rust crate `crates/aegis-forum-shell`: the
   D96 lifecycle, the D77 JSON-RPC 2.0 framing over a mocked socket, the
   DecisionRequest and CarbonTelemetry consumers typed on the Rust contracts,
   and the D74 canvas model with its tree exported as an
   `accesskit::TreeUpdate`, with no toolkit. M28 puts that model on gpui as a
   layer surface behind the P17 surface contract, exports the tree live over
   AT-SPI, emits the token constants, measures the focus ring on rendered
   pixels, and measures the 1,000-node frame and tree-update times. M29 runs the
   shell in a live session in a VM with xdg-desktop-portal and at-spi2.

Unchanged by this ADR:

- ADR-0003's decisions 1, 2 and 4. The shell engine still owns its own Wayland
  surface, mounted through `zwlr_layer_shell_v1`, and control between
  components is still line-delimited JSON-RPC 2.0 over `AF_UNIX` (D77).
- M04 is done and its evidence stands. Its gate keeps covering the P12 token
  component, which is CSS and one Svelte 5 component, and never covered the
  shell.
- The ESLint lint that enforces HISS-01, HISS-04 and HISS-08 on the JavaScript
  and Svelte under `ui/` (D100).
- P15 Hestia's WebKitGTK (wry) micro-frontend host (REQ-P15-01) and the P14 to
  P15 Svelte micro-frontend (REQ-P14-06). This ADR decides P05 only.

## Consequences

- **Positive**: the shell has one language and one memory-safety model, as
  ADR-0001 gave the compositor, and no browser engine, DOM or JavaScript
  runtime. Culled canvas nodes can stay in the tree the toolkit hands to
  AccessKit, which is what REQ-P05-11 requires and M16 tests, and the AT-SPI
  path is pure Rust. gpui maps the shell as
  a `zwlr_layer_shell_v1` surface through a public API, which is the surface
  ADR-0003 decision 1 describes. M16 can test the lifecycle, the consumers and
  the accessibility tree on any developer machine, with no display, no GPU and
  no D-Bus session.
- **Negative**: the toolkit is pinned to a commit of a repository Aegis does not
  control, whose crates.io release is eleven months old, so each refresh of the
  pin is an explicit, reviewed change. At zed `main` gpui resolves accesskit
  0.24.0, while M16 exports accesskit 0.25.1, so M28 has to align the two or
  convert between them, and it records the lag as D69 drift. gpui links
  libxkbcommon, a C library, through the `xkbcommon` crate (0.8.0 at zed `main`)
  for Linux keymaps, and loads libwayland-client with `dlopen`. That FFI lives
  in dependencies and not in Aegis code, as P17's libva does, and ADR-0001's
  single memory-safety model stays scoped to P04 as ADR-0003 scoped it. No
  published benchmark ties 1,000 nodes to a frame time for gpui; M28 measures
  one, and no figure is claimed before it (ADR-0001). gpui rebuilds its
  AccessKit tree every frame (`crates/gpui/src/window/a11y.rs`), so whether
  HISS-03 binds that work is a question M28 answers before its frame-time case
  counts, as D83 answered it for the decoder.
- **Negative**: M16's tree check is narrower than the axe-core scan it
  replaces. It asserts a role and a name on every node and a labelled empty
  state, not the WCAG 2.2 AA rule set axe-core applies to a DOM, and it runs on
  a model rather than on a painted surface. The criteria a native surface must
  meet are carried by REQ-P05-09 to REQ-P05-15 and REQ-P17-06, and only some of
  them are planned: M16 exercises REQ-P05-09 to REQ-P05-11 on the model, M28
  exercises REQ-P05-11 and REQ-P17-06 over AT-SPI and REQ-P05-13 on rendered
  pixels, and M29 exercises REQ-P17-06 in a live session. No milestone yet
  plans REQ-P05-12, REQ-P05-14 or REQ-P05-15. Until M28, no running shell is
  checked.
- **Negative**: three decisions taken for a JavaScript shell earlier on
  2026-09-29 are withdrawn before any delivery: D97 (`node:test`), D98 (Rust
  golden fixtures read by a JavaScript consumer) and D99 (one pnpm workspace
  under `ui/`). The shell's consumers now decode the producers' Rust types
  directly.
- M27 stops being a leaf: M28 and M29 both wait for it. D79's answer that M12
  does not wait on M27 is unchanged.
- ADR-0003's References quote the 2026-09-27 pillar selection, "Spatial Svelte
  shell, Rust web display engine, ...". Its Svelte and web-engine halves are
  superseded for P05 by this ADR and stay quoted there, not deleted.

## Alternatives considered

- **Servo** (D82): servo 0.6.0 paints through OpenGL with webrender 0.70 and
  surfman, starts its own tokio runtime, fetches a prebuilt SpiderMonkey at
  build time unless the archive is pinned, and records Linux
  assistive-technology detection as unreliable (servo/servo#46834).
- **WPE WebKit** (D82): a C and GObject engine, and therefore a second
  memory-safety model at the centre of the desktop.
- **An evaluation milestone before choosing** (D82): offered, not chosen.
- **A native canvas inside a Svelte and Wry chrome**: the register's previous
  model. It keeps two engines, two accessibility trees and the GTK
  window-realization problem REQ-P17-01 records, and it keeps a DOM gate
  (axe-core) that cannot see the canvas.
- **egui with an Aegis-written layer-shell host** (D102): egui 0.36.2 (MIT OR
  Apache-2.0) has AccessKit built in and a pan-and-zoom `Scene` container, but
  it reaches Wayland through winit, which has no `zwlr_layer_shell_v1` support,
  so Aegis would write the smithay-client-toolkit, wgpu and egui glue itself.
- **Xilem and Masonry** (D102): Apache-2.0, with AccessKit central to the design
  and culled widgets excluded from the tree deliberately
  (`masonry_core/src/passes/accessibility.rs`); the project calls itself
  experimental, reaches Wayland only through winit's `xdg_toplevel` windows, and
  had 17 commits in the 90 days before 2026-09-29.
- **A gpui-against-egui spike** (D102): offered, not chosen.
- **Disqualified on evidence** (D102): iced 0.14.0 has no AccessKit integration
  in mainline (iced-rs/iced#552, open since 2020); Floem has none; Makepad has
  one forwarding stub and no Linux AT-SPI backend; Freya depends on Skia, C++,
  unconditionally and has no layer-shell path; Slint 1.18.1 is licensed
  GPL-3.0-only or under two proprietary Slint licences, enables AccessKit only
  behind a feature, and no pan-and-zoom canvas container was found in it.

## References

- Roadmap decisions D82, D101 and D102, and D32, D87 (for M16) and D96 to D100,
  recorded on the same day, `docs/roadmap/README.md` (Risks and decisions) and
  `docs/roadmap/inventory.md` (D32).
- ADR-0003, `docs/adr/0003-display-runtime-p17-scaena.md`, decision 3.
- Requirements REQ-P05-02, REQ-P05-08, REQ-P12-01, REQ-P12-06 and REQ-P17-06,
  `docs/roadmap/requirements.md`.
- Milestones M16, M28 and M29 in `planning/roadmap.json`; components P05, P12
  and P17 in `planning/components.json`.
- Upstream facts read 2026-09-29: the crates.io records of accesskit,
  accesskit_unix, gpui, egui, iced, Slint, zbus and atspi; zed-industries/zed
  `Cargo.toml` and `crates/gpui/Cargo.toml` (gpui 0.2.2, Apache-2.0; accesskit
  0.24.0 and accesskit_unix 0.22 at `main`),
  `crates/gpui/src/platform/layer_shell.rs`, `crates/gpui/src/window/a11y.rs`,
  `crates/gpui/src/elements/canvas.rs` and `crates/gpui/src/scene.rs`; the
  AccessKit README; the repositories of Xilem, Floem, Makepad and Freya.
- The popup questions of 2026-09-29 are summarised below, not quoted; each
  answer is quoted verbatim.
- Maintainer answer, 2026-09-29, to the D82 question, which engine paints P05's
  canvas behind P17's surface contract (options offered: plan an evaluation
  milestone, native Rust + AccessKit, Servo, WPE WebKit): "Native Rust +
  AccessKit".
- Maintainer answer, 2026-09-29, to the question whether the native choice
  covers the whole P05 shell or only its canvas: "All-native shell
  (Recommended)".
- Maintainer answer, 2026-09-29, to the D102 question, which toolkit (options
  offered: gpui git-pinned, egui with its own layer-shell host, Xilem/Masonry, a
  spike of gpui against egui): "gpui, git-pinned (Recommended)".
- Maintainer answer, 2026-09-29, to the question how M16 is re-scoped under an
  all-native shell: "Rust P05 crate (Recommended)".
- Maintainer answer, 2026-09-29, to the question whether the live-session
  milestone D91 waits for is planned now: "Plan it now (Recommended)".
- Maintainer answer, 2026-09-29, to the D87 question, which `@sveltesentio/*`
  packages M16 declares: "None (Recommended)".
- Maintainer answer, 2026-09-29, to the D32 question, the P04 to P05 endpoint
  and budget: "Path + <100 µs target (Recommended)".
- Maintainer answer, 2026-09-29, to the D96 question, which lifecycle
  transitions and quarantine limit: "Source order + recovery (Recommended)".
- Maintainer answers, 2026-09-29, before D101, to the D97, D98 and D99
  questions, the unit-test runner, the typing of the consumers and the lockfile
  layout: "node:test (Recommended)", "Rust golden fixtures (Recommended)" and
  "One pnpm workspace (Recommended)".
- Maintainer answer, 2026-09-29, to the D100 question, how HISS is enforced on
  JavaScript and Svelte while praetorctl scans neither: "ESLint in the pinned
  container (Recommended)".
- Maintainer answer, 2026-09-29, after this ADR was drafted, to the question
  whether D101 covers only the P05 shell or the whole product, given P15
  Hestia's WebKitGTK/Wry host (REQ-P15-01) and the P14 to P15 Svelte
  micro-frontend (REQ-P14-06): "P05 only for now (Recommended)".
