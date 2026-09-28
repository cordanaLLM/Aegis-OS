<!--
SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# The accessibility harness, and what the P12 tokens told it

Status: recorded observations from milestone M04, reference profile, 2026-09-28

Milestone M04 builds the first accessibility gate: the P12 Concordia token
file, one Svelte 5 component that consumes it, and a Playwright and axe-core
suite that scans the component headless in a container with its network
disabled. To run it on a machine with podman or docker:

```sh
make a11y-fetch   # the one networked step: image, Node, pnpm, offline store
make verify-a11y  # the gate alone; make verify-all runs it too
```

Without an engine, the image or the cache, or with an offline store that
`make a11y-fetch` filled for another `pnpm-lock.yaml`, the gate prints
`SKIP: <reason>; the accessibility gate did not run.` and exits 0, so an exit 0
is evidence only when the case lines are above it. CI runs `make a11y-fetch`
before `make verify-all`, so the gate runs there instead of skipping
(`.github/workflows/ci.yml`, D88).

**M04 is `done`.** Every exit criterion and both epics rest on the run recorded
below and on `tools/test_a11y.py`, which runs inside `make verify-all` on every
platform of the matrix. The epic E04-2 requirements REQ-P12-01, REQ-P12-02 and
REQ-P12-03 are covered at the CSS level only, in browser emulation; their
portal, AT-SPI2 and UKI halves are not M04's (D89), and no milestone's criteria
carry them yet.

**A pass covers one component, not the desktop.** The gate scans the P12 token
component. It closes no accessibility gate for the P05 shell or for an image,
it proves nothing about the desktop portal, AT-SPI2 or UKI compilation, and it
is not release evidence. P12 stays a proposal in `planning/components.json`
(D86).

## What is tracked, and what is not

| Path | Role |
| :--- | :--- |
| `ui/concordia-tokens/concordia-tokens.css` | the token file: every colour, stroke, focus-ring and motion value is a custom property on `:root` (D76) |
| `ui/concordia-tokens/src/ConcordiaPanel.svelte` | the one Svelte 5 component; its style block uses tokens only |
| `ui/concordia-tokens/package.json`, `pnpm-lock.yaml`, `pnpm-workspace.yaml` | exact versions, `packageManager`, exact `engines`, `engineStrict` |
| `ui/concordia-tokens/toolchain.pin.json` | the image by digest, its browser revision, and the Node release by sha256 |
| `ui/concordia-tokens/tests/a11y.spec.js` | the eleven tests |
| `ui/concordia-tokens/tests/probe.js` | the in-page measurement axe-core cannot make |
| `ui/concordia-tokens/tests/en301549-clauses.json` | WCAG criterion to EN 301 549 V4.1.1 and V3.2.1 clause (D81) |
| `ui/concordia-tokens/tests/fixtures/stub-heads-up-theme.css` | the D76 stub, never part of the build |
| `tools/verify_a11y.py` | the gate: `make verify-a11y`, `make a11y-fetch`, and a step of `make verify-all` |
| `tools/test_a11y.py` | the half that needs no engine, inside `make verify-all` |

Not tracked: the image, the Node tarball, the pnpm binary, the offline pnpm
store and every run's logs. They live under `AEGIS_A11Y_DIR`, default
`${XDG_CACHE_HOME:-$HOME/.cache}/aegis-a11y`. A run keeps its step logs and the
suite's JSON reports under `runs/` and deletes the unpacked toolchain, the
installed packages and the build; the newest eight runs are kept. The gate
writes nothing into the repository, which the container sees read-only.

`make a11y-fetch` records the sha256 of the `pnpm-lock.yaml` it filled the
offline store from, in `offline/pnpm-lock.sha256` beside the store, and only
after `pnpm fetch` succeeded. The gate compares it with the checkout's
lockfile: a store filled before a lockfile bump lacks what the bump added, so
the gate skips and names `make a11y-fetch` as the cure, as it does when a bump
makes the image or an archive absent. Only a cached archive that no longer
hashes to its pin is a failure.

## The pins, and the digest a truncated one cannot be

| Input | Pin | Where |
| :--- | :--- | :--- |
| Playwright image | `mcr.microsoft.com/playwright:v1.63.0-noble@sha256:eff16c30e6f3f4af0a03fa4b706120d5e9b0891c344a27d64559aff5900a4a27`; linux/amd64 manifest `sha256:bc6ab0d6d44ff4826e4cb8c1e6d801e185bfc42bb0753f8e2a30efc70db054c7` | `toolchain.pin.json` |
| Browser | `chromium-headless-shell` revision 1243, Chromium 153.0.8010.12, as bundled by `@playwright/test` 1.63.0 and baked into the image | `toolchain.pin.json`, read back from `playwright-core/browsers.json` |
| Node.js | 26.10.0, `node-v26.10.0-linux-x64.tar.xz` sha256 `ca70e9e349de048b9522abb3adc05b3bd6f43c5ffd3ec57916c7da292f59f022` | `toolchain.pin.json`, `engines.node` |
| pnpm | 12.6.0; `@pnpm/exe.linux-x64` integrity `sha512-qFWBneHJAJ73W4whtbaFOL1M/7DBC6ILHXuxc7ZPtEhfPuT1zeZiGrmKHoMAfJA+mcm6xhOFljqVTUS+00Jabw==` | `packageManager`, `engines.pnpm`, `pnpm-lock.yaml` |
| npm packages | exact versions, each with its integrity | `package.json`, `pnpm-lock.yaml` |

The image digest was resolved on 2026-09-28 with
`skopeo inspect docker://mcr.microsoft.com/playwright:v1.63.0-noble`, and the
engine is asked for the image by that digest, never by tag: `make a11y-fetch`
pulls `mcr.microsoft.com/playwright@sha256:eff16c30…`, and the gate does not
run unless the engine lists that exact digest among the image's repo digests.
REQ-P12-04's source (export-020, section 7.3 of its P12 report) names its
validation container as
`ghcr.io/lusoris/concordia-validate@sha256:8f47c3...`: six hex digits and an
ellipsis, of an image M04 does not adopt. A truncated digest names no single
manifest, so it is recorded as non-pinnable (E04-2): the case
`a11y/truncated-digest-refused` accepts the full 64-digit digest of the pinned
image, refuses it cut to 63 digits and to 12, and refuses the source's own
digest.

The Node tarball's sha256 is the value in the release's `SHASUMS256.txt`,
whose signature verified on 2026-09-28 against the Node.js release key
`5BE8A3F6C8A5C01D106C0AD820B1A390B168D356` from `nodejs/release-keys`. The
image ships its own Node, 24.20.0; the gate does not use it. Node 26.10.0 is
mounted read-only at `/opt/node` and put first on `PATH`, and the exact
`engines` pin with `engineStrict` turns any other Node into a refusal
(`a11y/image-node-refused`). pnpm 12 is a native binary, so it needs no Node to
run; its integrity is the one pnpm itself records in the first document of
`pnpm-lock.yaml`.

The reference profile's own browser cache, `~/.cache/ms-playwright`, held
chromium-1228 on 2026-09-13 and chromium-1243 on 2026-09-28. Praetor's figure
engine writes it, not an Aegis admission, and the gate never reads it: the
browser the suite launches is the one inside the pinned image (D90).

## How one run works

Every step is one container: `run --rm --pull=never --read-only --tmpfs /tmp
--shm-size=1g --network=none`, the repository bind-mounted read-only at
`/repo`, a per-run scratch directory at `/work`, Node and pnpm read-only, and
the offline store and pnpm's metadata cache read-only at `/offline`. The first
step copies the package out of `/repo` into `/work/pkg`, so nothing the suite
writes reaches the checkout. Only `make a11y-fetch` runs a container with a
network, for `pnpm fetch`, and with `/offline` writable.

The engine is rootless podman when it is on `PATH`, else docker. Rootless podman
needs no daemon and no group, and what the container writes belongs to the user
who ran the gate; docker's daemon runs as root here, so the gate passes
`--user` with the caller's uid and gid (`host.uid()`, `host.gid()`).
`AEGIS_A11Y_ENGINE=docker` forces the fallback. Both were run on the reference
profile: podman 6.1.2 (run `r20260928T215941-925c`) and docker 29.8.1 (run
`r20260928T220230-fb99`), with the same thirteen case lines. The containers run
without `--init`: podman needs a separately packaged init binary for it, which
Ubuntu 24.04's podman only recommends, and none is needed because Playwright
starts `vite preview` itself and ends it by process group. A step that runs
past its deadline has its container removed by name, because ending the engine
client does not end the container.

## The cases, and what each proves

| Case | Proves | Criterion or epic |
| :--- | :--- | :--- |
| `a11y/pin` | the pin, `package.json` and the lockfile name one image, one Node, one pnpm | 1, 4 |
| `a11y/truncated-digest-refused` | 64 hex digits pin; 63, 12 and the source's six-digit form are refused | E04-2, REQ-P12-04 |
| `a11y/image-present` | the engine stores the pinned digest | 2, 5 |
| `a11y/lockfile-installs` | `pnpm install --offline --frozen-lockfile --frozen-store` on the pinned Node and pnpm | E04-1 positive |
| `a11y/lockfile-mismatch-refused` | a `package.json` loosened to `^5.57.1` is refused with `ERR_PNPM_OUTDATED_LOCKFILE` | E04-1 negative |
| `a11y/image-node-refused` | the image's Node 24.20.0 is refused with `ERR_PNPM_UNSUPPORTED_ENGINE` | 4, 6 |
| `a11y/toolchain-readback` | every version read back inside the container; Chromium 153.0.8010.12 launches | 1, 5 |
| `a11y/browser-revision-absent-refused` | without revision 1243 the launch fails, offline, and nothing is downloaded | 5 |
| `a11y/build` | the static page builds | cheapest exit |
| `a11y/suite` | eleven tests, the D81 report, the measured checks | 2, 7, 8, E04-2 |
| `a11y/planted-violation-fails` | one planted `image-alt` violation fails the run | E04-1 boundary, 2 |
| `a11y/stripped-outline-fails` | a stripped focus outline fails the run | 2, E04-2 negative |
| `a11y/d76-stub-theme-fails` | the D76 stub fails the existing boundary test | 8, REQ-P12-10 negative |

The suite has no retries and forbids `.only`; `tools/test_a11y.py` fails when
the spec, its harness or the probe gains a skip, a fixme, an expected failure,
a retry setting, a rule exclusion or an impact filter, and each spelling has a
planted negative there. The gate does not rely on that list alone: it counts a
test as passed only when Playwright's JSON report says the test was expected
to pass, its outcome was `expected` and its last result `passed`. Playwright's
own `ok` flag is also true for an expected failure, a skip and a flaky retry,
so a `test.fail()` added to a test that then fails would otherwise read as a
pass. The gate also reads the measured reports itself (the tab stops, the
stripped outline, both boundaries, the D76 stub, 200% text and the two
emulations) and fails when one records another value. The planted runs
select one test with `--grep`, and each case requires that test, and only that
test, to fail with its own reason in the message: `image-alt`,
`indicator 0px is below the D16 floor` and `indicator 1px is below the D16
floor`.

## The recorded run

From run `r20260928T215941-925c` on podman 6.1.2, 20 seconds, lines wrapped
at 80 columns and long digests shortened:

```text
PASS a11y/truncated-digest-refused
     full sha256:eff16c30e6f3...a4a27: accepted
     63 digits sha256:eff16c30e6f3...a4a2: refused
     12 digits sha256:eff16c30e6f3: refused
     REQ-P12-04's source (export-020)
       ghcr.io/lusoris/concordia-validate@sha256:8f47c3...: refused
PASS a11y/lockfile-installs
     pnpm install --offline --frozen-lockfile --frozen-store: exit 0
PASS a11y/lockfile-mismatch-refused
     svelte 5.57.1 loosened to ^5.57.1, pnpm install --frozen-lockfile:
       exit 1
     Error: ERR_PNPM_OUTDATED_LOCKFILE
PASS a11y/image-node-refused
     the same install on the image's own Node, without the pinned Node on
       PATH: exit 1
     Error: ERR_PNPM_UNSUPPORTED_ENGINE
PASS a11y/toolchain-readback
     node v26.10.0, pnpm 12.6.0, @playwright/test 1.63.0, axe-core 4.13.0,
       @axe-core/playwright 4.13.0, svelte 5.57.1, vite 8.3.1
     chromium-headless-shell revision 1243 in /ms-playwright: present True;
       launched 153.0.8010.12
PASS a11y/browser-revision-absent-refused
     browserType.launch: Executable doesn't exist at
       /work/no-browsers/chromium_headless_shell-1243/...
PASS a11y/suite
     11/11 tests passed; exit 0
     axe-core 4.13.0 on Chromium 153.0.8010.12, tags wcag2a, wcag2aa,
       wcag21a, wcag21aa, wcag22aa, no impact filter: violations 0,
       incomplete 0, passes 22, inapplicable 41
     target-size executed: passes, 3 targets
```

## Two replays from the review

The delivery's review found two ways the gate could report the wrong thing, and
both were replayed on this host against the fixed gate, each in a scratch copy
of `tools/` and `ui/` with a copy of the cache, so neither touched the checkout.

**An expected failure.** The review added `test.fail();` to the 200% text test
and made the lead paragraph clip (a fixed 700 px width, a 3 rem height, overflow
hidden). The test then fails, Playwright records it as an expected failure,
`playwright test` exits 0, and its JSON report still calls the spec `ok`; the
gate as first delivered printed `11/11 tests passed` and `PASS`. The fixed gate:

```text
FAIL a11y/suite
     10/11 tests passed; exit 0
     10 of 11 tests passed; 11 are expected
     not a pass: 200% text scaling: no overflow and no clipped text, and the
       detector sees a planted clip (outcome ['expected'])
     text-scaling-200 records [[], '32px', ['p "This panel draws only from
       the P12 base " clips its content'], True]; the gate requires
       [[], '32px', [], True]
FAIL: 1 accessibility case(s) did not match their recorded outcome.
```

It exits 1, from the test outcome and from the measured report independently.

**A store filled before a lockfile bump.** The replay moved `svelte` from 5.57.1
to 5.57.0 in `package.json` and `pnpm-lock.yaml`, with 5.57.0's integrity from
the npm registry, and kept the cache `make a11y-fetch` had filled for the
committed lockfile. The gate as first delivered failed `make verify-all` there,
at `pnpm install --offline`, with advice that pointed away from the cure. The
fixed gate, exit 0:

```text
SKIP: the offline pnpm store under <cache> was filled for another
  pnpm-lock.yaml (sha256 c2b89b1390d5, this checkout's is c6638cb0bfb3); run
  `make a11y-fetch`; the accessibility gate did not run.
```

On this host's own cache, which predates the marker, the first run after the
fix printed a SKIP line saying the store `records no pnpm-lock.yaml digest`,
and `make a11y-fetch` then recorded
`c2b89b1390d5297e06e73d46c144367652900763735d06e8870dd5b9752e2359`, the
sha256 of the committed `pnpm-lock.yaml`.

## D81: the report per criterion

axe-core runs exactly the D81 tag set, `wcag2a`, `wcag2aa`, `wcag21a`,
`wcag21aa` and `wcag22aa`, with no impact filter: one violation of any impact
fails the default state, and so does one `incomplete` result. The tag set alone
makes axe-core 4.13.0 run `target-size`, which it ships disabled; the suite
asserts from its own results that `target-size` evaluated the panel's three
targets and passed.

For each criterion an executed rule is tagged with, the report names the
clause in both editions from `tests/en301549-clauses.json`, whose every number
was checked against the headings of the two standards at the digests the
register cites. Clause 11 is non-web software; where V3.2.1 splits a clause
into open and closed functionality, the open-functionality sub-clause is named.
"Ran, no node" lists rules that ran and found nothing to evaluate.

| WCAG | V4.1.1 | V3.2.1 | Evaluated by | Ran, no node |
| :--- | :--- | :--- | :--- | :--- |
| 1.1.1 | 11.1.1.1 | 11.1.1.1.1 | none | image-alt and six more |
| 1.2.2 | 11.1.2.2 | 11.1.2.2 | none | video-caption |
| 1.3.1 | 11.1.3.1 | 11.1.3.1.1 | aria-hidden-body, list, listitem | six |
| 1.3.5 | 11.1.3.5 | 11.1.3.5.1 | autocomplete-valid | |
| 1.4.1 | 11.1.4.1 | 11.1.4.1 | none | link-in-text-block |
| 1.4.2 | 11.1.4.2 | 11.1.4.2 | none | no-autoplay-audio |
| 1.4.3 | 11.1.4.3 | 11.1.4.3 | color-contrast | |
| 1.4.4 | 11.1.4.4 | 11.1.4.4.1 | meta-viewport | |
| 1.4.12 | 11.1.4.12 | 11.1.4.12 | none | avoid-inline-spacing |
| 2.1.1 | 11.2.1.1 | 11.2.1.1.1 | none | three |
| 2.1.3 (AAA) | 11.2.1.3 (Void) | 11.2.1.3 (Void) | none | scrollable-region-focusable |
| 2.2.1 | 11.2.2.1 | 11.2.2.1 | none | meta-refresh |
| 2.2.2 | 11.2.2.2 | 11.2.2.2 | none | blink, marquee |
| 2.4.1 | 11.2.4.1 (Void) | 11.2.4.1 (Void) | bypass | |
| 2.4.2 | 11.2.4.2 | 11.2.4.2 (Void) | document-title | |
| 2.4.4 | 11.2.4.4 | 11.2.4.4 | link-name | area-alt |
| 2.5.8 | 11.2.5.8 | none in V3.2.1 | target-size | |
| 3.1.1 | 11.3.1.1 | 11.3.1.1.1 | html-has-lang, html-lang-valid | html-xml-lang-mismatch |
| 3.1.2 | 11.3.1.2 (Void) | 11.3.1.2 (Void) | none | valid-lang |
| 3.3.2 | 11.3.3.2 | 11.3.3.2 | form-field-multiple-labels | |
| 4.1.2 | 11.4.1.2 | 11.4.1.2.1 | eleven rules, button-name and label among them | seventeen |

Not covered by this gate: V4.1.1 11.2.4.11 (WCAG 2.4.11, Focus Not Obscured
(Minimum)) and 11.2.5.7 (WCAG 2.5.7, Dragging Movements), both "none in
V3.2.1". The suite asks axe-core itself, `axe.getRules(['wcag2411'])` and
`axe.getRules(['wcag257'])`, and both return no rule in 4.13.0.

The gate also covers REQ-P12-08, whose WCAG 2.2 AA pairing the tag set matches.
D81 does not re-rule that row's V3.2.1 edition, and neither does this page.

## The measured checks axe-core cannot make

axe-core 4.13.0 has no rule for the width or the contrast of a focus indicator,
so the suite measures them from computed style (`tests/probe.js`): the focused
element's outline, or a box-shadow ring when there is no outline, its width,
and its WCAG 2.2 contrast against the colour it is drawn on. They bear on WCAG
2.4.7 (V4.1.1 and V3.2.1 11.2.4.7) and 1.4.11 (11.1.4.11 in both); the 200%
text check bears on 1.4.4 (V4.1.1 11.1.4.4, V3.2.1 11.1.4.4.1).

| Check | Measured | Verdict |
| :--- | :--- | :--- |
| Every tab stop, default tokens | input, button, link: outline 3 px, 6.44:1 on white | pass |
| Outline stripped (`:focus-visible { outline: none }`) | no indicator, 0 px | fails; axe-core still reports no violation |
| D16 width boundary | token 3 px and 2 px | pass |
| D16 width boundary | 1 px | fails |
| Contrast boundary | ring `#fa34ff` on white, 3.000002:1 | pass |
| Contrast boundary | ring `#b278ff` on white, 2.989927:1 | fails |
| Threshold alone | 3:1 | pass |
| Threshold alone | 2.9999:1 and 2.99:1 | fail |
| D76 stub theme | 1 px | fails the existing boundary test |
| D76 token overrides | accent, surface, stroke, motion, ring width and colour | each reaches the component |
| 200% text | font 32 px, no page overflow, no clipped box | pass |
| 200% text detector | a planted 120 px clipping box | detected |

D16 fixes a 3 px token and a 2 px floor, so the boundary is the floor: 2 px
passes and one pixel less fails, and the 3 px token passes above it. The stub
in `tests/fixtures/stub-heads-up-theme.css` overrides one token and nothing
else, which is what D76 asks the base tokens to allow; applied over them it
lowers the ring to 1 px, and the existing boundary test fails (REQ-P12-10,
negative case). `tools/test_a11y.py` holds the token file and the component's
style block to the D76 rule: a colour, stroke or motion property with a literal
value fails `make verify-all`, and the D17 rule too: the package declares
exactly its six packages and no CSS framework.

## Emulation is not portal or AT-SPI2 evidence

Three tests are labelled in their titles and in the report as what they are.
The first two are the browser emulation D89 names:

- `[browser emulation, not portal evidence]` reduced motion: with Playwright's
  `reducedMotion: 'reduce'` the motion token is zero and the button's
  transition reads `0s` instead of `0.15s`. No
  `org.freedesktop.portal.Settings` value is read.
- `[browser emulation, not portal evidence]` forced colours: the ring stays
  3 px at every stop and axe-core finds nothing.

The third is extra, labelled coverage that D89 does not name and that counts
towards nothing:

- `[browser accessibility tree, not AT-SPI2]`: the visually hidden token values
  stay in Chromium's accessibility tree, and the toggle reports `[pressed]`. No
  AT-SPI2 bus is queried.

Under D89 the two emulation tests cover only the CSS level of REQ-P12-01,
REQ-P12-02 and REQ-P12-03. The portal half of REQ-P12-02, the AT-SPI2 half of
REQ-P12-03 and the UKI gating of REQ-P12-01 are not met by M04. D89 assigns
them to the milestones that own the shell and the image, M16 and M27, but no
milestone carries them yet: M16's epics list none of the three, and its D77
criterion keeps the portal settings and AT-SPI2 on D-Bus mocks; M27 is the P17
VA-API slice; and M11 says only that the accessibility gate attaches when the
UI enters an image. The same holds for REQ-P12-06's scan of the Forum Shell,
which M04 runs on the P12 component only. Until a recorded decision adds them
to a milestone's criteria and epics they are open and owned by no milestone,
listed with P12's activation blockers in `planning/components.json` and P12's
row in `docs/roadmap/inventory.md`.

## Renovate tracks the pins forward

`renovate.json` reads the image and the Node release out of
`toolchain.pin.json` with two regex managers, and groups `@playwright/test`
with the image, `engines.node` with the pinned Node, and `packageManager` with
`engines.pnpm`. Renovate 44.101.2, run locally as
`renovate --platform=local --dry-run=lookup` against a copy of the package on
2026-09-28, extracted every one of them: the six npm packages, `engines.node`
26.10.0, `engines.pnpm` and `packageManager` 12.6.0, the image with its digest
and the pinned Node, each with no update, because each is the newest release.
Against a copy with Node 26.8.2 and the image at `v1.62.0-noble` it proposed
26.10.0 on the branch `renovate/node-for-the-accessibility-gate` and
`v1.63.0-noble` with its digest on
`renovate/playwright-and-its-container-image`.

D65 tracks the latest release line, not the LTS one, and Renovate on its own
would not. The `node-version` datasource marks every release of a line that
has not reached LTS as unstable, and `node` versioning does the same, so from
the moment 26.x enters LTS on 2026-10-28 Renovate would offer the gate no Node
27 until that line reaches LTS too, about six months after its release.
`renovate.json` therefore sets `ignoreUnstable` to false for `node` in this
package. Run against a copy pinned to 24.21.0, an LTS release: without that
setting Renovate proposed nothing for either Node field; with it, it proposed
26.10.0, the Current line, for `engines.node` and the pinned Node together, on
`renovate/major-node-for-the-accessibility-gate`. Setting `versioning` to
`npm` instead proposed nothing, because the datasource's own marking still
applies. It cannot refresh the Node tarball's sha256, so such a pull request
fails `make a11y-fetch` until someone records the new digest from the signed
`SHASUMS256.txt`: Renovate proposes, the gate proves (D69).

## Scope, and what a pass here does not mean

- It is one component on one page. The P05 shell, its canvas rows REQ-P05-09 to
  REQ-P05-15 and the D76 heads-up theme are not built here.
- Browser emulation is not the desktop: see the section above.
- It builds no image, gates no UKI and closes no accessibility gate for the
  shell or the image; `make verify-all`'s last line says so.
- P12 stays a proposal (D86): `tools/verify_preparation.py` binds activation to
  a Cargo manifest, and M04 does not change that.
- The `@sveltesentio/*` packages D10 adopts are not declared here (D87); M16
  adopts the ones the shell needs.
