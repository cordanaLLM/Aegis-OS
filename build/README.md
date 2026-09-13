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

These are reviewed declarative inputs, not a built image. No image, artefact,
signature or boot evidence is produced or claimed here, and mkosi is not run by
any gate in this repository.

## Still reserved

Current candidates are indexed under `.workingdir/prepared/scaffold/build/`
(private, gitignored; not present in a clone); they are inactive proposal data.
See `planning/components.json` and `docs/integration/stack.md` for activation
prerequisites and shared ownership. No native pass is claimed by this directory.
