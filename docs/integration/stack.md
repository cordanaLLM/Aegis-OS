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
and reproducible failures go back to the owning repository. Each direction
needs a schema, correlation ID, exact revision, bounded retries, and a recorded
result before it counts as connected.

Fabrica's product requirements remain in Aegis while reusable builder logic is
provided through Imago and Nucleus. Shared package reuse must respect language
boundaries: a Go library is not a direct dependency of a Rust daemon. Use a
compatible library or an explicit protocol/FFI adapter with contract tests.
GPU templates are candidates only for components that require those backends.

Activation order:

1. Finish the component inventory and source provenance (preparation gate).
2. Pin image/kernel producer schemas and test one request/result pair locally.
3. Select one component's manifests, locks, templates, and real tests.
4. Build one minimal image and retain artifact/signature/boot evidence.
5. Enable remote delivery only after publication settings and consumers exist.
