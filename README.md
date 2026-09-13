# Aegis OS

[![HISS-16
Compliant](https://img.shields.io/badge/Standards-HISS--16%20Compliant-brightgreen)](https://github.com/cordanaLLM/Aegis-OS/blob/main/AGENTS.md)
[![Code: EUPL
1.2](https://img.shields.io/badge/code-EUPL--1.2-315c9b.svg)](https://github.com/cordanaLLM/Aegis-OS/blob/main/LICENSING.md)
[![Original content: CC BY-SA
4.0](https://img.shields.io/badge/original%20content-CC%20BY--SA%204.0-b85c00.svg)](https://github.com/cordanaLLM/Aegis-OS/blob/main/LICENSING.md)

This repository develops Aegis OS, an image-based Linux operating system. The
original sixteen-subsystem concept is preserved, not redesigned; components are
activated one at a time against recorded evidence, and `make readiness` lists
which components are activated and which are still proposals.

This repository qualifies preparation, governance, a source-cited roadmap,
library crates, and the declarative image, partition and update definitions. It
is not a working OS: it does not yet build a product image, boot on any machine,
or publish a release.

`make verify-all` is the gate every change runs; the `verify-all` recipe in the
`Makefile` is the authoritative list of what it runs. Its gates do not all fail
the same way on an under-equipped host: some fail loudly, others print why they
did not run and exit 0, so exit 0 is not evidence that every one of them
executed. `docs/roadmap/toolchain-admission.md` records, per tool a gate
executes, the pinned version and the file that pins it. The gate closes no
image, boot, hardware, accessibility or release gate; the `build`, `boot` and
`release` verbs fail by design.

`make readiness` prints the live component and milestone state. The canonical
remote is <https://github.com/cordanaLLM/Aegis-OS>; `CONTEXT.md` defines the
vocabulary.

| Path | Purpose |
| --- | --- |
| `planning/components.json` | Component inventory, per-component status, dependencies, activation blockers, cheapest first slice |
| `planning/roadmap.json` | Milestones with blocking states, ranked by unblocking value per cost |
| `docs/roadmap/` | Roadmap, cited requirements, dependency-ordered tasks, and the toolchain admission matrix |
| `docs/adr/` | Architecture decision records |
| `docs/integration/stack.md` | Shared ownership and cross-repository contracts |
| `LICENSING.md`, `REUSE.toml` | Split licence: EUPL-1.2 technical material, CC BY-SA 4.0 prose |
| `Cargo.toml`, `rust-toolchain.toml`, `crates/` | Rust workspace and pinned toolchain; `[workspace] members` is the live crate list |
| `build/` | Tracked image, partition and update definitions and the product input and kernel requirement manifests; `build/README.md` maps each file to the gate that reads it |
| `tests/` | Reserved for real component, image, boot and hardware acceptance tests; the only active content is the recorded negative and boundary fixtures for the P01/P02 definition gate |
| `Makefile` | Every gate verb, and the authoritative list of what `make verify-all` runs; `build`, `boot` and `release` are deliberately closed |
| `bpf/`, `ui/` | Reserved implementation locations |
| `.workingdir/notebookllmprep/` | Original private export, preserved byte-for-byte |
| `.workingdir/prepared/` | Organized, inactive candidate files with source hashes |
| `.workingdir/notebook-prepared/` | Praetor source snapshot and planning prompts |
| `.workingdir/ONBOARDING.md` | Private source index, workflow corrections, blockers |
| `.workingdir/OS_STACK_READINESS.md` | Observed connected-stack readiness |

The private preparation inputs are deliberately absent from version control. The
ordinary governance gate works without them; `make verify-sources` requires the
local archive and verifies every original hash. A future remote checkout must
receive reviewed implementation files, not a copy of the private archive.

`setup.repo.sh` stages exports as data through Praetor's bounded importer. It
never executes imported scripts, installs packages, or enables release
workflows. Set `PRAETOR_SOURCE_ROOT` if the local Praetor checkout is not at
`../praetor`.
