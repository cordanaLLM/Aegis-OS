<!-- markdownlint-disable MD013 -->
# Contributing to Aegis-OS

Thank you for contributing! This repository adheres strictly to the **High-Integrity Systems Standards (HISS-16)** and modernized **NASA JPL Power-of-10** rules.

## Core Directives & Verification

All changes must pass local verification before submitting:

```bash
make verify-all
```

### Modernized NASA JPL Power-of-10 Rules

1. **Simple Control Flow (HISS-01)**: Recursion is strictly banned; call graph must be an acyclic DAG; zero `goto`.
2. **Bounded Loops (HISS-02)**: All loops must have a statically verifiable scalar upper bound. Network and disk I/O require an explicit deadline or timeout (Rust: `tokio::time::timeout` or equivalent).
3. **Deterministic Memory (HISS-03)**: Zero dynamic heap allocations (`malloc` / `free`) in hot simulation or rendering loops.
4. **Function Length Cap (HISS-04)**: No function may exceed **60 lines of code** ($\le 60$ LOC).
5. **Assertion Density**: Functions must assert preconditions, state invariants, and postconditions.
6. **Data Scope**: Variables must be declared at the smallest possible scope.
7. **Checked Errors (HISS-07)**: Check return values of all non-void functions; zero `.unwrap()` or unchecked errors.
8. **Static Execution (HISS-08)**: Dynamic code evaluation (`eval` / `exec`) and banned unsafe libc calls (`gets` / `strcpy` / `sprintf`) are prohibited.
9. **Pointer Safety (HISS-09)**: Pointer arithmetic must be bounded; all `unsafe` blocks require `// SAFETY:` justifications.
10. **Zero-Warning Hygiene (HISS-10)**: Zero compiler, linter, or formatting warnings tolerated across all builds.

### 3D Testing Discipline (HISS-15)

Every public function requires:

- **Positive tests**: Expected valid operational inputs.
- **Negative tests**: Invalid inputs, expected error returns.
- **Boundary tests**: Zero, one, max limits, off-by-one bounds.

### Commit Messages

We use Conventional Commits (checked in review; breaking `!` commits are validated by `praetorctl forge check-commits --base=<ref>`):

- `feat:` New features
- `fix:` Bug fixes
- `chore:` Maintenance and governance
- `feat!:` / `fix!:` Breaking API changes (must include `Migration:` footer)
- All commits carry a Developer Certificate of Origin sign-off (`git commit -s`), as required by `AGENTS.md` (Local operation)

## Local setup

Praetor keeps session state in the gitignored `.workingdir/` directory. After
cloning, run `praetorctl state init . --if-absent` so that `praetorctl flavor
audit .` and the pre-push gate's Flavor Conformance stage find the state
ledgers. `make verify-sources` additionally needs the private source archive
and is expected to fail on a clone without it.

The `.vscode/` configuration is maintained here for this repository's actual
stack and deliberately differs from Praetor's generated editor template, so
`praetorctl editors verify` reports drift. That command gates nothing; do not
run `praetorctl editors` in this repository, because it would restore the
Go-oriented template. Configurations for other editors stay as generated.

## Licensing

Read [`LICENSING.md`](https://github.com/cordanaLLM/Aegis-OS/blob/main/LICENSING.md) before contributing: technical material is
accepted under EUPL-1.2 (or later), original prose under CC BY-SA 4.0.
