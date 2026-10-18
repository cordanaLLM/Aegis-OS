<!--
SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# The P05 Forum shell on gpui: mapping the layer surface

Status: recorded observations from M28 epic E28-2, reference profile,
2026-10-18

Milestone M28 puts the P05 Forum shell on gpui (ADR-0004). Epic E28-2 maps
the shell's `zwlr_layer_shell_v1` surface from a `MOUNT_SHELL_CANVAS` request
and waits for the compositor's first configure (ADR-0003 decision 3). The unit
tests for the request line, the window options and the outcome contract run
inside `make verify-all`. Mapping the surface needs a Wayland compositor, so
it runs here:

```sh
make verify-shell
```

**A pass is development evidence for the client half only (D79).** The gate
starts its own headless KWin (`kwin_wayland --virtual`) in a private
`XDG_RUNTIME_DIR` under `dbus-run-session`, and it never connects to the host
session's compositor. KWin is not P04: nothing here is evidence that P04
serves `zwlr_layer_shell_v1`. A pass closes no hardware, accessibility or
release gate.

## What is tracked

| Path | Role |
| :--- | :--- |
| `crates/aegis-forum-shell-gpui/src/surface.rs` | request decoding, gpui window options for the layer surface, the event loop, outcomes and exit codes |
| `crates/aegis-forum-shell-gpui/src/bin/aegis-forum-shell-gpui.rs` | the binary the gate drives: `aegis-forum-shell-gpui surface <request-line>` |
| `crates/aegis-forum-shell-gpui/tests/surface.rs` | Rust positive, negative and boundary tests that need no display |
| `tools/verify_shell.py` | the gate: builds the binary, starts KWin and two filter proxies, runs and judges the cases |
| `tools/wayland_filter.py` | a Wayland wire proxy that hides `zwlr_layer_shell_v1` or drops `zwlr_layer_surface_v1.configure` |
| `tools/test_verify_shell.py`, `tools/test_wayland_filter.py` | unit tests for both tools, run inside `make verify-all` |

## What the gate does

1. If the host lacks Linux, `AF_UNIX` with `SCM_RIGHTS`, `kwin_wayland`,
   `dbus-run-session`, `wayland-info` or `cargo`, it prints
   `SKIP: <reasons>; the native shell gate did not run.` and exits 0. A SKIP is
   not a pass.
2. It builds `aegis-forum-shell-gpui` with `cargo build --locked` at two jobs,
   unless `CARGO_BUILD_JOBS` says otherwise.
3. It starts KWin at 1280x720 and two `wayland_filter.py` proxies in front of
   it, one per mode.
4. It runs the eight surface cases below. Every child has a 30 s timeout,
   and the binary waits at most `FIRST_CONFIGURE_DEADLINE` (5 s) for the
   first configure. `surface/parameters-read-back` starts a second KWin of
   its own, described below, and stops it when the case ends.
5. It prints one `CASE` line per case, then the D79 note, then
   `RESULT pass` only when every one of `CASE_COUNT` cases ran and passed
   (`summary()` in `tools/verify_shell.py`).

The binary takes exactly one newline-terminated line, as the socket would
carry it. Its exit status names the outcome: 0 configured, 3 open or write
failure, 4 layer shell missing, 5 configure deadline, 6 request refused,
64 usage.

## The cases

| Case | Connection | Passes when |
| :--- | :--- | :--- |
| `surface/first-configure` | KWin directly | exit 0 and `OUTCOME configured 1280x48`: the full output width, anchored top, left and right, at the shell height |
| `surface/filter-control` | `wayland-info` through both proxies | the hide proxy lists `xdg_wm_base` but not `zwlr_layer_shell_v1`, and the drop proxy still lists `zwlr_layer_shell_v1`, so the two negative cases below test what they claim |
| `surface/layer-shell-missing` | hide proxy | exit 4 and an `OUTCOME layer-shell-missing:` line naming `LayerShellNotSupportedError` |
| `surface/first-configure-deadline` | drop proxy | exit 5 and an `OUTCOME deadline:` line naming `zwlr_layer_surface_v1.configure` |
| `surface/refused-line` | none | exit 6 and an `OUTCOME refused:` line naming `jsonrpc`, for a request whose `jsonrpc` member is `"1.0"` |
| `surface/unknown-schema` | none | exit 6 and an `OUTCOME refused:` line carrying `the mount request names a schema this build does not admit`, for the reference request with schema `aegis.p05-p17.mount-shell-canvas.v2` |
| `surface/unknown-layer` | none | exit 6 and an `OUTCOME refused:` line carrying `the layer is not background, bottom, top or overlay`, for the reference request with layer `middle` |
| `surface/parameters-read-back` | debug KWin directly | exit 0 and `OUTCOME configured 1280x48`, and KWin's own request log holds each parameter once with the reference value (table below) |

Each refused case must name its own reason. A refusal for any other reason,
such as a missing newline or the other changed member, fails the case.

### Reading the parameters back from KWin

`OUTCOME configured` shows only that a configure arrived. To show what the
shell asked for, `run_read_back_case()` starts a second KWin on the socket
`kwin-debug-0` with `WAYLAND_DEBUG=server`, inside its own
`dbus-run-session` under the gate's no-activation configuration. libwayland
then logs every request the compositor receives to `kwin-debug.log` in the
gate's runtime directory. The main KWin runs without the debug log, so the
timing cases do not pay for it.

The judge (`judge_parameters_read_back()`) reads the request lines of
`zwlr_layer_shell_v1` and `zwlr_layer_surface_v1` and skips events (lines
with `->`). It requires each request below exactly once with the wire value
of the reference request, as `wlr-layer-shell-unstable-v1` encodes it. A
missing request or a second, different one fails the case.

| Request | Logged arguments | Reference parameter |
| :--- | :--- | :--- |
| `get_layer_surface` | layer `2`, namespace `aegis-forum` | `layer: top`, `namespace: aegis-forum` |
| `set_anchor` | `13` (top 1 + left 4 + right 8) | `anchor: [top, left, right]` |
| `set_exclusive_zone` | `48` | `exclusive_zone: 48` |
| `set_exclusive_edge` | `1` (top) | `exclusive_edge: top` |
| `set_keyboard_interactivity` | `2` | `keyboard_interactivity: on_demand` |

KWin 6.8.0 logged these lines for the reference request (2026-10-18), without
the timestamps; the first line is wrapped after its first argument:

```text
zwlr_layer_shell_v1#16.get_layer_surface(new id zwlr_layer_surface_v1#28,
    wl_surface#27, nil, 2, "aegis-forum")
zwlr_layer_surface_v1#28.set_size(0, 48)
zwlr_layer_surface_v1#28.set_anchor(13)
zwlr_layer_surface_v1#28.set_keyboard_interactivity(2)
zwlr_layer_surface_v1#28.set_exclusive_zone(48)
zwlr_layer_surface_v1#28.set_exclusive_edge(1)
```

`set_size` is not judged: the shell sends `0, 48` and then the configured
`1280, 48`, and `surface/first-configure` already checks the size.

Recorded on 2026-10-18T12:02+02:00 with `make verify-shell` (kernel
7.2.10-1-cachyos, KWin 6.8.0, Intel Arc A380): all 25 cases passed, among
them `surface/unknown-schema`, `surface/unknown-layer` and
`surface/parameters-read-back`. The unit tests in `tools/test_verify_shell.py`
were run once each against a planted defect, then restored: a judge that
skipped `set_exclusive_edge` failed two tests, and one that took any refusal
failed both reason tests.

## The filter proxy

`tools/wayland_filter.py` relays one client to the compositor and passes file
descriptors through as `SCM_RIGHTS`. It parses only what it needs: registry
globals and binds, `get_layer_surface`, and `wl_display.delete_id`. When the
compositor deletes an id, the proxy forgets it, because the client reuses
freed ids for new objects. Without this, the proxy read a `wl_callback.done`
on a reused registry id as a truncated `wl_registry.global` and dropped the
connection.

A framing error ends that one relay and is reported on the proxy's stderr. The
client then sees the end of the stream, so the error cannot pass as a case
result.

## The recorded run

Reference profile, 2026-10-18T05:56+02:00, kernel 7.2.10-1-cachyos, packages
`kwin 6.8.0-1.1`, `wayland-utils 1.3.0-1.1` and `dbus 1.16.2-1.1`, gpui at the
revision pinned in `docs/roadmap/toolchain-admission.md`:

```text
CASE surface/first-configure pass
CASE surface/filter-control pass
CASE surface/layer-shell-missing pass
CASE surface/first-configure-deadline pass
CASE surface/refused-line pass
RESULT pass
```

Between the last `CASE` line and `RESULT pass`, the run printed its D79 note:
`NOTE D79: headless KWin kwin 6.8.0 (kwin_wayland --virtual); development
evidence for the client half only, not evidence that P04 serves
zwlr_layer_shell_v1` (one line in the output).

The first live runs failed, and each failure was a defect that is now fixed:

| Observed | Cause | Fix |
| :--- | :--- | :--- |
| three cases exit 6 | the gate passed the request without its newline | `REFERENCE_LINE` ends in `\n` |
| `surface/refused-line` passed anyway | that refusal was for the newline, not for `jsonrpc` | the judge requires `jsonrpc` |
| drop case exit 101, a wgpu EGL panic | the proxy dropped the connection on a reused id | `wl_display.delete_id` handling |
| hide case timed out (124) | gpui runs the launch callback before calloop's `EventLoop::run`, which clears an earlier stop request | `quit_from_loop` in `surface.rs` quits from inside the running loop |

## The AT-SPI cases

Milestone M28 epic E28-3 verifies that the shell's AccessKit tree reaches
AT-SPI live and a reader built on `atspi` and `zbus` reads it back on a
private D-Bus session.

### The private bus recipe

The gate never connects to the host session bus or operator compositor. It
provisions an isolated AT-SPI environment in `XDG_RUNTIME_DIR`:

1. Starts `dbus-daemon --session --nofork --print-address=1` with address
   `unix:path=<rt>/a11y-session-bus` and records `DBUS_SESSION_BUS_ADDRESS`.
2. Starts `at-spi-bus-launcher` with `GSETTINGS_BACKEND=memory`.
3. Sets accessibility status via `gdbus call --session -d org.a11y.Bus`
   `-o /org/a11y/bus -m org.freedesktop.DBus.Properties.Set`
   `org.a11y.Status IsEnabled '<true>'`.
4. Starts `at-spi2-registryd` directly in the same environment, because D-Bus
   service activation for the registry fails on a private non-systemd bus.
5. Queries the a11y bus address with `gdbus call --session -d org.a11y.Bus`
   `-o /org/a11y/bus -m org.a11y.Bus.GetAddress` and polls
   `Accessible.GetChildren` on `/org/a11y/atspi/accessible/root` until the
   registry responds.

### Cases and variants

The verification binary `aegis-atspi-readback` and fixture binary
`aegis-forum-shell-gpui a11y <request-line> <variant>` run across six cases:

| Case | Variant | Passes when |
| :--- | :--- | :--- |
| `atspi/sound` | `sound` | The fixture button has role button, name, and click action; focus is on the button; all 1,000 seeded nodes are reachable and named; hidden subtree is hidden; clicking the button mutates its name to "Forum opened" and emits `EVENT click`. |
| `atspi/late-client` | `sound` | A client started 0.5 s after the surface maps still receives the complete 1,000-node accessibility tree. |
| `atspi/negative-button-as-container` | `button-as-container` | Readback exits 1, fails only `atspi/button-exposed`, and names role "panel". |
| `atspi/negative-unlabelled` | `unlabelled` | Readback exits 1, fails only `atspi/every-node-named` (naming path index 517) and `atspi/seeded-reachable` (the row lost its "Node 517" name). |
| `atspi/negative-culled` | `culled` | Readback exits 1 and fails only `atspi/seeded-reachable` with "10 of 1000". |
| `atspi/negative-hidden-shown` | `hidden-shown` | Readback exits 1 and fails only `atspi/hidden-subtree-hidden`, naming "Hidden panel". |

`NEGATIVE_EXPECT` in `tools/verify_shell.py` holds the failing set per variant.
A negative that fails for any other reason, a read error (exit 3) included,
fails its gate case.

### AccessKit filtering and tree structure

- **Hidden subtree semantics**: `accesskit_consumer` (0.38.0 `src/filters.rs:22`)
  excludes hidden subtrees entirely from the exported AT-SPI tree. "Reported
  hidden" means no reachable node with a hidden label reports `State::Showing`.
  The fixture renders a visible sibling ("Shown panel") so the check proves
  the reader traversed that section of the tree.
- **Clips children finding**: `accesskit_consumer` (`src/filters.rs:54-90`) only
  culls off-screen list items when the parent element sets `clips_children()`.
  Because gpui never sets this property, all 1,000 seeded list items remain
  reachable in the AT-SPI hierarchy even though only 10 rows fit the viewport.
  The planted `culled` variant simulates incorrect culling by rendering only
  `0..IN_VIEW` rows.
- **Deadline unit fixture**: Subprocess calls against a hanging D-Bus interface
  risk blocking or destabilizing the live test harness. In accordance with
  HISS-02, `crates/aegis-forum-shell-gpui/tests/readback_deadline.rs` verifies
  `with_deadline` behavior against a mock `Silent` peer on a peer-to-peer
  `UnixStream`, ensuring zero-deadline, hanging calls, and responsive calls
  are tested deterministically without touching the live bus.
- **D79 note**: Live execution outputs
  `NOTE versions: at-spi2-core <v>, dbus <v>` and the D79 note verifying
  development evidence for the client half only.

### Evidence run

`make verify-shell` on the reference profile, 2026-10-18: all 11 cases passed,
with `NOTE versions: at-spi2-core 2.62.1, dbus 1.16.2` and headless KWin 6.8.0.
The readback read 1,005 nodes from the sound fixture: the application, the
window, the button, the list, 1,000 rows and the shown panel. The hidden
subtree was absent. The `hidden-shown` variant read 1,008 nodes, three more:
the panel and its two children. The `culled` variant read 15 nodes.

## The theme

The shell paints only with the Concordia tokens (E28-5, D05, D101).
`tools/concordia_theme.py` generates `crates/aegis-concordia/src/tokens.rs`
from `ui/concordia-tokens/concordia-tokens.css`. The shell's
`src/theme.rs` turns those constants into gpui colours, and it is the only
file in the crate that builds one. `tests/theme.rs` runs inside
`make verify-all` and needs no display. It reads every source file of the
crate and fails on a call to `rgb(`, `rgba(`, `hsl(` or `hsla(` anywhere
outside `src/theme.rs`. Before this change, it named the two hard-coded
backgrounds in `src/surface.rs` and `src/fixture.rs`.

## The focus-ring cases

The shell draws focus rings in screen space so their rendered width remains
invariant under camera zoom (E28-4, REQ-P05-13). When the camera zooms out
to `MIN_SCALE` (0.125) or in to `MAX_SCALE` (8.0), the focus ring must remain
at least 2.0 px wide and maintain a 3:1 contrast ratio against the surface.
A ring drawn inside the scaled layer would shrink with zoom and become
unreadable.

The fixture displays a node with a focus ring on KWin's workspace, and the
readback client captures the composited pixels via KWin's `ScreenShot2` D-Bus
interface (`org.kde.KWin.ScreenShot2.CaptureArea`). The client passes a pipe
writer file descriptor to KWin, drains the raw pixel bytes into memory under a
strict deadline, and measures the painted edges perpendicular to each side of
the node.

A case passes when at least one side measures at least 1.95 px (the 2.0 px
floor less the 0.05 px `WIDTH_TOLERANCE_PX` in `src/ring.rs`) at a contrast
of at least 3:1 (`judge_ring` in `src/pixels.rs`). One side suffices because
a panel may cover the rest of the ring. Eight cases cover the camera scales,
planted defects, occlusion and the width boundary. A negative case passes
only when the readback fails it, and only for the expected reason: its best
measured width must lie in the range `RING_NEGATIVE_WIDTH` in
`tools/verify_shell.py` names.

| Case | Variant | Expected |
| :--- | :--- | :--- |
| `ring/min` | `min` | Camera at `MIN_SCALE` (0.125): ring passes. |
| `ring/unit` | `unit` | Camera at 1.0: ring passes. |
| `ring/max` | `max` | Camera at `MAX_SCALE` (8.0): ring passes. |
| `ring/partly-covered` | `partly-covered` | A panel covers the left side; the other three pass. |
| `ring/width-2-0` | `width-2-0` | Boundary: a 2.0 px ring passes. |
| `ring/scaled-layer` | `scaled-layer` | Planted defect: ring drawn in layer space at scale 0.5 measures 1.4 to 1.6 px and fails. |
| `ring/under-panel` | `under-panel` | A panel covers the whole ring; no side measures above 0.5 px. |
| `ring/width-1-9` | `width-1-9` | Boundary: a 1.9 px ring measures 1.8 to 1.95 px and fails. |

A passing ring case prints its widest side as a `NOTE`, for example
`NOTE ring/width-1-9: best top 1.902 px, 5.85:1`.

### Making the capture deterministic

A capture taken as soon as the fixture prints `READY` showed the empty
workspace or a frame of KWin's fade-in, about one run in three. Three
measures close that, and each one alone was measured as insufficient:

- **READY after a drawn frame.** gpui runs `on_next_frame` callbacks at the
  start of a frame, before it draws and presents (`crates/gpui/src/window.rs`
  at the pinned zed revision). The fixture therefore prints the `RING` line
  and `READY` from a second nested `on_next_frame`, after the first frame
  with the ring has been drawn.
- **Animations off.** The gate starts KWin with `XDG_CONFIG_HOME` pointing at
  a directory it writes itself (`write_kwin_config`), holding `kdeglobals` and
  `kwinrc` with `[KDE] AnimationDurationFactor=0`. This also keeps KWin from
  reading the operator's own configuration.
- **Surface probe.** Before measuring, the readback captures the single pixel
  at `SURFACE_PROBE` (4, 4) until it shows `COLOR_SURFACE`, at most 50
  attempts 20 ms apart (`wait_for_pixel` in `src/capture.rs`). That point
  lies outside every ring and panel (`tests/ring.rs`), and the empty
  workspace reads black there.

### Exact stroke widths

gpui snaps border widths to whole device pixels: a 1.9 px border painted as
2.0 px, so the `width-1-9` case passed, and a 1.5 px border painted as
1.0 px. `stroke` in `src/ring_fixture.rs` paints a border of
`ceil(width)` px in a colour mixed from `COLOR_SURFACE` toward
`COLOR_FOCUS_RING` by `width / ceil(width)`. The coverage the readback sums
then equals the requested width: 1.902 px for 1.9 and 1.501 px for 1.5 in
the evidence run below. The lighter mix also lowers the measured contrast,
to 5.85:1 and 3.86:1.

### The ScreenShot2 call

KWin 6.8.0 declares
`CaptureArea(i x, i y, u width, u height, a{sv} options, h pipe)`: width
and height are unsigned, read by introspecting `org.kde.KWin.ScreenShot2`.
A call with signed sizes returns `UnknownMethod`. `capture_area` converts
the sizes and refuses negative ones. The reply must be raw format at
scale 1.0.

### Pixel measurement algorithm

- **Anti-aliased coverage**: Pixel coverage is computed as the vector
  projection of `(pixel - surface)` onto `(ring - surface)`, clamped to
  `[0.0, 1.0]`. This measures sub-pixel and anti-aliased edge contributions
  without hard thresholding.
- **Perpendicular scanlines**: For each side (Left, Right, Top, Bottom), a
  scanline perpendicular to the edge through the side's midpoint sums coverage
  across the band from `outer_edge - 1` to `inner_edge + 1`.
- **WCAG contrast ratio**: Relative luminance is linearised from sRGB using the
  standard 0.04045 piecewise threshold. Contrast is evaluated as
  `(L1 + 0.05) / (L2 + 0.05)` between the peak ring pixel and the adjacent
  surface pixel.
- **D-Bus activation isolation**: KWin and the readback client share a
  dedicated session bus configured via `write_no_activation_config`. The config
  omits `<standard_session_servicedirs />`, so no call on that bus activates
  a desktop portal or another KDE service. Before this, a ring run left an
  activated `xdg-desktop-portal` behind.

### Ring evidence run

`make verify-shell` on the reference profile, 2026-10-18, headless KWin
6.8.0: all 19 cases passed, the eight ring cases among them. The readback
measured, per side:

| Variant | Width (px) | Contrast | Verdict |
| :--- | :--- | :--- | :--- |
| `min`, `unit`, `max` | 3.000 on all sides | 6.44:1 | pass |
| `partly-covered` | 0.347 left, 3.000 on the other sides | 6.44:1 | pass |
| `width-2-0` | 2.000 on all sides | 6.44:1 | pass |
| `scaled-layer` | 1.501 on all sides | 3.86:1 | fails as expected |
| `under-panel` | 0.347 on all sides | 1.00:1 | fails as expected |
| `width-1-9` | 1.902 on all sides | 5.85:1 | fails as expected |

Three probe runs of all eight variants before it gave the same figures. With
the `width-1-9` variant changed to paint 2.0 px, the same gate failed
`ring/width-1-9` with "readback unexpectedly passed for negative variant
width-1-9".

## The frame-time cases

The shell measures frame durations and AccessKit tree-update times over an
active 1,000-node accessibility tree (M28 E28-6, REQ-P05-11, ADR-0001,
ADR-0004, D115). Rows scrolled out of the 400-pixel list stay in the tree
(REQ-P05-11), so every seeded row is a node in every update.

### What is measured

Each frame sample captures:

- **Frame duration**: The total duration of `Window::draw` in nanoseconds,
  recorded through gpui's profiler tracing (`FrameEvent::Draw`).
- **Post-paint residual (tree-update time)**: gpui exposes no direct timer for
  the accessibility tree update. Within `Window::draw`, `DrawPhases` records the
  layout, prepaint, and paint durations. Immediately following paint, while the
  accessibility adapter is active, gpui calls `self.a11y.end_frame()`, emits the
  debug log line `Sending a11y tree update: N nodes` (target `gpui::window`),
  and pushes the update to the platform window's AccessKit adapter. The
  residual `draw_duration.saturating_sub(request_layout + prepaint + paint)`
  contains the AccessKit tree synthesis and adapter update, plus the draw's
  own bookkeeping before layout and after paint. It is a derived upper bound
  on the tree-update time, not a direct timer.
- **Log pairing**: The fixture installs a safe logger ring buffer capturing
  `(Instant, node_count)` tuples. Each drawn frame is paired with the tree
  update whose instant falls within `[draw_start, draw_end]`, proving that an
  update was built in that frame and confirming its node count.

### Recorded frame count and warm-up

The measurement records exactly 600 frames following a 60-frame warm-up window
(approximately 10 seconds of steady animation at 60 Hz). This count is a
measurement sample size established by E28-6, not a performance target or
budget. Neither frame time nor tree-update duration enforces any threshold.

### Acceptance cases and refusals

Three cases evaluate positive, negative, and boundary criteria:

| Case | Seeded | Adapter | Expected Outcome |
| :--- | :--- | :--- | :--- |
| `timing/1000-nodes` | 1,000 | Active | Pass: 1,004 nodes per update (the 1,000 rows plus four other nodes); distributions (min, p50, p95, p99, max, mean), GPU, driver, compositor and zed commit recorded. |
| `timing/999-nodes` | 994 | Active | Planted defect: 994 rows yield 998 nodes, under the 1,000 floor; refused as too few nodes. |
| `timing/adapter-inactive` | 1,000 | Inactive | The session bus points at a socket that does not exist, so no AT-SPI client activates the adapter and no update is logged; refused as adapter inactive. |

Boundary behavior: exactly 600 post-warm-up frames are accepted; a run of 599
frames is refused as `TooFewFrames`. A tree holding exactly 1,000 nodes is
accepted. A software-emulated renderer (e.g. llvmpipe) is refused as
`SoftwareRenderer`. Any empty device, driver, compositor, or commit identifier
is refused as `MissingInfo`.

### Allocation share via perf (D115)

In accordance with decision D115, gpui's per-frame AccessKit allocation is a
scoped HISS-03 deviation. When Linux `perf` is installed on `PATH`, the harness
wraps the positive run with `perf record -F 999` and inspects the resulting
profile with `perf report --no-children --sort sym --stdio --percent-limit 0`.
It sums CPU samples attributed to memory allocation symbols (`malloc`, `free`,
`calloc`, `realloc`, `tcache_*`, `__rust_alloc`, `__rust_dealloc`,
`alloc::alloc::*`) across all threads in the fixture process, reporting the
allocator percentage as a `NOTE`.

Allocation measurement limits:

- **Sampled**: Profile figures reflect statistical sampling at 999 Hz, not exact
  call counts or heap byte totals.
- **Process-wide**: The sample captures all threads in the shell process,
  including gpui's event loop, async workers, and Wayland dispatch.
- **Symbol allow-list**: Only symbols matching the known glibc and Rust
  allocator allow-list are included in the sum.

When `perf` is absent, the gate logs `SKIP: perf not installed; allocation`
`share not measured` and completes without error. The allocation share value
never fails the case.

### Cargo timing feature

Frame profiling is gated behind the `timing` cargo feature in
`crates/aegis-forum-shell-gpui/Cargo.toml` (`[features] timing =`
`["gpui/profiler"]`). It is off by default, so the product build compiles
gpui without its profiler. Enabling it adds one package to `Cargo.lock`,
`hdrhistogram` 7.6.0 (MIT or Apache-2.0, default features off). The gate
builds the shell twice: the other cases run the build without the feature.

### Reference profile context (D79)

All figures measure the GPU, driver and KWin recorded in the evidence run,
on a headless `kwin_wayland --virtual` (D79). They do not characterise the
product's hardware or compositor.

### Frame-time evidence run

Recorded on 2026-10-18 with `python3 tools/verify_shell.py --timing-out
<file>`: all 22 cases passed. Reference profile: Intel Arc A380 (DG2), Mesa
26.2.4 (Intel open-source driver), KWin 6.8.0 headless, gpui at zed
`322cd509dc`. The host ran other work at a load average between 32 and 34
on 32 cores, so the figures carry that contention.

| `timing/1000-nodes`, 600 frames, 1,004 nodes | min | p50 | p95 | p99 | max | mean |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: |
| Frame (`Window::draw`), ms | 17.46 | 27.61 | 36.09 | 40.71 | 46.40 | 28.00 |
| Tree-update residual, ms | 2.30 | 4.35 | 6.67 | 8.22 | 9.94 | 4.51 |

Allocator share: 1.4 % of the CPU samples perf took in the fixture process,
all threads. A run on the same host at a load average of 35 to 49 measured
frame p50 23.21 ms and tree-update p50 3.76 ms; its perf report timed out
before the call-graph option was dropped. The two runs differ by about a
fifth at the median, which is the noise this host adds; neither run is a
benchmark.

## What is not claimed

- That P04 serves `zwlr_layer_shell_v1`. The compositor here is KWin.
- That the timing figures reflect final product performance: the figures measure
  that GPU, driver, and KWin, not the product, and no target is set.
- That the pixel cases measure P04's painted output: they measure KWin's
  composited workspace.
- A run on any host but the reference profile, or on any KWin version but the
  one recorded above.
