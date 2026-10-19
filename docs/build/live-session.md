<!--
SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# The live session gate, and what a pinned guest session told it

Status: recorded observations from milestone M29 epic E29-4, reference
profile, 2026-10-19

Milestone M29 runs the M28 shell inside a real desktop session in a VM: a
Wayland compositor that advertises zwlr_layer_shell_v1, xdg-desktop-portal with
a settings backend, at-spi2-core and a StatusNotifierWatcher. Epic E29-4 is the
part that needs no shell: boot a pinned distribution guest under QEMU with KVM,
refuse it before boot when its bytes do not match the pin, and read back from
inside the guest which version of each session service answers. The other three
epics (portal settings, the AT-SPI2 bridge, the real watcher) run the shell in
this session and are not done here.

**M29 stays `blocked`.** This page records E29-4 only.

**A pass is development evidence in a VM on the reference profile, nothing
more.** The guest's compositor is KDE KWin, not P04, so D79's client-half rule
applies to anything run against it. The guest is Fedora's live image, not the
Aegis image (M11 builds that). A pass closes no hardware, accessibility or
release gate, and the image's accessibility precondition stays M11's
(REQ-P12-01).

## What is tracked, and what is not

| Path | Role |
| :--- | :--- |
| `build/session/guest.pin.json` | the pin: image URL, sha256, size, version and floor, the signed CHECKSUM and its key fingerprint, the boot shape, the services to probe with their deadlines, the Wayland globals the compositor must advertise |
| `tools/guest/aegis-session-report.sh` | the in-guest read-back, handed to the guest as a systemd credential |
| `tools/verify_live_session.py` | the gate, run as `make verify-session` |
| `tools/test_live_session.py` | its positive, negative and boundary tests, inside `make verify-all` |

The image, the extracted kernel and initramfs, the keyring and every run's logs
live outside the repository, under
`${XDG_CACHE_HOME:-$HOME/.cache}/aegis-session` (override:
`AEGIS_SESSION_DIR`). A run keeps its logs and `result.json`; the
extracted kernel and initramfs are deleted when it ends, and the newest eight
runs are kept.

## Running it

```sh
python3 tools/verify_live_session.py --fetch  # once: fetch, verify, cache
make verify-session                           # every run after that
```

`make verify-session` is a separate target, outside `make verify-all`, like
`make verify-boot`. Without KVM, without the admitted tools, without 12 GiB of
available memory and 15 GiB of free swap, or without the cached and verified
image it prints `SKIP: <reason>; the live session gate did not run.` and exits
0, so an exit 0 is evidence only with the case lines above it.

The admission is M24's, reused: `qemu-system-x86_64`, `xorriso`, `gpg` and
`curl`, with the rule that a version below the floor is refused and a version
different from the reference profile's is printed next to it. The QEMU row moved
to 11.1.2 on 2026-10-19 (`docs/roadmap/toolchain-admission.md`).

## How the guest is driven without changing its bytes

The live ISO is not modified and nothing is written into it.

1. The gate verifies the image before anything else: the CHECKSUM file's
   clearsigned signature against the pinned key fingerprint, the signed digest
   against the pin, then the size and sha256 of the cached file
   (`harness.verify_artifact_gpg` and `harness.require_pinned` from
   `tools/verify_boot_harness.py`).
2. It checks the bytes against the pin again, then extracts the kernel and
   initramfs with `xorriso`, then checks once more, and boots them directly
   with `-kernel`/`-initrd`, the ISO attached read-only as the CD-ROM.
3. The kernel command line names the live volume, sends the console to the
   first serial line, and asks systemd to start `aegis-session-report.service`.
4. Three SMBIOS type 11 credentials carry the unit itself
   (`systemd.extra-unit.aegis-session-report.service`), the read-back script and
   the list of services to probe. systemd in the guest installs the unit from
   the credential, so nothing on the image changes.
5. The live image logs `liveuser` into a Plasma Wayland session on its own. The
   script waits, under the pinned session deadline, for that user's session bus
   and Wayland socket, then probes each service under its own deadline, reads the
   owning process's package version with `rpm -qf`, lists the compositor's
   Wayland globals with `wayland-info`, reads the portal's Settings version and
   one setting, and reads the watcher's protocol version. It writes the report,
   with a fresh nonce, to the second serial line, and powers the guest off.

Every wait in the guest and on the host has a deadline (HISS-02): the session,
each service, the report and the shutdown are pinned in `guest.pin.json`.

## The cases

| Case | Kind | Passes when |
| :--- | :--- | :--- |
| `session/guest-verified` | positive | the CHECKSUM signature, the signed digest, the version and the image bytes all match the pin |
| `session/sha256-mismatch-refused` | negative | a pin whose sha256 differs in its last digit is refused at the digest, before any program starts and before any file is written |
| `session/positive` | positive | the guest booted under KVM, wrote a complete report carrying this run's nonce, every pinned service answered within its deadline with a version, the compositor advertised every required global, and QEMU ended with the guest's own power-off |
| `session/service-deadline-boundary` | negative and boundary | a planted service name that nothing owns is reported missing at its 3 s deadline, and the case fails naming it |
| `session/pinned-bytes-unchanged` | positive | the cached image still hashes to the pin after the boot |

## The recorded run

Run `r20261019T033652-9e0b`, 2026-10-19, reference profile, QEMU 11.1.2 with
KVM, `xorriso` 1.5.8.pl02, `gpg` 2.4.9, `curl` 8.23.0. All five cases passed.

The image: `Fedora-KDE-Desktop-Live-44-1.7.x86_64.iso`, 3368683520 bytes,
sha256 `c8295961d4c41adbf785a31a17c21a971d3b7415fda72dcad0c11c49577bf03a`,
CHECKSUM signed by key `DBFCF71C6D9F90A6` (Fedora 44). The guest ran kernel
`6.19.10-300.fc44.x86_64`.

What the guest read back, each name with the package of the process that owns
it and the seconds it waited:

| Label | Bus name | Package and version | Waited |
| :--- | :--- | :--- | ---: |
| compositor | `org.kde.KWin` | kwin 6.6.4-2.fc44 | 4 s |
| portal | `org.freedesktop.portal.Desktop` | xdg-desktop-portal 1.21.1-1.fc44 | 1 s |
| portal-backend | `org.freedesktop.impl.portal.desktop.kde` | xdg-desktop-portal-kde 6.6.4-1.fc44 | 0 s |
| a11y-bus | `org.a11y.Bus` | at-spi2-core 2.60.0-1.fc44 | 0 s |
| at-spi2-registry | `org.a11y.atspi.Registry` (accessibility bus) | at-spi2-core 2.60.0-1.fc44 | 0 s |
| watcher | `org.kde.StatusNotifierWatcher` | kf6-kded 6.25.0-1.fc44 | 0 s |

The compositor advertised `zwlr_layer_shell_v1` version 5. The portal's
`org.freedesktop.portal.Settings` interface is version 2, and it answered
`org.freedesktop.appearance color-scheme` with 2. The watcher reports protocol
version 0. The planted name `org.aegis.M29.PlantedAbsent` was reported missing
at its 3 s deadline, and the boundary case failed naming it, as it must. A pin
whose sha256 differed in one digit was refused at the digest before any program
started, and the cached image hashed to the pin after the boot.

The setting names E29-1 needs are to be read from the guest's
xdg-desktop-portal interface description when that epic is delivered; this run
read only the color scheme, to show the Settings backend answers.

## What this does not show

- The shell. E29-1, E29-2 and E29-3 run the M28 shell in this session; none of
  them ran here.
- P04. The compositor is KWin, so anything layer-shell observed here is the
  client half only (D79).
- The Aegis image. M11 boots that; this guest is Fedora's.
- Any hardware, accessibility or release gate. None is closed.
