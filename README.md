# Aegis OS

[![HISS-16 Compliant](https://img.shields.io/badge/Standards-HISS--16%20Compliant-brightgreen)](https://github.com/cordanaLLM/Aegis-OS/blob/main/AGENTS.md)
[![Code: EUPL 1.2](https://img.shields.io/badge/code-EUPL--1.2-315c9b.svg)](https://github.com/cordanaLLM/Aegis-OS/blob/main/LICENSING.md)
[![Original content: CC BY-SA 4.0](https://img.shields.io/badge/original%20content-CC%20BY--SA%204.0-b85c00.svg)](https://github.com/cordanaLLM/Aegis-OS/blob/main/LICENSING.md)

Planning repository for an image-based Linux OS with sixteen proposed
subsystems. The original concept is preserved; this repository currently
qualifies preparation, governance, and a source-cited roadmap, not a working OS.

Run `make verify-all` for the preparation gate. Run `make readiness` for the
component inventory, the remaining activation blockers, and the roadmap
milestones that are ready to start. Build, boot, and release gates remain
blocked until their real inputs and checks exist. The canonical remote is
<https://github.com/cordanaLLM/Aegis-OS>; `CONTEXT.md` defines the vocabulary.

| Path | Purpose |
| --- | --- |
| `planning/components.json` | Component inventory, dependencies, blockers, cheapest first slice |
| `planning/roadmap.json` | Milestones with blocking states, ranked by unblocking value per cost |
| `docs/roadmap/` | Roadmap, cited requirements, and dependency-ordered tasks |
| `docs/integration/stack.md` | Shared ownership and cross-repository contracts |
| `LICENSING.md`, `REUSE.toml` | Split licence: EUPL-1.2 technical material, CC BY-SA 4.0 prose |
| `build/`, `crates/`, `bpf/`, `ui/`, `tests/` | Reserved implementation locations |
| `.workingdir/notebookllmprep/` | Original private export, preserved byte-for-byte |
| `.workingdir/prepared/` | Organized, inactive candidate files with source hashes |
| `.workingdir/notebook-prepared/` | Praetor source snapshot and planning prompts |
| `.workingdir/ONBOARDING.md` | Private source index, workflow corrections, blockers |
| `.workingdir/OS_STACK_READINESS.md` | Observed connected-stack readiness |

The private preparation inputs are deliberately absent from version control.
The ordinary governance gate works without them; `make verify-sources` requires
the local archive and verifies every original hash. A future remote checkout
must receive reviewed implementation files, not a copy of the private archive.

`setup.repo.sh` stages exports as data through Praetor's bounded importer. It
never executes imported scripts, installs packages, or enables release workflows.
Set `PRAETOR_SOURCE_ROOT` if the local Praetor checkout is not at `../praetor`.
