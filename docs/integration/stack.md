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
`@sveltesentio/*` packages published on npm, MIT, versioned per package). A Rust
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

The two producer edges are not at the same stage, and M09 should not treat them
as one step.

The Imago edge accepts this repository's schema today. `cordanaLLM/imago`
ADR-0020 implements `pkg/aegis` against `aegis.p01.product-input.v1` and vendors
`build/product-input.json` byte for byte; both copies hash to
`f17b6e32c300f07063f22caee2229633e0feaed46b510d235c03e632cb869793`. Running its
validator against this repository's live file accepts it, refuses a manifest with
no correlation id, accepts `retries.max-attempts` at 10 and refuses 11 — the
three criteria that ADR names for M09:

```console
$ imago aegis validate build/product-input.json
✓ Aegis product-input aegis-m18-product-input-0001 accepted (aegis.p01.product-input.v1).
```

That is the request half of M09 demonstrated against a real consumer rather than
inferred. It is not the whole milestone: Imago's ADR records that nothing
produces a result yet, so no artifact, digest or boot evidence has returned, and
`imago.p01.product-result.v1` is proposed by Imago rather than agreed here.

The Nucleus edge is not connected at all. `cordanaLLM/nucleus` has no surface
reading `aegis.p01-nucleus.kernel-requirement.v1`; its `verify-requirements.yml`
checks a symbol list hardcoded in the workflow, which satisfies two of the
thirteen features `build/kernel-requirement.json` declares. `CONFIG_PREEMPT_RT`
comes from its `realtime` stream rather than a fragment, and the remaining ten —
including `CONFIG_DEBUG_INFO_BTF`, which the P06 eBPF gate needs, and
`CONFIG_KVM`, where Nucleus sets the guest-side `CONFIG_KVM_GUEST` instead — are
absent. Reported as `cordanaLLM/nucleus` issue 20. Until that edge reads the
document, D70 building the kernel locally is not a stopgap but the only path.

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
   then pin the Imago/Nucleus schemas and test one request/result pair (M09).
4. Build one minimal image and retain artifact, signature and boot evidence
   (M11).
5. Enable release signing and remote delivery only after publication settings
   and consumers exist (M13).
