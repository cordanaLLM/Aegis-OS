<!--
SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# The contract pair, and what the pinned imago did with Aegis's payloads

Status: recorded observations from milestone M09, reference profile, 2026-09-28

Milestone M09 pins the cross-repository contract as far as the producers can
prove it today. Decision D92 scopes it to consumption: `cordanaLLM/imago`, built
from a pinned commit, decodes both payloads milestone M18 authored, refuses bad
ones with the payload's correlation id, and enforces its bounds at the edge.
`cordanaLLM/nucleus` reads no Aegis payload and is recorded by identity only. To
run it on a machine with go and git:

```sh
make contract-fetch   # the one networked step: ls-remote, clone, build imago
make verify-contract  # the gate alone; make verify-all runs it too
```

Without go or git, or without the cached checkout, binary or identity record,
the gate prints `SKIP: <reason>; the contract pair gate did not run.` and exits
0, so an exit 0 is evidence only when the case lines are above it. A cache that
is present but wrong fails and names `make contract-fetch`: one fetched for
another pin, a checkout holding anything the pinned commit lacks, or a binary
that is not the one the fetch built (D93). CI runs `make contract-fetch` before
`make verify-all`, so the gate runs there instead of skipping
(`.github/workflows/ci.yml`). The gate has run on Linux only; on macOS and
Windows it runs wherever go and git are on `PATH`, which no CI leg provides, so
those legs run `tools/test_contract_pair.py` alone.

**M09 is `done`, as D92 scopes it.** Every exit criterion and all three epics
rest on the runs recorded below and on `tools/test_contract_pair.py`, which runs
inside `make verify-all` on every platform of the matrix.

**No result came back, and none was expected to.** Nothing produces imago's
`imago.p01.product-result.v1` yet (its ADR-0020; `cordanaLLM/imago` issue 46),
and nucleus has built no kernel (`cordanaLLM/nucleus` issues 18 and 20). Under
D92 the Imago product result is M11's (a criterion and epic E11-7) and the
Nucleus kernel result is M10's (its first criterion and epic E10-4). Under D94,
M10 also rejects a requirement feature the built kernel's config does not
satisfy (epic E10-5), the check imago cannot run without a kernel. The imago
binary the gate runs is a decoder built from source, not a release artifact.

## What is tracked, and what is not

| Path | Tracked | What it is |
| --- | --- | --- |
| `build/contract/producers.pin.json` | yes | the pin: both producers' URL and commit, imago's Go floor and two decoder bounds, and the sha256 of the three fixtures imago vendors |
| `build/product-input.json`, `build/kernel-requirement.json`, `build/kernel-requirement.reference.json` | yes | the M18 payloads, kept LF on every platform by `.gitattributes` because the pin names bytes |
| `tools/verify_contract_pair.py` | yes | the fetch and the gate |
| `tools/test_contract_pair.py` | yes | positive, negative and boundary tests of every decision the gate makes |
| `${AEGIS_CONTRACT_DIR}/imago` | no | the depth-1 checkout of imago at the pinned commit, cloned fresh by every fetch |
| `${AEGIS_CONTRACT_DIR}/bin/imago` | no | the binary built from it (`imago.exe` on Windows) |
| `${AEGIS_CONTRACT_DIR}/identity.json` | no | the retained `git ls-remote --heads` of both producers, the pin they were fetched for and the built binary's sha256 |
| `${AEGIS_CONTRACT_DIR}/runs/<run id>/` | no | every invocation's argv, exit code, stdout and stderr, every payload fed to imago, and the run summary |

`AEGIS_CONTRACT_DIR` defaults to `${XDG_CACHE_HOME:-$HOME/.cache}/aegis-contract`.
Nothing is written into the repository, and nothing is sent to either producer.

## The pins

| Producer | Repository | Commit | Role in M09 |
| --- | --- | --- | --- |
| imago | `https://github.com/cordanaLLM/imago.git` | `16f964b4dafadac2b1f0a662c7dcbb4b7bb29bee` | consumes both payloads: `imago aegis validate` (`pkg/aegis`, ADR-0020) and `imago kernel requirement validate` (`pkg/kernel`, ADR-0021) |
| nucleus | `https://github.com/cordanaLLM/nucleus.git` | `8672247ff1bd22ed6b6b89d498116f20ff00c2ab` | identity only: publishes the `imago.nucleus.kernel-artifact.v1` manifest imago verifies, and reads no Aegis payload |

Both commits are each producer's `main` on 2026-09-28, read with
`git ls-remote --heads`. The 2026-09-13 local-only pins, imago `4f116fc` and
nucleus `78ca8f2`, never reached either producer and are not used.

imago's `go.mod` declares `go 1.27.1`, and the fetch builds with
`GOTOOLCHAIN=local`, so a Go below that floor is refused rather than replaced
by a downloaded toolchain. CI's Go comes from `actions/setup-go` reading
Praetor's `go.mod`, which declares `go 1.27`; setup-go resolves that to the
newest 1.27 patch, and the Verification gate run of pull request 145 logged
`Setup go version spec 1.27` and `go1.27.1` on 2026-09-28, so CI builds imago
with no Go change. `docs/roadmap/toolchain-admission.md` records both rows.

Why nucleus is identity only, from its own tree at the pinned commit: no file
reads `build/kernel-requirement.json`, a `correlation-id` or a `required-by`
field; `verify-requirements.yml` checks nineteen `CONFIG_*` symbols hardcoded in
the workflow (issue 20); `publish-release.yml` writes the
`imago.nucleus.kernel-artifact.v1` manifest from a kernel build the forge does
not yet perform (issue 18); and its `AGENTS.md`, under "The contract with
imago", names imago's own `kernel/requirement.json` as its inbound.

## How one run works

`make contract-fetch` runs, in order: `go version` against the floor;
`git ls-remote --heads` against both producers, keeping the output; a depth-1
fetch of imago at the pinned commit into a fresh directory, replacing any
cached checkout rather than reusing it; `go mod download` and `go mod verify`;
and
`GOTOOLCHAIN=local CGO_ENABLED=0 GOFLAGS=-mod=readonly go build -trimpath -buildvcs=true`.
The binary is kept only when `go version -m` reads the pinned build out of it.
The identity record is written last, with the binary's sha256, so it always
names a complete fetch and the binary that fetch produced.

`make verify-contract` then runs offline, with `GOPROXY=off` on every go
command. It checks the pin and this repository's payloads, the identity record,
the checkout and the binary first; only when all four pass does it run anything
the binary prints. Each payload it feeds imago is written under the run
directory, and each invocation is retained there.

Every git command, and every go command, since go runs git to stamp a build,
runs with `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL` set to an empty file
and no inherited `GIT_*` variable. A workstation setting such as
`status.showUntrackedFiles=no`, or a `GIT_DIR` a git hook exports, therefore
cannot change what the checks see. The checkout check lists untracked and
ignored files as well as tracked changes, with the repository's fsmonitor off,
and refuses an index entry marked assume-unchanged or skip-worktree: go would
compile an ignored `.go` file and still stamp `vcs.modified=false`.

## The cases, and what each proves

| Case | Positive, negative or boundary | What it proves |
| --- | --- | --- |
| `contract/pin` | positive | the pin loads, and the three payloads hash to the digests it records |
| `contract/identity` | positive | the retained `git ls-remote` names both producers at their canonical URLs and was fetched for this pin, the record carries the binary's sha256, and it notes whether `main` has moved past the pin |
| `contract/checkout` | positive | the cached checkout is the pinned commit with no tracked change, no untracked or ignored file and no hidden index entry, and declares the pinned Go floor and bounds |
| `contract/binary-provenance` | positive | the binary hashes to the sha256 the fetch recorded, and `go version -m` reads `vcs.revision` = the pin, `vcs.time` = its commit time, the module version go stamps at that commit, `vcs.modified=false`, `CGO_ENABLED=0`, `-trimpath=true` and a Go at or above 1.27.1 |
| `contract/simulated-output-refused` | negative (E09-3) | a stand-in rebuilt from other sources that claims imago's module path and replays imago's stdout byte for byte is refused by the revision and module version go stamped from its own commit, and does not hash to the recorded sha256 |
| `contract/fixtures-identical` | positive | imago's vendored fixtures are byte-identical to the three payloads |
| `product-input/accepted` | positive (E09-1) | `imago aegis validate build/product-input.json --json` exits 0 and prints every field back as sent |
| `product-input/tampered-refused` | negative (E09-1) | four one-field changes each exit 1 with `Error: aegis product-input aegis-m18-product-input-0001: <field>: <reason>` |
| `product-input/retry-bound` | boundary (E09-1) | `retries.max-attempts` 10 is accepted and 11 refused |
| `product-input/packages-bound` | boundary (E09-1) | 256 packages are accepted and 257 refused |
| `product-input/aegis-bounds-inside-imago` | positive | a payload at the Aegis schema's own maxima is accepted |
| `kernel-requirement/accepted` | positive (E09-2) | both kernel payloads exit 0 and list every feature they declare |
| `kernel-requirement/invalid-feature-refused` | negative (E09-2) | a bad state, an unknown probe and a duplicated symbol each exit 1 with the payload's correlation id |
| `kernel-requirement/empty-features-refused` | boundary (E09-2) | no feature exits 1 with `features: feature list is empty`; one feature exits 0 |

Every case asserts an exit code and the text beside it. A refusal is exit 1 and
the correlated error on stderr: exit 0 is an acceptance, exit 2 would be a
crash, and the right code with the wrong text fails too. An acceptance of the
product input is compared field by field with the build request the payload
maps to, not read off the exit code.

## The recorded runs

`make contract-fetch`, run `f20260928T233237-725b`, into a fresh checkout:

```text
PASS contract-fetch/toolchain
     go version go1.27.1-X:nodwarf5 linux/amd64
     git version 2.55.0
PASS contract-fetch/identity
     git ls-remote --heads https://github.com/cordanaLLM/imago.git: exit 0;
       main 16f964b4dafadac2b1f0a662c7dcbb4b7bb29bee equals the pin
     git ls-remote --heads https://github.com/cordanaLLM/nucleus.git: exit 0;
       main 8672247ff1bd22ed6b6b89d498116f20ff00c2ab equals the pin
PASS contract-fetch/checkout
     https://github.com/cordanaLLM/imago.git at
       16f964b4dafadac2b1f0a662c7dcbb4b7bb29bee, depth 1, into a fresh directory
PASS contract-fetch/build
     .../aegis-contract/bin/imago: vcs.revision
       16f964b4dafadac2b1f0a662c7dcbb4b7bb29bee,
       v0.0.0-20260916130155-16f964b4dafa, built by go1.27.1-X:nodwarf5
     binary sha256
       71186e11e2589ad86d6dae318b8fa337752ed89d0e7de7911ddf53dd8d5a67bd,
       recorded in .../aegis-contract/identity.json
```

`make verify-contract`, run `r20260928T233248-1584`, offline (long lines folded
here); `make verify-all` ran the same fourteen cases as run
`r20260928T233750-b450`:

```text
PASS contract/pin
PASS contract/identity
PASS contract/checkout
     .../aegis-contract/imago at 16f964b4dafadac2b1f0a662c7dcbb4b7bb29bee:
       no tracked change, untracked or ignored file, or hidden index entry
     go.mod: go 1.27.1; pkg/aegis/aegis.go: MaxRetryAttempts 10, MaxPackages 256
PASS contract/binary-provenance
     github.com/cordanaLLM/imago/cmd/imago v0.0.0-20260916130155-16f964b4dafa
       built by go1.27.1-X:nodwarf5: vcs.revision
       16f964b4dafadac2b1f0a662c7dcbb4b7bb29bee, vcs.modified false,
       vcs.time 2026-09-16T13:01:55Z (commit time 2026-09-16T13:01:55Z)
     binary sha256
       71186e11e2589ad86d6dae318b8fa337752ed89d0e7de7911ddf53dd8d5a67bd,
       as the fetch recorded
PASS contract/fixtures-identical
PASS product-input/accepted
PASS contract/simulated-output-refused
     stand-in github.com/cordanaLLM/imago/cmd/imago: exit 0, stdout identical
       to the pinned binary's (565 bytes), sha256 not the recorded one
     refused: mod version is 'v0.0.0-20260928213248-5f0c0660483a', ...;
       vcs.revision is '5f0c0660483a...', the pinned build has
       '16f964b4dafadac2b1f0a662c7dcbb4b7bb29bee'; vcs.time is
       '2026-09-28T21:32:48Z', the pinned build has '2026-09-16T13:01:55Z'
PASS product-input/tampered-refused
     schema-v2: exit 1, refused: aegis product-input
       aegis-m18-product-input-0001: schema: want "aegis.p01.product-input.v1"
     unknown-field: exit 1, refused: ... input: strict decode: json:
       unknown field "image-digest"
     kernel-not-listed: exit 1, refused: ... kernel.default-package: must be
       one of packages
     revision-uppercase: exit 1, refused: ... revision: must be exactly 40
       lowercase hex characters
PASS product-input/retry-bound
     max-attempts-10: exit 0, accepted aegis-m18-product-input-0001
     max-attempts-11: exit 1, refused: ... retries.max-attempts: 11 outside 1..10
PASS product-input/packages-bound
     packages-256: exit 0, accepted aegis-m18-product-input-0001
     packages-257: exit 1, refused: ... packages: count 257 outside 1..256
PASS product-input/aegis-bounds-inside-imago
     Aegis MAX_PACKAGES 64, MAX_RETRY_ATTEMPTS 5, MAX_BACKOFF_SECONDS 3600;
       imago MaxPackages 256, MaxRetryAttempts 10
PASS kernel-requirement/accepted
     requirement: exit 0, accepted aegis-m18-kernel-requirement-0001
     reference: exit 0, accepted aegis-m18-kernel-reference-0001
PASS kernel-requirement/invalid-feature-refused
     state-yes: exit 1, refused: kernel requirement
       aegis-m18-kernel-requirement-0001: features[0].state: must be built-in
       or module
     probe-dmesg: exit 1, refused: ... features[0].probe: unknown probe "dmesg"
     duplicate-symbol: exit 1, refused: ... features[1].symbol: duplicate
       symbol CONFIG_PREEMPT_RT
PASS kernel-requirement/empty-features-refused
     features-0: exit 1, refused: kernel requirement
       aegis-m18-kernel-requirement-0001: features: feature list is empty
     features-1: exit 0, accepted aegis-m18-kernel-requirement-0001
```

imago prints each refusal twice on stderr, once from its command library and
once from `main()`; the gate asserts that the text is present, not how often.
The binary's sha256 is the one the first fetch of this milestone and an
independent build during its research both gave, so the build reproduced.

## Replays of a wrong cache

Each ran against a copy of the cache, never the one CI or `make verify-all`
reads, and each exited 1 with the payload cases not run, except where noted:

- An ignored file: `cmd/imago/zz_*.go` added to the checkout's
  `.git/info/exclude` and `cmd/imago/zz_standin.go` written, which a plain
  `git status --porcelain` does not list (run `r20260928T233309-4088`):
  `contract/checkout` failed with
  `the checkout is modified: ['!! cmd/imago/zz_standin.go']`. A
  `make contract-fetch` over that copy (run `f20260928T233349-cbe2`) replaced
  the checkout with a fresh clone, rebuilt the binary to the same sha256
  `71186e11e258...`, and the gate then passed (run `r20260928T233351-e8e6`,
  exit 0).
- An untracked `cmd/imago/zz_untracked.go`, with `GIT_CONFIG_GLOBAL` naming a
  file that sets `status.showUntrackedFiles = no` (run `r20260928T233309-c66e`):
  `contract/checkout` failed with
  `the checkout is modified: ['?? cmd/imago/zz_untracked.go']`.
- An identity record naming another imago commit, the state a pin edit leaves
  behind (run `r20260928T233309-b21d`): `contract/identity` failed with
  `imago: the cache was fetched for 335a22a534d8..., the pin names
  16f964b4dafa...; run make contract-fetch`. No SKIP line was printed.
- A stand-in binary with the pinned revision byte-patched into it, copied over
  the cached binary (run `r20260928T233329-ee4e`): `go version -m` read
  `vcs.revision=16f964b4...`, and `contract/binary-provenance` failed on the
  module version `v0.0.0-20260928213248-5f0c0660483a`, on `vcs.time` and on
  `binary sha256 6b1d341a0f69..., the fetch recorded 71186e11e258...`.
- `GIT_DIR` and `GIT_INDEX_FILE` exported as a git hook exports them, naming
  another repository (run `r20260928T233309-0a61`): with them,
  `git -C <checkout> rev-parse HEAD` prints that repository's HEAD; the gate
  does not pass them on, and all fourteen cases passed, exit 0.

What the checks do not prove: whoever can write the cache can also rewrite the
identity record with a forged binary's digest. The record ties the binary to
the fetch that built it; it is not a signature.

## The bounds, and where the two schemas differ

E09-1's boundary once read "a retry at the bound is recorded, and one above is
refused". No producer executes a retry yet, so D92 replaced it with the bound
imago enforces: `retries.max-attempts` from 1 to 10, and the packages count from
1 to 256, both in `pkg/aegis/aegis.go` at the pin. The pin records both values
and `contract/checkout` reads them back from the source, so imago changing a
bound fails the gate instead of moving the boundary silently.

The Aegis crate is stricter: at most 64 packages, 5 attempts and a backoff from
1 to 3600 s (`crates/aegis-fabrica-defs/src/manifest.rs`), where imago allows
256, 10 and 0 to 3600 s. Every payload Aegis can emit is therefore inside
imago's bounds, which `product-input/aegis-bounds-inside-imago` exercises at all
three Aegis maxima; the converse does not hold and does not need to.

## Scope, and what a pass here does not mean

- Consumption evidence on the reference profile. imago decoded, mapped and
  refused; it built nothing, signed nothing and returned no result.
- No hosted dispatch is claimed. Nothing was sent to either producer, no
  `repository_dispatch` was involved, and no issue or pull request was opened
  from here.
- It closes no image, boot, hardware or release gate. P01 and P02 stay
  proposals in `planning/components.json`.
- M11 and M10 become `ready` in the register, because every milestone in their
  `blocked_by` is done. Each still has an external BLOCKED-until criterion that
  holds: M11 waits for Imago to return an image/UKI and the product result, M10
  for a Nucleus-published kernel manifest. `ready` there is a register state,
  not an unblocking.
