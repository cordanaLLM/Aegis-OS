<!-- markdownlint-disable MD013 -->
# Aegis OS domain glossary

Shared vocabulary for this repository. Terms are defined once here and used
consistently in `planning/`, `docs/`, and the governance files.

- **Concept**: the original sixteen-subsystem operating-system design captured
  in the private source archive. It is preserved, not redesigned; changing it
  requires an ADR.
- **Subsystem (P01–P16)**: one of the sixteen parts of the OS design,
  identified by its `P` number and `aegis-*` name in `planning/components.json`.
- **Blueprint**: the private per-subsystem report from the concept archive.
  Thirteen exist; P09, P13, and P16 are described only by the architecture
  documents and the trade-off guide.
- **Component**: a subsystem as a unit of activation with an owner, candidate
  sources, and activation blockers.
- **Proposal**: a component that has not yet met the activation bar. Its
  remaining activation blockers are listed in `planning/components.json`.
- **Activated**: a component that has met the activation bar, with its evidence
  recorded in `planning/components.json`. `make readiness` prints which
  components are in which state today. No component status implies a product
  image, a boot, or a release; those gates are separate and still blocked.
- **Activation**: promotion of one component from `proposal` to `activated`.
  Those two are the only statuses `tools/verify_preparation.py` admits; there is
  no `active` status. The gate requires an `activation_evidence` block it
  accepts: a component directory named after the component, a git-tracked
  manifest inside that directory whose `[package] name` is the component's name,
  a git-tracked dependency lock, a test command that names the component, and a
  cited milestone. The component's remaining `activation_blockers` stay recorded
  and non-empty. A pinned interface contract and real positive, negative and
  boundary tests are required by `AGENTS.md` and checked in review; this gate
  checks only the recorded evidence above, and `tools/verify_preparation.py` is
  the authority on that half.
- **Producer contract**: the pinned request/result schema between Aegis and a
  builder repository (Imago for images, Nucleus for kernels) or a shared package
  source (Golusoris). An edge counts as connected only with schema, correlation
  ID, exact revision, bounded retries, and a recorded result.
- **Milestone**: a bounded, verifiable state in `planning/roadmap.json` with a
  rank, a cost, and explicit blocking relations.
- **Blocking state**: the `state` of a milestone. `done` needs recorded
  evidence; `ready` means every blocker is done; `blocked` otherwise. `make
  readiness` lists the ready set.
- **Unblocking value**: what a milestone makes ready downstream. The roadmap
  ranks milestones by unblocking value per cost, cheapest first.
- **Epic**: a group of tasks inside one milestone that cites requirement IDs.
- **Requirement**: a statement cited to a source ID, its SHA-256, and a verbatim
  quote, validated by `praetorctl notebook validate`.
- **Verification gate**: `make verify-all`, and the GitHub status check of the
  same name that runs it on a pull request. The `verify-all` recipe in the
  `Makefile` is the authoritative list of what the local target runs, and
  `.github/workflows/ci.yml` of what the check adds around it. Neither run
  subsumes the other: the check adds licence, format, lint and sign-off steps
  the local target never invokes, while the host-dependent gates inside `make
  verify-all` execute only on a host that meets their recorded floors. It
  verifies definitions and library code; image, boot, hardware, accessibility
  and release are separate, still-blocked gates. `CONTRIBUTING.md` says which
  to run locally.
- **Private working directory**: `.workingdir/`, gitignored. Holds the source
  archive, prepared candidates, readiness evidence, and Praetor session state.
  Never staged.
- **Praetor**: the fleet governance tool (`praetorctl`) that generates client
  contexts, audits invariants, and keeps session state.
