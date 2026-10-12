<!--
SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Praetor pin auto-bump

Status: tooling, installed per workstation; adopted 2026-09-28

CI builds `praetorctl` from the Praetor commit that `PRAETOR_REF` in
`.github/workflows/ci.yml` names. The variable carries that name because
Praetor's hook launcher reads `PRAETOR_REF` under `.github/workflows` to pick
the engine local hooks run; under its former name, `PRAETOR_COMMIT`, the
launcher would find no pin and fall back to whatever `praetorctl` is on `PATH`.
The maintainer decided on 2026-09-28 that this
pin follows Praetor's `main` whenever `main` moves, and that the bump runs on
the maintainer's workstation rather than in GitHub Actions. A pull request
opened with the Actions token starts no CI in this repository; one pushed from
the maintainer's `gh` account does.

`tools/praetor_bump.py` performs the bump that pull requests #122 and #130 did
by hand. Three systemd user units run it: when `praetorctl` is reinstalled, and
once a day.

## Install, watch, stop

Install from the primary checkout, as yourself and not as root:

```sh
make install-praetor-bump
```

The target copies the three units from `tools/praetor-bump/` to
`~/.config/systemd/user/`, points them at this checkout, and enables
`aegis-praetor-bump.path` and `aegis-praetor-bump.timer`. It requires `gh auth
status` to succeed for the account that should open the pull requests, and the
seven state ledgers under `.workingdir/` that `flavor audit` reads (`LEDGERS` in
the script). `praetorctl state init . --if-absent` creates the five Markdown
ledgers, and only in a checkout that has none yet; it never repairs a partial
set, so a missing ledger has to be restored explicitly. `bugs.meta.json` and
`questions.meta.json` are written by the first `praetorctl state bug add` and
`praetorctl state question add`; `praetorctl state migrate-bugs` also writes
`bugs.meta.json` from the inline metadata of `BUGS.md`.

| Question | Answer |
| :--- | :--- |
| What did the last run do? | `journalctl --user -u aegis-praetor-bump.service` |
| When does it run next? | `systemctl --user list-timers aegis-praetor-bump.timer` |
| Why did a bump fail? | The journal first: every `FAIL:` and `RED:` line and any traceback is there. `.workingdir/evidence/praetor-bump-<sha7>.log` in the primary checkout exists only for a failure after the survey, not for a failed fetch or `gh pr list`, an unexpected exception or a stopped run |
| Who is told when a run fails? | Whoever the local notifier tells: see [Failure notice](#failure-notice). Without one, nobody, and the journal line `not announced:` says so |
| Run it once now | `systemctl --user start aegis-praetor-bump.service`, or `python3 tools/praetor_bump.py` |
| Pause it | `systemctl --user disable --now aegis-praetor-bump.path aegis-praetor-bump.timer` |
| Remove it | `make uninstall-praetor-bump` removes the units; [Clean up](#clean-up) removes the rest |

## What one run does

1. **Nothing to do?** It fetches Praetor's `main` into a bare clone under
   `${XDG_CACHE_HOME:-~/.cache}/aegis-praetor-bump/`. When Aegis `main` already
   pins that head, it prints `up to date` and exits 0. It also exits 0 when the
   open bump pull request already names that head and was built on the current
   Aegis `main` (the `Target: praetor` and `Base: aegis` lines of its body).
   When `main` has moved since, the bump is rebuilt on it: the ruleset requires
   an up-to-date branch, and a rebase made on the forge is an unsigned commit
   the signature rule refuses. The shared Praetor checkout
   next to this one only lends objects to the first clone (`git clone
   --reference-if-able --dissociate`); the script never writes to it.
2. **Build.** It builds `praetorctl` at that commit with the Verification
   gate's build steps and flags: `go mod download`, `go mod verify`,
   `go vet ./...`, then `go build -trimpath -buildvcs=false
   ./cmd/standardsctl` with `CGO_ENABLED=0` and `GOFLAGS=-mod=readonly`. A vet
   finding stops the bump before a binary exists, as it stops the gate.
   `tools/test_praetor_bump.py` holds the workflow to the script's step list.
   The Go toolchain is the
   workstation's, under its own `GOTOOLCHAIN` setting, whereas CI installs the
   version Praetor's `go.mod` declares and sets `GOTOOLCHAIN=local`.
3. **Adopt.** In a fresh worktree of Aegis `main` under the cache directory it
   runs `praetorctl adopt --force --profile os-image` against that commit.
   adopt reinstalls the git hooks and rewrites `info/lefthook.checksum`, which
   lefthook reads before each hook run; both are shared with the primary
   checkout, so both are put back byte for byte afterwards, also when adopt
   fails.
4. **Reconcile.** Every file in `HAND_MAINTAINED` in `tools/praetor_bump.py`
   goes back to its content on `main`: `lefthook.yml`, the evasion interceptor
   and its six hook registrations, the editor settings and adopt's `*.bak`
   files. `.standards-baseline.json` goes back when only its timestamp and
   commit changed. Its entries are never recorded by a bump: a finding the
   new engine reports and the file does not hold fails the gates, and a
   maintainer fixes it or records it with a reason, as with the
   [ten planted lint cases](#limits). `AGENTS.md` goes back as well,
   followed by
   `praetorctl compile-context`, so only the generated register block and the
   projections move. Each remaining change is then reverted unless
   `praetorctl compile-context --verify`, `praetorctl audit` or
   `praetorctl flavor audit .` fails without it. Changes are tried all
   together, then one file at a time, then in pairs, because some files only
   pass together, such as `.paperclip/harness.json` and the register digest
   in `.standards.yaml`.
5. **Pin.** It moves `PRAETOR_REF` and the praetorctl row of the
   [toolchain admission matrix](../roadmap/toolchain-admission.md), where the
   previous pin becomes the "Replaces" value.
6. **Gates.** With the new `praetorctl` first on `PATH` it runs
   `make verify-all`, which also runs `node tools/markdownlint/verify.mjs`
   (the Documentation Governance check) through its `docs-lint`
   prerequisite, then `praetorctl compile-context --verify`,
   `praetorctl audit`, `praetorctl flavor audit .`, markdownlint-cli2 at the
   version `ci.yml` pins, `yamllint .`, `flake8 .`, `black --check`,
   `reuse lint` and `mkdocs build --strict`. The portal build runs from a venv
   of `docs/requirements.txt` in the cache directory; when that venv cannot be
   made, the pull request body lists the build as a skip, not as a pass. The
   cargo target directory is `target/` under the cache directory. The state
   ledgers are copied into the worktree, never linked, because `flavor audit`
   rejects a symlinked `.workingdir/`.
7. **Publish.** On a pass it commits with `git commit --signoff`, with the
   repository's hooks running, and force-pushes `chore/praetor-pin-auto`. It
   opens that branch's pull request, or rewrites the title and body of the one
   already open, so at most one bump pull request exists. Every worktree and
   binary in the cache is then removed. On a failure it writes the failing
   lines and the whole log to the evidence file named above, keeps the
   worktree under `work/` in the cache directory until the next run that has
   work, hands one line to the [local notifier](#failure-notice), exits
   non-zero, and does not retry the same Praetor and Aegis pair until
   `--retry` is passed. A failure
   before the push pushes nothing. A failure after it, while opening or
   updating the pull request or queuing its merge, leaves the branch pushed,
   and the open pull request may still name the previous target. The `RED:`
   journal line and the evidence file's `forge:` line say which case happened;
   check the pull request's title against the branch, or rerun with `--retry`.

A second run while one is active prints `SKIP` and exits 0: a lock file in the
cache directory serialises them. Every command runs under its own deadline, and
the service itself stops after four hours. Stopping the service, or Ctrl-C on
a manual run, ends the running command with everything it started and, when
adopt was running, puts the shared git files back before the script exits.
Such a run leaves its journal but no evidence file and no failure marker.

## Options

| Option | Effect |
| :--- | :--- |
| `--merge` | After a pass, queue `gh pr merge --squash --auto`, but only when the repository allows auto-merge. It does not today, so the flag only reports that. |
| `--no-publish` | Commit in the worktree and stop: no push, no pull request. The worktree is kept until the next run that has work. |
| `--retry` | Run a Praetor and Aegis pair that already failed. |
| `--cache-dir`, `--repo`, `--praetor-url`, `--praetor-checkout` | Relocate the cache, the Aegis checkout, the Praetor source and the object donor. |

## Failure notice

A failed run starts `.workingdir/hooks/praetor-bump-notify` in the primary
checkout, when that file exists, with one argument: a line naming the old pin,
the target, the Aegis commit, the reason and the evidence file. The file is the
workstation's own and is not committed; what it does with the line (mail, chat,
an inbox file) is its choice. It runs under a 120 second deadline, and its own
failure is a `not announced:` line in the journal, not a retry.

The notice is sent for a `RED` run, for a `FAIL` before the survey finished and
for an unexpected exception. It is not sent again for a pair that already
failed (`RED (unchanged)`), and nothing is sent when systemd stops the service
at its four-hour limit: that case is in the journal only.

## Clean up

A failed run keeps its worktree so the failure can be inspected. The next run
that has work, for the same Praetor head with `--retry` or for a newer one,
first removes every worktree and binary that earlier runs kept, so the cache
holds one run's worktrees at most and the primary checkout's `git worktree
list` does not grow with each failed head. A successful run removes them as
well, together with the failure records. Past 64 kept worktrees per repository
(`MAX_STALE_WORKTREES`) a run removes 64, prints how many stay, and the run
after it continues. To delete a kept worktree once it has been inspected, run
from the primary checkout:

```sh
cache="${XDG_CACHE_HOME:-$HOME/.cache}/aegis-praetor-bump"
git worktree remove --force "$cache/work/aegis-<sha7>"
```

`make uninstall-praetor-bump` stops and removes the three units and nothing
else. To also remove the cache (the bare Praetor clone, the cargo target
directory, the documentation venv, binaries and kept worktrees), run from the
primary checkout afterwards:

```sh
cache="${XDG_CACHE_HOME:-$HOME/.cache}/aegis-praetor-bump"
for tree in "$cache"/work/aegis-*; do
  [ -d "$tree" ] && git worktree remove --force "$tree"
done
rm -rf "$cache"
git worktree prune
```

## Limits

- The linters, the Go toolchain and the Python behind the portal venv are the
  workstation's, not the versions CI pins; the pull request's own CI re-runs
  the pinned ones. The toolchain admission matrix lists them under
  [Not yet admitted](../roadmap/toolchain-admission.md#not-yet-admitted).
- Reverting in pairs finds two files that only pass together, not three.
  A larger group stays kept, and the pull request body lists every kept file
  for review.
- Pairs are tried only while at most 24 files (`MAX_PAIR_FILES`) are left
  after the one-at-a-time pass; past that no pair is tried. Past 256 candidates
  (`MAX_CANDIDATES`) the rest are kept without being tried. In both cases the
  pull request body lists those files as kept.
- The debt baseline holds ten entries, the planted cases of the M16 lint
  under `ui/concordia-tokens/hiss-lint/plants/` (decision D109 in the
  [roadmap](../roadmap/README.md)). adopt keeps an existing baseline and
  reports the ratchet's verdict only (`reconcileBaseline` in the engine at
  `299d3be`), so a bump never records one. An engine whose scan finds more
  fails the bump until the finding is fixed or the baseline is recorded
  again by hand.
- The script commits the pin, the admission row and adopt's output, nothing
  else. A change that the new engine requires and the pinned engine refuses
  has to enter with the bump itself and is added by hand: the HISS-11
  exception of 2026-10-11 (see `CHANGELOG.md`) was such a change, because the
  engine at `11ac5d8` refuses the `exceptions` key of `.standards.yaml`.
  cordanaLLM/praetor#1254 asks the engine for a bump command that carries
  such a change.
- The script never bypasses a hook. A bump that the hooks reject is a failure
  with evidence, not something to force.
- It never merges on its own. Merging stays a maintainer decision unless
  auto-merge is enabled on the repository and `--merge` is passed.

The decisions the script makes are covered by `tools/test_praetor_bump.py`,
which `make verify-all` runs; the lock and process-group helpers it borrows
from `tools/host.py` are covered by `tools/test_host.py`.
