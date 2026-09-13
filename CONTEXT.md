<!-- markdownlint-disable MD013 -->
# Aegis OS domain glossary

Shared vocabulary for this planning repository. Terms are defined once here and used consistently in `planning/`, `docs/`, and the governance files.

- **Concept**: the original sixteen-subsystem operating-system design captured in the private source archive. It is preserved, not redesigned; changing it requires an ADR.
- **Subsystem (P01–P16)**: one of the sixteen proposed parts of the OS, identified by its `P` number and `aegis-*` name in `planning/components.json`.
- **Blueprint**: the private per-subsystem report from the concept archive. Thirteen exist; P09, P13, and P16 are described only by the architecture documents and the trade-off guide.
- **Component**: a subsystem as a unit of activation with an owner, candidate sources, and activation blockers.
- **Proposal**: the status of every component today; nothing is built, verified, or released.
- **Activation**: promotion of one component from proposal to active implementation. Requires a component manifest, a dependency lock, an interface contract, and positive, negative, and boundary tests.
- **Producer contract**: the pinned request/result schema between Aegis and a builder repository (Imago for images, Nucleus for kernels) or a shared package source (Golusoris). An edge counts as connected only with schema, correlation ID, exact revision, bounded retries, and a recorded result.
- **Milestone**: a bounded, verifiable state in `planning/roadmap.json` with a rank, a cost, and explicit blocking relations.
- **Blocking state**: the `state` of a milestone. `done` needs recorded evidence; `ready` means every blocker is done; `blocked` otherwise. `make readiness` lists the ready set.
- **Unblocking value**: what a milestone makes ready downstream. The roadmap ranks milestones by unblocking value per cost, cheapest first.
- **Epic**: a group of tasks inside one milestone that cites requirement IDs.
- **Requirement**: a statement cited to a source ID, its SHA-256, and a verbatim quote, validated by `praetorctl notebook validate`.
- **Preparation gate**: `make verify-all`. It verifies planning structure, privacy, licence texts, and governance only; build, boot, hardware, and release are separate blocked gates.
- **Private working directory**: `.workingdir/`, gitignored. Holds the source archive, prepared candidates, readiness evidence, and Praetor session state. Never staged.
- **Praetor**: the fleet governance tool (`praetorctl`) that generates client contexts, audits invariants, and keeps session state.
