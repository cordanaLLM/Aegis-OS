# build

OS-specific configuration adapters (P01 mkosi/UKI synthesis, P02 repart,
sysupdate and TPM2 enrolment) that feed Imago image construction and consume
Nucleus kernel artifacts. Full activation still requires pinned producer schemas
and one locally tested request/result pair.

## Reviewed and gated (M03)

| Path | What it declares |
| :--- | :--- |
| `repart.d/` | the five partition drop-ins: ESP, root slots A and B, the dm-verity hash partition and the TPM2-sealed `/var` |
| `sysupdate.d/10-root.transfer` | the dual-slot A/B root transfer |

Each file cites the private source it was derived from by export id and sha256
in its own header, and says what changed and why. `make verify-all` runs
`tools/verify_systemd_definitions.py` over them with the host's own systemd, and
`crates/aegis-fabrica-defs` parses them without systemd. The recorded exit codes
and diagnostics, the open points, and the PCR acceptance target for M11 are in
`docs/build/definitions.md`.

## Reviewed and gated (M18)

| Path | What it declares |
| :--- | :--- |
| `mkosi.conf` | the P01 image definition: the pinned Arch snapshot (D18), the output format, the reviewed `repart.d` set and the package content list |
| `product-input.json` | the product input manifest: correlation id, exact revision, distribution pin, the four configuration references, package set, boot kernel identity (D07) and bounded retries |
| `kernel-requirement.json` | the kernel a conforming product must be built with, as a payload of Kconfig symbol rows |
| `kernel-requirement.reference.json` | what the reference profile must keep providing for the local milestones to stay runnable |

`make verify-mkosi` parses `mkosi.conf` with the host's own mkosi and requires
the Output stanza of the main image; the crate validates the two JSON payloads
and checks the kernel requirement against `planning/hardware-profile.json`. JSON
carries no comment syntax, so those three files cite their sources in-band
instead: each feature row names the recorded requirement it comes from. The
observations, the probe commands and their output are in
`docs/build/product-input.md`.
These files say what an A/B update *is*. What happens to a candidate release
moving through them -- signature check, delta acquisition, slot swap, boot
watchdog, and bless or rollback -- is modelled by `crates/aegis-janus-lifecycle`
and described in `docs/build/ab-lifecycle.md`. That model stubs every effect: it
runs no systemd command, opens no device and reboots nothing.

These are reviewed declarative inputs, not a built image. No image, artefact,
signature or boot evidence is produced or claimed here. `mkosi summary` resolves
configuration and prints it; no gate in this repository runs `mkosi build`.

## Reviewed and gated (M26)

| Path | What it declares |
| :--- | :--- |
| `kernel/source.pin.json` | the pinned Linux source: version, tarball digest, signature keys, the named base configuration, the fragment order and the expected kernel release |
| `kernel/10-base-support.config` | the Kconfig and guest-boot prerequisites the requirement rows depend on; it states no product requirement |
| `kernel/50-aegis-requirement.config` | the product requirement as a Kconfig fragment, byte-identical to what `KernelRequirement::config_fragment` renders from `kernel-requirement.json` |

Decision D70 puts kernel construction here while Nucleus is a scaffold, for the
same reason D56 keeps image-definition validation here while Imago is a
scaffold. `make verify-kernel` verifies the pinned source's digest and
signature, applies the two fragments to `x86_64_defconfig`, builds, boots the
result in a guest and makes the guest report its own `/proc/config.gz`. It is
**not** part of `make verify-all`: the reasoning is on the `verify-kernel`
target in the `Makefile`, and what CI does re-run is the crate and Python tests
that keep the fragment tied to the schema. Nothing it produces is a release
artifact and nothing is written into this repository. The commands, the guest's
own output and the scope limits are in `docs/build/kernel.md`.

## Still reserved

Current candidates are indexed under `.workingdir/prepared/scaffold/build/`
(private, gitignored; not present in a clone); they are inactive proposal data.
See `planning/components.json` and `docs/integration/stack.md` for activation
prerequisites and shared ownership. No native pass is claimed by this directory.
