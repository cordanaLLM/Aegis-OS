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

While Imago and Nucleus are scaffolds, Aegis performs the parts it cannot defer:
it validates image definitions locally (D56) and builds the kernel locally (D70,
milestone M26). Neither produces a release artifact, and both hand back to the
owning repository once it returns real artifacts against the contract pinned in
M09. Ownership in the table above is unchanged; only who currently does the work
is.

Activation order: the ranked, blocking-state roadmap in `docs/roadmap/README.md`
and `planning/roadmap.json` is authoritative (`make readiness` lists the ready
set). It keeps the original sequence below but orders local, hardware-free
component work ahead of the producer contract pin, because both builder
identities are currently unresolved on GitHub and a blocked external step must
not stall work that can already be verified.

1. Finish the component inventory and source provenance (roadmap M00, M01).
2. Promote one component end-to-end with manifests, locks, template selection,
   and real tests (M02), then the other hardware-free slices.
3. Author the Aegis-side product input and kernel requirement schemas (M18),
   then pin the Imago/Nucleus schemas and test one request/result pair (M09).
4. Build one minimal image and retain artifact, signature and boot evidence
   (M11).
5. Enable release signing and remote delivery only after publication settings
   and consumers exist (M13).
