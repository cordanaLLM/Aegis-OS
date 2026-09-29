# Connected stack contract

Status: proposed integration boundaries; no cross-repository build is qualified.
Exact local observations and blockers live in the private readiness matrix.

| Owner | Responsibility | Consumer proof needed |
| --- | --- | --- |
| Aegis OS | Product requirements, component composition, OS-specific configuration and acceptance | Versioned product input manifest and minimal real component test |
| cordanaLLM/nucleus | Kernel configuration, patch selection, kernel artifact production | Kernel version/config digest, ABI, artifact digest and provenance verified by consumer |
| cordanaLLM/imago | Reusable image construction, kernel consumption, image artifact production | Aegis configuration accepted, pinned kernel consumed, output digest and boot evidence |
| golusoris | Reusable libraries, frameworks and language-specific templates | Actual package/language boundary, supported version, license and consumer contract tests |
| cordanaLLM/praetor | Governance, preparation, templates, skills, readiness and dogfood orchestration | Local-source identity, applied configuration and real tool readback |

Requirements flow from Aegis to the builders and shared packages. Validated
artifacts, diagnostics, and compatibility evidence return to Aegis. New needs
and reproducible failures go back to the owning repository. Each direction needs
a schema, correlation ID, exact revision, bounded retries, and a recorded result
before it counts as connected.

The proposed build subsystem (P01 aegis-fabrica, see `planning/components.json`)
keeps its product requirements in Aegis while reusable builder logic is provided
through Imago and Nucleus. Shared package reuse must respect language
boundaries: a Go library is not a direct dependency of a Rust daemon. Use a
compatible library or an explicit protocol/FFI adapter with contract tests. GPU
templates are candidates only for components that require those backends.

The shared package producers are identified and released, so a consumer pins a
published version rather than waiting for maturity: `golusoris/golusoris` for Go
(module `github.com/golusoris/golusoris`, v0.12.0, EUPL-1.2, resolvable on the
Go module proxy) and `golusoris/sveltesentio` for the UI surface (nineteen
`@sveltesentio/*` packages published on npm, MIT, versioned per package). Since
ADR-0004 (2026-09-29) sveltesentio no longer supplies the P05 shell, which is
native Rust on gpui; it stays available for any web surface. A Rust
shared-package producer from the same family is expected but does not exist yet;
until one does, Rust components select crates through the template matrix, and
that absence is the reason to keep the row generic rather than to add a producer
that cannot be pinned. Identification is not qualification: a consumer still
owes the package/language boundary, the pinned version, the licence and the
consumer contract tests named in the table above.

While Imago and Nucleus are scaffolds, Aegis performs the parts it cannot defer:
it validates image definitions locally (D56) and builds the kernel locally (D70,
milestone M26). Neither produces a release artifact, and both hand back to the
owning repository once it returns real artifacts against the contract pinned in
M09. Ownership in the table above is unchanged; only who currently does the work
is.

The two producer edges are not at the same stage, and M09 did not treat them as
one step (D92).

The Imago edge consumes both of this repository's payloads, and M09 pins that.
`cordanaLLM/imago` ADR-0020 implements `pkg/aegis` against
`aegis.p01.product-input.v1`, and ADR-0021 implements `pkg/kernel` against
`aegis.p01-nucleus.kernel-requirement.v1`; imago vendors `build/product-input.json`
and the kernel requirement pair byte for byte, and
`build/contract/producers.pin.json` names the imago commit and the sha256 of each
copy (`f17b6e32c300f07063f22caee2229633e0feaed46b510d235c03e632cb869793` for the
product input). `make verify-contract` runs both validators, built from that
commit, against this repository's live files: each payload is accepted, a
tampered copy is refused with the payload's correlation id,
`retries.max-attempts` at 10 and the packages count at 256 are accepted and one
above each is refused, and an empty feature list is refused explicitly
([the contract pair](../build/contract-pair.md)):

```console
$ imago aegis validate build/product-input.json
✓ Aegis product-input aegis-m18-product-input-0001 accepted (aegis.p01.product-input.v1).
```

That is the consumption contract, and under D92 it is all of M09. It does not
make the edge connected: Imago's ADR records that nothing produces a result
yet, so no artifact, digest or boot evidence has returned.
`imago.p01.product-result.v1` is proposed by Imago and moved with the result to
M11, and `cordanaLLM/imago` issue 46 tracks its emitter.

The Nucleus edge reads the kernel requirement since `cordanaLLM/nucleus` pull
request 35, merged as `0a4eac9` on 2026-09-29, and M09 pins it under D106, at
`82aa6b7` after that day's re-pin and at `852be74` since D107. Its
`scripts/verify_kernel_requirement.py` decodes `build/kernel-requirement.json`
the way `crates/aegis-fabrica-defs` does and holds it against the kconfig
fragments of the streams its `versions.json` binds Aegis to, `realtime` alone,
in place of the hardcoded symbol list its issue 20 reported; its ADR-0007,
accepted on 2026-09-29, is the policy. `make
verify-contract` runs that script from the pinned checkout and reads its JSON
report: the requirement passes on `realtime`, a planted symbol no fragment sets
is refused with the requirement's correlation id, an empty feature list is
rejected, and the digest or correlation id of another document is rejected ([the
contract pair](../build/contract-pair.md)). The evidence level is `declared`:
the fragments as merged, not a built configuration, so `make olddefconfig` can
still drop a declared symbol. Since `82aa6b7` the verifier also offers a
`resolved` level, against a `.config` resolved from the stream's verified
source, which needs the network, so the offline gate stays at `declared`; and
the forge compiles and gates every leg (nucleus issues 18 and 31, closed by its
pull request 37). A Nucleus-published kernel, its configuration and its digests
are M10's to record (D92, D94): E10-4 and E10-5 read the release asset
`kernel-realtime-x86_64.config`, which the manifest pins by
`kernel.config_digest`. Nucleus's outbound role is unchanged: the
`imago.nucleus.kernel-artifact.v1` manifest imago verifies. Its first release,
`v7.2.8-realtime-lusoris1`, was published on 2026-09-29 and M10 verifies and
records it ([the Nucleus kernel gate](../build/nucleus-kernel.md)). That kernel
satisfied the requirement as it was then written and still cannot attach a BPF
LSM program, because the requirement did not ask for the function tracer the BPF
trampoline patches. Decision D107 (2026-09-29) adds
`CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS`, built-in, to
`build/kernel-requirement.json`; imago re-vendored the new bytes, nucleus
enables the tracer on every stream, M09 pins both at those commits, and M10
waits for a Nucleus release built to it. D70's locally built kernel, rebuilt
with the row, stays the interim for M23's latency fixture.

Activation order: the ranked, blocking-state roadmap in `docs/roadmap/README.md`
and `planning/roadmap.json` is authoritative (`make readiness` lists the ready
set). It keeps the original sequence below but orders local, hardware-free
component work ahead of the producer contract pin. Both builder identities now
resolve (`cordanaLLM/imago`, `cordanaLLM/nucleus`), so the ordering no longer
rests on an unknown address; it rests on the contract still being unverified,
because neither producer has returned an artifact against an Aegis-pinned
schema, and a blocked external step must not stall work that can already be
verified.

1. Finish the component inventory and source provenance (roadmap M00, M01).
2. Promote one component end-to-end with manifests, locks, template selection,
   and real tests (M02), then the other hardware-free slices.
3. Author the Aegis-side product input and kernel requirement schemas (M18),
   then pin the producers' consumption of them (M09; under D92 the result legs
   are M11's and M10's).
4. Build one minimal image and retain artifact, signature and boot evidence
   (M11).
5. Enable release signing and remote delivery only after publication settings
   and consumers exist (M13).
