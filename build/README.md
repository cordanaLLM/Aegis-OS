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

These are reviewed declarative inputs, not a built image. No image, artefact,
signature or boot evidence is produced or claimed here. `mkosi summary` resolves
configuration and prints it; no gate in this repository runs `mkosi build`.

## Still reserved

Current candidates are indexed under `.workingdir/prepared/scaffold/build/`
(private, gitignored; not present in a clone); they are inactive proposal data.
See `planning/components.json` and `docs/integration/stack.md` for activation
prerequisites and shared ownership. No native pass is claimed by this directory.
