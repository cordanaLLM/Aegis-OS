# Toolchain admission matrix

Status: recorded admissions, current as of 2026-09-13

## Purpose and limits

Every milestone in this roadmap carries a toolchain-admission clause: a tool a
gate runs is selected through the template matrix with a pinned version before
that gate runs, and installation is never admission. This page is that matrix.
It records, for each tool a gate actually executes, the pinned version, the file
or workflow that pins it, what it replaced if it replaced anything, and the
milestone that admitted it.

The page is a record of pins, not of results. A row here says that a tool is
selected and pinned; it says nothing about whether the gate that uses it passed,
and it closes no blocked gate. Native build, image, boot, hardware,
accessibility and release remain separate blocked gates.

Two rules follow from the clause and are applied below:

- A tool a gate runs and that nothing pins is a gap, not an admission. Those
  tools are listed under [Not yet admitted](#not-yet-admitted) rather than
  quietly inherited from whatever the runner or the workstation happens to ship.
- A pin that replaces an earlier source of the same tool cites both values, so
  the substitution is auditable from this page alone rather than from a shell
  history.

## Toolchains and gate tools

| Tool | Pinned version | How it is pinned | Replaces | Admitted by |
| :--- | :--- | :--- | :--- | :--- |
| rustup | 1.29.1 | distribution package `extra/rustup 1.29.1-1.1` on the reference profile; CI uses the runner's rustup | nothing; rustup was absent before D61 | M02 (D61) |
| Rust (`rustc`, `cargo`) | 1.98.1 | `rust-toolchain.toml`, `channel = "1.98.1"`, materialised by rustup | distribution package `rust 1:1.98.1-1.1`, removed 2026-09-13 because a distribution package moves with system updates and cannot satisfy a pinned admission | M02 (D61) |
| clippy | 0.1.98 | `rust-toolchain.toml`, `components = ["rustfmt", "clippy"]` | the clippy shipped inside the removed `rust 1:1.98.1-1.1` package | M02 (D61) |
| rustfmt | 1.9.0-stable | `rust-toolchain.toml`, same `components` list | the rustfmt shipped inside the removed `rust 1:1.98.1-1.1` package | M02 (D61) |
| praetorctl | source commit `8d617bddf08042ad828d8a5c3c7852c4f000ce5d` | `.github/workflows/ci.yml`, `PRAETOR_COMMIT`, verified with `git rev-parse` before the build | nothing | M00 |
| Go toolchain | the version Praetor's own `go.mod` declares | `actions/setup-go` with `go-version-file: praetor-src/go.mod`, `GOTOOLCHAIN=local` | nothing; it is a build input for praetorctl, not a gate of its own | M00 |
| lefthook | 2.1.12 | `.github/workflows/ci.yml`, `LEFTHOOK_VERSION` plus the `LEFTHOOK_SHA256` checksum of the downloaded binary | nothing | M00 |
| reuse | 6.2.0 | `.github/workflows/ci.yml`, `REUSE_VERSION`, run through `pipx run` | nothing | M00 |
| markdownlint-cli2 | 0.23.2 | `.github/workflows/ci.yml`, `MARKDOWNLINT_VERSION`, run through `npx --yes` | nothing | M00 |
| yamllint | 1.38.0 | `.github/workflows/ci.yml`, `YAMLLINT_VERSION`, run through `pipx run` | nothing | M00 |
| flake8 | 7.3.0 | `.github/workflows/ci.yml`, `FLAKE8_VERSION`, run through `pipx run` | nothing | M00 |
| black | 26.5.1 | `.github/workflows/ci.yml`, `BLACK_VERSION`, run through `pipx run` | nothing | M00 |
| systemd (`systemd-repart`, `systemd-sysupdate`) | floor 261; reference profile `systemd 261 (261.3-1-arch)` | `tools/verify_systemd_definitions.py`, `SYSTEMD_FLOOR = 261` and `REFERENCE_PROFILE_SYSTEMD`, read back from `systemctl --version` before the gate runs | nothing | M03 |
| mkosi | 27; distribution package `extra/mkosi 27-1` on the reference profile | `build/mkosi.conf`, `MinimumVersion=27`, which mkosi itself enforces, and `tools/verify_mkosi_definitions.py`, `MKOSI_FLOOR = 27` and `REFERENCE_PROFILE_MKOSI`, read back from `mkosi --version` before the gate runs | the M01 register's inherited `mkosi v24+` floor from export-007, which was a range rather than a pin and which no gate enforced | M18 (D14, D56) |

The pinned Rust toolchain is materialised explicitly before the first cargo
gate, because a runner image that ships rustup does not thereby ship the pinned
channel or its components. `make verify-rust` reports
`rustup show active-toolchain` and `rustup which rustc` before the gates run, so
the evidence names the compiler that actually executed rather than a version
string copied from this page.

## Crates the workspace pins

Direct dependencies are declared once in `[workspace.dependencies]` in the root
`Cargo.toml` and referenced by the crate manifest, so a member cannot introduce
a second version of the same crate. `Cargo.lock` (lock format 4) is committed,
and every gate runs `--locked`, so a resolution change is a refused build rather
than a silent upgrade. No dependency here replaces an earlier one: the crate is
new at M02, and the MD5 hashing the imported sketch used was dropped for the
SHA-256 trait boundary (D02) rather than substituted crate for crate.

| Crate | Pinned version | How it is pinned | Role | Admitted by |
| :--- | :--- | :--- | :--- | :--- |
| base16ct | 1.0.0 | `[workspace.dependencies]`, `default-features = false` | lower-case hexadecimal digest rendering and parsing | M02 |
| sha2 | 0.11.0 | `[workspace.dependencies]`, `default-features = false` | the single `LedgerHasher` implementation (D02) | M02 |
| thiserror | 2.0.20 | `[workspace.dependencies]`, `features = ["std"]` | typed error enums at every boundary | M02 |
| serde | 1.0.229 | `[workspace.dependencies]`, optional, behind the `jsonl` feature | JSON Lines rendering, off the hashing path | M02 |
| serde_json | 1.0.151 | `[workspace.dependencies]`, optional, behind the `jsonl` feature | JSON Lines rendering, off the hashing path | M02 |

The resolved graph those five pull in is fixed by `Cargo.lock`:
block-buffer 0.12.1, cfg-if 1.0.4, cpufeatures 0.3.1, crypto-common 0.2.2,
digest 0.11.3, hybrid-array 0.4.15, itoa 1.0.18, libc 0.2.189, memchr 2.8.3,
proc-macro2 1.0.107, quote 1.0.47, serde_core 1.0.229, serde_derive 1.0.229,
syn 3.0.5, thiserror-impl 2.0.20, typenum 1.20.1, unicode-ident 1.0.24 and
zmij 1.0.23. `crates/aegis-justitia/tests/manifest_hygiene.rs` asserts that the
lock contains the SHA-256 implementation and no MD5 implementation under any
spelling.

## The systemd floor, and why it is a floor rather than a pin

The Rust rows above pin an exact toolchain because `rust-toolchain.toml` can
materialise it. systemd cannot be materialised that way: it is the host's init
system, and the gate runs the copy the machine already has. The admission is
therefore a **floor plus a recorded reference value**, and both halves are
mechanical:

- `SYSTEMD_FLOOR = 261` in `tools/verify_systemd_definitions.py`. Below it the
  gate prints why it did not run, naming the host version and the floor, and
  does not report a pass.
- `REFERENCE_PROFILE_SYSTEMD = "systemd 261 (261.3-1-arch)"`, the exact first
  line of `systemctl --version` on the reference profile on 2026-09-13. It is
  the value the negative and boundary outcomes in `docs/build/definitions.md`
  were observed on.

`tools/test_systemd_definitions.py` asserts that the floor and the recorded
reference value agree, so raising one without the other fails the gate rather
than passing silently. A future floor change is then a visible diff in this
page and in that constant, not an assumption inherited from whatever the
workstation happens to ship.

What the floor does **not** claim: that an older systemd cannot read these
definitions. It claims only that the exit codes and diagnostics this repository
records were observed on 261, and that a gate running below the floor would be
reporting against unobserved behaviour. CI runners at the time of writing ship
an older systemd, so the gate skips there and says so; the definition parser
`crates/aegis-fabrica-defs` runs everywhere and covers the same files without
systemd.

## The mkosi pin, and why its floor enforces itself

mkosi is a distribution package like systemd, so it cannot be materialised from
a file in this repository the way `rust-toolchain.toml` materialises a Rust
toolchain. Its admission is therefore the same shape as the systemd one -- a
floor plus a recorded reference value -- with one addition that systemd has no
equivalent for:

- `MinimumVersion=27` in `build/mkosi.conf`. mkosi reads it and refuses a
  configuration that asks for a newer mkosi than the one running, so the floor
  is enforced by the tool rather than by a document. Raising the `MinimumVersion`
  above the installed version was observed to print `mkosi 28 or newer is
  required by this configuration (found 27)` and exit 1 on the reference
  profile; setting it to the floor itself exits 0. Both outcomes are cases in
  `tools/verify_mkosi_definitions.py`.
- `MKOSI_FLOOR = 27` and `REFERENCE_PROFILE_MKOSI = "mkosi 27"` in that gate.
  Below the floor the gate prints why it did not run, naming the host version,
  the floor and the recorded package, and does not report a pass.
  `tools/test_mkosi_definitions.py` asserts that the floor and the recorded
  reference value agree.

What the pin replaces is recorded in the register: export-007 carried a floor of
"version 24 or later", a range that nothing enforced and that no gate read. M03
recorded mkosi as installed and deliberately **not** admitted, because no gate
there ran it. M18 admits it, because decision D56 keeps image-definition
validation in Aegis while Imago is a scaffold, and `make verify-mkosi` now runs
`mkosi summary` over `build/mkosi.conf`.

What the row does **not** claim: that an image was built. `mkosi summary`
resolves configuration and prints it; it downloads nothing, writes nothing into
the repository and constructs no image. The image gate stays blocked.

## Not yet admitted

These are run by a gate but pinned by nothing, so they are gaps recorded here
rather than admissions:

- `shellcheck`, run by the CI shell-lint step, comes from the runner image. No
  version is pinned and none is asserted.
- `python3`, which runs the preparation validator and its unit tests, comes from
  the runner image and from the workstation distribution.
- `npx` and `pipx` are the delivery mechanism for four pinned linters; the
  Node.js and Python runtimes behind them are the runner's.

Pinning them is a separate decision, not an implicit part of M02. Until it is
taken, no milestone may cite them as admitted toolchain.

## How to read a row back

- The Rust rows are checked by `crates/aegis-justitia/tests/manifest_hygiene.rs`,
  which fails if `rust-toolchain.toml` names a moving channel such as `stable`,
  if the channel is not an exact three-component version, or if the `rustfmt`
  and `clippy` components are missing.
- The workflow rows are the `env:` block of `.github/workflows/ci.yml`; each
  version appears exactly once and is interpolated into the command that runs.
- The crate rows are `[workspace.dependencies]` in the root `Cargo.toml` and the
  committed `Cargo.lock`.
- The replaced distribution package is recorded in the local package-manager log
  as `removed rust (1:1.98.1-1.1)` immediately before
  `installed rustup (1.29.1-1.1)` on 2026-09-13.
- The systemd row is checked by `tools/test_systemd_definitions.py`, which reads
  the floor and the reference banner back through the same parser the gate uses,
  and by the gate itself, which refuses to run below the floor.
- The mkosi row is checked by `tools/test_mkosi_definitions.py` the same way,
  and additionally by mkosi itself: `MinimumVersion=` in `build/mkosi.conf` is
  read by the tool, so the floor cannot drift away from what the tool accepts
  without the gate failing.
