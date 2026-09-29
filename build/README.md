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
| `kernel-requirement.schema.json`, `product-input.schema.json` | the JSON Schemas of both contracts, generated from the Rust types for consumers in other languages (D105); `crates/aegis-fabrica-defs/tests/json_schema.rs` fails when either drifts from the types |

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

## Reviewed and gated (M24)

| Path | What it declares |
| :--- | :--- |
| `boot/artifact.pin.json` | the externally supplied artifact the boot harness boots: compose URL, sha256, size, producer version and floor, the clearsigned CHECKSUM and its signing-key fingerprint, and the recorded login-prompt timeout |

This is not an image definition. It pins an upstream image that Aegis boots and
does not construct (D72, D84): `make verify-boot` verifies the CHECKSUM
signature by that fingerprint, hashes the cached bytes immediately before every
boot, and boots them headless under QEMU with KVM, OVMF and swtpm on a throwaway
overlay. It is **not** part of `make verify-all`; the reasoning is on the
`verify-boot` target in the `Makefile`. The bytes live outside the repository,
and the run, the PCR read-back and the scope limits are in
`docs/build/boot-harness.md`.

## Reviewed and gated (M10)

| Path | What it declares |
| :--- | :--- |
| `kernel/nucleus-artifact.pin.json` | the kernel cordanaLLM/nucleus built and published: release tag and commit, the `imago.nucleus.kernel-artifact.v1` manifest and every asset by sha256 and size, `kernel.release`, `kernel.config_digest`, and the keyless signer identity and issuer |

This pins a kernel Aegis boots and does not construct (D92). `make
nucleus-kernel-fetch` downloads the assets, refuses any that differ from the
pin and checks the cosign signature on `SHA256SUMS`; `make
verify-nucleus-kernel` records what imago verifies, hashes the image
immediately before every boot, checks `kernel-requirement.json` against the
configuration the kernel reports from inside the guest (D94), and only then
repeats the M19 loads. It is **not** part of `make verify-all`; the reasoning
is on the `verify-nucleus-kernel` target in the `Makefile`, and the run, its
one failure and the scope limits are in `docs/build/nucleus-kernel.md`.

## Still reserved

Current candidates are indexed under `.workingdir/prepared/scaffold/build/`
(private, gitignored; not present in a clone); they are inactive proposal data.
See `planning/components.json` and `docs/integration/stack.md` for activation
prerequisites and shared ownership. No native pass is claimed by this directory.
