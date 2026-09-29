# Toolchain admission matrix

Status: recorded admissions, current as of 2026-09-30

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
| rustup | 1.29.1 | distribution package `cachyos-extra-znver4/rustup 1.29.1-1.1` on the reference profile; CI uses the runner's rustup | nothing; rustup was absent before D61 | M02 (D61) |
| Rust (`rustc`, `cargo`) | 1.98.1 | `rust-toolchain.toml`, `channel = "1.98.1"`, materialised by rustup | distribution package `rust 1:1.98.1-1.1`, removed 2026-09-13 because a distribution package moves with system updates and cannot satisfy a pinned admission | M02 (D61) |
| clippy | 0.1.98 | `rust-toolchain.toml`, `components = ["rustfmt", "clippy"]` | the clippy shipped inside the removed `rust 1:1.98.1-1.1` package | M02 (D61) |
| rustfmt | 1.9.0-stable | `rust-toolchain.toml`, same `components` list | the rustfmt shipped inside the removed `rust 1:1.98.1-1.1` package | M02 (D61) |
| praetorctl | source commit `25451d888c8710822dd578907625dc69a0975142` | `.github/workflows/ci.yml`, `PRAETOR_COMMIT`, verified with `git rev-parse` before the build | the previous pin, source commit `11ac5d8ea360c2d71dd5647d1049d2f182d599f6`; the pin follows Praetor's `main` (see below) | M00 |
| Go toolchain | the version Praetor's own `go.mod` declares | `actions/setup-go` with `go-version-file: praetor-src/go.mod`, `GOTOOLCHAIN=local` | nothing; a build input for praetorctl and, since M09, for the imago `make contract-fetch` builds, held to imago's own floor below | M00; M09 for the imago build |
| lefthook | 2.1.12 | `.github/workflows/ci.yml`, `LEFTHOOK_VERSION` plus the `LEFTHOOK_SHA256` checksum of the downloaded binary | nothing | M00 |
| reuse | 6.2.0 | `.github/workflows/ci.yml`, `REUSE_VERSION`, run through `pipx run` | nothing | M00 |
| markdownlint-cli2 | 0.23.2 | `.github/workflows/ci.yml`, `MARKDOWNLINT_VERSION`, run through `npx --yes`; separately locked by Praetor's `tools/markdownlint/package-lock.json` for `make docs-lint`, which `praetorctl audit` requires byte-for-byte | nothing | M00; the Praetor gate with the `7e7746a` pin |
| yamllint | 1.38.0 | `.github/workflows/ci.yml`, `YAMLLINT_VERSION`, run through `pipx run` | nothing | M00 |
| flake8 | 7.3.0 | `.github/workflows/ci.yml`, `FLAKE8_VERSION`, run through `pipx run` | nothing | M00 |
| black | 26.5.1 | `.github/workflows/ci.yml`, `BLACK_VERSION`, run through `pipx run` | nothing | M00 |
| systemd (`systemd-repart`, `systemd-sysupdate`) | floor 261; reference profile `systemd 261 (261.3-1-arch)`, the value the outcomes were recorded on; the reference profile reads `systemd 262 (262-1-arch)` since 2026-09-25 | `tools/verify_systemd_definitions.py`, `SYSTEMD_FLOOR = 261` and `REFERENCE_PROFILE_SYSTEMD`; each tool is taken from `PATH`, else from `/usr/lib/systemd`, printed, and held to the floor by its own `--version` before the gate runs (`systemctl --version` is printed as a cross-check only) | nothing | M03 |
| mkosi | 27; distribution package `extra/mkosi 27-1` on the reference profile | `build/mkosi.conf`, `MinimumVersion=27`, which mkosi itself enforces, and `tools/verify_mkosi_definitions.py`, `MKOSI_FLOOR = 27` and `REFERENCE_PROFILE_MKOSI`, read back from `mkosi --version` before the gate runs | the M01 register's inherited `mkosi v24+` floor from export-007, which was a range rather than a pin and which no gate enforced | M18 (D14, D56) |
| Linux source | `linux-7.2.5`, sha256 `55ddf0df8325d9dad96fcff7bd93977d22e3f50af06527572af59b77c7632b78` | `build/kernel/source.pin.json`; the digest is re-checked on every run and the detached signature is verified against key `647F28654894E3BD457199BE38DBBDC86092693E` | nothing; no kernel source was pinned before | M26 (D70) |
| Kernel base configuration | `x86_64_defconfig` of the pinned source, plus two tracked fragments | `build/kernel/source.pin.json`; the requirement fragment is byte-identical to `KernelRequirement::config_fragment` and no full `.config` is tracked | nothing | M26 (D70) |
| clang (BPF target) | floor 19.0.0; reference profile `clang version 22.1.8`, distribution package `extra/clang 22.1.8-2` | `tools/verify_bpf_objects.py`, `CLANG_FLOOR` and `REFERENCE_PROFILE_CLANG`, read back from `clang --version` and `clang -print-targets` before anything is compiled | the M26 row recording clang as installed and **not** admitted, because no gate ran it; M19 runs it | M19 |
| bpftool | floor 7.4.0; reference profile `bpftool v7.8.0`, distribution package `core/bpf 7.2.5-1` | `tools/verify_bpf_objects.py`, `BPFTOOL_FLOOR` and `REFERENCE_PROFILE_BPFTOOL`, read back from `bpftool version` | nothing | M19 |
| libbpf (linked by the loader) | floor 1.5; reference profile `1.7.0`, distribution package `core/libbpf 1.7.0-1.1` | `tools/verify_bpf_objects.py`, `LIBBPF_FLOOR` and `REFERENCE_PROFILE_LIBBPF`, read back from `pkg-config --modversion libbpf` and again at runtime from `libbpf_version_string()` | the M01 register's `libbpf v1.8`, which was a reading of the wrong artefact (see below) | M19 |
| llvm-strip | floor 19.0.0; reference profile LLVM 22.1.8, same `extra/clang 22.1.8-2` toolchain | `tools/verify_bpf_objects.py`, `LLVM_STRIP_FLOOR`, read back from `llvm-strip --version` | nothing | M19 |
| rt-tests (`cyclictest`) | floor 2.10; reference profile `cyclictest V 2.10`, distribution package `rt-tests 2.10-1.1` from `cachyos-extra-znver4` | `tools/verify_latency_fixture.py`, `TOOLCHAIN`, read back from `cyclictest --help` before anything is measured | nothing; no latency tool was admitted before, and none was installed | M23 |
| Realtime kernel source | the M26 pin reused unchanged: `linux-7.2.5` with `CONFIG_PREEMPT_RT=y` from `build/kernel/50-aegis-requirement.config` | `build/kernel/source.pin.json`; the guest reports the release and the option back from inside itself | the distribution `linux-rt` package D57 recommended, which is **not** downloaded, installed or booted here | M23 (D57, D70) |
| Firecracker | 1.17.0, release archive `firecracker-v1.17.0-x86_64.tgz` sha256 `06094a1108ae9e82aa4c23a775aa92758f53f1175d422270d9d6162cb9ade558`, binary sha256 `99ad0f5cd0514a88aad0e9ae8cfdb3cc3b4ab9d190e1194602406c786b5de7a5`, Apache-2.0 | `build/sandbox/firecracker.pin.json`; fetched by `make workstation-fetch`, hashed again and read back with `--version` before every run; the jailer is not used | the distribution package `firecracker 1.17.0-1.1` at `/usr/bin/firecracker`, which M06 probed and no gate runs | M21 (D58) |
| Guest kernel for Firecracker | `vmlinux-6.18.44` from Firecracker's CI artifacts, sha256 `d8ced68bd61e27b6813e2c993cc53a4029c59e13210672180591c84109684fe4`, configuration sha256 `9fb2be18303d2f6e8ec35b3a20ecf1209f54a4ece10114a893cb37beabee7030` | `build/sandbox/firecracker.pin.json`; the gate hashes both and requires five configuration options before every run | nothing; no Firecracker guest was pinned before | M21 |

The pinned Rust toolchain is materialised explicitly before the first cargo
gate, because a runner image that ships rustup does not thereby ship the pinned
channel or its components. `make verify-rust` reports
`rustup show active-toolchain` and `rustup which rustc` before the gates run, so
the evidence names the compiler that actually executed rather than a version
string copied from this page.

The praetorctl pin follows Praetor's `main` branch. The maintainer's standing
rule, which this paragraph records, is that `PRAETOR_COMMIT` in
`.github/workflows/ci.yml` moves to the head of Praetor's `main` whenever `main`
moves. Between two bumps the pin lags `main`, so the praetorctl CI builds can
be older than a workstation build of the current head.

Bumps are automated. `tools/praetor_bump.py`, run on the maintainer's
workstation by the systemd user units `make install-praetor-bump` installs,
moves the pin whenever Praetor's `main` has a new head; see
[Praetor pin auto-bump](../build/praetor-bump.md). Each bump lands as a pull
request from the one rolling branch `chore/praetor-pin-auto` and updates the
praetorctl row above, where the previous pin becomes the "Replaces" value. It
runs `praetorctl adopt --force` from the new commit and puts every
hand-maintained file adopt rewrites back to its content on `main`: the hook and
editor settings, `lefthook.yml` and the evasion interceptor. It restores
`AGENTS.md`, whose rule table carries corrections that apply to this
repository, and recompiles only its text-register block and the projections.
Of the remaining changes it keeps only those that `praetorctl audit`,
`praetorctl compile-context --verify` and `praetorctl flavor audit` fail
without, and the pull request lists each kept and each reverted file. The
bump's own local gate decides only whether a pull request is proposed: it runs
the workstation's linters and Go toolchain, listed under
[Not yet admitted](#not-yet-admitted), and only the pull request's
Verification gate runs the versions pinned above. The bumps to `7e7746a` (#122)
and `7325896` (#130) were made by hand under the same rule.

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
| schemars | 1.2.2 | `[workspace.dependencies]`, `default-features = false`, `features = ["derive"]`; used by `crates/aegis-fabrica-defs` only | generates the JSON Schemas of both M18 contracts from their Rust types, committed under `build/` (D105) | D105, 2026-09-29 |
| accesskit | 0.25.1 (MIT OR Apache-2.0), crate checksum `ad442f58ee04714aaa0ba0a2768c1ea1935b29507bb22ddece8cbbc76db02932` | `[workspace.dependencies]`, `default-features = false`, used only by `crates/aegis-forum-shell` | the accessibility tree's data model: the P05 canvas model is exported as an `accesskit::TreeUpdate`; no platform adapter | M16 (D101) |
| cros-libva | git revision `59384456ac2ae78c0c3e5515f41ef1efd9b802cf` of chromeos/cros-libva (package 0.0.13, BSD-3-Clause), the merge of #37 on 2026-09-01 and the head of `main` on 2026-09-29 | `[workspace.dependencies]`, `git` with `rev =`; locked as `git+https://github.com/chromeos/cros-libva?rev=59384456...#59384456...`; used only by `crates/aegis-scaena` | VA-API decode and DMA-BUF export through `Surface::export_prime` (D80) | M27 (D80) |
| smithay-client-toolkit | 0.21.1 (MIT), crate checksum `74dc9ee14b0fdcb535f9556141bacac070c994a977d2240ee455d7438a617f00` | `[workspace.dependencies]`, `default-features = false`; used only by `crates/aegis-scaena` | the layer surface and the `zwp_linux_dmabuf_v1` client over wayland-client's pure-Rust backend; libwayland is neither linked nor loaded | M27 (D75) |
| rustix | 1.1.5 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT), crate checksum `891efababe418670775f199f0d233d84843c227a0949a883ce15b37c78d6629d` | `[workspace.dependencies]`, `features = ["net", "fs", "event"]`; used by `crates/aegis-scaena` and, since M21, by `crates/aegis-vesta-sandbox`, which adds the `system` feature | `sendmsg` and `recvmsg` with `SCM_RIGHTS` and `MSG_CMSG_CLOEXEC`, `SO_RCVTIMEO` and `SO_SNDTIMEO`, `fstatfs`, `fstat`, `lseek`, `poll` on the Wayland connection fd under a deadline, and in tests `memfd_create` (M27); `reboot(RebootCommand::Restart)`, with which the M21 guest init ends its microVM (M21) | M27 (D77); the `system` feature M21 |
| socket2 | 0.6.5 (MIT OR Apache-2.0), crate checksum `c3d1e2c7f27f8d4cb10542a02c49005dbd6e93095799d6f3be745fae9f8fedd4` | `[workspace.dependencies]`, `default-features = false`, `features = ["all"]`; used only by `crates/aegis-vesta-sandbox` | the guest's `AF_VSOCK` listener: `SockAddr::vsock` is a safe constructor of a `sockaddr_vm`, which rustix 1.1.5 does not offer and the workspace's `unsafe_code = "forbid"` would otherwise require | M21 |

The resolved graph the first seven pull in is fixed by `Cargo.lock`: block-buffer
0.12.1, cfg-if 1.0.4, cpufeatures 0.3.1, crypto-common 0.2.2, digest 0.11.3,
dyn-clone 1.0.20, hybrid-array 0.4.15, itoa 1.0.18, libc 0.2.189, memchr 2.8.3,
proc-macro2 1.0.107, quote 1.0.47, ref-cast 1.0.27, ref-cast-impl 1.0.27,
schemars_derive 1.2.2, serde_core 1.0.229, serde_derive 1.0.229,
serde_derive_internals 0.30.0, syn 3.0.5, thiserror-impl 2.0.20, typenum 1.20.1,
unicode-ident 1.0.24, uuid 1.26.1 and zmij 1.0.23.
`crates/aegis-justitia/tests/manifest_hygiene.rs` asserts that the lock
contains the SHA-256 implementation and no MD5 implementation under any
spelling.

schemars was admitted on 2026-09-29 against the crates.io API, read with a
`User-Agent`: 1.2.2 of 2026-07-27 is its newest stable release (D69), licensed
MIT, and the lock's checksum `687274d293b6...` is the one crates.io publishes.
It adds five crates, each at its newest release and each with the checksum
crates.io publishes: schemars_derive 1.2.2 (MIT) and dyn-clone 1.0.20, ref-cast
1.0.27, ref-cast-impl 1.0.27 and serde_derive_internals 0.30.0 (each MIT OR
Apache-2.0). Every one is a permissive licence a EUPL-1.2 work may depend on,
none is vendored into the repository, so REUSE is unchanged, and all of them
resolve on the syn 3.0.5 already in the lock. `default-features = false` drops
schemars' `std` feature, which implements schemas for standard-library
collections these types do not use.

accesskit 0.25.1, admitted by M16 on 2026-09-29, was the newest release on the
crates.io API that day, published 2026-09-25. With its features off it
resolves one crate more, uuid 1.26.1 (Apache-2.0 OR MIT, the newest release,
published 2026-09-10), with no feature and no dependency of its own. Its
`rust-version` is 1.87, above the 1.85 the workspace declared when it was
admitted; the pinned toolchain is 1.98.1, so every gate compiles it, and
Cargo's MSRV-aware resolver picks it only because `0.25.1` is the lowest
version the requirement admits. Since M27 the workspace declares 1.87, the
highest `rust-version` in the resolved graph (see below).
`crates/aegis-forum-shell/tests/manifest_hygiene.rs` asserts the
declaration, the lock entry, and that the crate's own dependency closure
(D78) holds no gpui, winit, wayland-client, zbus, atspi, accesskit_unix or
tokio.

The three M27 crates were admitted on 2026-09-29, each read that day from the
crates.io API or with `git ls-remote`: smithay-client-toolkit 0.21.1
(published 2026-07-23) and rustix 1.1.5 (2026-09-16) are the newest releases,
and each lock checksum is the one crates.io publishes; cros-libva's newest
release is still 0.0.13 of 2024-12-06, which fails to compile against libva
2.24.1, so the git revision D80 names is pinned and is the head of `main`.
With Cargo's MSRV fallback off (`CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=allow`,
D69), the three resolve 46 crates more, each at the newest release its
dependents' requirements admit: aho-corasick 1.1.5, bindgen 0.70.1, bitflags
2.13.2, cc 1.5.1, cexpr 0.6.0, clang-sys 1.9.1, cursor-icon 1.2.0, downcast-rs
1.2.1, either 1.18.0, errno 0.3.14, find-msvc-tools 0.1.14, glob 0.3.4,
itertools 0.13.0, libloading 0.8.9, linux-raw-sys 0.12.1, log 0.4.34, memmap2
0.9.11, minimal-lexical 0.2.1, nom 7.1.3, pkg-config 0.3.34, prettyplease
0.2.37, quick-xml 0.41.0, regex 1.13.1, regex-automata 0.4.18, regex-syntax
0.8.11, rustc-hash 1.1.0, shlex 1.3.0 and 2.0.1, smallvec 1.16.2, syn 2.0.119,
thiserror 1.0.69, thiserror-impl 1.0.69, wayland-backend 0.3.17,
wayland-client 0.31.15, wayland-csd-frame 0.3.0, wayland-cursor 0.31.14,
wayland-protocols 0.32.13, wayland-protocols-experimental 20251230.0.3,
wayland-protocols-misc 0.3.12, wayland-protocols-wlr 0.3.12, wayland-scanner
0.31.11, wayland-sys 0.31.11, windows-link 0.2.1, windows-sys 0.61.2, xcursor
0.3.11 and xkeysym 0.2.1. Their licences, read from `cargo metadata --locked`,
are MIT, Apache-2.0, BSD-3-Clause, ISC, or a choice among MIT, Apache-2.0,
Zlib and the Unlicense; every one is permissive and none is vendored into the
repository, so REUSE is unchanged. No existing lock entry changed version.

Two crates resolve a second major version beside the workspace's own:
thiserror 1.0.69 through cros-libva beside thiserror 2.0.20, and syn 2.0.119
through bindgen, prettyplease and thiserror-impl 1.0.69 beside syn 3.0.5.
Since two thiserror versions are locked, Cargo writes every thiserror 2
dependency line version-qualified (`"thiserror 2.0.20"`), and the lock-entry
tests of `crates/aegis-athena` and `crates/aegis-tellus` strip that suffix
before they compare their exact name lists.

socket2 was admitted on 2026-09-29 against the crates.io API, read with a
`User-Agent`: 0.6.5 of 2026-07-13 is its newest stable release (D69), licensed
MIT OR Apache-2.0, and the lock's checksum is the one crates.io publishes. It
resolves no crate the lock did not already carry: its dependencies are libc
0.2.189 on Unix and windows-sys 0.61.2 on Windows, both locked before M21. The
lock gained exactly three packages, socket2 and the two new members, and no
existing entry changed version. socket2 declares `rust-version` 1.70, below the
workspace's 1.87. rustix's `system` feature enables linux-raw-sys's `system`
module and adds no package; Cargo does not lock features.

The highest `rust-version` any resolved package declares is 1.87
(accesskit 0.25.1), above smithay-client-toolkit's and wayland-protocols' 1.86,
so the workspace's `rust-version` moved from 1.85 to 1.87 and no further;
`crates/aegis-scaena/tests/manifest_hygiene.rs` reads the highest value back
from `cargo metadata --locked` and holds the workspace to it.
`rust-toolchain.toml` stays at 1.98.1. Raising the declared MSRV switched on
clippy's `manual_is_multiple_of`, which checks nothing below 1.87, and its
three findings were rewritten with `is_multiple_of` in the same change.

## The systemd floor, and why it is a floor rather than a pin

The Rust rows above pin an exact toolchain because `rust-toolchain.toml` can
materialise it. systemd cannot be materialised that way: it is the host's init
system, and the gate runs the copy the machine already has. The admission is
therefore a **floor plus a recorded reference value**, and both halves are
mechanical:

- `SYSTEMD_FLOOR = 261` in `tools/verify_systemd_definitions.py`. Each tool
  the gate runs is held to it by that binary's own `--version`. Below it the
  gate prints why it did not run, naming the tool, its path, its banner and the
  floor, and does not report a pass.
- `REFERENCE_PROFILE_SYSTEMD = "systemd 261 (261.3-1-arch)"`, the exact first
  line of `systemctl --version` on the reference profile on 2026-09-13. It is
  the value the negative and boundary outcomes in `docs/build/definitions.md`
  were observed on.

`tools/test_systemd_definitions.py` asserts that the floor and the recorded
reference value agree, so raising one without the other fails the gate rather
than passing silently. A future floor change is then a visible diff in this
page and in that constant, not an assumption inherited from whatever the
workstation happens to ship.

The reference profile moved to `systemd 262 (262-1-arch)` on 2026-09-25, and
the floor and the recorded reference value were left at 261 on purpose. On
2026-09-28 the gate ran on 262, and eight of its nine cases reproduced the
outcomes recorded on 261. The ninth, the `--root=` case, was restaged so that it
holds on both releases. It then passed on 262 and again on the 261.3 tools
extracted from the cached package; `docs/build/definitions.md` records both
runs. Raising the floor to 262 would drop that 261 evidence without adding any.

What the floor does **not** claim: that an older systemd cannot read these
definitions. It claims only that the exit codes and diagnostics this repository
records were observed on 261, and that a gate running below the floor would be
reporting against unobserved behaviour. CI runners at the time of writing ship
an older systemd, so the gate skips there and says so; the definition parser
`crates/aegis-fabrica-defs` runs everywhere and covers the same files without
systemd. Until 2026-09-28 the stated reason on the ubuntu-24.04 runner was the
wrong one: its systemd 255.4-1ubuntu8.17 ships `systemd-sysupdate` only in
`/usr/lib/systemd`, and the gate looked on `PATH` alone, so CI printed
`SKIP: systemd-sysupdate not on PATH` (read in the logs at `61f2fe1` and
`020bcd5`). With the libexec step the gate should find the tool there and skip
on the floor; no CI run of that change has been read yet.

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

## The kernel build toolchain, and the source it builds (M26)

Every row below is a tool `tools/verify_kernel_build.py` runs. Its floor is the
floor the **pinned source itself declares** -- `scripts/min-tool-version.sh` and
`Documentation/process/changes.rst` of `linux-7.2.5` -- not a number chosen
here, and its reference value was read back from the tool on the reference
profile on 2026-09-13. The gate reads each one again before it builds anything
and refuses to run below a floor, so the table cannot drift away from what
actually compiled.

`tools/test_kernel_build.py` reads this table back and requires it to state the
same admission as the gate's own `TOOLCHAIN` list. The tool names are compared
as **sets**, so neither side can carry a row the other does not, and each row's
reference value and its Floor cell must be the ones the gate actually enforces.
A row added here for a tool the gate never runs, or a floor edited on this page
alone, fails `make verify-all`.

| Tool | Reference-profile version | Floor declared by the pinned source | Role |
| :--- | :--- | :--- | :--- |
| gcc | 16.2.1 | 8.1.0 (`scripts/min-tool-version.sh gcc`) | the compiler |
| ld (binutils) | 2.47 | 2.30 (`scripts/min-tool-version.sh binutils`) | link |
| make | 4.4.1 | 4.0 | the build driver |
| bc | 1.08.2 | 1.06.95 | timeconst generation |
| flex | 2.6.4 | 2.5.35 | kconfig lexer |
| bison | 3.8.2 | 2.0 | kconfig parser |
| pahole | 1.31 | 1.26 | DWARF to BTF, for `CONFIG_DEBUG_INFO_BTF` |
| tar | 1.35 | 1.28 | source extraction |
| perl | 5.42.2 | none declared | kbuild scripts |
| cpio | 2.15 | none declared | the read-back initramfs |
| xz | 5.8.3 | none declared | tarball decompression, and signature verification |
| gzip | 1.14 | none declared | the guest reads `/proc/config.gz` with it |
| bash | 5.3.15 | 4.2 | `merge_config.sh`, and the guest's `/init` |
| mount (util-linux) | 2.42.3 | 2.10 | the guest mounts its own `/proc` |
| gpg | 2.4.9 | none declared | the pinned tarball's signature |
| qemu-system-x86_64 | 11.1.1 | none declared | the read-back guest |

clang 22.1.8 is installed on the reference profile and is **not** admitted *by
this row*: the kernel gate builds with gcc, and `LLVM=1` remains unadmitted for
it. M19 admits the same clang for a different job -- `-target bpf` for the
objects under `bpf/`, and the native target for their loader -- so a later
`LLVM=1` kernel build is still a visible admission rather than an inherited one.

## The eBPF toolchain, and the two libbpf versions that differ (M19)

Every row in the M19 block above is a tool `tools/verify_bpf_objects.py` runs,
and every one is read back from the tool before anything is compiled. The
admissions are floors plus recorded reference values, the shape M03 set for
systemd and M18 for mkosi, for the same reason: these are distribution packages
and nothing in this repository can materialise them.

The libbpf row is the one worth reading twice, because the M01 register recorded
`libbpf v1.8` and that value is a reading of a different artefact:

- `pkg-config --modversion libbpf` prints **1.7.0** on the reference profile, and
  `/usr/include/bpf/libbpf_version.h` declares `LIBBPF_MAJOR_VERSION 1`,
  `LIBBPF_MINOR_VERSION 7`. That is the shared library `bpf/loader/aegis_bpf_probe.c`
  links against, and the probe prints it again at runtime from
  `libbpf_version_string()`, which reports `v1.7`. This is the version that
  matters: it is the code that fills the verifier-log buffer and creates the
  struct_ops link.
- `bpftool version` prints `using libbpf v1.8`. That is the libbpf **bpftool was
  built against**, statically, inside `core/bpf 7.2.5-1`. It says nothing about
  the shared library on the system, and on this host the two genuinely differ.

The register's row is therefore corrected rather than confirmed: the admitted
libbpf is 1.7.0, and the 1.8 reading is recorded beside it as what it actually
measures. `tools/test_bpf_objects.py` asserts that both values and both commands
appear on this page, so the two cannot be collapsed back into one number.

What the M19 rows do **not** claim: that the objects run correctly, that any
scheduling, power or policy figure was measured, or that a kernel this
repository controls accepted them. They were verified by the workstation's own
running kernel, which is a non-qualifying local fixture -- see
`docs/build/bpf.md`.

## The latency fixture's toolchain, and the kernel it measures (M23)

Every row below is a tool `tools/verify_latency_fixture.py` runs, read back from
the tool before anything is measured, on the reference profile on 2026-09-13.
`tools/test_latency_fixture.py` reads this table back and requires it to state
the same admission as the gate's own `TOOLCHAIN` list: the tool names are
compared as **sets**, and each row's reference value and its Floor cell are
compared against the gate's entry, so a row added here for a tool the gate never
runs, or a floor edited on this page alone, fails `make verify-all`.

| Tool | Reference-profile version | Floor | Role |
| :--- | :--- | :--- | :--- |
| qemu-system-x86_64 | 11.1.1 | none declared | the guest that boots the kernel under test |
| cyclictest | 2.10 | 2.10 | the measurement itself |
| cpio | 2.15 | none declared | the guest initramfs |
| ldd | 2.44 | none declared | the guest programs' shared-library closure |
| bash | 5.3.15 | 4.2 | the guest's `/init`, and the host's interpreter for the shared probe |
| mount | 2.42.3 | 2.10 | the guest mounts its own `/proc`, `/dev` and `/sys` |
| coreutils | 9.11 | none declared | `cat`, `sleep` and `uname` in the guest |
| grep | 3.12 | none declared | the probe's selector, and the nonce on `/proc/cmdline` |
| gzip | 1.14 | none declared | the probe decompresses `/proc/config.gz` |
| pacman | 7.1.0 | none declared | the D57 evidence, read-only package-ownership queries |

Two rows are worth reading twice.

**cyclictest's floor is its own version, and the reason is the output the gate
parses.** The gate does not read the terminal summary; it reads `--json`, and it
refuses a payload whose `resolution_in_ns` is not 1, which is what `--nsecs`
sets. That output shape was observed on 2.10 and on nothing else, so a gate
running below the floor would be parsing unobserved output. This is the same
shape of admission M03 set for systemd: a floor plus a recorded reference value,
because a distribution package cannot be materialised from a file in this
repository. The exact installed package is `rt-tests 2.10-1.1`, read back with
`pacman -Qi rt-tests`, and `cyclictest --help` prints `cyclictest V 2.10` --
`--version` is not an option it accepts, which is why the gate reads the banner
from `--help`.

**The realtime kernel source row is a reuse, not a new pin, and it replaces a
path this milestone deliberately did not take.** D57 recommended a distribution
`linux-rt` package booted as a guest kernel. D70 then put kernel construction in
this repository while Nucleus is a scaffold, and M26 built one with
`CONFIG_PREEMPT_RT=y` from the pinned `linux-7.2.5` source. M23 consumes that
image and nothing else: no `linux-rt` package is downloaded, installed or
booted, and `pacman -Qq linux-rt` reports it is not installed, which the gate
records. D57's actual constraint -- that the reference host is not modified --
is therefore satisfied **by construction** rather than by a download-only
procedure: the kernel is a file under `AEGIS_KERNEL_BUILD_DIR` that no package
owns, no `/lib/modules` entry exists for, and nothing in this repository
installs.

What these rows do **not** claim: that a latency figure measured here qualifies
hardware, that it bounds a worst case over anything but the run that produced
it, or that the guest measurement isolates what `PREEMPT_RT` buys. The guest's
virtual CPU is scheduled by a host that is not realtime, so the guest figure is
a composite; `docs/build/latency.md` records that and the rest of the
methodology.

### How the pinned source was verified

Two independent checks, both re-run by the gate on every invocation, and neither
of them a claim that the digest in this page is trustworthy because it is
written here:

- **Signature over the source.** `linux-7.2.5.tar.sign` is a detached signature
  over the *uncompressed* tar, so verification streams `xz --decompress
  --stdout` into `gpg --verify`. It reports `Good signature from "Greg
  Kroah-Hartman <gregkh@linuxfoundation.org>"`, primary key fingerprint
  `647F 2865 4894 E3BD 4571 99BE 38DB BDC8 6092 693E`. The gate refuses the
  source if that fingerprint is not the one that signed it, so a good signature
  by some other key is still a refusal.
- **Digest against the published, signed checksum list.**
  `sha256sums.asc` for `v7.x` is signed by
  `B886 8C80 BA62 A1FF FAF5 FDA9 632D 3A06 589D A6B1`,
  `Kernel.org checksum autosigner <autosigner@kernel.org>`, and lists
  `55ddf0df8325d9dad96fcff7bd93977d22e3f50af06527572af59b77c7632b78` for
  `linux-7.2.5.tar.xz`. That is the value `build/kernel/source.pin.json`
  carries and the value `sha256sum` prints for the downloaded file.

Both keys are fetched once into a keyring under the build directory, never into
the developer's own keyring. Neither key is certified by a local trust path, so
gpg prints its usual "not certified with a trusted signature" warning; what the
pin asserts is the fingerprint, not a web of trust.

The configuration is a **fragment applied to a named base**, never a copied
`.config`. The base is `x86_64_defconfig` of the pinned source. The two
fragments are applied in order by the kernel's own
`scripts/kconfig/merge_config.sh`, followed by `make olddefconfig`:

1. `build/kernel/10-base-support.config` -- Kconfig prerequisites and guest-boot
   options only. It states no product requirement, and a crate test fails if it
   ever assigns a symbol the payload demands.
2. `build/kernel/50-aegis-requirement.config` -- byte-identical to what
   `KernelRequirement::config_fragment` renders from
   `build/kernel-requirement.json`. It is generated, not written.

## The boot harness's toolchain, and the artifact it boots (M24)

Every row below is a tool or firmware image `tools/verify_boot_harness.py`
runs, read back before anything boots, on the reference profile on
2026-09-28. `tools/test_boot_harness.py` reads this table back and requires it
to state the same admission as the gate's own `TOOLCHAIN` and `FIRMWARE` lists:
the names are compared as **sets**, and each row's reference value and its
Floor cell are compared against the gate's entry, so a row added here for a
tool the gate never runs, or a value edited on this page alone, fails
`make verify-all`. The same test holds the set of programs the gate may start
at all, and requires every admitted tool to be one the gate actually runs.

The three OVMF images print no version, so they are admitted by file digest.
virt-fw-vars prints none either; the gate runs it once and reads the version
from the `virt-firmware` Python distribution metadata. The last-but-one column
is the D69 comparison: the newest upstream release, read on 2026-09-28 from
each project's own release listing.

| Tool | Reference-profile version | Floor | Latest upstream, read 2026-09-28 | Role |
| :--- | :--- | :--- | :--- | :--- |
| qemu-system-x86_64 | 11.1.1 | none declared | 11.1.1, `download.qemu.org` | every guest, `accel=kvm` with no fallback |
| qemu-img | 11.1.1 | none declared | 11.1.1 | each boot's throwaway overlay, and the read-only ESP copy |
| swtpm | 0.10.2 | none declared | 0.10.2, `v0.10.2` of 2026-08-19 | the guest's TPM 2.0, one instance per boot |
| swtpm_setup | 0.10.2 | none declared | 0.10.2, same release | manufactures each boot's TPM state: EK, sha256 bank only |
| virt-firmware (`virt-fw-vars`) | 26.9 | none declared | 26.9, PyPI | every guest variable store, from the shipped template |
| sbsign | 0.9.5 | none declared | 0.9.5, tag `v0.9.5` | signs the artifact's UKI with the run's key |
| sbverify | 0.9.5 | none declared | 0.9.5, same tag | verifies the signed and the unsigned copy against that key |
| xorriso | 1.5.8.pl02 | none declared | 1.5.8.pl02 of 2026-05-22 | writes the NoCloud `cidata` seed |
| mcopy (mtools) | 4.0.49 | none declared | 4.0.49 | copies the UKI out of the ESP copy |
| openssl | 3.6.4 | none declared | 4.0.2 of 2026-08-25; the distribution ships the 3.6 series | makes the run's guest key pair |
| gpg | 2.4.9 | none declared | 2.5.24 of 2026-09-23; the distribution ships 2.4.9 of the 2.4 branch, which reached upstream end of life on 2026-06-30 | verifies the clearsigned CHECKSUM, with `--no-autostart` so no gpg-agent outlives the gate |
| curl | 8.22.0 | none declared | 8.22.0 of 2026-09-02 | `--fetch` only: the artifact, the CHECKSUM and the keys |
| OVMF_CODE.secboot.4m.fd | sha256 `cc150d941d4f1d39e596dedc545384a66ccfb3c9ba5cf9bc3a54d8d427d4d88f` | none declared | edk2-stable202608 of 2026-08-21, packaged as edk2-ovmf 202608-1 | the firmware of every Secure Boot guest |
| OVMF_VARS.4m.fd | sha256 `5d2ac383371b408398accee7ec27c8c09ea5b74a0de0ceea6513388b15be5d1e` | none declared | same release | the template every guest store is generated from |
| OVMF_CODE.4m.fd | sha256 `2febd26c0b4cf95a636a941afa37d64a552723443ed9ac72f763f6840da98cb4` | none declared | same release | the firmware of the contrast boot, without Secure Boot |

Two rows are behind upstream and stay as recorded: openssl and gpg are the
distribution's packages, and nothing this gate does depends on a feature of the
newer series. gpg is the one that matters most, because it is the signature
verifier: GnuPG's download page lists the 2.4 branch as past its end of life
on 2026-06-30 (read 2026-09-28), with 2.5 and 2.6 current. It stays admitted
because the distribution ships 2.4.9 and nothing newer, and the gate uses it
only to import public keys and to check one clearsigned file against a pinned
fingerprint. Moving the row to a 2.5 or 2.6 release once the distribution ships
one is a D69 refresh. xorriso's version carries its patch level, `.pl02`, which
the gate reads back as part of the version.
The pin is a reference value rather than a floor for every row,
the shape M23 and M26 used: no source this gate relies on declares a minimum,
so a different installed version prints beside the recorded one instead of
being refused.

**The artifact is pinned by a file, not by a tool.**
`build/boot/artifact.pin.json` names Fedora-Cloud-Base-UEFI-UKI 44-1.7 at its
compose URL under `/releases/44/`, sha256
`2b0af3e6bf4add3695e52df5db3b2eb48d081ab38963ae48c35fca45d5c31b64`, 630784000
bytes, producer-version floor `44-1.7`, and the signing key of the clearsigned
`Fedora-Cloud-44-1.7-x86_64-CHECKSUM`: fingerprint
`36F612DCF27F7D1A48A835E4DBFCF71C6D9F90A6`, `Fedora (44)
<fedora-44-primary@fedoraproject.org>`, the value
`https://fedoraproject.org/security/` lists for Fedora 44. The gate verifies the
signature by that fingerprint in gpg's `VALIDSIG` status line, not by a web of
trust, and hashes the cached bytes against the pin immediately before every
boot. The bytes live under `AEGIS_BOOT_HARNESS_DIR`, never in the repository;
`*.qcow2` is ignored by `.gitignore`.

What these rows do **not** claim: that the artifact is an Aegis image, that a
boot here qualifies hardware, or that the guest's Secure Boot is the host's.
The programs inside the guest -- `cat`, `od`, `systemctl` and cloud-init itself
-- belong to the pinned artifact and are covered by its digest, not by a row
here. `docs/build/boot-harness.md` records the run.

## The accessibility gate's toolchain (M04)

Every row below is a tool, package or image `tools/verify_a11y.py` runs, read
back before the suite runs, on the reference profile on 2026-09-28.
`tools/test_a11y.py` reads this table back and requires each version to be the
one the pin, `package.json` and the lockfile state, so a value edited on this
page alone fails `make verify-all`. The pins live in
`ui/concordia-tokens/toolchain.pin.json` (the image and Node),
`ui/concordia-tokens/package.json` (`packageManager`, exact `engines` and
devDependencies) and `ui/concordia-tokens/pnpm-lock.yaml` (every package's
integrity, and the pnpm binary's). The last-but-one column is the D69
comparison: the newest upstream release, read on 2026-09-28 from the npm
registry's `latest` tag, `nodejs.org/dist/index.json` or the project's release
listing.

| Tool | Pinned version | How it is pinned | Latest upstream, read 2026-09-28 | Role |
| :--- | :--- | :--- | :--- | :--- |
| Node.js | 26.10.0; tarball sha256 `ca70e9e349de048b9522abb3adc05b3bd6f43c5ffd3ec57916c7da292f59f022` | `toolchain.pin.json` and `engines.node`, exact, with `engineStrict`; the sha256 is the one in `SHASUMS256.txt`, signed by `5BE8A3F6C8A5C01D106C0AD820B1A390B168D356` | 26.10.0 of 2026-09-21, the Current line, which enters Active LTS on 2026-10-28; the newest LTS is 24.21.0 | runs vite and Playwright inside the container |
| pnpm | 12.6.0; `@pnpm/exe.linux-x64` integrity `sha512-qFWBneHJAJ73W4whtbaFOL1M/7DBC6ILHXuxc7ZPtEhfPuT1zeZiGrmKHoMAfJA+mcm6xhOFljqVTUS+00Jabw==` | `packageManager` and `engines.pnpm`; the integrity is pnpm-lock.yaml's | 12.6.0 of 2026-09-22 (`latest`); `next-12` is 12.8.1 | installs from the lockfile, offline |
| `@playwright/test` | 1.63.0 | `package.json`, exact; pnpm-lock.yaml | 1.63.0 of 2026-09-04 | the test runner; the image tag must carry the same version |
| `chromium-headless-shell` | revision 1243, Chromium 153.0.8010.12 | `toolchain.pin.json`, read back from `playwright-core/browsers.json` and from the launched browser | the revision `@playwright/test` 1.63.0 bundles | the browser the suite drives, baked into the image |
| `axe-core` | 4.13.0 | `package.json`, exact; pnpm-lock.yaml | 4.13.0 of 2026-08-05 | the rule engine; D81 runs its WCAG 2.2 AA tag set |
| `@axe-core/playwright` | 4.13.0 | `package.json`, exact; pnpm-lock.yaml | 4.13.0 of 2026-08-11 | injects axe-core into the page |
| `svelte` | 5.57.1 | `package.json`, exact; pnpm-lock.yaml | 5.57.1 of 2026-09-18 | the one component |
| `vite` | 8.3.1 | `package.json`, exact; pnpm-lock.yaml | 8.3.1 of 2026-09-24 | the static build and `vite preview` |
| `@sveltejs/vite-plugin-svelte` | 7.3.1 | `package.json`, exact; pnpm-lock.yaml | 7.3.1 of 2026-09-23 | compiles the component in the build |
| Playwright image | `mcr.microsoft.com/playwright:v1.63.0-noble@sha256:eff16c30e6f3f4af0a03fa4b706120d5e9b0891c344a27d64559aff5900a4a27`; linux/amd64 `sha256:bc6ab0d6d44ff4826e4cb8c1e6d801e185bfc42bb0753f8e2a30efc70db054c7` | `toolchain.pin.json`; pulled and run by digest only | `v1.63.0-noble`, built 2026-09-04, 912 MiB compressed | Ubuntu 24.04 with the browsers and their libraries; its own Node 24.20.0 is not used |
| `podman` | 6.1.2 on the reference profile (`podman 6.1.2-1.1`), rootless | the preferred engine when on `PATH`; `AEGIS_A11Y_ENGINE` overrides | 6.1.2 of 2026-09-16 | runs each step; the CI runner's is 4.9.3 |
| `docker` | 29.8.1 on the reference profile (`docker 1:29.8.1-1.1`) | the fallback engine, run with `--user` | 29.8.1 of 2026-09-15 | runs each step where podman is absent |
| `curl` | 8.22.0 on the reference profile | recorded, not pinned; started only by `--fetch` (`make a11y-fetch`), with `--proto =https` and a deadline | 8.22.0 of 2026-09-02 | `--fetch` only: downloads the Node tarball and the pnpm binary, each kept only when it hashes to its pin |

Every row is at the newest release except the Node line's LTS status, which D65
accepts: 26.x is the latest line and becomes LTS on 2026-10-28, and D42 is
settled by D65. The engines and curl are recorded, not pinned: the gate prints
which engine ran, and a pass on either is the same claim because the image, the
Node and the lockfile are the same bytes; curl runs only in `make a11y-fetch`,
and a download that does not hash to its pin is discarded, so curl's version
changes no byte the gate runs. `tools/test_a11y.py` requires a row here for
every program the gate may start on the host. The reference profile's own Node
(v26.10.0) and pnpm (10.29.3) are not what the gate runs; the M01 drift
register's "Node 20 vs 22" row is closed with the exact 26.10.0 pin above, and
pnpm 12.6.0 replaces the reference profile's 10.29.3 for this gate.

What these rows do **not** claim: that the host's Playwright browser cache is
pinned. `~/.cache/ms-playwright` held chromium-1228 on 2026-09-13 and
chromium-1243 on 2026-09-28, written by Praetor's figure engine; the gate never
reads it. `docs/build/accessibility-harness.md` records the run.

## The contract pair's toolchain and producer pins (M09)

Every row below is a program `tools/verify_contract_pair.py` starts, or a
producer commit it runs against, read back on the reference profile on
2026-09-28, and the nucleus and `python3` rows again on 2026-09-29, when D106
moved the nucleus pin and when it was re-pinned to `82aa6b7`. Both producer
rows were read again that evening, when D107 moved imago to `987b95a` and
nucleus to `852be74`.
`tools/test_contract_pair.py` reads this table back and
requires the Go floor, both commits and nucleus's verifier to be the ones
`build/contract/producers.pin.json` records, and a row for every program the
gate may start. The last-but-one column is the D69 comparison, read from
`go.dev/dl`, the tags of `github.com/git/git` and the `python.org` release API,
and for the producers from `git ls-remote --heads`.

| Tool or producer | Pinned version or commit | How it is pinned | Latest upstream, read 2026-09-28 | Role |
| :--- | :--- | :--- | :--- | :--- |
| `go` | floor 1.27.1, from imago's `go.mod` (`go 1.27.1`); reference profile go1.27.1 (`go 2:1.27.1-2`); CI 1.27.1 | `imago.go` in the pin, checked against the cached `go.mod`; every go command runs with `GOTOOLCHAIN=local`, so a Go below the floor is refused by the fetch rather than replaced by a download. CI's Go is the M00 row above: setup-go resolves Praetor's `go 1.27` to the newest 1.27 patch, 1.27.1, which meets the floor | 1.27.1 (stable), `go.dev/dl` | `make contract-fetch`: `go mod download`, `go mod verify` and the `-trimpath -buildvcs=true` build; the gate: `go version -m` and the offline stand-in build, with `GOPROXY=off` |
| `git` | reference profile 2.55.0 (`git 2.55.0-1.1`) | recorded, not pinned; no source this gate relies on declares a minimum beyond the 2.32 that `GIT_CONFIG_GLOBAL` needs | 2.56.0, tag `v2.56.0`; the distribution ships 2.55.0 | `make contract-fetch`: `ls-remote --heads`, the depth-1 fetch and checkout; the gate: `rev-parse`, `status --untracked-files=all --ignored`, `ls-files -v` and `log` of the imago checkout, `rev-parse` and `status` of this repository, and the stand-in's commit. Every git command, and go's own git calls, run with `GIT_CONFIG_NOSYSTEM=1`, an empty `GIT_CONFIG_GLOBAL` and no inherited `GIT_*` variable |
| `cordanaLLM/imago` | commit `987b95a432e5c9aac1b4885b97fa2d3ff1b9d311` (was `16f964b4dafadac2b1f0a662c7dcbb4b7bb29bee` until D107) | `imago.commit` in the pin; fetched by commit into a fresh directory, checked by `git rev-parse HEAD`, by the binary's `vcs.revision`, `vcs.time` and module version, and by the binary sha256 the fetch recorded | `main` at `987b95a432e5c9aac1b4885b97fa2d3ff1b9d311`, read 2026-09-29 | consumes both M18 payloads (`pkg/aegis`, `pkg/kernel`) |
| `cordanaLLM/nucleus` | commit `852be742eb173700d5ef93b0c6f867b855f9c640` (was `82aa6b7a3c68a42a6330370c81ec642482014c9f` from the re-pin of 2026-09-29 until D107, `0a4eac93f29fef432bfa9d892ad236568ce2f482` from D106 until that re-pin, and `8672247ff1bd22ed6b6b89d498116f20ff00c2ab` until D106) | `nucleus.commit` in the pin; fetched by commit into a fresh directory and checked by `git rev-parse HEAD`, a clean status and index, the presence of `scripts/verify_kernel_requirement.py` and the `versions.json` row that binds `aegis-os` to this repository's payload on `realtime` | `main` at `852be742eb173700d5ef93b0c6f867b855f9c640`, read 2026-09-29 | verifies `build/kernel-requirement.json` against the kconfig fragments of its bound stream at evidence level `declared` (nucleus#35, D106); its `--resolved-config` level (nucleus#36) needs a networked kernel-source resolve and is not run |
| `python3` | reference profile 3.14.7 (`python3 --version`); CI runs the Python of the `ubuntu-24.04` image | recorded, not pinned: the gate starts nucleus's verifier with the interpreter running the gate (`sys.executable`, which `make` starts as `python3`), `-I`, which ignores every `PYTHON*` variable and the user site, and `-B`, which keeps the sibling import from writing `scripts/__pycache__` into the checkout; the script and the sibling module it imports, `scripts/versions_query.py`, are standard-library only, and nucleus lints them for Python 3.11 (`target-version = "py311"`) and runs the script on CPython 3.12 in its CI | 3.14.7 of 2026-08-05, `python.org` | the gate: `scripts/verify_kernel_requirement.py --report-json`, six runs per gate run, from the nucleus checkout |

A producer's `main` moving past its pin is recorded by the fetch, not followed:
the gate runs the pinned commit until the pin is edited, and a pin edit leaves a
cache fetched for another pin, which the gate reports as a FAIL that names
`make contract-fetch` (D93: only an absent cache skips). The imago binary is
built from source to run the contract and is not a release artifact;
`docs/build/contract-pair.md` records the runs.

## The display slice's build toolchain (M27, D80)

`crates/aegis-scaena` builds a C binding: cros-libva's build script finds libva
through pkg-config, and bindgen, its build dependency, loads libclang to
generate the bindings from the libva headers. None of those is a Cargo input,
so `tools/display_toolchain.py` reads every row below back before
`make verify-rust` builds anything, prints each value beside its floor, and
fails the crate gate on a missing tool or a value below its floor.
`tools/test_display_toolchain.py` reads this table back and requires it to
state the same admission as the tool's `TOOLCHAIN` and `LOCKED` lists: names
compared as sets, and each row's reference and floor compared. Each reference
value was read on the reference profile on 2026-09-29.

| Tool or crate | Reference-profile version | Floor | Role |
| :--- | :--- | :--- | :--- |
| pkgconf | 3.0.7 (`pkgconf 3.0.7-1.1`) | 1.8.1 | locates libva for cros-libva's build script; the floor is the Verification gate runner's `pkg-config 1.8.1-2build1`, the oldest any gate builds with |
| libva | 2.24.1 (`libva 2.24.1-1.1`), read as `pkg-config --variable=libva_version libva` | 2.24.1 | the VA-API loader and the headers the bindings are generated from; the floor is the version CI builds (below), never lowered |
| VA-API | 1.24.0, read as `pkg-config --modversion libva` | 1.24.0 | the API version those headers declare |
| clang | 22.1.8 (`clang 22.1.8-2`; libclang 22.1.8) | 6.0 | libclang, which bindgen loads; bindgen 0.70.1 builds clang-sys with its `clang_6_0` feature, and its book at that tag asks for Clang 5.0 or later, so 6.0 is the stricter floor. The row reads the `clang` on `PATH`; clang-sys loads the newest libclang it finds, which on both hosts belongs to that same LLVM |
| bindgen | 0.70.1, read from `Cargo.lock` | 0.70.1 | cros-libva's build dependency at the pinned revision (`bindgen = "0.70.1"`), locked exactly |
| cros-libva | 0.0.13 from git revision `59384456ac2ae78c0c3e5515f41ef1efd9b802cf`, read from `Cargo.lock` | 0.0.13 | the binding itself; the lock's source must name the pinned revision |

`vainfo` and `wayland-info` established the three display capabilities in
`planning/hardware-profile.json`, but they are probes, not gate tools. The iHD
driver, `intel-media-driver 26.2.4-1.1` on the reference profile, is a host
driver that is recorded, not pinned: the display gate reads it back through
the VA vendor string before it decodes (M27 criterion 5;
`crates/aegis-scaena/src/decode.rs`, `require_ihd`), and the per-frame
regression pins are recorded with that string. ffmpeg n9.0.2 generated the
committed Motion-JPEG fixture once, drawing the frame-index blocks M27 reads
back; the gate never runs it, so the command is recorded as the fixture's
provenance on `docs/build/display.md` rather than admitted. The CRC-32 that
checks the MPEG-2 frame and pins the Motion-JPEG frames against regression is
the computation cros-libva's own test uses (`crc_nv12_image` in
`lib/src/lib.rs` at the pinned revision, through crc32fast): M27 writes it in
the crate, `content::Crc32`, ISO-HDLC with its check value tested, and admits
no crate for it.

### The display gate's programs

`make verify-display` (`tools/verify_display.py`) starts only the programs in
its `PROGRAMS` list, and `tools/test_display_slice.py` holds that list, refuses
a program outside it before it starts, and requires every process to start
from one call site with a deadline. None of them is admitted here with a
version floor: the build rows above are read back through
`tools/display_toolchain.py` before the gate builds, and the rest are queries.

| Program | Invocation | Role |
| :--- | :--- | :--- |
| cargo | `cargo build --locked -p aegis-scaena --bin aegis-scaena-display` | builds the run binary on the locked graph |
| rustc | `rustc --version` | printed with every run |
| pkg-config | `--variable=libva_version libva` and `--modversion libva` | libva and VA-API printed with every run |
| clang | through `tools/display_toolchain.py` | the libclang row above |
| loginctl | `show-session <id> -p LockedHint --value` | a locked session presents no client surface, so the gate skips |
| pacman | `-Qo <compositor executable>` | names the compositor's package; the compositor binary is never executed |
| aegis-scaena-display | `run` and `driver-probe` | the run itself, with `LIBVA_DRIVER_NAME` in its environment only |

### libva on the Verification gate's runner

The runner is ubuntu-24.04, whose archive carries libva 2.20.0 (and
ubuntu-26.04 carries 2.23.0), both below the floor. As the maintainer decided on
2026-09-29 (D80), the job builds the pinned release tarball rather than
lowering the floor or changing the runner image, in
`.github/workflows/ci.yml` before `make verify-all`, and caches the installed
prefix under `~/.cache/aegis-libva/2.24.1`, keyed on every pin below.
`tools/test_display_toolchain.py` holds the workflow's `env:` values to this
table.

| Input | Pinned value | How it is checked | Role |
| :--- | :--- | :--- | :--- |
| libva release tarball | `https://github.com/intel/libva/releases/download/2.24.1/libva-2.24.1.tar.bz2`, sha256 `eec6050b52876f229bd35e9df17cd31a06785e18e6f7990c445b584628483d67` | `LIBVA_VERSION` and `LIBVA_SHA256`, `sha256sum -c` before extraction; the digest is the one the GitHub release publishes for the asset, and on 2026-09-29 the detached `libva-2.24.1.tar.bz2.asc` verified against key `421E5C3C1148E3DB9372378E8B99990502A81421` (Intel Media Stack Release) | the headers and the `libva` and `libva-drm` libraries the crate builds and runs its tests against |
| meson | 1.12.1, wheel sha256 `930bc7542cbd9f57009e182fd014eba48cf1a0180a7b9006d2d32d8f168d8b02` | `MESON_VERSION` and `MESON_WHEEL_SHA256`, `pip install --require-hashes --only-binary=:all: --no-deps` into a throwaway virtual environment | configures the libva build; the newest release on PyPI on 2026-09-29 |
| ninja | 1.13.2, wheel sha256 `65a24341b5ac09fcadcc37082660be40a94174e51a937fabf6e2cae26225fa2c` (manylinux x86_64) | `NINJA_VERSION` and `NINJA_WHEEL_SHA256`, the same install | runs the libva build; the newest release on PyPI on 2026-09-29 |
| libdrm headers | unpinned: the runner's `libdrm-dev` from its signed Ubuntu archive | `pkg-config --atleast-version=2.4.75 libdrm`, the floor libva's `meson.build` requires for `libva-drm`, printed and held before the build | `libva-drm`, which cros-libva links; the image ships no libdrm headers |

The build runs `meson setup` with `--prefix=$HOME/.cache/aegis-libva/2.24.1
--libdir=lib --buildtype=release -Dwith_x11=no -Dwith_glx=no -Dwith_wayland=no
-Dwith_win32=no -Denable_docs=false`, then `meson compile` and `meson install`;
only the DRM backend, which cros-libva links, is built. The step after it
exports `PKG_CONFIG_PATH`, so cros-libva's build script finds the prefix, and
`LD_LIBRARY_PATH`, because Cargo adds a build script's search path to the
loader path only inside the target directory, and it reads the installed
`libva_version` back and requires it to equal `LIBVA_VERSION`.

What these rows establish and what they do not. On the reference profile,
`cargo build --locked` built the crate against libva 2.24.1 with clang 22.1.8
and pkgconf 3.0.7. The workflow's three libva steps were run, extracted from
`ci.yml` and unmodified, in an `ubuntu:24.04` container with the runner's
pkg-config 1.8.1 and clang 18.1.3 on 2026-09-29: libva 2.24.1 built, the
read-back printed VA-API 1.24.0, `tools/display_toolchain.py` passed and
`cargo build --locked -p aegis-scaena` succeeded against the built prefix. A
container is not the runner; the first Verification gate run on this change is
the runner's own evidence. That the pinned revision compiles against apt's
libva 2.20.0 was not tried, because the floor is not lowered. No frame is
decoded by any of this; decoding is the display gate's.

## Proposed for M16 and M28: the native shell's crates (D101, D102)

These rows are proposals, not admissions, in the shape M27's rows had until M27
admitted them.
M16 admitted accesskit on 2026-09-29, and its row moved to
[Crates the workspace pins](#crates-the-workspace-pins); M28 admits the rest,
each when its gate reads the row back before it runs; until then no gate may
cite them. They are recorded now
because decision D101 makes the P05 shell native Rust and D102 names its
toolkit, gpui, git-pinned (ADR-0004), and because none of these crates is named
anywhere else on this page. Each value was read on 2026-09-29 from the crates.io
API, from zed-industries/zed or from the reference profile's package manager.

| Tool or crate | Reference version, read 2026-09-29 | Proposed pin or floor | Role |
| :--- | :--- | :--- | :--- |
| gpui | zed-industries/zed `main` at `bd747337d7be`, `crates/gpui` version 0.2.2 (Apache-2.0); the crates.io release is 0.2.2 of 2025-10-22 | one zed commit with `rev =`, chosen and locked at M28 (D102) | the toolkit: the layer-shell surface, canvas painting and the AccessKit integration |
| accesskit_unix | 0.22 as zed `main` resolves it; the newest release is 0.24.0 (MIT OR Apache-2.0) of 2026-09-25 | the version the pinned zed commit resolves, recorded as D69 drift while it lags (M28) | exports the AccessKit tree to AT-SPI over D-Bus |
| zbus, atspi | 5.19.0 and 0.30.0 (both Apache-2.0 OR MIT), the newest releases; accesskit_unix 0.24.0 requires zbus ^5.19 and atspi ^0.29 | as the pinned accesskit_unix resolves them; atspi also for the tests that read the tree back (M28) | pure-Rust D-Bus and AT-SPI below accesskit_unix |
| libxkbcommon | 1.13.2 (`libxkbcommon 1.13.2-1.1`) | the floor the pinned xkbcommon crate (0.8.0 at zed `main`) needs, read at admission (M28) | the C keymap library gpui links for Linux keyboard input |
| libwayland-client | 1.26.0 (`wayland 1.26.0-1.1`) | read at admission (M28) | loaded by wayland-backend through `dlopen` |
| at-spi2-core | 2.60.7 (`at-spi2-core 2.60.7-1.1`), with `dbus-run-session` from `dbus 1.16.2-1.1` | recorded and read back before the AT-SPI cases run (M28) | the bus launcher and registry of the private AT-SPI session M28's tests start |

What these rows do **not** claim: that any zed commit is chosen, that gpui
builds on the Verification gate's runner, that the accesskit gpui resolves and
the 0.25.1 M16 exports can share one tree without a conversion, or that any
frame was painted. The zed repository holds crates other than gpui; the build
graph takes only what the pin resolves, and M28 records the licence of every
package it resolves. xdg-desktop-portal and the guest session M29 runs are M29's
admission, not this page's yet.

The D100 lint's packages, eslint, eslint-plugin-svelte and svelte-eslint-parser,
are M16's admission as well; M16 admitted them on 2026-09-29, and they are
recorded in the next section.

## The ui/ HISS lint's toolchain (M16, D100)

`praetorctl audit` scans Go, Python and Rust for the HISS invariants and no
JavaScript, TypeScript or Svelte (cordanaLLM/praetor#589, filed 2026-09-29: a
planted recursion, an `eval` and a 73-line function all pass it). Until a
scanner ships there, ESLint enforces HISS-01, HISS-04 and HISS-08 on the
JavaScript and Svelte under `ui/`, which is `ui/concordia-tokens` today,
offline, inside the accessibility gate's pinned container:
`ui/concordia-tokens/eslint.config.js` sets `max-lines-per-function` 60,
`complexity` 10, `max-statements` 50, `no-eval`, `no-implied-eval`,
`no-new-func` and the local rule
`ui/concordia-tokens/hiss-lint/no-self-recursion.js`, switches inline
configuration off, and the gate runs it with `--max-warnings 0`. The local
rule reports a function that calls itself by name, directly or from a closure
inside it, and `this.name()` inside the method `name`. It does not detect
mutual recursion, a cycle through two or more functions, because there is no
call-graph check.

The three packages are devDependencies of `ui/concordia-tokens/package.json`,
exact, beside M04's six, and are locked in `ui/concordia-tokens/pnpm-lock.yaml`,
which M04's pnpm 12.6.0 regenerated with `--lockfile-only`: every package
M04 had locked kept its version and integrity, and the lint's closure was
added. D99, the one pnpm workspace under `ui/`, was withdrawn by D101, so the
lint has no workspace root of its own. `tools/test_a11y.py` reads this table
back against the manifest, and the gate reads each version back from inside
the container before it lints. The last-but-one column is the D69
comparison, the npm registry's `latest` on 2026-09-29.

| Tool | Pinned version | How it is pinned | Latest upstream, read 2026-09-29 | Role |
| :--- | :--- | :--- | :--- | :--- |
| `eslint` | 10.11.0 (MIT) | `package.json`, exact; pnpm-lock.yaml | 10.11.0 of 2026-09-18 (`latest`); `maintenance` is 9.39.5 | runs the rules below over every `.js`, `.mjs`, `.cjs` and `.svelte` file of the package |
| `eslint-plugin-svelte` | 3.23.0 (MIT) | `package.json`, exact; pnpm-lock.yaml | 3.23.0 of 2026-08-13 | its `base` configuration hands `.svelte` files to the parser and turns on only its two bookkeeping rules, `svelte/comment-directive` and `svelte/system` |
| `svelte-eslint-parser` | 1.8.1 (MIT) | `package.json`, exact; pnpm-lock.yaml | 1.8.1 of 2026-08-15 | parses a Svelte component's script for the rules; its manifest declares `engines.pnpm` 10.34.5, which pnpm 12.6.0 does not enforce for a dependency, so `engineStrict` does not refuse it |

The parser loads the `svelte` compiler M04 already admitted, 5.57.1, so no
second Svelte enters the lockfile. Each rule family has a planted violation
under `ui/concordia-tokens/hiss-lint/plants/` that the gate lints on its own
and requires to produce exactly its findings, and `at-the-limits.js` sits
exactly on the 60-line, complexity-10 and 50-statement limits and must lint
clean, while `function-length.js` holds a 61-line function that must not. The
first run over M04's suite found `ui/concordia-tokens/tests/probe.js` wrapped
in a 170-line immediately invoked function; it is now a strict-mode block,
with no change to what it measures.

## The Nucleus kernel gate's toolchain, and the kernel it boots (M10)

Every row below is a tool `tools/verify_nucleus_kernel.py` runs, read back
from the tool before anything boots, on the reference profile on 2026-09-29.
`tools/test_nucleus_kernel.py` reads this table back and requires it to state
the same admission as the gate's own `TOOLCHAIN` list: the names are compared
as a set, and each row's reference value and Floor cell against the gate's
entry, so a row added here for a tool the gate never runs, or a value edited on
this page alone, fails `make verify-all`. The same test holds the set of
programs the gate may start at all.

Eight rows are M23's admissions reused unchanged, because M10 boots its guest
through M23's `boot()` and builds its initramfs through M23's helpers (the
criterion that no second harness is built): the gate reads them from
`tools/verify_latency_fixture.py`'s own list, and their reference values are
the ones M23 recorded on 2026-09-13. A newer installed version prints beside
the recorded one instead of being refused. The four eBPF rows reuse M19's
floors from `tools/verify_bpf_objects.py`. QEMU is the version M24 admitted as
well, so criterion 3's pinned emulator is shared rather than admitted twice.

| Tool | Reference-profile version | Floor | Latest upstream, read 2026-09-29 | Role |
| :--- | :--- | :--- | :--- | :--- |
| qemu-system-x86_64 | 11.1.1 | none declared | 11.1.1, as M24 records | both guests, `-enable-kvm`; the gate skips without a read-write `/dev/kvm` |
| cpio | 2.15 | none declared | as M23 records | the two initramfs archives, through M23's `archive_tree` |
| ldd | 2.44 | none declared | as M23 records | the guest userspace's library closure |
| bash | 5.3.15 | 4.2 | as M23 records | the guest's PID 1 and the host half of the probe |
| mount | 2.42.3 | 2.10 | as M23 records | proc, sysfs, securityfs, tracefs and cgroup2 in the guest |
| coreutils | 9.11 | none declared | as M23 records | `cat`, `sha256sum`, `stat`, `mkdir`, `base64`, `sleep` and the marker binary in the guest |
| grep | 3.12 | none declared | as M23 records | the probe and the attach poll in the guest |
| gzip | 1.14 | none declared | as M23 records | `/proc/config.gz` and the retained verifier logs in the guest |
| setpriv | 2.42.4 | none declared | 2.42.4, tag `v2.42.4` | every guest load, as uid 65534 with CAP_BPF and CAP_PERFMON only |
| cosign | 2.6.3 | 2.6.3 | 3.1.3 of 2026-08-06; the 2.6 series is at 2.6.5 of the same day | verifies SHA256SUMS against the pinned keyless identity, tag commit and tag ref, `--offline` in the gate; admitted below 3, so the reference profile's second copy, 3.1.3, is refused |
| clang | 22.1.8 | 19.0.0 | as M19 records | the objects and the loader, against the Nucleus kernel's own BTF |
| bpftool | 7.8.0 | 7.4.0 | as M19 records | `btf dump` of the kernel image's own BTF into `vmlinux.h` |
| llvm-strip | 22.1.8 | 19.0.0 | as M19 records | drops DWARF from each object and keeps BTF |
| libbpf | 1.7.0 | 1.5 | as M19 records | the loader links it; the guest runs this host's copy |

`curl` 8.22.0, M24's admission, runs only in `make nucleus-kernel-fetch`, and
cosign runs there once without `--offline`, so its trust root may refresh. The
gate itself never touches the network. cosign is behind upstream on two counts:
the installed binary is 2.6.3 from `go install`, the 2.6 series has reached
2.6.5, and upstream's current line is 3.x. Nothing here depends on a feature of
either newer release; moving the row is a D69 refresh.

The cosign row is the only one with a ceiling. The reference profile carries
two copies, and a bare `cosign` names 2.6.3 only because `~/go/bin` precedes
`/usr/bin` in the developer account's `PATH`
(`planning/hardware-profile.json`). The other copy, the distribution's 3.1.3,
prints `Flag --offline has been deprecated` for the flag the gate passes. The
gate and the fetch therefore read the version back and refuse anything below
2.6.3 or at 3 and above, with a printed reason, rather than trust the order of
`PATH`. `tools/test_nucleus_kernel.py` holds the ceiling to this row.

**The kernel is pinned by a file, not by a tool.**
`build/kernel/nucleus-artifact.pin.json` names cordanaLLM/nucleus release
`v7.2.8-realtime-lusoris1`, tag commit
`9e050cf8fc2fb62de943229f22ab104dce455e92`, the
`imago.nucleus.kernel-artifact.v1` manifest by sha256
`c57416a1fc286eab54df65249d7502de1a6dbf601f1b064336b3abbf80cf0f4e`, every
asset by sha256 and size, `kernel.release` `7.2.8-lusoris1-realtime`,
`kernel.config_digest`
`sha256:e9510d2d1928d4e153b19cf3312c4184ce44b4186ec6fde286e001060ffd731a`, and
the keyless signer
`https://github.com/cordanaLLM/nucleus/.github/workflows/publish-release.yml@refs/tags/v7.2.8-realtime-lusoris1`
with issuer `https://token.actions.githubusercontent.com`. imago is the binary
M09's `make contract-fetch` built at the commit M09 pins: it was
`16f964b4dafadac2b1f0a662c7dcbb4b7bb29bee` for the runs of 2026-09-29 and is
`987b95a432e5c9aac1b4885b97fa2d3ff1b9d311` since D107, which changed only
imago's vendored copy of the kernel requirement and two of its test files. The
gate checks its sha256 against the fetch's identity record before running it,
and passes imago's own `versions.json` from that checkout.

## The workstation gate's toolchain, and the bytes it boots (M21)

`make verify-workstation` (`tools/verify_workstation.py`) boots Firecracker
microVMs and reads the RAPL counter; `make workstation-fetch` is its one
networked step. Every value below was read on the reference profile on
2026-09-29.

**Firecracker is pinned by a file.** `build/sandbox/firecracker.pin.json`
names the v1.17.0 GitHub release of 2026-09-10, the newest release on the
GitHub API that day and the version D58 names. Its x86_64 archive,
`firecracker-v1.17.0-x86_64.tgz`, is 7464385 bytes with sha256
`06094a1108ae9e82aa4c23a775aa92758f53f1175d422270d9d6162cb9ade558`: the value
the GitHub API reports as the asset's digest and the one the release's own
`firecracker-v1.17.0-x86_64.tgz.sha256.txt` publishes, which the fetch
re-reads and compares. Inside it, `firecracker-v1.17.0-x86_64` hashes to
`99ad0f5cd0514a88aad0e9ae8cfdb3cc3b4ab9d190e1194602406c786b5de7a5`, the value
the archive's own `SHA256SUMS` lists; it is a static-pie executable and prints
`Firecracker v1.17.0`. The licence is the archive's `LICENSE`, Apache-2.0,
sha256 `cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30`; its
`NOTICE` adds that libseccomp, LGPL-2.1, is used in the build to produce the
seccomp filters shipped beside the binary. Only the binary and the licence are
extracted; nothing is vendored into the repository. The jailer is **not
used**: it needs root to chroot and change user, and the maintainer's decision
of 2026-09-29 admits no privilege beyond one scoped counter read. The
distribution package `firecracker 1.17.0-1.1` on the reference profile is the
same version and is not run by any gate.

**The guest kernel is pinned by a file.** Firecracker supports 6.18 guests
from v1.16.1 until 2028-06-01 and 6.1 guests only to 2026-09-02
(`docs/kernel-policy.md` at v1.17.0), so the guest is the 6.18 build from the
CI artifact set current when v1.17.0 was published,
`firecracker-ci/20260909-a8e1c3830545-0/x86_64/vmlinux-6.18.44`, 27846248
bytes, sha256
`d8ced68bd61e27b6813e2c993cc53a4029c59e13210672180591c84109684fe4`, and its
configuration `vmlinux-6.18.44.config`, sha256
`9fb2be18303d2f6e8ec35b3a20ecf1209f54a4ece10114a893cb37beabee7030`. The bucket
publishes no digest, so each is the sha256 computed at admission. The gate
requires `CONFIG_BLK_DEV_INITRD=y`, `CONFIG_KVM_GUEST=y`,
`CONFIG_SERIAL_8250_CONSOLE=y`, `CONFIG_VIRTIO_MMIO=y` and
`CONFIG_VIRTIO_VSOCKETS=y` in that configuration. The kernel is GPL-2.0 and is
downloaded, never vendored.

**The guest init is built, not fetched.** `aegis-vesta-guest` is built by the
pinned Rust toolchain with `--release --target x86_64-unknown-linux-gnu` and
`RUSTFLAGS=-C target-feature=+crt-static`, which links the workstation's glibc
statically: glibc 2.44 (`glibc 2.44+r24+g16be1518495f-1` on the reference
profile). That archive is recorded, not pinned, like the iHD driver M27
records: the gate refuses a guest whose ELF program headers name an
interpreter, and the initramfs it packs is a local run artifact that is never
distributed. The initramfs digest is printed with every run; every run of
2026-09-29 packed the same bytes.

### The workstation gate's programs

`tools/test_workstation_slices.py` holds the gate's `PROGRAMS` list, refuses a
program outside it before it starts, and requires every process to start from
one of two call sites, each with a deadline.

| Program | Invocation | Role |
| :--- | :--- | :--- |
| sudo | exactly `sudo -n cat /sys/class/powercap/intel-rapl:0/energy_uj`, under a 10 s deadline | the one privileged read; any other argument vector, sudo named bare or by path, is refused before it starts |
| cargo | `cargo build --locked -p aegis-tellus-rapl`, `-p aegis-vesta-sandbox`, and the static guest build above | builds the three binaries on the locked graph |
| rustc | `rustc --version` | printed with every run and kept in its `host.json` |
| firecracker-v1.17.0-x86_64 | `--version` from the gate; `--no-api --config-file` from `aegis-vesta-sandbox` | the pinned monitor |
| aegis-tellus-rapl | `run <readings document>` | the RAPL cases |
| aegis-vesta-sandbox | `run <firecracker> <kernel> <initramfs> <run-dir>`, in its own session | the sandbox cases |
| curl | in `make workstation-fetch` only | M24's admission (8.22.0); the gate itself never touches the network |

## Not yet admitted

These are run by a gate but pinned by nothing, so they are gaps recorded here
rather than admissions:

- `shellcheck`, run by the CI shell-lint step, comes from the runner image. No
  version is pinned and none is asserted.
- The glibc static archive M21's guest init links comes from the
  workstation's glibc package, recorded above and not pinned.
- `python3`, which runs the preparation validator and its unit tests, comes from
  the runner image and from the workstation distribution.
- `npx` and `pipx` are the delivery mechanism for four pinned linters; the
  Node.js and Python runtimes behind them are the runner's.
- `libdrm-dev` on the Verification gate's runner, which the pinned libva build
  compiles `libva-drm` against, comes from the runner's Ubuntu archive at
  whatever version it carries; only the libdrm floor of 2.4.75 that libva's
  `meson.build` requires is held, read back with `pkg-config` before the
  build.
- `node`, which `make docs-lint` (part of `make verify-all`) runs directly, is
  the runner's or the workstation's; `.github/workflows/praetor-docs.yml` asks
  `actions/setup-node` for major version 24. Praetor supplies both files. The
  accessibility gate's Node is a different one, admitted above: 26.10.0 by
  sha256, inside the container.
- The praetor pin auto-bump (`tools/praetor_bump.py`) runs `yamllint`,
  `flake8`, `black` and `reuse` as the workstation installs them, not through
  `pipx` at the versions above, and builds the documentation portal with the
  workstation's Python. It builds `praetorctl` with the workstation's `go`
  under its own `GOTOOLCHAIN` setting, whereas CI installs the Go version
  Praetor's `go.mod` declares and sets `GOTOOLCHAIN=local`. These versions
  decide only whether a bump pull request is proposed; its CI re-runs the
  pinned tools.

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
- The kernel rows are checked by `tools/test_kernel_build.py`, which asserts
  that every tool name and reference version in the M26 table also appears in
  the gate's `TOOLCHAIN` list, that the pinned digest, base configuration and
  signing-key fingerprint appear on this page, and that every recorded
  reference version is at or above its floor. `make verify-kernel` then reads
  each version back from the tool itself before it builds anything, so the
  evidence names the compiler that actually ran.
- The M23 rows are checked by `tools/test_latency_fixture.py` the same way, and
  that test additionally holds the set of programs the latency gate is allowed
  to invoke at all. That set is the D57 statement in mechanical form: it
  contains no installer, no bootloader tool and nothing that writes outside the
  build directory, so adding one fails `make verify-all` rather than being
  noticed in review.
- The M27 display-toolchain rows are checked by
  `tools/test_display_toolchain.py`, which holds this page's table to the
  `TOOLCHAIN` and `LOCKED` lists of `tools/display_toolchain.py` and the libva
  pins to the `env:` block of `.github/workflows/ci.yml`. `make verify-rust`
  runs `tools/display_toolchain.py` before it builds, so the evidence names the
  pkgconf, libva and clang the crate was built with.
- The M10 rows are checked by `tools/test_nucleus_kernel.py` the same way,
  and that test also holds the programs the Nucleus kernel gate may start:
  cosign, imago, bash, bpftool and, in the fetch only, curl. The emulator is
  not among them, because the gate starts it only through M23's `boot()`.
  `make verify-nucleus-kernel` then reads each version back before it boots
  anything.
- The M21 rows are checked by `tools/test_workstation_slices.py`, which holds
  every digest in `build/sandbox/firecracker.pin.json` against an independent
  copy, the one sudo command and the programs the gate may start.
  `make verify-workstation` hashes the fetched binary, licence, kernel and
  configuration against the pin and reads `firecracker --version` back before
  anything boots; the socket2 row is held by
  `crates/aegis-vesta-sandbox/tests/manifest_hygiene.rs`.
- The M24 rows are checked by `tools/test_boot_harness.py` the same way, with
  the three OVMF images compared by digest. That test also holds the programs
  the boot harness may start, which contain nothing that writes a firmware
  variable or a boot entry, and it requires every admitted tool to be one the
  gate runs. `make verify-boot` then reads each version and digest back before
  it boots anything.
