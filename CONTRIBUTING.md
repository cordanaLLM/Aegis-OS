<!-- markdownlint-disable MD013 -->
# Contributing to Aegis-OS

Thank you for contributing! This repository adheres strictly to the
**High-Integrity Systems Standards (HISS-16)** and modernized **NASA JPL
Power-of-10** rules.

## Core Directives & Verification

All changes must pass local verification before submitting:

```bash
make verify-all
```

The `verify-all` recipe in the `Makefile` is the authoritative list of what that
command runs; read it there rather than from this page. It verifies definitions
and library code only. It closes no image, boot, hardware, accessibility or
release gate; those remain separately blocked, and `make build`, `make boot` and
`make release` exit non-zero by design. `make readiness` prints the current
component and milestone state; never restate that state here.

That is not the whole gate a pull request faces, so a green local run is not yet
a green pull request. The `Makefile` also declares verification targets that
`verify-all` never invokes, and beyond `make verify-all` the `Verification gate`
status check runs further licence, Markdown, YAML, Python and shell linting and
checks the Developer Certificate of Origin sign-off on the pull request's
commits.
[`.github/workflows/ci.yml`](https://github.com/cordanaLLM/Aegis-OS/blob/main/.github/workflows/ci.yml)
is the authority on that set and on the version each tool is pinned to; run the
ones that cover the files you touched before opening a pull request, rather than
listing them again here.

### Modernized NASA JPL Power-of-10 Rules

1. **Simple Control Flow (HISS-01)**: Recursion is strictly banned; call graph
   must be an acyclic DAG; zero `goto`.
2. **Bounded Loops (HISS-02)**: All loops must have a statically verifiable
   scalar upper bound. Network and disk I/O require an explicit deadline or
   timeout (Rust: `tokio::time::timeout` or equivalent).
3. **Deterministic Memory (HISS-03)**: Zero dynamic heap allocations (`malloc` /
   `free`) in hot simulation or rendering loops.
4. **Function Length Cap (HISS-04)**: No function may exceed **60 lines of
   code** ($\le 60$ LOC).
5. **Assertion Density**: Functions must assert preconditions, state invariants,
   and postconditions.
6. **Data Scope**: Variables must be declared at the smallest possible scope.
7. **Checked Errors (HISS-07)**: Check return values of all non-void functions;
   zero `.unwrap()` or unchecked errors.
8. **Static Execution (HISS-08)**: Dynamic code evaluation (`eval` / `exec`) and
   banned unsafe libc calls (`gets` / `strcpy` / `sprintf`) are prohibited.
9. **Pointer Safety (HISS-09)**: Pointer arithmetic must be bounded; all
   `unsafe` blocks require `// SAFETY:` justifications.
10. **Zero-Warning Hygiene (HISS-10)**: Zero compiler, linter, or formatting
    warnings tolerated across all builds.

### 3D Testing Discipline (HISS-15)

Every public function requires:

- **Positive tests**: Expected valid operational inputs.
- **Negative tests**: Invalid inputs, expected error returns.
- **Boundary tests**: Zero, one, max limits, off-by-one bounds.

### Commit Messages

We use Conventional Commits (checked in review; breaking `!` commits are
validated by `praetorctl forge check-commits --base=<ref>`):

- `feat:` New features
- `fix:` Bug fixes
- `chore:` Maintenance and governance
- `feat!:` / `fix!:` Breaking API changes (must include `Migration:` footer)
- All commits carry a Developer Certificate of Origin sign-off (`git commit
  -s`), as required by `AGENTS.md` (Local operation)

## Local setup

Praetor keeps session state in the gitignored `.workingdir/` directory. After
cloning, run `praetorctl state init . --if-absent` so that `praetorctl flavor
audit .` and the pre-push gate's Flavor Conformance stage find the state
ledgers. `make verify-sources` additionally needs the private source archive and
is expected to fail on a clone without it.

The gates inside `make verify-all` do not all fail the same way on an
under-equipped host: some fail loudly on a missing tool, others print why they
did not run and exit 0, so a green local run is not proof that all of them
executed. Read each gate's own output.
[`docs/roadmap/toolchain-admission.md`](https://github.com/cordanaLLM/Aegis-OS/blob/main/docs/roadmap/toolchain-admission.md)
is the authority on which tool a gate runs, at which pinned version, and which
tools are not yet admitted — read the versions there rather than from this page.

The `.vscode/` configuration is maintained here for this repository's actual
stack and deliberately differs from Praetor's generated editor template, so
`praetorctl editors verify` reports drift. That command gates nothing; do not
run `praetorctl editors` in this repository, because it would restore the
Go-oriented template. Configurations for other editors stay as generated.

## Files that are machine-checked

Some tracked prose is verified, not just linted.

- `AGENTS.md` is canonical and its per-client projections are generated. Edit
  `AGENTS.md` and run `praetorctl compile-context`; hand-editing a projection
  fails the gate and the pre-commit hook.
- `docs/integration/stack.md` is pinned by digest from
  `planning/candidates.json`, so any edit to it must update both citations in
  the same commit.
- `docs/roadmap/README.md` is parsed against `planning/roadmap.json` and must
  carry each milestone's rank and state exactly as the register records them, in
  both the ranked table and the section preambles.

## Licensing

Read
[`LICENSING.md`](https://github.com/cordanaLLM/Aegis-OS/blob/main/LICENSING.md)
before contributing: technical material is accepted under EUPL-1.2 (or later),
original prose under CC BY-SA 4.0.
