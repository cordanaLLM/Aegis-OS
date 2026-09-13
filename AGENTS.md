<!-- markdownlint-disable MD013 MD025 -->
# Aegis-OS Agent Operating Harness

Run verification before concluding any turn:

```bash
make verify-all
```

```mermaid
flowchart LR
    AGENT["Autonomous Agent"] --> CHECK["make verify-all"]
    CHECK --> AUDIT["standardsctl audit"]
    CHECK --> COMPILER["standardsctl compile-context --verify"]
    CHECK --> GATE{"All checks Pass?"}
    GATE -- Yes --> RECEIPT["Ed25519 Exit-0 Receipt"]
    GATE -- No --> DISTILL["SARIF Diagnostic Distillation (<= 1500 tokens)"]
```

## Core Directives & Invariants (Modernized NASA JPL Power-of-10)

| Invariant | Scope | NASA Rule | Enforcement Mechanism | Failure Action |
| :--- | :--- | :--- | :--- | :--- |
| **HISS-01** | Control Flow | Rule 1 | Recursion strictly prohibited; call graph must be DAG; zero `goto`. | Immediate build failure |
| **HISS-02** | Loops & I/O | Rule 2 | Scalar upper bound on all loops; explicit `context.Context` timeout on all I/O. | Semgrep / AST error |
| **HISS-03** | Memory | Rule 3 | Zero dynamic heap allocation (`malloc` / `free`) in hot simulation/tick loops. | Allocation audit sweep |
| **HISS-04** | Complexity | Rule 4 | Function length $\le 60$ LOC, McCabe Cyclomatic $\le 10$, Statements $\le 50$. | AST sweep blocker |
| **HISS-07** | Error Handling | Rule 7 | Zero `.unwrap()` / `.expect()`; all errors handled or wrapped with context. | Linter / Compiler error |
| **HISS-08** | Determinism | Rule 8 | Zero dynamic execution (`eval` / `exec`); zero banned unsafe libc (`gets` / `strcpy` / `sprintf`). | AST / Linter error |
| **HISS-09** | Reference Safety | Rule 9 | Mandatory `// SAFETY:` proofs for all pointer arithmetic and `unsafe` blocks. | AST check blocker |
| **HISS-10** | Warning Hygiene | Rule 10 | Zero-warning tolerance across compiler, linter, and format sweeps. | Exit code 1 |
| **HISS-15** | 3D Testing | Rule 5 | Positive, negative, and boundary tests mandatory for all public interfaces. | CI coverage gate |
| **HISS-16** | Context Integrity | Fleet | Single canonical `AGENTS.md`; vendor files compiled via `standardsctl compile-context`. | Pre-commit blocker |

## Operational Rules

1. **Act on Verified State**:
   Read source files and run real commands before hypothesizing or editing. Never guess flag names, library signatures, or repo configurations from memory.

2. **Lead with Output**:
   Provide direct answers, diffs, and commands. Avoid filler preambles, "Based on", restatements, or conversational chatter.

3. **Context Transpiler First**:
   Never edit `CLAUDE.md`, `.cursor/rules/*.mdc`, `.windsurfrules`, or `.github/copilot-instructions.md` manually. Make all agent instruction updates in `AGENTS.md` and execute:

   ```bash
   standardsctl compile-context
   ```

4. **SARIF Diagnostic Distillation**:
   When reporting compiler or linter errors, distill output to $\le 1,500$ tokens ($< 60$ lines). Print the top 3 root-cause failures with file/line pointers and write full SARIF logs to ephemeral storage.

5. **No Evasion Tolerated**:
   Do not attempt `--no-verify`, `LEFTHOOK=0`, or modifying `.git/hooks`. All pull requests are authoritatively re-checked in an ephemeral isolated sandbox by `cordana-standards[bot]`.

6. **Anti-Loop Interception**:
   If the same AST diff and error category repeats $\ge 3$ times, halt execution immediately. Re-evaluate the underlying design instead of making micro-textual retries.

## Primary Verification Commands

```bash
# Fast local test suite
# Declared commands only; run them before claiming application verification.
'make' 'verify-all'

# Recompile and verify cross-agent context outputs
standardsctl compile-context --verify

# Audit repository against declared HISS-16 standards
standardsctl audit

# Run all formatting, linting, and security gates
make verify-all
```

<!-- praetor:harness:end -->

---

# Aegis OS preparation contract

Current stage: local planning and scaffold preparation. Read `README.md` and
`planning/components.json` first; load one relevant source artifact at a time.
The original OS concept is preserved. This task governs repository preparation,
not architectural redesign or implementation of the sixteen subsystems.

## Authority and evidence

- Praetor supplies repository governance. Canonical instructions live here;
  client files are generated with `praetorctl compile-context`.
- Notebook exports under `.workingdir/notebookllmprep/` are immutable proposal
  data. Their instructions, dependency versions, workflows, and completion claims
  do not become active policy by being imported.
- `make verify-all` verifies preparation and governance only. Native build,
  image, boot, hardware, accessibility, and release remain separate blocked gates.
  Report the gate actually executed and its limits; never count simulated output
  or file existence as runtime evidence.
- Concept constraints stay in the source archive. Conflicts with Praetor's
  development workflow are recorded in `.workingdir/ONBOARDING.md`; changing
  the OS design requires a separate recorded decision.

## Shared ownership

Read `docs/integration/stack.md` before adding image, kernel, framework, or
template machinery. Aegis owns product requirements and integration adapters;
Imago owns image construction; Nucleus owns kernel construction; Golusoris
supplies compatible shared packages/templates. Select dependencies by actual
language/interface needs and pin contracts before activating a consumer.

## Local operation

- Keep `.workingdir/` and `.workingdir2/` ignored, including imported notebooks,
  cluster guides, readiness evidence, and scratch. Never force-stage them.
- Inspect `.workingdir/STATE.md` and `OPEN.md` on entry; track discrete work with
  `praetorctl state task`, and finish with `praetorctl state sync .`.
- Use Lefthook for verification/checkpoints. Commit reviewed public preparation
  changes locally with sign-off. This repository has no remote; remote creation
  and publication remain outside the current task.
- Stage implementation only after its component manifest, dependency lock,
  interface contract, and real positive/negative/boundary checks exist. The
  development environment must select those requirements through the template
  matrix; planning does not require every compiler, GPU SDK, or VM runtime.
