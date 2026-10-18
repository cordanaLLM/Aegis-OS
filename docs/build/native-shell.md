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
4. It runs the five cases below. Every child has a 30 s timeout, and the
   binary waits at most `FIRST_CONFIGURE_DEADLINE` (5 s) for the first
   configure.
5. It prints one `CASE` line per case, then the D79 note, then
   `RESULT pass` only when all five cases ran and passed (`summary()` in
   `tools/verify_shell.py`).

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

The refused case must name `jsonrpc`. A refusal for any other reason, such as
a missing newline, fails the case.

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

## What is not claimed

- That P04 serves `zwlr_layer_shell_v1`. The compositor here is KWin.
- Anything about frame timing.
- That the pixel cases measure P04's painted output: they measure KWin's
  composited workspace.
- A run on any host but the reference profile, or on any KWin version but the
  one recorded above.
