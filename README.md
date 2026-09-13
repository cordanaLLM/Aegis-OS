# Aegis OS

[![HISS-16 Compliant](https://img.shields.io/badge/Standards-HISS--16%20Compliant-brightgreen)](AGENTS.md)
[![Code: EUPL 1.2](https://img.shields.io/badge/code-EUPL--1.2-315c9b.svg)](LICENSING.md)
[![Original content: CC BY-SA 4.0](https://img.shields.io/badge/original%20content-CC%20BY--SA%204.0-b85c00.svg)](LICENSING.md)

Local planning repository for an image-based Linux OS with sixteen proposed
subsystems. The NotebookLM concept is preserved; this checkout currently
qualifies repository preparation, not a working OS.

Run `make verify-all` for the current preparation gate. Run `make readiness`
for the component state and remaining activation blockers. Build, boot, and
release gates remain blocked until their real inputs and checks exist.

| Path | Purpose |
| --- | --- |
| `planning/components.json` | Component inventory and explicit readiness |
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
