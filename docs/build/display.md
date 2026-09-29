<!--
SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# The P17 display slice: VA-API frames to a layer surface

Status: recorded observations from milestone M27, reference profile, 2026-09-29

Milestone M27 is P17 Scaena's first slice (ADR-0003). Decoded video reaches
the compositor as a DMA-BUF, never as copied pixels (D75): the decoder exports
each surface as one fd, sends it over an `AF_UNIX` socket pair as `SCM_RIGHTS`
beside one line-delimited JSON-RPC 2.0 descriptor (D77), and P17 checks what
it received and attaches it to a `zwlr_layer_shell_v1` surface through
`zwp_linux_dmabuf_v1`. The half that needs no GPU runs inside
`make verify-all`; the half that needs the reference profile runs here:

```sh
make verify-display
```

**A pass is development evidence for the client half only (D79).** The
compositor it runs against is the host session's KDE KWin, not P04, and
nothing here is evidence that P04 serves `zwlr_layer_shell_v1` or
`zwp_linux_dmabuf_v1`: P04 serves no Wayland protocol, and no milestone yet
plans one that does. A pass closes no hardware, accessibility or release gate.

## What is tracked

| Path | Role |
| :--- | :--- |
| `crates/aegis-scaena/src/device.rs` | the decode node from the compositor's main device, through `/sys/class/drm` |
| `crates/aegis-scaena/src/decode.rs` | cros-libva: vendor check, MPEG-2 and JPEG decode, the ready poll, `export_prime` |
| `crates/aegis-scaena/src/jpeg.rs` | the baseline JPEG reader that fills the VA-API buffers |
| `crates/aegis-scaena/src/content.rs` | the index-row readback and CRC-32 over visible NV12 lines |
| `crates/aegis-scaena/src/reference.rs` | the reference MPEG-2 intra frame, third-party data |
| `crates/aegis-scaena/src/fixture.rs` | the committed fixture and its per-frame regression pins |
| `crates/aegis-scaena/fixtures/index-blocks-60.mjpeg` | the 60-frame Motion-JPEG fixture |
| `crates/aegis-scaena/src/present.rs` | the layer surface, the feedback table, import, commit, presentation |
| `crates/aegis-scaena/src/slice.rs` | the run and its cases; `src/bin/aegis-scaena-display.rs` starts it |
| `tools/verify_display.py` | the gate, run as `make verify-display` |
| `tools/test_display_slice.py` | the gate's half that runs inside `make verify-all` |

The transport, the descriptor, the attach checks, the surface state machine
and the Wayland waits (criteria 1 to 4, E27-1 and E27-3) are the same crate's;
`crates/README.md` lists its modules and tests, and
`docs/roadmap/toolchain-admission.md` records the toolchain the crate builds
with and the libva CI builds.

## What the gate does

`tools/verify_display.py` reads the build toolchain back through
`tools/display_toolchain.py`, holds the committed fixture to its pinned
sha256, builds `aegis-scaena-display` with `cargo build --locked`, and starts
it twice. `LIBVA_DRIVER_NAME` is set in each child's environment only; the
gate's own environment is never changed.

1. `aegis-scaena-display run` with `LIBVA_DRIVER_NAME=iHD` connects to the
   compositor `WAYLAND_DISPLAY` names, reads its registry under a deadline,
   creates a 256 x 64 layer surface on the overlay layer, bottom right, with
   `keyboard_interactivity` none, and waits for its first configure and its
   `zwp_linux_dmabuf_feedback_v1`. The main device comes from that feedback;
   the decode node is the `renderD` entry whose sysfs `dev` equals it, and its
   device file's `st_rdev` must equal it too. The VA display is opened on that
   node and its vendor string must name iHD before anything decodes.
2. The reference MPEG-2 frame decodes and is read back with `create_image`.
   A planted zero-filled surface is read back through the same check. Then
   each fixture frame is decoded, polled ready, read back against its index
   row, exported with `export_prime`, sent over the socket pair with its fd
   and received by P17, which checks it (`DMA_BUF_MAGIC`, the plane layout
   against the object size), admits its format and modifier against the
   surface's feedback, imports it with `zwp_linux_buffer_params_v1.create`,
   attaches and commits it with a `wp_presentation` feedback, and waits for
   `presented`. After frame 30 a descriptor for NV12 with
   `INTEL_4_TILED_DG2_RC_CCS`, over a duplicate of the frame's fd, is sent the
   same way and must be refused before any request; frame 31 must present.
3. `aegis-scaena-display driver-probe` with `LIBVA_DRIVER_NAME=nvidia`, the
   session's own value on the reference profile, resolves the same node and
   must be refused before anything is decoded or attached.

Without the reference profile's capabilities the gate prints `SKIP: <reason>;
the display slice gate did not run.` and exits 0, before it builds anything:
not Linux, no Wayland session socket, fewer than two DRM render nodes (negative
(b) needs a node that is not the compositor's main device), no iHD driver, no
cargo or rustc, or a session logind reports locked. Every case below must
report `PASS`. A case the binary reports with a `SKIP` line instead fails the
gate with the binary's reason; the one case it may report with a `NOTE` line
is the regression pin, as the fixture section says.

The gate prints rustc, libva, the VA vendor string, the compositor, the
decode node and the modifier the driver chose, and labels the run client half
only. It names the compositor from the Wayland socket's peer
(`SO_PEERCRED`), its executable from `/proc` and its package from
`pacman -Qo`; the compositor binary is never executed. On the reference
profile the socket's peer is `kwin_wayland_wrapper`, which holds the
listening socket for `kwin_wayland`; running it with `--version` prints the
version and then tries to take the session's socket lock, which is why the
gate reads the package database instead.

## The cases

| Case | Criterion | What passes |
| :--- | :--- | :--- |
| `display/attach-before-ack-refused` | boundary | an attach before the first `ack_configure` is refused by the surface state machine |
| `display/other-render-node-refused` | negative (b) | every render node whose device number is not the main device is refused, before anything is opened on it |
| `display/driver-is-ihd` | 5 | the VA vendor string names the iHD driver |
| `display/mpeg2-reference-crc` | 5 | the reference frame's visible NV12 lines hash to CRC-32 `0xa5713e52` |
| `display/planted-zero-surface-refused` | E27-2 negative | a zero-filled surface fails the index-row check |
| `display/fixture-frames-present` | 5, boundary | 60 frames read back, 60 `created`, 0 `failed`, a `presented` per commit, frame 1 and frame 60 included |
| `display/received-fd-is-the-exported-one` | 5 | every received fd has the exported one's `(st_dev, st_ino)` |
| `display/descriptor-carries-the-exported-modifier` | 6 | every descriptor carries `export_prime`'s modifier unchanged |
| `display/unadvertised-pair-refused` | negative (c) | NV12 with `INTEL_4_TILED_DG2_RC_CCS` is refused client-side, and the next frame presents |
| `display/frame-crc-regression-pin` | 5 | each frame's CRC-32 equals its recorded pin under the recorded driver; under another, a `NOTE` reports it, not held |
| `display/presented-deadline` | boundary | a presentation feedback on a surface never committed fails at its 250 ms deadline, naming the event |
| `display/driver-not-ihd-refused` | negative (a) | under `LIBVA_DRIVER_NAME=nvidia` the node's VA display is refused on its vendor string |

Every Wayland wait -- the registry roundtrip, the first configure, the
surface feedback, each `created` or `failed` and each `presented` or
`discarded` -- polls the connection fd under its own deadline, and a missed
deadline fails the case and names the event (HISS-02). cros-libva's `sync`
wraps `vaSyncSurface`, which takes no timeout, so every decode is polled with
`Surface::query_status` until `VASurfaceReady`, at most 4,000 polls within
2 s, before `sync` or `create_image` is called; a missed deadline names the
surface.

## The Motion-JPEG fixture

`crates/aegis-scaena/fixtures/index-blocks-60.mjpeg` is 48,477 bytes, sha256
`fd45d15a2837113fc2d581f04a9b82ca7992a6d2b8940f20ab9cc95fe6767be5`, pinned in
`tools/verify_display.py` and `tools/test_display_slice.py`. It is 60 baseline
JPEG frames of 256 x 64, 4:2:0, and each draws its frame number, 1 to 60, as a
row of 16 x 16 luma blocks on the MCU grid at (16, 16): one white guard block,
six bit blocks, most significant first, and one black guard block. The check
reads each block's mean luma from the decoded surface with `create_image`,
before the surface is exported, and thresholds it at 128, so a blank frame, a
frame out of order and a zero-filled surface all fail; the expected number
comes from the generator, never from an earlier run of the decoder.

It was generated for Aegis once, with ffmpeg n9.0.2, by the command below,
and carries the project licence through `REUSE.toml`. The gate never runs
ffmpeg; the command is its provenance, and running it again on 2026-09-29
reproduced the committed bytes exactly.

```sh
bit() { # the drawbox for bit $1, set in frame n when bit $1 of n+1 is 1
  x=$((32 + 16 * (5 - $1)))
  printf ',drawbox=x=%d:y=16:w=16:h=16:c=white:t=fill' "$x"
  printf ":enable='eq(mod(floor((n+1)/%d),2),1)'" "$((1 << $1))"
}
filter="color=c=0x808080:s=256x64:r=30,format=yuvj420p"
filter="$filter,drawbox=x=16:y=16:w=16:h=16:c=white:t=fill"
filter="$filter,drawbox=x=32:y=16:w=112:h=16:c=black:t=fill"
for b in 5 4 3 2 1 0; do filter="$filter$(bit "$b")"; done
ffmpeg -nostdin -hide_banner -loglevel error -f lavfi -i "$filter" \
  -frames:v 60 -c:v mjpeg -q:v 2 -huffman default \
  -fflags +bitexact -flags +bitexact -f mjpeg index-blocks-60.mjpeg
```

`FIXTURE_COMMAND` in `tools/verify_display.py` holds the same arguments as
one vector, and `tools/test_display_slice.py` checks that it draws two guards
and six bits.

The per-frame CRC-32 of the decoded visible NV12 lines is a regression pin
only (`RECORDED_FRAME_CRCS` in `src/fixture.rs`), recorded with the driver
that produced it, the iHD driver 26.2.4. A run under the same driver must
match every pin, and one that differs fails the case. A run under another
prints a `NOTE` line with how many differ and every measured value, and holds
none of them: the gate accepts that line as reported, not held, and names the
case so in its summary, so a driver update alone does not turn the gate red.
Re-recording means a new measurement under the driver that produced it (the
D73 pattern). The content check is the index row, never the pin.

## The reference MPEG-2 frame

`src/reference.rs` carries the one intra frame of cros-libva's
`libva_utils_mpeg2vldemo` test at the revision D80 pins, and the VA-API
parameters that decode it: third-party data, BSD-3-Clause through cros-libva
(© 2022 The ChromiumOS Authors), adapted there from libva-utils
`decode/mpeg2vldemo.cpp` (MIT, © 2007-2008 Intel Corporation). The file
carries both as file-level SPDX headers, not as a `REUSE.toml` table, because
`verify_licensing()` holds every table to the project's two identifiers;
`LICENSES/BSD-3-Clause.txt` is the canonical text, pinned by digest, and
`reuse lint` passes. The expected CRC-32, `0xa5713e52`, is upstream's
(`lib/src/lib.rs:265`) and is not re-recorded here: a driver that yields
another fails the case and names the driver, and adopting a new value is a
recorded decision.

## The recorded run

Run r20260929T151938-0d35 of `make verify-display`, on 2026-09-29, on the
reference profile; both children's output is kept under
`~/.cache/aegis-display/r20260929T151938-0d35`. The gate printed:

- rustc: rustc 1.98.1 (48a229cea 2026-09-01)
- libva: 2.24.1 (VA-API 1.24.0), read back with pkg-config
- VA vendor string: Intel iHD driver for Intel(R) Gen Graphics - 26.2.4 ()
- compositor: /usr/bin/kwin_wayland_wrapper (pid 3089, package kwin 6.7.5-1.1);
  client half only (D79)
- compositor main device 226:129, decode node /dev/dri/renderD129 (226:129,
  kernel driver i915)
- modifier the driver chose: 0x0100000000000009 (I915_FORMAT_MOD_4_TILED)

Every case passed:

- `display/attach-before-ack-refused`: refused before the first ack_configure: a
  buffer may not be attached before the first ack_configure.
- `display/other-render-node-refused`: refused (nvidia): render node renderD128
  is 226:128, not the compositor's main device 226:129; refused (amdgpu): render
  node renderD130 is 226:130, not the compositor's main device 226:129.
- `display/driver-is-ihd`: the vendor string above.
- `display/mpeg2-reference-crc`: CRC-32 `0xa5713e52` over the visible NV12
  lines.
- `display/planted-zero-surface-refused`: refused before export: the white guard
  block's mean luma is 0, below 128.
- `display/fixture-frames-present`: 60 frames decoded and read back against
  their index rows, frame 1 and frame 60 included; 60 created, 0 failed, 60
  committed, 60 presented, 0 discarded, 59 released; slowest decode reached
  VASurfaceReady after 3 polls.
- `display/received-fd-is-the-exported-one`: 60 of 60 received fds have the
  exported (st_dev, st_ino).
- `display/descriptor-carries-the-exported-modifier`: 60 of 60 descriptors carry
  export_prime's modifier unchanged.
- `display/unadvertised-pair-refused`: refused client-side after frame 30: the
  compositor did not advertise fourcc NV12 with modifier 0x010000000000000a;
  frame 31 then presented.
- `display/frame-crc-regression-pin`: frame 1 CRC-32 0x895e99b6, frame 60
  0x8fa24d23, under the recorded driver.
- `display/presented-deadline`: failed at its 250 ms deadline:
  wp_presentation_feedback presented or discarded did not arrive before its
  deadline.
- `display/driver-not-ihd-refused`, under `LIBVA_DRIVER_NAME=nvidia`: the vendor
  string was "VA-API NVDEC driver [direct backend]", refused before any decode.

Three later runs passed every case of both children again against the same
compositor process, with the same modifier and all 60 regression pins equal:
r20260929T163147-73a8 at 18:31 +02:00 on the same revision and
r20260929T163503-1020 at 18:35 +02:00, the slowest decode reaching
`VASurfaceReady` after 2 polls in each, and r20260929T170629-d8c5 at 19:06
+02:00 on the final revision, after 3. Between the first two the gate came to
take an absolute `WAYLAND_DISPLAY` by `Path.is_absolute()` and to split
`LIBVA_DRIVERS_PATH` on `os.pathsep`, as libva splits it (`va/va.c`,
`ENV_VAR_SEPARATOR`), so that its tests hold on the Windows leg of the
portability matrix (HISS-21); both read the same on Linux. Before the last,
the planted zero-filled surface came to be polled ready under the same
deadline before `create_image` and again before `sync`, as every decode is;
the earlier runs skipped that poll for that one surface, so the poll before
every `sync` and `create_image` rests on r20260929T170629-d8c5. The gate came
at the same time to accept the regression pin's `NOTE` under another driver,
to fail a case the binary skips with the binary's reason and to skip a host
with one render node; on the reference profile the case lines read as before.

Earlier on 2026-09-29 the session was locked. `make verify-display` then printed
`SKIP: logind reports session 3 locked; its compositor presents no client
surface while the lock screen is up; the display slice gate did not run.` and
exited 0, and the binary run directly passed the first five cases and failed the
first frame's presented wait at its 1 s deadline, naming the event: a locked
session's compositor presents no client surface.

## What is not claimed

- **HISS-03 on the frame path.** P17's own frame loop, the receive, the
  checks, the admission, the import, the commit and the waits in
  `slice.rs`, `transport.rs`, `attach.rs` and `present.rs`, allocates nothing
  in Aegis code per frame. cros-libva heap-allocates for every decoded
  picture (`Picture::new` boxes its state, and each JPEG and MPEG-2 parameter
  buffer is a `Box`), the deviation D83 limits to that binding. wayland-client
  also allocates for every protocol object it creates, and each frame creates
  a `zwp_linux_buffer_params_v1`, the `wl_buffer` its `created` event
  announces and a `wp_presentation_feedback`, which the criterion's per-frame
  `created` and `presented` events require. That allocation is third-party
  and outside D83's scope as recorded; it is disclosed with M27's evidence,
  not waived. No count is produced, because a counting allocator needs a
  `GlobalAlloc` implementation the crate's forbid lint does not admit.
- **A stuck decode on the hardware.** The deadline on the ready poll is
  exercised by `tests/decode_rules.rs` on a status source that never leaves
  `VASurfaceRendering`; the Arc A380 finished every decode within its first
  polls, and no run provoked a surface that stays busy.
- **The compositor's `invalid_format`.** The unadvertised pair is refused
  client-side, which is what the criterion asks; whether KWin would raise
  `invalid_format` for it is not tested, because a client test does not rest
  on one compositor's conformance.
- **Mapping.** Aegis code never maps a file: the received DMA-BUF is handed
  to the compositor, the compositor's format table is read with `pread`, and
  the crate's hygiene test finds no mapping call and rustix's `mm` feature is
  not admitted. The decoder's own images, read back with `create_image`
  before export, are mapped inside libva.
- **CI.** The Verification gate's runner has no GPU and no Wayland session,
  so `make verify-display` is not part of `make verify-all`; its
  hardware-free half is.

## Host safety

The run attaches one 256 x 64 layer surface without keyboard focus to the host
session for about sixty frames, a second or two, and destroys it with every
buffer on every path the binary can catch; the gate kills the binary at its
120 s deadline. Nothing is installed, no driver is bound, no device is
changed, and `LIBVA_DRIVER_NAME` exists only in the two children's
environments. Each run prints a run id and keeps both children's output in
`AEGIS_DISPLAY_DIR`, default `${XDG_CACHE_HOME:-$HOME/.cache}/aegis-display`,
under that id; nothing is written into the repository but cargo's target
directory.
