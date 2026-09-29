<!--
SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# The contract pair, and what the pinned producers did with Aegis's payloads

Status: recorded observations from milestone M09, reference profile,
2026-09-28; nucleus's verifier added under D106 on 2026-09-29, and nucleus
re-pinned to `82aa6b7` the same day

Milestone M09 pins the cross-repository contract as far as the producers can
prove it today. Decision D92 scopes it to consumption: `cordanaLLM/imago`, built
from a pinned commit, decodes both payloads milestone M18 authored, refuses bad
ones with the payload's correlation id, and enforces its bounds at the edge.
Since D106, `cordanaLLM/nucleus` at its pinned commit verifies
`build/kernel-requirement.json` too, with the
`scripts/verify_kernel_requirement.py` its pull request 35 added, and the gate
reads what it decided from its JSON report. To run it on a machine with go and
git:

```sh
make contract-fetch   # the one networked step: ls-remote, clone both, build imago
make verify-contract  # the gate alone; make verify-all runs it too
```

Without go or git, or without either cached checkout, the binary or the
identity record, the gate prints
`SKIP: <reason>; the contract pair gate did not run.` and exits 0, so an exit 0
is evidence only when the case lines are above it. A cache that is present but
wrong fails and names `make contract-fetch`: one fetched for another pin, a
checkout holding anything its pinned commit lacks, or a binary that is not the
one the fetch built (D93). The identity record is read before any absent piece
can skip the run, so the cache a fetch from before D106 left -- its record names
nucleus `8672247` and it holds no nucleus checkout -- fails `contract/identity`
rather than skipping. CI runs `make contract-fetch` before
`make verify-all`, so the gate runs there instead of skipping
(`.github/workflows/ci.yml`). The gate has run on Linux only; on macOS and
Windows it runs wherever go and git are on `PATH`, which no CI leg provides, so
those legs run `tools/test_contract_pair.py` alone.

**M09 is `done`, as D92 scopes it.** Every exit criterion and all three epics
rest on the runs recorded below and on `tools/test_contract_pair.py`, which runs
inside `make verify-all` on every platform of the matrix.

**No result came back, and none was expected to.** Nothing produces imago's
`imago.p01.product-result.v1` yet (its ADR-0020; `cordanaLLM/imago` issue 46),
and nucleus has published no kernel: since `82aa6b7` its build matrix compiles
and gates every leg (issues 18 and 31, closed by its pull request 37), but it
has no release yet. Its verdict here is at evidence level `declared`, the
kconfig fragments as its merge script merges them, and `make olddefconfig` can
still drop a declared symbol. Under
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
| `${AEGIS_CONTRACT_DIR}/nucleus` | no | the depth-1 checkout of nucleus at the pinned commit, cloned fresh by every fetch; its verifier runs from it |
| `${AEGIS_CONTRACT_DIR}/bin/imago` | no | the binary built from it (`imago.exe` on Windows) |
| `${AEGIS_CONTRACT_DIR}/identity.json` | no | the retained `git ls-remote --heads` of both producers, the pin they were fetched for and the built binary's sha256 |
| `${AEGIS_CONTRACT_DIR}/runs/<run id>/` | no | every invocation's argv, exit code, stdout and stderr, every payload fed to a producer, every nucleus `--report-json` under `reports/`, and the run summary |

`AEGIS_CONTRACT_DIR` defaults to `${XDG_CACHE_HOME:-$HOME/.cache}/aegis-contract`.
Nothing is written into the repository, and nothing is sent to either producer.

## The pins

| Producer | Repository | Commit | Role in M09 |
| --- | --- | --- | --- |
| imago | `https://github.com/cordanaLLM/imago.git` | `16f964b4dafadac2b1f0a662c7dcbb4b7bb29bee` | consumes both payloads: `imago aegis validate` (`pkg/aegis`, ADR-0020) and `imago kernel requirement validate` (`pkg/kernel`, ADR-0021) |
| nucleus | `https://github.com/cordanaLLM/nucleus.git` | `82aa6b7a3c68a42a6330370c81ec642482014c9f` | verifies `build/kernel-requirement.json`: `scripts/verify_kernel_requirement.py` under the `versions.json` label `aegis-os`, bound to the `realtime` stream, report `nucleus.kernel-requirement-report.v1` at evidence level `declared` (D106); also publishes the `imago.nucleus.kernel-artifact.v1` manifest imago verifies |

imago's commit was its `main` on 2026-09-28 and still was on 2026-09-29;
nucleus's was its `main` on 2026-09-29 once its pull requests 36 and 37 had
merged, both read with `git ls-remote --heads`. nucleus was pinned at
`8672247ff1bd22ed6b6b89d498116f20ff00c2ab`, its `main` on 2026-09-28, by
identity only until D106, and at `0a4eac93f29fef432bfa9d892ad236568ce2f482`,
its pull request 35, from D106 until the re-pin. The 2026-09-13 local-only
pins, imago `4f116fc` and nucleus `78ca8f2`, never reached either producer and
are not used.

imago's `go.mod` declares `go 1.27.1`, and the fetch builds with
`GOTOOLCHAIN=local`, so a Go below that floor is refused rather than replaced
by a downloaded toolchain. CI's Go comes from `actions/setup-go` reading
Praetor's `go.mod`, which declares `go 1.27`; setup-go resolves that to the
newest 1.27 patch, and the Verification gate run of pull request 145 logged
`Setup go version spec 1.27` and `go1.27.1` on 2026-09-28, so CI builds imago
with no Go change. `docs/roadmap/toolchain-admission.md` records both rows.

Why nucleus was identity only until D106, from its own tree at `8672247`: no
file read `build/kernel-requirement.json`, a `correlation-id` or a
`required-by` field; `verify-requirements.yml` checked nineteen `CONFIG_*`
symbols hardcoded in the workflow (issue 20); `publish-release.yml` wrote the
`imago.nucleus.kernel-artifact.v1` manifest from a kernel build the forge did
not yet perform (issue 18); and its `AGENTS.md`, under "The contract with
imago", named imago's own `kernel/requirement.json` as its inbound.

What it reads since `0a4eac9`, and still at `82aa6b7`, from the same tree:
`versions.json` declares each requirement document under
`downstream.requirements`, and binds the label `aegis-os` to
`cordanaLLM/Aegis-OS` `build/kernel-requirement.json` and the `realtime`
stream; `scripts/verify_kernel_requirement.py`, standard-library Python with
its sibling `scripts/versions_query.py`, decodes a document the way
`crates/aegis-fabrica-defs` does and holds every feature, on every listed
architecture, against the kconfig fragments of each bound stream; `AGENTS.md`
now lists both documents as its inbound. Its ADR-0007, still Proposed, asked
this repository two questions, and D103 and D104 answer them.

What `82aa6b7` changes for this gate. Pull request 36 adds a second evidence
level, `resolved` (its ADR-0008, Proposed): given
`--resolved-config STREAM:ARCH=PATH`, the verifier holds the document against a
`.config` resolved from the stream's fetched, signature-verified kernel source.
Producing one needs the network and a kernel tree, so the gate never passes the
flag and the pin keeps `declared`. Without it the report keeps its schema, its
top-level fields and every document field; each stream row gains `evidence`,
`declared` here, and the stream versions follow `versions.json` (`realtime` is
`7.2.8`, was `7.2-rt`). The verifier now puts its own directory on `sys.path`
to import that sibling module, which `python3 -I` otherwise hides (its commit
`229f164`, before the merge). Pull request 37 compiles and gates every stream
and architecture leg before anything is signed.

## How one run works

`make contract-fetch` runs, in order: `go version` against the floor;
`git ls-remote --heads` against both producers, keeping the output; a depth-1
fetch of imago, then of nucleus, each at its pinned commit into a fresh
directory, replacing any cached checkout rather than reusing it;
`go mod download` and `go mod verify`; and
`GOTOOLCHAIN=local CGO_ENABLED=0 GOFLAGS=-mod=readonly go build -trimpath -buildvcs=true`.
The binary is kept only when `go version -m` reads the pinned build out of it.
The identity record is written last, with the binary's sha256, so it always
names a complete fetch and the binary that fetch produced.

`make verify-contract` then runs offline, with `GOPROXY=off` on every go
command. It checks the pin and this repository's payloads, the identity record,
both checkouts and the binary first; only when all five pass does it run
anything a producer prints. Each payload it feeds a producer is written under
the run directory, and each invocation is retained there.

nucleus's verifier runs from the nucleus checkout, where it reads `kconfig/` and
`versions.json`, under the interpreter running the gate with `-I`, so no
`PYTHON*` variable, user site or working-directory module takes part, with
`-B`, so importing its sibling module writes no `scripts/__pycache__` into the
checkout the next run requires clean, and with every path absolute. Each run
passes `--requirement aegis-os=<payload>`,
`--report-json <run directory>/reports/<row>.json` and `--nucleus-revision
<pin>`. The gate asserts on that report and never on the text the script prints,
which its interface lets grow: the schema
`nucleus.kernel-requirement-report.v1`, the revision, the evidence level the pin
records, the verdict, and in the one document the label, the status, the sha256
of the bytes sent, and for a pass the correlation id, the bound and held streams
and an empty reason list; for a refusal every reason and the per-feature check;
for a rejection its kind. The exit code must match too: 0 when every document
passes, 1 otherwise.

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
| `contract/nucleus-checkout` | positive (D106) | the nucleus checkout is the pinned commit with no tracked change, untracked or ignored file or hidden index entry, holds `scripts/verify_kernel_requirement.py`, and its `versions.json` binds exactly one row to `aegis-os`: `cordanaLLM/Aegis-OS` `build/kernel-requirement.json` on `realtime` |
| `nucleus/accepted` | positive (D106) | the committed requirement, sent with `--sha256` and `--correlation-id` bound to its own bytes and id, exits 0: PASS, held by `realtime`, no reason, at evidence level `declared` |
| `nucleus/correlated-refusal` | negative (D106) | the requirement with a built-in `CONFIG_AEGIS_CONTRACT_UNSET` planted exits 1: FAIL, every reason opening with `aegis-m18-kernel-requirement-0001:` and one naming the symbol, which the report records as unset and unmet on `realtime` `x86_64` |
| `nucleus/empty-features-refused` | boundary (D106) | no feature exits 1, REJECTED `NoFeatures`; one feature exits 0, PASS |
| `nucleus/dispatch-binding-refused` | negative (D106) | the committed requirement with the sha256 of `build/kernel-requirement.reference.json`, or with that document's correlation id, exits 1: REJECTED `DigestMismatch` and `CorrelationMismatch`, the digest proven before the document is parsed |

Every case asserts an exit code and the text or report beside it. For imago a
refusal is exit 1 and the correlated error on stderr: exit 0 is an acceptance,
exit 2 would be a crash, and the right code with the wrong text fails too. For
nucleus the report decides, and the exit code must agree with it. An acceptance
of the product input is compared field by field with the build request the
payload maps to, not read off the exit code.

## The recorded runs of 2026-09-28

These are the runs M09 closed on, with nucleus pinned at `8672247` by identity
only. `make contract-fetch`, run `f20260928T233237-725b`, into a fresh checkout:

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

## The recorded runs of 2026-09-29 (D106)

`make contract-fetch`, run `f20260929T084215-2847`, into a fresh cache, with the
nucleus pin moved to `0a4eac9` (long lines folded here):

```text
PASS contract-fetch/identity
     git ls-remote --heads https://github.com/cordanaLLM/imago.git: exit 0;
       main 16f964b4dafadac2b1f0a662c7dcbb4b7bb29bee equals the pin
     git ls-remote --heads https://github.com/cordanaLLM/nucleus.git: exit 0;
       main 0a4eac93f29fef432bfa9d892ad236568ce2f482 equals the pin
PASS contract-fetch/checkout
PASS contract-fetch/nucleus-checkout
     https://github.com/cordanaLLM/nucleus.git at
       0a4eac93f29fef432bfa9d892ad236568ce2f482, depth 1, into a fresh directory
PASS contract-fetch/build
     binary sha256
       71186e11e2589ad86d6dae318b8fa337752ed89d0e7de7911ddf53dd8d5a67bd,
       recorded in .../identity.json
```

The imago binary rebuilt to the same sha256 as on 2026-09-28. `make
verify-contract`, run `r20260929T085636-c155`, offline, printed the fourteen
imago cases as before, unchanged, and these five (folded); after review, run
`r20260929T092808-d300` printed the same nineteen:

```text
PASS contract/nucleus-checkout
     .../nucleus at 0a4eac93f29fef432bfa9d892ad236568ce2f482: no tracked
       change, untracked or ignored file, or hidden index entry
     versions.json binds aegis-os to cordanaLLM/Aegis-OS
       build/kernel-requirement.json on realtime; verifier
       scripts/verify_kernel_requirement.py
PASS nucleus/accepted
     requirement (--sha256 --correlation-id): exit 0, PASS
       (aegis-m18-kernel-requirement-0001 held by realtime), sha256
       d796c408b4db, evidence level declared
PASS nucleus/correlated-refusal
     planted-unset-symbol: exit 1, FAIL (aegis-m18-kernel-requirement-0001:
       realtime x86_64: CONFIG_AEGIS_CONTRACT_UNSET (required-by REQ-P07-01)
       requires built-in, observed unrecorded), sha256 706ba4ec256d, evidence
       level declared
PASS nucleus/empty-features-refused
     features-0: exit 1, REJECTED (NoFeatures), sha256 cfdcdf932ece, ...
     features-1: exit 0, PASS (aegis-m18-kernel-requirement-0001 held by
       realtime), sha256 57de5dce775b, evidence level declared
PASS nucleus/dispatch-binding-refused
     sha256-of-another-document (--sha256): exit 1, REJECTED (DigestMismatch),
       sha256 d796c408b4db, evidence level declared
     correlation-id-of-another (--correlation-id): exit 1, REJECTED
       (CorrelationMismatch), sha256 d796c408b4db, evidence level declared
```

`make verify-all` ran the same nineteen cases as run `r20260929T085813-f455`,
and after review as run `r20260929T092828-797e`.

## The recorded runs of 2026-09-29, nucleus re-pinned to 82aa6b7

`make contract-fetch`, run `f20260929T101312-03f2`, with the nucleus pin moved
from `0a4eac9` to `82aa6b7` (folded; the imago lines are as before):

```text
PASS contract-fetch/identity
     git ls-remote --heads https://github.com/cordanaLLM/nucleus.git: exit 0;
       main 82aa6b7a3c68a42a6330370c81ec642482014c9f equals the pin
PASS contract-fetch/nucleus-checkout
     https://github.com/cordanaLLM/nucleus.git at
       82aa6b7a3c68a42a6330370c81ec642482014c9f, depth 1, into a fresh directory
PASS contract-fetch/build
     binary sha256
       71186e11e2589ad86d6dae318b8fa337752ed89d0e7de7911ddf53dd8d5a67bd,
       recorded in .../identity.json
```

`make verify-contract`, run `r20260929T101320-e64d`, then passed all nineteen
cases. The next gate run, inside `make verify-all` (run
`r20260929T102148-b707`), failed:

```text
FAIL contract/nucleus-checkout
     the checkout is modified:
       ['!! scripts/__pycache__/versions_query.cpython-314.pyc']
```

At `82aa6b7` the verifier imports its sibling `scripts/versions_query.py`, and
`python3 -I` alone writes that import's bytecode into the checkout, which the
next run refuses. By hand, from a separate clone, `-I` wrote
`scripts/__pycache__/` and `-I -B` wrote nothing, with byte-identical reports.
The gate now passes `-B` as well, and `tools/test_contract_pair.py` runs a
stand-in that imports a sibling module both ways.

With that change, `make contract-fetch` (run `f20260929T102410-28e2`) cloned a
fresh checkout, and `make verify-contract` ran twice in a row, runs
`r20260929T102414-7f8a` and `r20260929T102414-cc48`, each passing all
nineteen cases, the fourteen imago cases unchanged, with
`git status --porcelain --ignored` of the nucleus checkout empty between them.
The nucleus lines (folded):

```text
PASS contract/nucleus-checkout
     .../nucleus at 82aa6b7a3c68a42a6330370c81ec642482014c9f: no tracked
       change, untracked or ignored file, or hidden index entry
     versions.json binds aegis-os to cordanaLLM/Aegis-OS
       build/kernel-requirement.json on realtime; verifier
       scripts/verify_kernel_requirement.py
PASS nucleus/accepted
     requirement (--sha256 --correlation-id): exit 0, PASS
       (aegis-m18-kernel-requirement-0001 held by realtime), sha256
       d796c408b4db, evidence level declared
PASS nucleus/correlated-refusal
     planted-unset-symbol: exit 1, FAIL (aegis-m18-kernel-requirement-0001:
       realtime x86_64: CONFIG_AEGIS_CONTRACT_UNSET (required-by REQ-P07-01)
       requires built-in, observed unrecorded), sha256 706ba4ec256d, evidence
       level declared
PASS nucleus/empty-features-refused
     features-0: exit 1, REJECTED (NoFeatures), sha256 cfdcdf932ece, ...
     features-1: exit 0, PASS (aegis-m18-kernel-requirement-0001 held by
       realtime), sha256 57de5dce775b, evidence level declared
PASS nucleus/dispatch-binding-refused
     sha256-of-another-document (--sha256): exit 1, REJECTED (DigestMismatch),
       sha256 d796c408b4db, evidence level declared
     correlation-id-of-another (--correlation-id): exit 1, REJECTED
       (CorrelationMismatch), sha256 d796c408b4db, evidence level declared
```

`make verify-all` ran the same nineteen cases as run `r20260929T102617-ece2`.
Every report the `-B` runs wrote is byte-identical to run
`r20260929T101320-e64d`'s. The reports `tools/test_contract_pair.py` keeps were
re-captured from them; beyond the revision, the fields they keep read as they
did at `0a4eac9`.

A copy of the cache whose identity record names nucleus `0a4eac9`, the state
every cache fetched before this re-pin is in (run `r20260929T102000-a8c1`),
failed `contract/identity` with `nucleus: the cache was fetched for
0a4eac93f29f..., the pin names 82aa6b7a3c68...; run make contract-fetch`,
exit 1, with no other case run.

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

On 2026-09-29, against copies of the D106 cache, each exited as noted:

- An identity record naming the nucleus commit of before D106, `8672247`, in a
  cache that also held the pinned nucleus checkout (run
  `r20260929T085243-2c5e`): `contract/identity` failed with `nucleus: the cache
  was fetched for 8672247ff1bd..., the pin names 0a4eac93f29f...; run make
  contract-fetch`, exit 1, and no nucleus row ran.
- An untracked `scripts/zz_planted.py` in the nucleus checkout (run
  `r20260929T085243-9938`): `contract/nucleus-checkout` failed with `the
  checkout is modified: ['?? scripts/zz_planted.py']`, exit 1.
- `versions.json` in the nucleus checkout edited to bind `aegis-os` to `lts`
  (run `r20260929T085243-a0d5`): `contract/nucleus-checkout` failed twice over,
  `the checkout is modified: [' M versions.json']` and `versions.json aegis-os
  streams is ['lts'], pinned ['realtime']`, exit 1.
- The cache a fetch from before D106 left, the state the pin edit leaves
  behind: its identity record (fetch run `f20260928T233237-725b`) names nucleus
  `8672247` and it holds no nucleus checkout. The gate first printed `SKIP: no
  nucleus checkout under <cache>; run make contract-fetch; the contract pair
  gate did not run.` and exited 0, the absence hiding the wrong record; review
  found it. The record is now read before any absent piece, and a copy of that
  cache (run `r20260929T092142-aeb6`) failed `contract/identity` with
  `nucleus: the cache was fetched for 8672247ff1bd..., the pin names
  0a4eac93f29f...; run make contract-fetch`, exit 1, with no other case run
  and no SKIP line.

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
- nucleus's verdict is at evidence level `declared`: the kconfig fragments of
  its `realtime` stream as its merge script merges them, not a built
  configuration, so it is not the check of a feature against a built kernel
  that D94 gives M10 (E10-5), and no Nucleus kernel result came back (E10-4).
  The runtime probes the requirement names are checked through their Kconfig
  symbol only; nucleus defers the probes themselves to boot.
- The `resolved` level nucleus offers since `82aa6b7` is M10's input, not this
  gate's: E10-4 and E10-5 read the release asset `kernel-realtime-x86_64.config`
  that the `imago.nucleus.kernel-artifact.v1` manifest pins by
  `kernel.config_digest`, and nucleus has published no release yet.
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
